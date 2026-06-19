## Learned User Preferences

- Prefer plain-language explanations when the user asks to simplify technical answers.
- Wants structured reports on what the app does, its components, architecture, and Windows integration.
- `installer.ps1` must default to `<cwd>\Roust`; override with `--path=`. Reinstall: stop **Roust** service → stop `roust` processes → delete install dir contents → build → install → update user PATH.
- Routing rules must target interface default gateways (or `rewrite-to` MAC/NIC/gateway), not legacy NIC-name-only fields in JSON.
- The router must run as a Windows **service** (daemon), not only as a foreground CLI process.
- Rule add/edit/remove must apply **live** without manual service restart when the service is running.
- `roust stop` and `roust restart` must succeed reliably (force-stop hung service/PIDs) in any state.
- When removing features or unused code, delete dead modules, dependencies, and APIs—not only silence warnings.
- The desktop UI is a **Tauri** app in `gui/` for Windows 11; it should support JSON file import for routing rules.

## Learned Workspace Facts

- **roust** is Windows-only: Rust service/core in `core/`, Tauri GUI in `gui/`, WinDivert for IPv4 packet steering.
- `roust.exe` is the Windows service binary; `roust-setup.exe` is first-run setup CLI—neither is the GUI. Double-clicking them opens a brief console; use the Roust app or PowerShell.
- The **Roust** Windows service loads `routes.json` from its install directory (often `C:\Program Files\Roust`), not the git `Roust\` folder unless installed there.
- Running service auto-reloads `routes.json` within ~1 second on file change; invalid JSON keeps previous rules.
- Egress steering uses Windows host routes at service start/reload; WinDivert cannot rely on outbound `IfIdx` alone.
- Service status in the GUI shows running/stopped state, install directory, and version.
- Primary rule/gateway/predict workflows live in the Tauri GUI; legacy CLI subcommands like `roust route`, `roust nics`, and `roust update` were removed.
- Detailed operational notes (SCM, WinDivert files, `rewrite-to`, validation) are in `.cursor/key-points.mdc`—read before debugging routing or install issues.
