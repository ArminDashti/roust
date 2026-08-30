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
| GET | `/api/routes` | List routing rules (with index) |
| POST | `/api/routes` | Add a rule |
| PUT | `/api/routes/{index}` | Replace a rule |
| DELETE | `/api/routes/{index}` | Delete a rule |

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
