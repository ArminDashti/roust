#Requires -Version 5.1
<#
.SYNOPSIS
  Reinstall: full remove then install via sibling scripts.
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$DeployDir = $PSScriptRoot

function Write-Step([string]$Message) { Write-Host ">> $Message" -ForegroundColor Cyan }
function Write-Ok([string]$Message) { Write-Host "OK  $Message" -ForegroundColor Green }
function Write-Fail([string]$Message) { Write-Host "ERR $Message" -ForegroundColor Red }

if ($args.Count -gt 0) {
    Write-Fail 'This script accepts no CLI arguments.'
    exit 1
}

try {
    $removeScript = Join-Path $DeployDir 'remove-on-local-windows.ps1'
    $installScript = Join-Path $DeployDir 'install-on-local-windows.ps1'
    if (-not (Test-Path -LiteralPath $removeScript)) { throw "Missing $removeScript" }
    if (-not (Test-Path -LiteralPath $installScript)) { throw "Missing $installScript" }

    Write-Step 'Reinstall local Windows: remove'
    & $removeScript
    if ($LASTEXITCODE -ne 0) { throw 'remove-on-local-windows.ps1 failed' }

    Write-Step 'Reinstall local Windows: install'
    & $installScript
    if ($LASTEXITCODE -ne 0) { throw 'install-on-local-windows.ps1 failed' }

    Write-Ok 'Local Windows reinstall complete'
}
catch {
    Write-Fail $_.Exception.Message
    exit 1
}
