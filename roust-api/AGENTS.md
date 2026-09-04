## Learned User Preferences

- Prefer Windows Filtering Platform (WFP) for routing going forward (stated product direction; exact WinDivert replacement approach still open).
- Per-app NIC binding should use true per-process enforcement (not launch-wrapper-only or destination-only routing).
- Identify bound apps by full executable path when available, with process image name as a fallback.
- When the bound NIC is down or has no IPv4, fail closed (block that app’s traffic) rather than fail open.
- NIC-bound ping should be a temporary UI/API action that restores prior host routes and must not change saved `routes.json` or the Roust service config.
- Keep per-app NIC binds in a separate config from destination routes (`app-binds.json`, not mixed into `routes.json`).

## Learned Workspace Facts

- Roust is Windows-only: `roust.exe` (WinDivert packet-router service), `roust-api` (local HTTP API), and `roust-webui` (Vue management UI).
- Persistence is JSON files on disk (`routes.json`, `app-binds.json`), not a SQL/NoSQL database (often under `C:\ProgramData\roust` or the working directory).
- Packet capture/rewrite uses WinDivert at the NETWORK layer (`IfIdx` on reinject); it does not use TUN/Wintun.
- Destination routing combines host routes (`route.exe` / IP Helper in `network/routes.rs`) with WinDivert interface forcing in `core/`; those rules match IP/CIDR/NIC/MAC only.
- Per-app NIC binding is enforced with user-mode WFP ALE filters inside elevated `roust.exe` (Roust provider/sublayer, fail-closed, adapter resync); API `GET /api/processes` and `/api/app-binds` CRUD; WebUI App binds page.
- Temporary NIC-bound ping is implemented (`network/ping.rs`, `POST /api/ping`, Ping page): source-bound ICMP plus a short-lived `/32` host route cleaned up after the run.
- Default local API listen port is `8787`; the WebUI Vite proxy (`VITE_API_PROXY_TARGET`) must point at that same port.
