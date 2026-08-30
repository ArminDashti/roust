#Requires -Version 5.1
<#
.SYNOPSIS
  Build roust.exe and install it to .\Roust under the current directory.

.DESCRIPTION
  1. Stops the Roust Windows service (if installed and running).
  2. Stops any other running roust processes.
  3. Deletes all files in the install folder.
  4. Builds the release binary with Cargo.
  5. Installs roust.exe and runtime files into the install folder.
  6. Adds the install folder to the user PATH environment variable.
  7. Ensures routes.json exists in the install folder.
  8. Copies WinDivert runtime files (WinDivert.dll, WinDivert64.sys) beside roust.exe.

  Default install folder: <current directory>\Roust
  Custom install folder:   .\install-on-windows.ps1 --path=C:\path\to\folder
#>
param(
    [string]$InstallDir,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

# Capture the caller's working directory before we cd into the repo for the build.
$WorkingDir = (Get-Location).Path

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

# App root is the folder that contains this script; the Rust crate lives in the current directory.
$AppRoot = $PSScriptRoot
if (-not $AppRoot) {
    $AppRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
}
$CoreRoot = Join-Path $AppRoot '..'
if (-not (Test-Path -LiteralPath $CoreRoot)) {
    throw "Expected Rust project at $CoreRoot"
}
Set-Location -LiteralPath $CoreRoot

$ExeName = 'roust.exe'
$BuiltExe = Join-Path $CoreRoot 'target\release\roust.exe'
$InstallExe = Join-Path $InstallDir $ExeName
$InstallRoutes = Join-Path $InstallDir 'routes.json'
$SourceRoutes = Join-Path $AppRoot '..\routes.json'

. (Join-Path $AppRoot 'msvc-env.ps1')

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message"
}

function Stop-RoustService {
    # Stop the Windows service so install files are not locked.
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
    # Stop any running roust process so files can be replaced.
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
    # Remove every file and subdirectory in the install folder before a fresh install.
    if (-not (Test-Path -LiteralPath $InstallDir)) {
        return
    }
    Write-Step "Deleting all files in $InstallDir ..."
    Get-ChildItem -LiteralPath $InstallDir -Force | Remove-Item -Recurse -Force
}

function Build-RoustRelease {
    # Compile the release executable with Cargo (export step).
    if ($SkipBuild -and (Test-Path -LiteralPath $BuiltExe)) {
        Write-Step "Skipping build; using existing $BuiltExe"
        return
    }
    Ensure-MsvcLinker -ScriptName 'installer.ps1'
    Write-Step 'Building release binary (cargo build --release)...'
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release failed with exit code $LASTEXITCODE"
    }
    if (-not (Test-Path -LiteralPath $BuiltExe)) {
        throw "Expected binary not found after build: $BuiltExe"
    }
}

function Install-RoustExe {
    # Create install folder and move (cut) the built exe into the install directory.
    if (-not (Test-Path -LiteralPath $InstallDir)) {
        Write-Step "Creating install directory $InstallDir"
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    }
    Write-Step "Moving $BuiltExe -> $InstallExe"
    Move-Item -LiteralPath $BuiltExe -Destination $InstallExe -Force
}

function Test-InstallDirOnUserPath {
    # Return true when the install folder is already present on the user PATH.
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
    # Append install directory to user PATH when it is missing (PowerShell picks this up in new sessions).
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

function Install-WinDivertRuntime {
    # roust.exe loads WinDivert.dll from its own directory at runtime.
    $windivertDir = Join-Path $AppRoot '..\WinDivert-2.2.2-A\x64'
    $required = @('WinDivert.dll', 'WinDivert64.sys')
    foreach ($name in $required) {
        $source = Join-Path $windivertDir $name
        if (-not (Test-Path -LiteralPath $source)) {
            throw "WinDivert runtime missing at $source — ensure WinDivert-2.2.2-A/x64 is present in the repo."
        }
        $dest = Join-Path $InstallDir $name
        Write-Step "Copying $name -> $dest"
        Copy-Item -LiteralPath $source -Destination $dest -Force
    }
}

function Ensure-RoutesJson {
    # Copy default routes from the repo when the install folder has no routes.json yet.
    if (-not (Test-Path -LiteralPath $InstallRoutes)) {
        if (-not (Test-Path -LiteralPath $SourceRoutes)) {
            throw "routes.json missing in install dir and no source at $SourceRoutes"
        }
        Write-Step "Copying routes.json to $InstallRoutes"
        Copy-Item -LiteralPath $SourceRoutes -Destination $InstallRoutes
    }
    if (-not (Test-Path -LiteralPath $InstallRoutes)) {
        throw "routes.json not found at $InstallRoutes"
    }
    Write-Step "routes.json present: $InstallRoutes"
}

function Test-RoustOnPath {
    # Verify the shell can resolve roust.exe (current session after PATH update).
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

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)
if ((Test-InstallDirRequiresElevation) -and -not $isAdmin) {
    throw "Install directory '$InstallDir' is under Program Files. Run this script in an elevated PowerShell (Run as administrator), or choose another folder with --path=."
}

Write-Step "App root: $AppRoot"
Write-Step "Core project: $CoreRoot"
Write-Step "Install directory: $InstallDir"

Build-RoustRelease
Stop-RoustService
Stop-RoustProcesses
Clear-InstallDirectory
Install-RoustExe
Add-InstallDirToUserPath
Ensure-RoutesJson
Install-WinDivertRuntime
Test-RoustOnPath

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
