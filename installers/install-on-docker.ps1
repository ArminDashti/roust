#Requires -Version 5.1
<#
.SYNOPSIS
  Cross-compile roust via Docker and install it locally.

.DESCRIPTION
  1. Verifies Docker Desktop is running.
  2. Runs the Docker cross-compile build to produce Windows binaries.
  3. Stops the Roust Windows service (if running) and any running roust processes.
  4. Installs the binaries, WinDivert runtime, and routes.json into the install folder.
  5. Adds the install folder to the user PATH environment variable.
  6. Optionally registers and starts the Windows service.

  Default install folder: <current directory>\Roust
  Custom install folder:   .\install-on-docker.ps1 --path=C:\path\to\folder

  Skip the Docker build (reuse existing ./dist artifacts):
  .\install-on-docker.ps1 --skip-build
#>
param(
    [string]$InstallDir,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

$WorkingDir = (Get-Location).Path

# ── Parse --path argument ─────────────────────────────────────────────
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    foreach ($arg in $args) {
        if ($arg -match '^\-\-path\s*=\s*(.+)$') {
            $InstallDir = $Matches[1].Trim().Trim('"', "'")
            break
        }
    }
    if ([string]::IsNullOrWhiteSpace($InstallDir)) {
        for ($i = 0; $i -lt $args.Count; $i++) {
            if ($args[$i] -eq '--path' -and ($i + 1) -lt $args.Count) {
                $InstallDir = $args[$i + 1].Trim().Trim('"', "'")
                break
            }
        }
    }
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $WorkingDir 'Roust'
}

$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)

# ── Resolve repo root and scripts directory ────────────────────────────
$ScriptDir = $PSScriptRoot
if (-not $ScriptDir) {
    $ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
}
$RepoRoot = Split-Path -Parent $ScriptDir
$DistDir = Join-Path $RepoRoot 'dist'

# ── Helpers ───────────────────────────────────────────────────────────
function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message"
}

function Stop-RoustService {
    $serviceName = 'Roust'
    $svc = Get-Service -Name $serviceName -ErrorAction SilentlyContinue
    if ($svc) {
        if ($svc.Status -in @('Running', 'StartPending', 'PausePending', 'ContinuePending')) {
            Write-Step "Stopping Windows service '$serviceName'..."
            Stop-Service -Name $serviceName -Force -ErrorAction Stop
            $svc.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped, (New-TimeSpan -Seconds 30))
        }
        return
    }

    Write-Step 'No Roust Windows service registered; stopping roust processes before install...'
    Stop-RoustProcesses
}

function Stop-RoustProcesses {
    $names = @('roust')
    foreach ($name in $names) {
        $procs = Get-Process -Name $name -ErrorAction SilentlyContinue
        if ($procs) {
            Write-Step "Stopping $($procs.Count) running '$name' process(es)..."
            $procs | Stop-Process -Force
            Start-Sleep -Seconds 1
        }
    }
}

function Clear-InstallDirectory {
    if (-not (Test-Path -LiteralPath $InstallDir)) {
        return
    }
    Write-Step "Deleting all files in $InstallDir ..."
    Get-ChildItem -LiteralPath $InstallDir -Force | Remove-Item -Recurse -Force
}

function Test-DockerAvailable {
    try {
        $null = & docker info 2>&1
        return $LASTEXITCODE -eq 0
    }
    catch {
        return $false
    }
}

function Build-RoustDocker {
    if ($SkipBuild) {
        if (-not (Test-Path -LiteralPath $DistDir)) {
            throw "Dist directory $DistDir does not exist. Run without --skip-build first."
        }
        Write-Step "Skipping Docker build; reusing existing artifacts in $DistDir"
        return
    }

    if (-not (Test-DockerAvailable)) {
        throw "Docker is not running or not installed. Please start Docker Desktop and try again."
    }

    Write-Step "Cross-compiling roust via Docker (docker compose run --rm build)..."
    Push-Location $ScriptDir
    try {
        & docker compose run --rm build
        if ($LASTEXITCODE -ne 0) {
            throw "Docker cross-compile failed with exit code $LASTEXITCODE"
        }
    }
    finally {
        Pop-Location
    }

    if (-not (Test-Path -LiteralPath (Join-Path $DistDir 'roust.exe'))) {
        throw "Expected roust.exe not found in $DistDir after Docker build"
    }
    Write-Step "Docker build succeeded. Artifacts in $DistDir"
    & Get-ChildItem -LiteralPath $DistDir | Format-Table Name, Length, LastWriteTime
}

function Install-RoustArtifacts {
    if (-not (Test-Path -LiteralPath $InstallDir)) {
        Write-Step "Creating install directory $InstallDir"
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }

    $artifacts = @('roust.exe', 'roust-setup.exe', 'WinDivert.dll', 'WinDivert64.sys')
    foreach ($name in $artifacts) {
        $source = Join-Path $DistDir $name
        if (-not (Test-Path -LiteralPath $source)) {
            throw "Artifact missing: $source"
        }
        $dest = Join-Path $InstallDir $name
        Write-Step "Copying $name -> $dest"
        Copy-Item -LiteralPath $source -Destination $dest -Force
    }
}

