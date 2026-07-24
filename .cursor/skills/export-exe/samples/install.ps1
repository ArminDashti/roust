#Requires -Version 5.1
<#
.SYNOPSIS
  Stop APP_NAME if running, build/export Windows .exe to ./release, then run it.

.DESCRIPTION
  1) Stops matching app process or Windows service (so files are not locked)
  2) Builds the app for Windows
  3) Copies the .exe and required runtime files into an output folder (default: .\release)
  4) Starts the exported app from that folder

.EXAMPLE
  .\install.ps1
  .\install.ps1 --out=.\release
  .\install.ps1 --skip-stop
  .\install.ps1 --skip-run
  .\install.ps1 --help
#>

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

# AGENT: set these for the project
$AppName = "APP_NAME"
$AppExe = "APP_EXE"
# Leave empty if not a Windows service
$AppService = "APP_SERVICE"
# Optional sidecars to stop before export, e.g. @("sidecar")
$ExtraProcessNames = @()  # AGENT: EXTRA_PROCESS_NAMES

function Write-Info([string]$Message)  { Write-Host "[*] $Message" -ForegroundColor Cyan }
function Write-Ok([string]$Message)    { Write-Host "[+] $Message" -ForegroundColor Green }
function Write-Warn([string]$Message)  { Write-Host "[!] $Message" -ForegroundColor Yellow }
function Write-Err([string]$Message)   { Write-Host "[-] $Message" -ForegroundColor Red }
function Write-Step([string]$Message)  { Write-Host "" ; Write-Host "==> $Message" -ForegroundColor Magenta }

function Show-Help {
    Write-Host ""
    Write-Host "$AppName install.ps1 - stop, build, export, run .exe" -ForegroundColor White
    Write-Host ""
    Write-Host "Usage:" -ForegroundColor Cyan
    Write-Host "  .\install.ps1 [--out=<path>] [--skip-stop] [--skip-run] [--help]"
    Write-Host ""
    Write-Host "Parameters:" -ForegroundColor Cyan
    Write-Host "  --out=<path>      Folder to copy built binaries into (default: .\release)"
    Write-Host "  --skip-stop       Do not stop running app/service before build"
    Write-Host "  --skip-run        Do not start the exported app after copy"
    Write-Host "  --help            Show this help"
    Write-Host ""
    Write-Host "Examples:" -ForegroundColor Cyan
    Write-Host "  .\install.ps1"
    Write-Host "  .\install.ps1 --out=.\release"
    Write-Host "  .\install.ps1 --skip-run"
    Write-Host ""
    Write-Host "Output:" -ForegroundColor Cyan
    Write-Host "  Export folder: .\release\$AppExe  (+ required runtime files)"
    Write-Host ""
}

function Get-ArgValue {
    param(
        [string[]]$ArgList,
        [string]$Name,
        [string]$Default = $null
    )
    foreach ($a in $ArgList) {
        if ($a -eq $Name -or $a -eq ($Name + "=")) {
            return ""
        }
        if ($a.StartsWith($Name + "=")) {
            return $a.Substring($Name.Length + 1)
        }
    }
    return $Default
}

function Test-HasFlag {
    param(
        [string[]]$ArgList,
        [string]$Name
    )
    foreach ($a in $ArgList) {
        if ($a -eq $Name -or $a.StartsWith($Name + "=")) { return $true }
    }
    return $false
}

function Assert-Command {
    param([string]$Name)
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        Write-Err ("Required command not found: " + $Name)
        Write-Host "Install it, then re-run. See README prerequisites." -ForegroundColor Yellow
        Show-Help
        exit 1
    }
}

function Invoke-Step {
    param(
        [string]$Title,
        [scriptblock]$Action
    )
    Write-Step $Title
    & $Action
    if ($LASTEXITCODE -ne 0 -and $null -ne $LASTEXITCODE) {
        Write-Err ("Step failed: " + $Title + " (exit " + $LASTEXITCODE + ")")
        exit $LASTEXITCODE
    }
    Write-Ok ("Done: " + $Title)
}

function Get-AppProcessNames {
    $base = [System.IO.Path]::GetFileNameWithoutExtension($AppExe)
    $names = @($base)
    foreach ($n in $ExtraProcessNames) {
        if (-not [string]::IsNullOrWhiteSpace($n)) { $names += $n }
    }
    return ($names | Select-Object -Unique)
}

