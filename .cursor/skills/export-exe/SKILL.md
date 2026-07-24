---
name: export-exe
description: >-
  Creates a repo-root install.ps1 that stops any running app/service process,
  builds a Windows .exe, copies binary plus deps into ./release, then launches
  the exported app. Supports Go, C#, Python, Tauri, Node, and similar stacks.
  Use when the user asks to export an exe, create install.ps1, build a release
  folder, or package a desktop/CLI app for Windows.
---

# Export EXE

## Overview

- Owns create / edit of repo-root `install.ps1` that **stops** the running app or Windows service, builds a Windows `.exe`, exports it (plus deps) to `./release`, then **runs** the exported app
- Base the script on [samples/install.ps1](samples/install.ps1); fill build/copy steps from [reference.md](reference.md)
- Exclusions: signing/notarization, MSI/NSIS installers (unless user asks), Linux/macOS binaries, app source changes unrelated to packaging

## Objectives

1. Detect the project stack and app name
2. Write `install.ps1` at the repo root (Netvan-style CLI + logging)
3. Script **stops** matching process(es) / service before build so files are not locked
4. Script builds release `.exe`, then copies exe + required runtime files into `./release`
5. Script **starts** the exported app from `./release` after a successful export
6. Optionally run `.\install.ps1` when the user asks to build now

## Workflow

### Step 1: Detect project

| Signal | Stack |
|--------|-------|
| `go.mod` | Go |
| `*.csproj` / `*.sln` | C# / .NET |
| `pyproject.toml` / `requirements.txt` + entry script | Python |
| `src-tauri/tauri.conf.json` | Tauri (Rust + frontend) |
| `Cargo.toml` (bin) without Tauri | Rust |
| `package.json` with pkg / nexe / electron-builder | Node |

Confirm app display name, primary exe name, and (if any) Windows service name with the user if ambiguous.

### Step 2: Create `install.ps1`

1. Copy [samples/install.ps1](samples/install.ps1) into the **target repo root** as `install.ps1`
2. Replace placeholders (`APP_NAME`, `APP_EXE`, `APP_SERVICE`, build commands, copy sources) using the matching recipe in [reference.md](reference.md)
3. Keep the sample contract:
   - `#Requires -Version 5.1`
   - Logging helpers: `Write-Info`, `Write-Ok`, `Write-Warn`, `Write-Err`, `Write-Step`
   - Flags: `--out=<path>` (default `.\release`), `--help`, `--skip-stop`, `--skip-run`, plus stack-specific skips if useful (`--skip-restore`, `--skip-npm`, …)
   - `Assert-Command` for required tools before build
   - **Stop** running app/service → build → copy to `$OutDir` → **run** exported exe
   - Create `$OutDir`, copy `.exe` + necessary files, print summary paths
4. Do **not** execute the skill sample in place — only the project copy

### Step 3: Stop before build / export

**Always** stop the current app or service before build/copy (unless `--skip-stop`):

1. If `APP_SERVICE` is set and the service exists → `Stop-Service` and wait until stopped
2. Else stop processes whose name matches `APP_EXE` without extension (and any extra process names listed in the project script)
3. Prefer graceful stop; if still running after a short wait, force-kill (`Stop-Process -Force`)
4. Fail the script if processes remain that would lock the output files

Do this **before** build and copy so locked `.exe` / DLLs do not break export.

### Step 4: Required files into `./release`

Copy everything needed to run the exe offline on a typical Windows machine:

| Stack | Typical extras besides `.exe` |
|-------|-------------------------------|
| Go / Rust (static) | Often exe only |
| .NET (self-contained) | Framework-dependent: runtime DLLs; self-contained single-file: exe (+ `.pdb` optional) |
| Python (PyInstaller) | Entire `_internal/` folder if onedir; one-file: exe only |
| Tauri | Main exe; optional sidecar/service exes; leave installers under `target/release/bundle` (do not require copying bundles into `./release` unless asked) |
| Electron | Full `dist` / `win-unpacked` contents |

Never leave `./release` with only the exe when satellite DLLs, `_internal`, or config files are required at runtime.

### Step 5: Run after export

**Always** launch the exported app after a successful copy (unless `--skip-run`):

1. Resolve `$ReleasedExe = Join-Path $OutDir "APP_EXE"` (must exist)
2. If the project is a Windows service → `Start-Service` for `APP_SERVICE` (after copy), not a detached GUI start
3. Else `Start-Process -FilePath $ReleasedExe -WorkingDirectory $OutDir`
4. Log the started path; do not wait for the app to exit (non-blocking)

### Step 6: Verify (when user wants a build)

```powershell
.\install.ps1
.\install.ps1 --out=.\release
.\install.ps1 --skip-stop
.\install.ps1 --skip-run
.\install.ps1 --help
```

Confirm `./release` contains the exe and required companions, that any prior process was stopped, and that the app started from `./release`. Fix build errors before declaring done.

## Safety rules

1. **Never** invent absolute paths to toolchains or secrets; use `Get-Command` / PATH.
2. **Never** force-overwrite user customizations in an existing `install.ps1` without reading it first and preserving intentional flags.
3. **Never** commit build artifacts under `./release` unless the user asks.
4. **Always** default `--out` to `.\release` relative to the script root.
5. **Always** fail fast (`$ErrorActionPreference = "Stop"`) and non-zero exit on missing tools or missing built exe.
6. **Never** execute scripts from `.cursor/skills/export-exe/samples/` — copy into the target repo first.
7. **Always** stop matching app/service processes before build/export (unless `--skip-stop`).
8. **Always** run the exported app from `$OutDir` after a successful export (unless `--skip-run`).
9. **Never** kill unrelated processes — match by `APP_EXE` base name and optional `APP_SERVICE` only.

## Key facts & reference

| Item | Value |
|------|-------|
| Script path | `install.ps1` (repo root) |
| Default out | `./release` |
| Sample | [samples/install.ps1](samples/install.ps1) |
| Language recipes | [reference.md](reference.md) |
| Style source | Netvan `install.ps1` (arg parse, logging, copy-to-out) |
| Supported stacks | Go, C#/.NET, Python, Tauri, Rust, Node (pkg/nexe/electron) |
| Stop target | Process name = `APP_EXE` without `.exe`; optional Windows service `APP_SERVICE` |
| Run target | `Join-Path $OutDir APP_EXE` (or `Start-Service` when service) |

### Placeholder map (sample → project)

| Placeholder | Replace with |
|-------------|--------------|
| `APP_NAME` | Product / exe base name (e.g. `Netvan`) |
| `APP_EXE` | Output exe filename (e.g. `Netvan.exe`) |
| `APP_SERVICE` | Windows service name if applicable; otherwise leave empty / remove service branch |
| `BUILD_BLOCK` | Stack build commands from reference |
| `COPY_BLOCK` | Copy exe + deps into `$OutDir` |
| `TOOL_ASSERTS` | `Assert-Command` lines for required CLIs |
| `EXTRA_PROCESS_NAMES` | Optional extra process names to stop (sidecars) |
