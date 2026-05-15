# roust

Windows packet router with rule-based NIC routing (WinDivert).

## Install (Windows)

### Recommended: setup wizard

1. Download **roust-setup-0.1.0-x64.exe** from the latest [GitHub Actions](https://github.com/ArminDashti/roust/actions) Windows build (artifact `roust-installer-x86_64`).
2. Run the installer (administrator rights are required for WinDivert).
3. Open a **new** PowerShell window and run:

```powershell
roust --help
roust nics list
```

The wizard installs to `C:\Program Files\roust`, bundles WinDivert, downloads Iran/private IP lists, and adds that folder to your **user** PATH.

### Manual / developer setup

Build release binaries, then run the console helper next to them:

```powershell
cargo build --release --bins
.\target\release\roust-setup.exe
```

Optional flags: `--dir <folder>`, `--install-rust` (downloads rustup only when needed), `--skip-lists`, `--skip-windivert`, `--skip-path`.

To build the wizard locally on Windows:

```powershell
cargo build --release --bins
.\installer\stage.ps1
# Install Inno Setup 6, then:
iscc installer\roust.iss
# Output: installer\output\roust-setup-0.1.0-x64.exe
```

## Uninstall

Use **Settings → Apps → roust → Uninstall**. The uninstaller removes the install folder from your user PATH.
