#Requires -Version 5.1
<#
.SYNOPSIS
  Build roust.exe and install it to C:\Program Files\Roust.

.DESCRIPTION
  1. Stops running roust processes.
  2. Removes any existing roust.exe in the install folder.
  3. Builds the release binary with Cargo.
  4. Moves roust.exe into the install folder.
  5. Ensures the install folder is on the user PATH (for PowerShell/cmd).
  6. Ensures routes.json exists in the install folder.
  7. Copies WinDivert runtime files (WinDivert.dll, WinDivert64.sys) beside roust.exe.
#>
param(
    [string]$InstallDir = 'C:\Program Files\Roust',
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'

# Resolve repository root from the folder that contains this script.
$RepoRoot = $PSScriptRoot
if (-not $RepoRoot) {
    $RepoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
}
Set-Location -LiteralPath $RepoRoot

$ExeName = 'roust.exe'
$BuiltExe = Join-Path $RepoRoot 'target\release\roust.exe'
$InstallExe = Join-Path $InstallDir $ExeName
$InstallRoutes = Join-Path $InstallDir 'routes.json'
$SourceRoutes = Join-Path $RepoRoot 'routes.json'

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message"
}

function Stop-RoustProcesses {
    # Stop any running roust CLI or router process so files can be replaced.
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

function Remove-InstalledExe {
    # Delete the previous install binary when it is already on disk.
    if (Test-Path -LiteralPath $InstallExe) {
        Write-Step "Removing existing $InstallExe"
        Remove-Item -LiteralPath $InstallExe -Force
    }
}

function Build-RoustRelease {
    # Compile the release executable with Cargo (export step).
    if ($SkipBuild -and (Test-Path -LiteralPath $BuiltExe)) {
        Write-Step "Skipping build; using existing $BuiltExe"
        return
    }
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
    # Create install folder and move (cut) the built exe into Program Files.
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
    $windivertDir = Join-Path $RepoRoot 'WinDivert-2.2.2-A\x64'
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

# Program Files writes require elevation on Windows.
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)
if (-not $isAdmin) {
    throw 'Run this script in an elevated PowerShell (Run as administrator) to install under Program Files.'
}

Write-Step "Repository root: $RepoRoot"
Write-Step "Install directory: $InstallDir"

Stop-RoustProcesses
Remove-InstalledExe
Build-RoustRelease
Install-RoustExe
Add-InstallDirToUserPath
Ensure-RoutesJson
Install-WinDivertRuntime
Test-RoustOnPath

Write-Host ''
Write-Step 'Registering Windows service (requires elevation)...'
& $InstallExe service install 2>&1 | ForEach-Object { Write-Host $_ }
if ($LASTEXITCODE -ne 0) {
    Write-Warning 'Service install failed. Run manually as Administrator: roust service install'
} else {
    Write-Step 'Starting Windows service...'
    & $InstallExe start 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) {
        Write-Warning 'Service start failed. After fixing config, run: roust start'
    }
}

Write-Host ''
Write-Host 'Install finished.'
Write-Host "  Binary:  $InstallExe"
Write-Host "  Routes:  $InstallRoutes"
Write-Host '  Service: roust status   (Windows service name: Roust)'
Write-Host '  Logs:    logs\roust-service.log under the install folder'
Write-Host '  Open a new PowerShell window if `roust` is not found yet.'
