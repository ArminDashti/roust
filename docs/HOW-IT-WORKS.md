# How roust works

Technical overview of **roust** — a Windows-only CLI that routes inbound and outbound IPv4 traffic to specific network interfaces using rule-based configuration and [WinDivert](https://www.reqrypt.org/windivert.html) packet interception.

## Purpose

roust lets you send traffic destined for certain IP addresses (or CIDR ranges) out through a chosen NIC (Ethernet, Wi‑Fi, VPN adapter, etc.), optionally rewriting the packet’s destination IPv4 address before reinjection. Typical use cases include split routing (e.g. Iran or private IP blocks via one interface, everything else via another) without changing the Windows routing table for every prefix.

The tool does **not** replace the full TCP/IP stack. It sits in user space, captures **inbound and outbound** IPv4 packets with WinDivert, adjusts metadata (and optionally headers), and reinjects them so the kernel delivers or sends them on the interface you configured.

## High-level architecture

```mermaid
flowchart TB
    subgraph CLI["roust.exe (main binary)"]
        Parse[CLI parser - clap]
        ConfigIO[Config load/save - JSON]
        RuleList[rule list]
        PredictCmd[predict --ip]
        RuleCmd[add / delete / edit rules]
        Start[roust start]
    end

    subgraph Core["Packet engine"]
        Router[PacketRouter]
        Match[Rule matching - IP/CIDR/*]
        GwMap[default gateway → if_index map]
        WinDiv[WinDivert recv/send loop]
    end

    subgraph OS["Windows"]
        Adapters[GetAdaptersInfo]
        BestRoute[GetBestRoute]
        Driver[WinDivert.sys driver]
    end

    Parse --> ConfigIO
    Parse --> RuleList
    Parse --> PredictCmd
    Parse --> RuleCmd
    Parse --> Start
    Start --> Router
    ConfigIO --> Router
    Router --> Match
    Router --> GwMap
    GwMap --> Adapters
    PredictCmd --> BestRoute
    BestRoute --> Adapters
    Router --> WinDiv
    WinDiv --> Driver
```

## Binaries and crate layout

| Artifact | Path | Role |
|----------|------|------|
| `roust.exe` | `src/main.rs` | Main CLI: rules, rule list, egress prediction, `start` / `stop` / `status` |
| `roust-setup.exe` | `src/bin/roust-setup.rs` | Post-install: WinDivert ZIP, IP lists, user PATH |
| Library | `src/lib.rs` | Shared `setup` and `update` modules |

Cargo is configured for **Windows MSVC only** (`build.rs` panics on non-Windows targets). WinDivert is linked at build time from `WinDivert-2.2.2-A/` (or `ROUST_WINDIVERT_SDK`).

### Source modules

| Module | Responsibility |
|--------|----------------|
| `cli/` | Clap command tree and global `--config` |
| `config/` | `routes.json` (or `%ProgramData%\roust\routes.json`) — rules as JSON array |
| `network/` | `GetAdaptersInfo`, `GetBestRoute`, egress prediction |
| `core/` | `PacketRouter` + WinDivert FFI and safe handle wrapper |
| `update/` | Download Iran aggregated blocks and private IP lists |
| `setup/` | Installer helper: ZIP extract, PATH scripts, optional rustup |

## Configuration model

Rules live in `routes.json`. Default resolution order:

1. `%ProgramData%\roust\routes.json` if it exists  
2. Otherwise `./routes.json` in the current working directory (created on first `add` if missing)

You can override with `--config <path>`.

### Rule shape

```json
[
  {
    "ip": "192.168.1.0/24",
    "gateway": "192.168.1.1",
    "rewrite_to": "10.0.0.1"
  }
]
```

| Field | Meaning |
|-------|---------|
| `ip` | Exact IPv4/IPv6 string, CIDR (e.g. `10.0.0.0/8`), or `*` (match all) |
| `gateway` | Default gateway IPv4 of the target interface |
| `rewrite_to` | Optional: replace destination IPv4 in the packet before reinject |

**Matching order:** Rules are scanned in array order; the **first** matching rule wins (`config::Config::find_compiled` on the in-memory compiled table at runtime).

**Validation:** CIDR and single IPs are parsed with `ipnetwork` / `std::net::IpAddr`; `rewrite_to` must parse as an IP.

## CLI commands

### Egress prediction (no WinDivert)

- **`roust predict --ip <ipv4>`** — Calls `GetBestRoute` for the destination and prints `if_index`, next hop, and matched NIC name/description. This is what Windows would use **before** any roust rule is applied.

### Rule management

- **`roust rule list`** — Prints all rules from `routes.json` (or `--config` path).

Subcommands `add`, `delete`, `edit` use a shared `rule` action with flags:

- `--ip` — Single destination or CIDR  
- `--gateway` — Default gateway IPv4 of the target interface  
- `--rewrite-to` — Optional destination rewrite  
- `--file` — Bulk import from `.json` array or line-oriented text file  

On `add` and `edit`, the gateway must match a default gateway on a local interface (adapter list + IPv4 forward table). While the service is running, rule changes take effect automatically within about one second (config file watcher); restart is only needed after binary updates.

### Router lifecycle

| Command | Behavior |
|---------|----------|
| `roust start` | Start the Windows service (SCM runs `roust.exe --run-as-service`) |
| `roust stop` | Stop the Windows service |
| `roust restart` | Stop then start the service |
| `roust status` | Print SCM state and config path |

Service registration is done by **installer.ps1** or the hidden flag `roust --install-service` (elevated). Unregister with `roust --uninstall-service`.

### IP list updates

**roust-setup** downloads Iran aggregated and private IP lists via `update::run` and `update::run_private_ips` → `iran_aggregated.json`, `ipv4.txt`, `ipv6.txt`, `ipv4_cidr.txt`, `ipv6_cidr.txt`, `private_ips.json`, etc. in the install directory.

## Packet routing pipeline (`roust start`)

When the router runs, this is the per-packet flow:

```mermaid
sequenceDiagram
    participant App as Application
    participant Stack as Windows TCP/IP
    participant WD as WinDivert
    participant R as PacketRouter
    participant CFG as Config

    App->>Stack: IPv4 packet (inbound or outbound)
    Stack->>WD: intercept (filter: ip)
    WD->>R: WinDivertRecv(packet, address)
    R->>R: outbound? match dst : match src
    R->>CFG: find_compiled(peer IP)
    alt rule matches
        CFG-->>R: gateway + optional rewrite_to
        R->>R: kernel routes installed at start (if_index from gateway map)
        opt rewrite_to set
            R->>R: outbound: patch dst; inbound: patch src
            R->>R: recalc header checksum
        end
        R->>R: WinDivertHelperCalcChecksums (when header changed)
    end
    R->>WD: WinDivertSend(packet, address)
    WD->>Stack: reinject
    Stack->>App: packet continues on chosen interface
```

### WinDivert setup

- **Filter:** `"ip"` (all IPv4/IPv6 at network layer; the router only processes IPv4 headers)  
- **Layer:** `WINDIVERT_LAYER_NETWORK` (layer 0)  
- **Buffer:** up to `WINDIVERT_MTU_MAX` bytes per packet  

### Rule application

1. **Direction** — Read `WinDivertAddress.Outbound`: outbound packets use destination matching; inbound packets use source matching (the remote peer for traffic in both directions).  
2. **Match** — `Config::find_compiled` against pre-parsed rule patterns (exact, CIDR, or `*`).  
3. **Redirect interface** — At start, each rule’s `gateway` is resolved to `if_index` via `GetAdaptersAddresses` gateways and the IPv4 forward table (`0.0.0.0/0` per interface). Matching prefixes get `route add … via <gateway> IF <if_index>`; WinDivert does not set outbound `IfIdx`.  
4. **Optional rewrite** — If `rewrite_to` is set: outbound packets rewrite **destination**; inbound packets rewrite **source** (symmetric to split-tunnel semantics). Recompute the IPv4 header checksum.  
5. **Checksums** — `WinDivertHelperCalcChecksums` when the IPv4 header was modified.  
6. **Reinject** — **Every** packet is sent back (matched or not) so nothing is dropped.

### Shutdown

A console Ctrl+C handler sets a global atomic flag and calls `WinDivertShutdown` on the open handle so `WinDivertRecv` unblocks. On exit, the router prints separate routed vs passed-through counts for inbound and outbound.

## Network layer details

### Interface enumeration (`network/win.rs`)

Uses `GetAdaptersAddresses` (`GAA_FLAG_INCLUDE_GATEWAYS`) to collect:

- `AdapterName` → `name`  
- `Description` → `display_name`  
- `FriendlyName` → optional alias  
- `IfIndex` → `if_index` (used for host routes and route prediction)  
- First IPv4 unicast, first IPv4 gateway, MAC, coarse type (Ethernet / WiFi / Other)

### Egress prediction (`predict --ip`)

`GetBestRoute(dest, 0, &mut MIB_IPFORWARDROW)` returns the forward interface index and next hop. That index is correlated with the adapter list for human-readable NIC output. This is the **kernel routing table** view, independent of roust rules.

## Runtime bootstrap

On every `roust` launch, `main` ensures in the **current directory**:

- `settings.json` — created as `{}` if missing  

This file is a placeholder for future persistence; the active routing config is `routes.json` (or the path passed via `--config`).

## Setup and installation (`roust-setup`)

`setup::run` orchestrates:

1. **Logs directory** under the install folder  
2. **Optional Rust** — `rustup-init` if `--install-rust` or `ROUST_INSTALL_RUST` (skipped by default for end users)  
3. **WinDivert** — Download ZIP from GitHub releases (or `ROUST_WINDIVERT_ZIP_URL`), extract under install dir unless `WinDivert.dll` already exists  
4. **IP lists** — `update::run` + `update::run_private_ips` unless skipped  
5. **User PATH** — PowerShell script appends install dir (unless `--skip-path` / `ROUST_SKIP_PATH`)

The Inno Setup wizard (`installer/roust.iss`) installs to `C:\Program Files\roust`, runs staging, and bundles the same flow.

**Uninstall:** `roust-setup --uninstall-path` removes the install directory from the user PATH.

## Build and dependencies

- **Link time:** `build.rs` adds `WinDivert.lib` from `WinDivert-2.2.2-A/x64` (or x86).  
- **Run time:** `WinDivert.dll` and driver must be beside `roust.exe` (setup or manual copy). Administrator rights are typically required for WinDivert.  
- **Crates:** `clap`, `serde`/`serde_json`, `ipnetwork`, `windows` Win32 IP Helper APIs, `ureq` (HTTP), `zip` (setup).

## Environment variables

| Variable | Purpose |
|----------|---------|
| `ROUST_WINDIVERT_SDK` | Override path to WinDivert SDK for linking |
| `ROUST_WINDIVERT_ZIP_URL` | WinDivert ZIP URL for setup |
| `ROUST_IR_AGGREGATED_JSON_URL` | Iran IP JSON source for `update` |
| `ROUST_PRIVATE_IPS_JSON_URL` | Private IP JSON source |
| `ROUST_INSTALL_RUST` / `ROUST_SKIP_*` | Control setup steps (lists, path, windivert, rust) |
| `RUST_LOG` | Standard `env_logger` filter for verbose diagnostics |

## Security and operational notes

- **Privileges:** WinDivert installation and capture usually require elevation.  
- **Scope:** **Inbound and outbound IPv4** at the network layer; IPv6 packets are not matched or rewritten (they pass through unchanged).  
- **Rule vs route table:** `roust predict --ip` shows Windows’ choice; `start` installs more-specific host routes for matched prefixes so egress uses the rule’s gateway/interface, which can differ from `GetBestRoute` for those IPs.  
- **Windows service:** The router runs under SCM as service **Roust**; logs go to `logs/roust-service.log` in the install directory. Install via `installer.ps1` or `roust --install-service` (admin).  
- **Traffic integrity:** Modified packets get checksum recalculation; unmodified packets pass through unchanged.

## Example end-to-end workflow

```powershell
# 1. See what Windows would do without roust
roust predict --ip 8.8.8.8

# 2. Add rules (file or single IP) — use each interface's default gateway IPv4
roust add rule --file private_ips.json --gateway 192.168.1.1
roust add rule --ip 2.144.0.0/14 --gateway 10.0.0.1

# 3. List configured rules
roust rule list

# 4. Start interception (admin PowerShell)
roust start
```

## Planned / partial features

- Service recovery actions (restart on failure) and richer `status` output.  
- Deeper use of `settings.json` for state beyond routing rules.  
- Old configs used `"nic": "Ethernet"`; that field is rejected with a migration message. Use `"gateway": "<ipv4>"` (default gateway of the target interface).

## Related files

- User-facing install guide: [README.md](../README.md)  
- WinDivert SDK (vendored): `WinDivert-2.2.2-A/`  
- CI build: `.github/workflows/windows-build.yml`  
- Sample private ranges: [private_ips.json](../private_ips.json)