function Test-InstallDirOnUserPath {
    $installFull = [System.IO.Path]::GetFullPath($InstallDir)
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ([string]::IsNullOrWhiteSpace($userPath)) {
        return $false
    }
    foreach ($segment in ($userPath -split ';')) {
        if ([string]::IsNullOrWhiteSpace($segment)) { continue }
        try {
            $full = [System.IO.Path]::GetFullPath($segment)
            if ($full -ieq $installFull) {
                return $true
            }
        }
        catch {
            continue
        }
    }
    return $false
}

function Add-InstallDirToUserPath {
    if (Test-InstallDirOnUserPath) {
        Write-Step "Install directory is already on user PATH: $InstallDir"
        return
    }
    Write-Step "Adding $InstallDir to user PATH..."
    $installFull = [System.IO.Path]::GetFullPath($InstallDir)
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($null -eq $userPath) { $userPath = '' }
    $tail = if ($userPath -eq '' -or $userPath.EndsWith(';')) { '' } else { ';' }
    [Environment]::SetEnvironmentVariable('Path', ($userPath + $tail + $installFull), 'User')
    $env:Path = "$env:Path;$installFull"
}

function Ensure-RoutesJson {
    $InstallRoutes = Join-Path $InstallDir 'routes.json'
    $SourceRoutes = Join-Path $RepoRoot 'routes.json'
    if (-not (Test-Path -LiteralPath $InstallRoutes)) {
        if (-not (Test-Path -LiteralPath $SourceRoutes)) {
            Write-Warning "routes.json not found at $SourceRoutes; skipping."
            return
        }
        Write-Step "Copying routes.json to $InstallRoutes"
        Copy-Item -LiteralPath $SourceRoutes -Destination $InstallRoutes
    }
    Write-Step "routes.json present: $InstallRoutes"
}

function Test-RoustOnPath {
    $cmd = Get-Command -Name 'roust' -ErrorAction SilentlyContinue
    if ($cmd) {
        Write-Step "PATH check OK: roust -> $($cmd.Source)"
        return $true
    }
    Write-Warning "roust is not on PATH in this session yet. Open a new PowerShell window and run: Get-Command roust"
    return $false
}

function Test-InstallDirRequiresElevation {
    $installFull = [System.IO.Path]::GetFullPath($InstallDir)
    foreach ($root in @(
            [Environment]::GetFolderPath('ProgramFiles'),
            [Environment]::GetFolderPath('ProgramFilesX86')
        )) {
        if ([string]::IsNullOrWhiteSpace($root)) { continue }
        $rootFull = [System.IO.Path]::GetFullPath($root)
        if ($installFull.StartsWith($rootFull, [StringComparison]::OrdinalIgnoreCase)) {
            return $true
        }
    }
    return $false
}

# ── Main ──────────────────────────────────────────────────────────────
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)
if ((Test-InstallDirRequiresElevation) -and -not $isAdmin) {
    throw "Install directory '$InstallDir' is under Program Files. Run this script in an elevated PowerShell (Run as administrator), or choose another folder with --path=."
}

Write-Step "Repo root:   $RepoRoot"
Write-Step "Scripts dir: $ScriptDir"
Write-Step "Dist dir:    $DistDir"
Write-Step "Install dir: $InstallDir"

Build-RoustDocker
Stop-RoustService
Stop-RoustProcesses
Clear-InstallDirectory
Install-RoustArtifacts
Add-InstallDirToUserPath
Ensure-RoutesJson
Test-RoustOnPath

$InstallExe = Join-Path $InstallDir 'roust.exe'
$InstallRoutes = Join-Path $InstallDir 'routes.json'

Write-Host ''
Write-Step 'Registering Windows service (requires elevation)...'
& $InstallExe --install-service 2>&1 | ForEach-Object { Write-Host $_ }
if ($LASTEXITCODE -ne 0) {
    Write-Warning 'Service install failed. Run manually as Administrator: roust --install-service'
} else {
    Write-Step 'Starting Windows service...'
    try {
        Start-Service -Name 'Roust' -ErrorAction Stop
    } catch {
        Write-Warning 'Service start failed. After fixing config, start it from roust-setup.exe or run: Start-Service Roust'
    }
}

Write-Host ''
Write-Host 'Install finished.'
Write-Host "  Binary:  $InstallExe"
Write-Host "  Routes:  $InstallRoutes"
Write-Host '  Service: Start-Service Roust   (Windows service name: Roust)'
Write-Host '  Logs:    logs\roust-service.log under the install folder'
Write-Host '  Open a new PowerShell window if `roust` is not found yet.'
