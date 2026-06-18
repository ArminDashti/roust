import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

type TargetKind = "nic" | "ip" | "cidr" | "mac";
type DestinationKind = "nic" | "ip" | "mac";

type ServiceStatus = {
  state: string;
  installed: boolean;
  directory: string;
  version: string;
};

type RoutingRule = {
  target: TargetKind;
  "target-value": string;
  destination: DestinationKind;
  "destination-value": string;
};

type GatewayRow = {
  nic_name: string;
  gateway_ip: string;
};

type PredictResult = {
  destination: string;
  if_index: number;
  next_hop: string;
  nic_name: string | null;
  nic_display: string | null;
};

type RuleMutationResult = {
  message: string;
  live_apply_hint: string | null;
};

const views = ["dashboard", "rules", "gateways", "predict"] as const;
type ViewId = (typeof views)[number];

const viewTitle: Record<ViewId, string> = {
  dashboard: "Dashboard",
  rules: "Routing Rules",
  gateways: "Gateways",
  predict: "Route Predict",
};

const TARGET_OPTIONS: TargetKind[] = ["nic", "ip", "cidr", "mac"];
const DESTINATION_OPTIONS: DestinationKind[] = ["nic", "ip", "mac"];

let editingRuleIndex: number | null = null;
let pendingImportPath: string | null = null;

const toastEl = document.getElementById("toast")!;
const viewTitleEl = document.getElementById("view-title")!;
const configPathLabel = document.getElementById("config-path-label")!;
const serviceStateEl = document.getElementById("service-state")!;
const serviceVersionEl = document.getElementById("service-version")!;
const serviceDirectoryEl = document.getElementById("service-directory")!;
const statRulesEl = document.getElementById("stat-rules")!;
const statGatewaysEl = document.getElementById("stat-gateways")!;
const rulesBody = document.getElementById("rules-body")!;
const rulesEmpty = document.getElementById("rules-empty")!;
const gatewaysBody = document.getElementById("gateways-body")!;
const gatewaysEmpty = document.getElementById("gateways-empty")!;
const predictResult = document.getElementById("predict-result")!;
const ruleDialog = document.getElementById("rule-dialog") as HTMLDialogElement;
const ruleForm = document.getElementById("rule-form") as HTMLFormElement;
const ruleDialogTitle = document.getElementById("rule-dialog-title")!;
const ruleTargetSelect = document.getElementById("rule-target") as HTMLSelectElement;
const ruleTargetValueInput = document.getElementById("rule-target-value") as HTMLInputElement;
const ruleDestinationSelect = document.getElementById("rule-destination") as HTMLSelectElement;
const ruleDestinationValueInput = document.getElementById(
  "rule-destination-value",
) as HTMLInputElement;
const importDialog = document.getElementById("import-dialog") as HTMLDialogElement;
const importForm = document.getElementById("import-form") as HTMLFormElement;
const importFileLabel = document.getElementById("import-file-label")!;

function fillSelect(select: HTMLSelectElement, options: string[], selected?: string) {
  select.replaceChildren();
  for (const option of options) {
    const el = document.createElement("option");
    el.value = option;
    el.textContent = option;
    if (option === selected) {
      el.selected = true;
    }
    select.append(el);
  }
}

function showToast(message: string, isError = false) {
  toastEl.textContent = message;
  toastEl.classList.toggle("error", isError);
  toastEl.classList.remove("hidden");
  window.setTimeout(() => toastEl.classList.add("hidden"), 4500);
}

async function invokeOrToast<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T | null> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    showToast(String(error), true);
    return null;
  }
}

function setActiveView(view: ViewId) {
  views.forEach((id) => {
    document.getElementById(`view-${id}`)?.classList.toggle("active", id === view);
    document
      .querySelector(`.nav-item[data-view="${id}"]`)
      ?.classList.toggle("active", id === view);
  });
  viewTitleEl.textContent = viewTitle[view];
}

function formatServiceState(state: string): { label: string; className: string } {
  switch (state) {
    case "started":
      return { label: "Running", className: "running" };
    case "not_installed":
      return { label: "Not Installed", className: "warn" };
    default:
      return { label: "Stopped", className: "stopped" };
  }
}

async function loadStatus() {
  const status = await invokeOrToast<ServiceStatus>("get_status");
  if (!status) return;

  const { label, className } = formatServiceState(status.state);
  serviceStateEl.textContent = label;
  serviceStateEl.className = `status-pill ${className}`;
  serviceVersionEl.textContent = `v${status.version}`;
  serviceDirectoryEl.textContent = status.directory;
}

async function loadRules() {
  const rules = await invokeOrToast<RoutingRule[]>("list_rules");
  if (!rules) return;

  statRulesEl.textContent = String(rules.length);
  rulesBody.replaceChildren();
  rulesEmpty.classList.toggle("hidden", rules.length > 0);

  rules.forEach((rule, index) => {
    const row = document.createElement("tr");
    row.innerHTML = `
      <td><code>${escapeHtml(rule.target)}</code></td>
      <td><code>${escapeHtml(rule["target-value"])}</code></td>
      <td><code>${escapeHtml(rule.destination)}</code></td>
      <td><code>${escapeHtml(rule["destination-value"])}</code></td>
      <td class="actions">
        <button type="button" class="btn btn-ghost btn-sm" data-action="edit">Edit</button>
        <button type="button" class="btn btn-ghost btn-sm danger" data-action="delete">Delete</button>
      </td>
    `;

    row.querySelector('[data-action="edit"]')?.addEventListener("click", () => {
      openRuleDialog(index, rule);
    });
    row.querySelector('[data-action="delete"]')?.addEventListener("click", async () => {
      const label = `${rule.target}:${rule["target-value"]}`;
      if (!confirm(`Delete rule ${label}?`)) return;
      const result = await invokeOrToast<RuleMutationResult>("delete_rule", { index });
      if (result) {
        showToast(result.message + (result.live_apply_hint ? ` ${result.live_apply_hint}` : ""));
        await refreshAll();
      }
    });

    rulesBody.append(row);
  });
}

