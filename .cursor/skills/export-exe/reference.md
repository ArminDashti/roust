# Export EXE — language recipes

Fill `BUILD_BLOCK`, `COPY_BLOCK`, and `TOOL_ASSERTS` in the project `install.ps1`. Prefer self-contained / single-folder layouts that run without a pre-installed runtime when practical.

## Stop then run (required)

Every project `install.ps1` must keep the sample order:

1. **Stop** — `Stop-AppOrService` (unless `--skip-stop`)
2. **Build** — stack `BUILD_BLOCK`
3. **Copy** — exe + deps into `$OutDir`
4. **Run** — `Start-ReleasedApp` on `Join-Path $OutDir APP_EXE` (unless `--skip-run`)

| Placeholder | Meaning |
|-------------|---------|
| `APP_EXE` | Exe filename; process name = filename without `.exe` |
| `APP_SERVICE` | Windows service name; leave empty / `APP_SERVICE` placeholder removed if not a service |
| `EXTRA_PROCESS_NAMES` | Optional sidecar process names to stop |

Services: stop/start via `Stop-Service` / `Start-Service`. Plain apps: `Stop-Process` then `Start-Process` from `$OutDir`.

## Go

**Tools:** `go`

```powershell
Assert-Command "go"

Invoke-Step "go build (windows amd64)" {
    $env:GOOS = "windows"
    $env:GOARCH = "amd64"
    $env:CGO_ENABLED = "0"
    New-Item -ItemType Directory -Force -Path (Join-Path $Root "dist") | Out-Null
    go build -ldflags "-s -w" -o (Join-Path $Root "dist\APP_EXE") .
}

$BuiltExe = Join-Path $Root "dist\APP_EXE"
Copy-Item -Force $BuiltExe (Join-Path $OutDir "APP_EXE")
# Copy configs, migrations, or embedded-asset sidecars if the app expects them beside the exe
```

## C# / .NET

**Tools:** `dotnet`

Prefer self-contained single-file for a portable `./release`:

```powershell
Assert-Command "dotnet"

$PublishDir = Join-Path $Root "publish"
Invoke-Step "dotnet publish (win-x64 self-contained)" {
    dotnet publish -c Release -r win-x64 --self-contained true `
        -p:PublishSingleFile=true `
        -p:IncludeNativeLibrariesForSelfExtract=true `
        -o $PublishDir
}

$BuiltExe = Get-ChildItem -Path $PublishDir -Filter "*.exe" -File |
    Where-Object { $_.Name -notmatch "(?i)createdump|dotnet" } |
    Select-Object -First 1
if (-not $BuiltExe) { Write-Err "No published exe under publish/"; exit 1 }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Copy-Item -Force $BuiltExe.FullName (Join-Path $OutDir $BuiltExe.Name)
# If not single-file: Copy-Item -Recurse -Force "$PublishDir\*" $OutDir
```

Optional flag: `--skip-restore` → add `-p:Restore=false` or skip `dotnet restore` if you add an explicit restore step.

## Python (PyInstaller)

**Tools:** `python` (or `py`), preferably a venv with `pyinstaller` installed

```powershell
Assert-Command "python"

Invoke-Step "pip install pyinstaller (if needed)" {
    python -m pip install --quiet pyinstaller
}

# onedir is safer for apps with many data files; onefile for a single exe
Invoke-Step "pyinstaller" {
    python -m PyInstaller --noconfirm --clean --onedir --name APP_NAME `
        --distpath (Join-Path $Root "dist") `
        path\to\entry.py
}

$DistApp = Join-Path $Root "dist\APP_NAME"
if (-not (Test-Path $DistApp)) { Write-Err "PyInstaller output missing"; exit 1 }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Copy-Item -Recurse -Force "$DistApp\*" $OutDir
# Expect APP_EXE and _internal\ (or equivalent) under ./release
```

## Tauri

**Tools:** `node`, `npm`, `cargo`, `rustc`

Mirror Netvan: optional `--skip-npm`, then `npm run tauri build`, copy `target\release\APP_EXE` (or workspace `target\release\`) into `$OutDir`. Leave MSI/NSIS under `target\release\bundle\` unless the user wants those copied too.

```powershell
Assert-Command "node"
Assert-Command "npm"
Assert-Command "cargo"
Assert-Command "rustc"

if (-not $SkipNpm) {
    Invoke-Step "npm install" { npm install }
}

Invoke-Step "tauri build" {
    npm run tauri build
}

$ReleaseDir = Join-Path $Root "target\release"
# Also try src-tauri\target\release if not a Cargo workspace
$BuiltExe = @(
    (Join-Path $ReleaseDir "APP_EXE"),
    (Join-Path $ReleaseDir "app.exe")
) | Where-Object { Test-Path $_ } | Select-Object -First 1

Copy-Item -Force $BuiltExe (Join-Path $OutDir "APP_EXE")
```

## Rust (non-Tauri)

**Tools:** `cargo`, `rustc`

```powershell
Assert-Command "cargo"
Invoke-Step "cargo build --release" { cargo build --release }
$BuiltExe = Join-Path $Root "target\release\APP_EXE"
Copy-Item -Force $BuiltExe (Join-Path $OutDir "APP_EXE")
```

## Node (pkg / nexe)

**Tools:** `node`, `npm`

```powershell
Assert-Command "npm"
Invoke-Step "npm run build:exe" { npm run build:exe }
# Or: npx pkg . --targets node18-win-x64 --output dist\APP_EXE
$BuiltExe = Join-Path $Root "dist\APP_EXE"
Copy-Item -Force $BuiltExe (Join-Path $OutDir "APP_EXE")
# Copy adjacent .node bindings, config, or assets the binary loads at runtime
```

## Electron

Copy the full unpacked Windows build (not only the exe):

```powershell
$Unpacked = Join-Path $Root "dist\win-unpacked"
Copy-Item -Recurse -Force "$Unpacked\*" $OutDir
```

## Checklist before finishing

- [ ] `Assert-Command` covers every CLI used
- [ ] Stop runs before build/copy (unless `--skip-stop`)
- [ ] Build produces a real `.exe` path checked with `Test-Path`
- [ ] `$OutDir` default is `Join-Path $Root "release"`
- [ ] Runtime companions (DLLs, `_internal`, configs) are copied
- [ ] Exported app is started from `$OutDir` after copy (unless `--skip-run`)
- [ ] Help text lists real flags and output paths (`--skip-stop`, `--skip-run`)
- [ ] Placeholders `APP_NAME` / `APP_EXE` / `APP_SERVICE` / `REPLACE_ME` are gone