function Stop-AppOrService {
    param([int]$WaitSeconds = 15)

    $serviceName = $AppService
    if (-not [string]::IsNullOrWhiteSpace($serviceName) -and $serviceName -ne "APP_SERVICE") {
        $svc = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
        if ($svc) {
            if ($svc.Status -ne "Stopped") {
                Write-Info ("Stopping service: " + $serviceName)
                Stop-Service -Name $serviceName -Force -ErrorAction SilentlyContinue
                $svc.WaitForStatus("Stopped", [TimeSpan]::FromSeconds($WaitSeconds))
            }
            $svc = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
            if ($svc -and $svc.Status -ne "Stopped") {
                Write-Err ("Service still running: " + $serviceName)
                exit 1
            }
            Write-Ok ("Service stopped: " + $serviceName)
            return
        }
        Write-Warn ("Service not found (continuing with process stop): " + $serviceName)
    }

    foreach ($procName in Get-AppProcessNames) {
        $procs = @(Get-Process -Name $procName -ErrorAction SilentlyContinue)
        if ($procs.Count -eq 0) {
            Write-Info ("No running process: " + $procName)
            continue
        }
        Write-Info ("Stopping process: " + $procName + " (count=" + $procs.Count + ")")
        foreach ($p in $procs) {
            try { $p.CloseMainWindow() | Out-Null } catch { }
        }
        Start-Sleep -Seconds 2
        $procs = @(Get-Process -Name $procName -ErrorAction SilentlyContinue)
        if ($procs.Count -gt 0) {
            Write-Warn ("Force-killing process: " + $procName)
            $procs | Stop-Process -Force -ErrorAction SilentlyContinue
        }
        $deadline = (Get-Date).AddSeconds($WaitSeconds)
        while ((Get-Date) -lt $deadline) {
            $left = @(Get-Process -Name $procName -ErrorAction SilentlyContinue)
            if ($left.Count -eq 0) { break }
            Start-Sleep -Milliseconds 400
        }
        $left = @(Get-Process -Name $procName -ErrorAction SilentlyContinue)
        if ($left.Count -gt 0) {
            Write-Err ("Process still running (file may stay locked): " + $procName)
            exit 1
        }
        Write-Ok ("Process stopped: " + $procName)
    }
}

function Start-ReleasedApp {
    param([string]$ReleasedExe, [string]$WorkDir)

    $serviceName = $AppService
    if (-not [string]::IsNullOrWhiteSpace($serviceName) -and $serviceName -ne "APP_SERVICE") {
        $svc = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
        if ($svc) {
            Write-Info ("Starting service: " + $serviceName)
            Start-Service -Name $serviceName
            Write-Ok ("Service started: " + $serviceName)
            return
        }
        Write-Warn ("Service not found; starting exe instead: " + $serviceName)
    }

    if (-not (Test-Path $ReleasedExe)) {
        Write-Err ("Cannot run; missing: " + $ReleasedExe)
        exit 1
    }
    Write-Info ("Starting: " + $ReleasedExe)
    Start-Process -FilePath $ReleasedExe -WorkingDirectory $WorkDir
    Write-Ok ("Started: " + $ReleasedExe)
}

# --- parse args ---
$RawArgs = @($args)
if (Test-HasFlag -ArgList $RawArgs -Name "--help") {
    Show-Help
    exit 0
}

$OutDir = Get-ArgValue -ArgList $RawArgs -Name "--out" -Default (Join-Path $Root "release")
$SkipStop = Test-HasFlag -ArgList $RawArgs -Name "--skip-stop"
$SkipRun = Test-HasFlag -ArgList $RawArgs -Name "--skip-run"

# reject unknown flags
foreach ($a in $RawArgs) {
    if (-not $a.StartsWith("--")) {
        Write-Err ("Unknown argument: " + $a)
        Show-Help
        exit 1
    }
    $known = @("--out", "--help", "--skip-stop", "--skip-run")
    # AGENT: add stack-specific flags to $known (e.g. --skip-restore, --skip-npm)
    $ok = $false
    foreach ($k in $known) {
        if ($a -eq $k -or $a.StartsWith($k + "=")) { $ok = $true; break }
    }
    if (-not $ok) {
        Write-Err ("Unknown parameter: " + $a)
        Show-Help
        exit 1
    }
}

if ([string]::IsNullOrWhiteSpace($OutDir)) {
    Write-Err "Missing path for --out. Example: --out=.\release"
    Show-Help
    exit 1
}

if (-not [System.IO.Path]::IsPathRooted($OutDir)) {
    $OutDir = Join-Path $Root $OutDir
}

Write-Host ""
Write-Host "  $AppName release build" -ForegroundColor White
Write-Host "  --------------------" -ForegroundColor DarkGray
Write-Info ("Root: " + $Root)
Write-Info ("Out:  " + $OutDir)
Write-Host ""

# AGENT: TOOL_ASSERTS — e.g. Assert-Command "go" / "dotnet" / "python" / "cargo"
Assert-Command "REPLACE_ME"

Push-Location $Root
try {
    if (-not $SkipStop) {
        Invoke-Step "stop running app/service" {
            Stop-AppOrService
        }
    } else {
        Write-Warn "Skipped stop (--skip-stop)"
    }

    # AGENT: BUILD_BLOCK — replace with stack build from reference.md
    Invoke-Step "build release" {
        throw "AGENT: replace BUILD_BLOCK with real build commands"
    }

    # AGENT: locate built exe — set $BuiltExe to the absolute path of the produced .exe
    $BuiltExe = Join-Path $Root "REPLACE_ME\$AppExe"
    if (-not (Test-Path $BuiltExe)) {
        Write-Err ("Expected exe not found: " + $BuiltExe)
        exit 1
    }

    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

    # AGENT: COPY_BLOCK — copy exe + required runtime files into $OutDir
    Copy-Item -Force $BuiltExe (Join-Path $OutDir $AppExe)
    Write-Ok ("Copied $AppExe -> " + $OutDir)

    $ReleasedExe = Join-Path $OutDir $AppExe

    Write-Host ""
    Write-Ok "Build complete"
    Write-Host ("  App:    " + $BuiltExe) -ForegroundColor Green
    Write-Host ("  Export: " + $ReleasedExe) -ForegroundColor Green
    Write-Host ""

    if (-not $SkipRun) {
        Invoke-Step "run exported app" {
            Start-ReleasedApp -ReleasedExe $ReleasedExe -WorkDir $OutDir
        }
    } else {
        Write-Warn "Skipped run (--skip-run)"
    }
}
finally {
    Pop-Location
}