async function loadGateways() {
  const gateways = await invokeOrToast<GatewayRow[]>("list_gateways");
  if (!gateways) return;

  statGatewaysEl.textContent = String(gateways.length);
  gatewaysBody.replaceChildren();
  gatewaysEmpty.classList.toggle("hidden", gateways.length > 0);

  for (const gw of gateways) {
    const row = document.createElement("tr");
    row.innerHTML = `
      <td>${escapeHtml(gw.nic_name)}</td>
      <td><code>${escapeHtml(gw.gateway_ip)}</code></td>
    `;
    gatewaysBody.append(row);
  }
}

function openRuleDialog(index: number | null, rule?: RoutingRule) {
  editingRuleIndex = index;
  ruleDialogTitle.textContent = rule ? "Edit Rule" : "Add Rule";
  fillSelect(ruleTargetSelect, TARGET_OPTIONS, rule?.target);
  ruleTargetValueInput.value = rule?.["target-value"] ?? "";
  fillSelect(ruleDestinationSelect, DESTINATION_OPTIONS, rule?.destination);
  ruleDestinationValueInput.value = rule?.["destination-value"] ?? "";
  ruleDialog.showModal();
}

async function openImportDialog(filePath: string) {
  pendingImportPath = filePath;
  importFileLabel.textContent = filePath;
  importDialog.showModal();
}

async function startRulesImport() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "Rules", extensions: ["json"] }],
  });

  if (typeof selected !== "string") return;
  await openImportDialog(selected);
}

function escapeHtml(value: string): string {
  return value
    .split("&").join("&amp;")
    .split("<").join("&lt;")
    .split(">").join("&gt;")
    .split('"').join("&quot;");
}

async function refreshAll() {
  await Promise.all([loadStatus(), loadRules(), loadGateways()]);
}

document.querySelectorAll(".nav-item").forEach((button) => {
  button.addEventListener("click", () => {
    const view = button.getAttribute("data-view") as ViewId;
    setActiveView(view);
  });
});

document.getElementById("refresh-btn")?.addEventListener("click", () => {
  void refreshAll();
});

document.getElementById("start-btn")?.addEventListener("click", async () => {
  const msg = await invokeOrToast<string>("start_service");
  if (msg) {
    showToast(msg);
    await loadStatus();
  }
});

document.getElementById("stop-btn")?.addEventListener("click", async () => {
  const msg = await invokeOrToast<string>("stop_service");
  if (msg) {
    showToast(msg);
    await loadStatus();
  }
});

document.getElementById("restart-btn")?.addEventListener("click", async () => {
  const msg = await invokeOrToast<string>("restart_service");
  if (msg) {
    showToast(msg);
    await loadStatus();
  }
});

document.getElementById("add-rule-btn")?.addEventListener("click", () => {
  openRuleDialog(null);
});

document.getElementById("import-rules-btn")?.addEventListener("click", () => {
  void startRulesImport();
});

document.getElementById("import-cancel")?.addEventListener("click", () => {
  importDialog.close();
  pendingImportPath = null;
});

importForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!pendingImportPath) return;

  const result = await invokeOrToast<RuleMutationResult>("import_rules", {
    file_path: pendingImportPath,
  });

  if (result) {
    showToast(result.message + (result.live_apply_hint ? ` ${result.live_apply_hint}` : ""));
    importDialog.close();
    pendingImportPath = null;
    await refreshAll();
  }
});

document.getElementById("rule-cancel")?.addEventListener("click", () => {
  ruleDialog.close();
});

ruleForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const target = ruleTargetSelect.value as TargetKind;
  const target_value = ruleTargetValueInput.value.trim();
  const destination = ruleDestinationSelect.value as DestinationKind;
  const destination_value = ruleDestinationValueInput.value.trim();

  const args = { target, target_value, destination, destination_value };
  const result =
    editingRuleIndex !== null
      ? await invokeOrToast<RuleMutationResult>("edit_rule", {
          index: editingRuleIndex,
          ...args,
        })
      : await invokeOrToast<RuleMutationResult>("add_rule", args);

  if (result) {
    showToast(result.message + (result.live_apply_hint ? ` ${result.live_apply_hint}` : ""));
    ruleDialog.close();
    await refreshAll();
  }
});

document.getElementById("predict-form")?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const ip = (document.getElementById("predict-ip") as HTMLInputElement).value.trim();
  if (!ip) return;

  const result = await invokeOrToast<PredictResult>("predict_ip", { ip });
  if (!result) {
    predictResult.classList.add("hidden");
    return;
  }

  predictResult.classList.remove("hidden");
  predictResult.replaceChildren(
    row("Destination", result.destination),
    row("Interface Index", String(result.if_index)),
    row("Next Hop", result.next_hop),
    row("NIC Name", result.nic_name ?? "—"),
    row("NIC Description", result.nic_display ?? "—"),
  );
});

function row(label: string, value: string) {
  const dt = document.createElement("dt");
  dt.textContent = label;
  const dd = document.createElement("dd");
  dd.textContent = value;
  const wrap = document.createDocumentFragment();
  wrap.append(dt, dd);
  return wrap;
}

void (async () => {
  fillSelect(ruleTargetSelect, TARGET_OPTIONS, "cidr");
  fillSelect(ruleDestinationSelect, DESTINATION_OPTIONS, "ip");
  const path = await invokeOrToast<string>("get_config_path");
  if (path) {
    configPathLabel.textContent = path;
  }
  await refreshAll();
})();
