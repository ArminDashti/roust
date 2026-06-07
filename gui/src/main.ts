import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

type ServiceStatus = {
  state: string;
  installed: boolean;
  directory: string;
  version: string;
};

type RoutingRule = {
  ip: string;
  gateway: string;
  rewrite_to: string | null;
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

let editingRuleIp: string | null = null;
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
const ruleIpInput = document.getElementById("rule-ip") as HTMLInputElement;
const ruleGatewayInput = document.getElementById("rule-gateway") as HTMLInputElement;
const ruleRewriteInput = document.getElementById("rule-rewrite") as HTMLInputElement;
const importDialog = document.getElementById("import-dialog") as HTMLDialogElement;
const importForm = document.getElementById("import-form") as HTMLFormElement;
const importFileLabel = document.getElementById("import-file-label")!;
const importGatewaySelect = document.getElementById("import-gateway") as HTMLSelectElement;
const importRewriteInput = document.getElementById("import-rewrite") as HTMLInputElement;

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

  for (const rule of rules) {
    const row = document.createElement("tr");
    row.innerHTML = `
      <td><code>${escapeHtml(rule.ip)}</code></td>
      <td><code>${escapeHtml(rule.gateway)}</code></td>
      <td>${rule.rewrite_to ? `<code>${escapeHtml(rule.rewrite_to)}</code>` : "—"}</td>
      <td class="actions">
        <button type="button" class="btn btn-ghost btn-sm" data-action="edit">Edit</button>
        <button type="button" class="btn btn-ghost btn-sm danger" data-action="delete">Delete</button>
      </td>
    `;

    row.querySelector('[data-action="edit"]')?.addEventListener("click", () => {
      openRuleDialog(rule);
    });
    row.querySelector('[data-action="delete"]')?.addEventListener("click", async () => {
      if (!confirm(`Delete rule for ${rule.ip}?`)) return;
      const result = await invokeOrToast<RuleMutationResult>("delete_rule", { ip: rule.ip });
      if (result) {
        showToast(result.message + (result.live_apply_hint ? ` ${result.live_apply_hint}` : ""));
        await refreshAll();
      }
    });

    rulesBody.append(row);
  }
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

function openRuleDialog(rule?: RoutingRule) {
  editingRuleIp = rule?.ip ?? null;
  ruleDialogTitle.textContent = rule ? "Edit Rule" : "Add Rule";
  ruleIpInput.value = rule?.ip ?? "";
  ruleIpInput.readOnly = Boolean(rule);
  ruleGatewayInput.value = rule?.gateway ?? "";
  ruleRewriteInput.value = rule?.rewrite_to ?? "";
  ruleDialog.showModal();
}

async function populateImportGatewaySelect() {
  const gateways = await invokeOrToast<GatewayRow[]>("list_gateways");
  importGatewaySelect.replaceChildren();

  const placeholder = document.createElement("option");
  placeholder.value = "";
  placeholder.textContent = "— Select if needed —";
  importGatewaySelect.append(placeholder);

  for (const gw of gateways ?? []) {
    const option = document.createElement("option");
    option.value = gw.gateway_ip;
    option.textContent = `${gw.gateway_ip} (${gw.nic_name})`;
    importGatewaySelect.append(option);
  }
}

async function openImportDialog(filePath: string) {
  pendingImportPath = filePath;
  importFileLabel.textContent = filePath;
  importRewriteInput.value = "";
  await populateImportGatewaySelect();
  importGatewaySelect.value = "";
  importDialog.showModal();
}

async function startRulesImport() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "Rules", extensions: ["json", "txt"] }],
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
  openRuleDialog();
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

  const defaultGatewayRaw = importGatewaySelect.value.trim();
  const default_gateway = defaultGatewayRaw.length > 0 ? defaultGatewayRaw : null;
  const rewriteRaw = importRewriteInput.value.trim();
  const rewrite_to = rewriteRaw.length > 0 ? rewriteRaw : null;

  const result = await invokeOrToast<RuleMutationResult>("import_rules", {
    file_path: pendingImportPath,
    default_gateway,
    rewrite_to,
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
  const ip = ruleIpInput.value.trim();
  const gateway = ruleGatewayInput.value.trim();
  const rewriteRaw = ruleRewriteInput.value.trim();
  const rewrite_to = rewriteRaw.length > 0 ? rewriteRaw : null;

  const result = editingRuleIp
    ? await invokeOrToast<RuleMutationResult>("edit_rule", { ip, gateway, rewrite_to })
    : await invokeOrToast<RuleMutationResult>("add_rule", { ip, gateway, rewrite_to });

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
  const path = await invokeOrToast<string>("get_config_path");
  if (path) {
    configPathLabel.textContent = path;
  }
  await refreshAll();
})();
