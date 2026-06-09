#Requires -Version 5.1
<#
.SYNOPSIS
  Build release binaries and publish a portable Roust folder.

.DESCRIPTION
  1. Builds roust.exe and roust-setup.exe (cargo build --release --bins).
  2. Copies executables, WinDivert runtime, and routes.json into the output folder.

  Unlike installer.ps1, this script does not stop services, modify PATH, or register
  the Windows service. Use it to produce a folder you can zip, copy, or deploy.

  Default output folder: <current directory>\Roust
  Custom output folder:  .\publish.ps1 --path=C:\path\to\folder

.EXAMPLE
  .\publish.ps1

.EXAMPLE
  .\publish.ps1 -SkipBuild -Clean
#>
param(
    [string]$OutputDir,
    [switch]$SkipBuild,
    [switch]$Clean
)

$ErrorActionPreference = 'Stop'

$WorkingDir = (Get-Location).Path

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    foreach ($arg in $args) {
        if ($arg -match '^\-\-path\s*=\s*(.+)$') {
            $OutputDir = $Matches[1].Trim().Trim('"', "'")
            break
        }
    }
    if ([string]::IsNullOrWhiteSpace($OutputDir)) {
        for ($i = 0; $i -lt $args.Count; $i++) {
            if ($args[$i] -eq '--path' -and ($i + 1) -lt $args.Count) {
                $OutputDir = $args[$i + 1].Trim().Trim('"', "'")
                break
            }
        }
    }
}

if ([string]::IsNullOrWhiteSpace($OutputDir)) {
    $OutputDir = Join-Path $WorkingDir 'Roust'
}

$OutputDir = [System.IO.Path]::GetFullPath($OutputDir)

$AppRoot = $PSScriptRoot
if (-not $AppRoot) {
    $AppRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
}
$CoreRoot = Join-Path $AppRoot 'core'
if (-not (Test-Path -LiteralPath $CoreRoot)) {
    throw "Expected Rust project at $CoreRoot"
}
Set-Location -LiteralPath $CoreRoot

$ReleaseDir = Join-Path $CoreRoot 'target\release'
$BuiltRoust = Join-Path $ReleaseDir 'roust.exe'
$BuiltSetup = Join-Path $ReleaseDir 'roust-setup.exe'
$SourceRoutes = Join-Path $CoreRoot 'routes.json'
$PublishRoutes = Join-Path $OutputDir 'routes.json'

. (Join-Path $AppRoot 'msvc-env.ps1')

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message"
}

function Build-RoustRelease {
    if ($SkipBuild -and (Test-Path -LiteralPath $BuiltRoust) -and (Test-Path -LiteralPath $BuiltSetup)) {
        Write-Step "Skipping build; using existing binaries in $ReleaseDir"
        return
    }
    Ensure-MsvcLinker -ScriptName 'publish.ps1'
    Write-Step 'Building release binaries (cargo build --release --bins)...'
    & cargo build --release --bins
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release --bins failed with exit code $LASTEXITCODE"
    }
    foreach ($exe in @($BuiltRoust, $BuiltSetup)) {
        if (-not (Test-Path -LiteralPath $exe)) {
            throw "Expected binary not found after build: $exe"
        }
    }
}

function Initialize-PublishDirectory {
    if (-not (Test-Path -LiteralPath $OutputDir)) {
        Write-Step "Creating publish directory $OutputDir"
        New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
        return
    }
    if ($Clean) {
        Write-Step "Cleaning publish directory $OutputDir"
        Get-ChildItem -LiteralPath $OutputDir -Force | Remove-Item -Recurse -Force
    }
}

function Copy-PublishFile {
    param(
        [string]$Source,
        [string]$Destination
    )
    if (-not (Test-Path -LiteralPath $Source)) {
        throw "Missing publish file: $Source"
    }
    Write-Step "Copying $(Split-Path -Leaf $Source) -> $Destination"
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

function Publish-Executables {
    Copy-PublishFile -Source $BuiltRoust -Destination (Join-Path $OutputDir 'roust.exe')
    Copy-PublishFile -Source $BuiltSetup -Destination (Join-Path $OutputDir 'roust-setup.exe')
}

function Publish-WinDivertRuntime {
    $windivertDir = Join-Path $CoreRoot 'WinDivert-2.2.2-A\x64'
    foreach ($name in @('WinDivert.dll', 'WinDivert64.sys')) {
        $source = Join-Path $windivertDir $name
        Copy-PublishFile -Source $source -Destination (Join-Path $OutputDir $name)
    }
}

function Publish-RoutesJson {
    if (Test-Path -LiteralPath $PublishRoutes) {
        Write-Step "routes.json already present: $PublishRoutes"
        return
    }
    if (-not (Test-Path -LiteralPath $SourceRoutes)) {
        throw "routes.json missing and no source at $SourceRoutes"
    }
    Copy-PublishFile -Source $SourceRoutes -Destination $PublishRoutes
}

Write-Step "App root: $AppRoot"
Write-Step "Core project: $CoreRoot"
Write-Step "Publish directory: $OutputDir"

Build-RoustRelease
Initialize-PublishDirectory
Publish-Executables
Publish-WinDivertRuntime
Publish-RoutesJson

Write-Host ''
Write-Host 'Publish finished.'
Write-Host "  Folder:  $OutputDir"
Write-Host '  Files:   roust.exe, roust-setup.exe, WinDivert.dll, WinDivert64.sys, routes.json'
Write-Host '  Next:    copy or zip the folder, or run installer.ps1 to install locally.'
