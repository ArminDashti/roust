# roust-api

Rust HTTP management API and Windows packet-router service for **roust**, split from [`ArminDashti/roust`](https://github.com/ArminDashti/roust).

Companion UI: [`ArminDashti/roust-webui`](https://github.com/ArminDashti/roust-webui).

## Binaries

| Binary | Role |
|--------|------|
| `roust.exe` | Windows service daemon (WinDivert packet router) |
| `roust-setup.exe` | First-run setup (WinDivert, IP lists, PATH) |
| `roust-api.exe` | Management HTTP API on `127.0.0.1:8787` |

## Management API

```powershell
cargo run --bin roust-api
# optional:
# cargo run --bin roust-api -- --bind 127.0.0.1:8787 --config .\routes.json
```

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/status` | Service installed/state, config path, rule count, version |
| GET | `/api/adapters` | List NICs (name, MAC, IPv4, status) |
| GET | `/api/processes` | Running processes (pid, image name, exe path when available) |
| POST | `/api/ping` | Temporary NIC-bound IPv4 ping (restores host route after) |
| GET | `/api/routes` | List routing rules (with index) |
| POST | `/api/routes` | Add a rule |
| PUT | `/api/routes/{index}` | Replace a rule |
| DELETE | `/api/routes/{index}` | Delete a rule |
| GET | `/api/app-binds` | List per-app NIC binds with status (`healthy` / `nic-down` / `unresolved`) |
| POST | `/api/app-binds` | Add an app→NIC bind |
| PUT | `/api/app-binds/{index}` | Replace a bind |
| DELETE | `/api/app-binds/{index}` | Delete a bind |

Ping body:

```json
{
  "host": "192.168.0.254",
  "nic": "Ethernet",
  "count": 4
}
```

`nic` matches friendly name, display name, or internal adapter name. A temporary `/32` host route is added only for the run (when missing), ICMP uses the NIC’s IPv4 as source, then the temp route is removed. Does not modify `routes.json` or restart the service. Adding the temporary route requires the same elevated privileges as other Roust route changes (run `roust-api` as Administrator).

Route body / fields match `routes.json`:

```json
{
  "target": "cidr",
  "target-value": "10.0.0.0/8",
  "destination": "ip",
  "destination-value": "192.168.1.1"
}
```

Successful writes persist `routes.json` and restart the **Roust** Windows service when it is Running.

### Per-app NIC binds (`app-binds.json`)

Stored beside `routes.json`. Prefer a full `exe-path`; `image-name` alone is resolved from a running process when the service applies WFP filters.

```json
[
  {
    "exe-path": "C:\\Program Files\\AppExample\\AppExample.exe",
    "image-name": "AppExample.exe",
    "nic": "Realtek"
  }
]
```

The elevated `roust` service installs user-mode WFP ALE filters (IPv4) so that app’s traffic is allowed only on the chosen NIC. If the NIC is down or has no IPv4, the app is **blocked** (fail-closed). Destination IP/CIDR routes in `routes.json` are unchanged (still WinDivert).

Successful app-bind writes persist `app-binds.json` and restart the **Roust** service when it is Running (so WFP filters reload).

CORS allows local Vite origins (`localhost:5173`, `127.0.0.1:5173`, and preview ports).

## Service install

Same as the original roust project:

```powershell
cargo build --release --bins
.\target\release\roust.exe --install-service
```

See the original [roust README](https://github.com/ArminDashti/roust) for WinDivert SDK setup and installers.

## Local build note

Copy the WinDivert 2.2.2 SDK to `WinDivert-2.2.2-A/` in the repo root (or set `ROUST_WINDIVERT_SDK`). For running `roust-api.exe`, also place `WinDivert.dll` next to the binary (from the SDK `x64` folder).
