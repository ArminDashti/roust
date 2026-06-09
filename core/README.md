# roust

Windows packet router with rule-based routing by interface default gateway for inbound and outbound IPv4 (WinDivert). Release binaries are `roust.exe` (CLI) and `roust-setup.exe` (first-run setup).

**Technical overview:** [docs/HOW-IT-WORKS.md](docs/HOW-IT-WORKS.md) — architecture, packet flow, config, CLI, and setup.

## Install (Windows)

### Recommended: setup wizard

1. Download **roust-setup-0.1.0-x64.exe** from the latest [GitHub Actions](https://github.com/ArminDashti/roust/actions) Windows build (artifact `roust-installer-x86_64`).
2. Run the installer (administrator rights are required for WinDivert).
3. Open a **new** PowerShell window and run:

```powershell
roust --help
roust rule list
```

The wizard installs to `C:\Program Files\roust`, bundles WinDivert, downloads Iran/private IP lists, adds that folder to your **user** PATH, and registers a **Windows service** so the router runs in the background.

After install (elevated PowerShell):

```powershell
roust start             # start the Windows service
roust status            # check service state
roust stop              # stop the service
```

Service logs: `logs\roust-service.log` in the install directory.

### Manual / developer setup

Build release binaries, then run the console helper next to them:

```powershell
cd core
cargo build --release --bins
.\target\release\roust-setup.exe
```

Optional flags: `--dir <folder>`, `--install-rust` (downloads rustup only when needed), `--skip-lists`, `--skip-windivert`, `--skip-path`.

To build the wizard locally on Windows:

```powershell
cd core
cargo build --release --bins
.\installer\stage.ps1
# Install Inno Setup 6, then:
iscc installer\roust.iss
# Output: core\installer\output\roust-setup-0.1.0-x64.exe
```

## Uninstall

Use **Settings → Apps → roust → Uninstall**. The uninstaller removes the install folder from your user PATH.

## Build Windows `.exe` files yourself

You need a **64-bit Windows** machine (or VM) with the **MSVC** Rust toolchain so Cargo can link the vendored `WinDivert.lib`.

### Prerequisites

1. **Rust (stable), MSVC target** — Install from [https://rustup.rs](https://rustup.rs) and choose the default **`x86_64-pc-windows-msvc`** profile for 64-bit Intel/AMD Windows. The vendored WinDivert SDK in this repo ships `x64` and `x86` import libraries used by the build.
2. **Visual Studio Build Tools** (or full Visual Studio) with the **Desktop development with C++** workload so `link.exe` and the Universal CRT libraries are available.
3. **This repository** — Clone it and `cd` into `core/`. A WinDivert 2.2.2 SDK tree is already included as `WinDivert-2.2.2-A/` (headers and `x64` / `x86` import libraries).

### Optional: WinDivert folder somewhere else

If your WinDivert SDK lives outside the repo, set **`ROUST_WINDIVERT_SDK`** to the directory that contains `x64/WinDivert.lib` (and `x86/` if you build 32-bit) before invoking Cargo. The build script reads that variable on Windows.

### Compile release executables

From the `core/` directory:

```powershell
cd core
cargo build --release --bins
```

Artifacts:

- `core/target/release/roust.exe` — main CLI
- `core/target/release/roust-setup.exe` — downloads WinDivert beside the install if missing and can help with PATH setup

Debug builds use the same paths under `target/debug/`.

### After building

Copy `WinDivert-2.2.2-A\x64\WinDivert.dll` (and driver files as required by WinDivert) next to `roust.exe`, or run **`roust-setup.exe`** once from the folder where you want the install so it can lay down WinDivert and list files. See WinDivert licensing in `WinDivert-2.2.2-A/LICENSE`.

### CI reference

GitHub Actions builds the same way on `windows-latest`; see `.github/workflows/windows-build.yml` for the exact `cargo` invocation.

### Docker build (Linux / macOS / Windows with Docker Desktop)

From the **repository root** (not `core/`), cross-compile release Windows binaries inside a container. The image uses [cargo-xwin](https://github.com/rust-cross/cargo-xwin) and downloads the WinDivert SDK when import libraries are missing from the checkout.

```bash
docker compose run --rm build
```

Artifacts land in `./dist/`:

- `roust.exe`, `roust-setup.exe`
- `WinDivert.dll`, `WinDivert64.sys` (copy beside the executables on Windows)

**Note:** roust cannot *run* inside a Linux container — it needs WinDivert and host network adapters on Windows. Docker here is for reproducible **builds** only.

Override the WinDivert ZIP URL at build time:

```bash
docker compose build --build-arg WINDIVERT_ZIP_URL=https://example.com/WinDivert-2.2.2-A.zip build
```

### Cross-compiling from Linux or macOS (without Docker)

The build script rejects non-Windows host targets. From Linux or macOS you can still produce `.exe` files by cross-compiling, for example:

```bash
cd core
rustup target add x86_64-pc-windows-msvc
cargo xwin build --release --bins --target x86_64-pc-windows-msvc
```

You need `cargo install cargo-xwin` and a fetched WinDivert SDK (see `ROUST_WINDIVERT_SDK`). For the least friction, use Docker or build on Windows.
