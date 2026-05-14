# roust

Windows-oriented routing helper built in Rust. Release binaries are `roust.exe` (CLI) and `roust-setup.exe` (first-run setup).

## Build Windows `.exe` files yourself

You need a **64-bit Windows** machine (or VM) with the **MSVC** Rust toolchain so Cargo can link the vendored `WinDivert.lib`.

### Prerequisites

1. **Rust (stable), MSVC target** — Install from [https://rustup.rs](https://rustup.rs) and choose the default **`x86_64-pc-windows-msvc`** profile for 64-bit Intel/AMD Windows. The vendored WinDivert SDK in this repo ships `x64` and `x86` import libraries used by the build.
2. **Visual Studio Build Tools** (or full Visual Studio) with the **Desktop development with C++** workload so `link.exe` and the Universal CRT libraries are available.
3. **This repository** — Clone it and `cd` into the root. A WinDivert 2.2.2 SDK tree is already included as `WinDivert-2.2.2-A/` (headers and `x64` / `x86` import libraries).

### Optional: WinDivert folder somewhere else

If your WinDivert SDK lives outside the repo, set **`ROUST_WINDIVERT_SDK`** to the directory that contains `x64/WinDivert.lib` (and `x86/` if you build 32-bit) before invoking Cargo. The build script reads that variable on Windows.

### Compile release executables

From the repository root:

```powershell
cargo build --release --bins
```

Artifacts:

- `target/release/roust.exe` — main CLI
- `target/release/roust-setup.exe` — downloads WinDivert beside the install if missing and can help with PATH setup

Debug builds use the same paths under `target/debug/`.

### After building

Copy `WinDivert-2.2.2-A\x64\WinDivert.dll` (and driver files as required by WinDivert) next to `roust.exe`, or run **`roust-setup.exe`** once from the folder where you want the install so it can lay down WinDivert and list files. See WinDivert licensing in `WinDivert-2.2.2-A/LICENSE`.

### CI reference

GitHub Actions builds the same way on `windows-latest`; see `.github/workflows/windows-build.yml` for the exact `cargo` invocation.

### Cross-compiling from Linux or macOS

Producing `.exe` with the WinDivert import library is not documented here; use Windows (or a Windows runner) for reliable links.
