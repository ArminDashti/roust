#Requires -Version 5.1
<#
.SYNOPSIS
  Reinstall: full remove then install. Forwards port set args to install.ps1.
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$DeployDir = $PSScriptRoot

function Write-Step([string]$Message) { Write-Host ">> $Message" -ForegroundColor Cyan }
function Write-Ok([string]$Message) { Write-Host "OK  $Message" -ForegroundColor Green }
function Write-Fail([string]$Message) { Write-Host "ERR $Message" -ForegroundColor Red }

try {
    $removeScript = Join-Path $DeployDir 'remove.ps1'
    $installScript = Join-Path $DeployDir 'install.ps1'
    if (-not (Test-Path -LiteralPath $removeScript)) { throw "Missing $removeScript" }
    if (-not (Test-Path -LiteralPath $installScript)) { throw "Missing $installScript" }

    if ($args.Count -gt 0) {
        if (-not ($args.Count -eq 3 -and $args[0] -eq 'port' -and $args[1] -eq 'set')) {
            throw "Unknown arguments. Usage: .\reinstall.ps1   or   .\reinstall.ps1 port set <NUMBER>"
        }
    }

    Write-Step 'Reinstall local Windows: remove'
    & $removeScript
    if ($LASTEXITCODE -ne 0) { throw 'remove.ps1 failed' }

    Write-Step 'Reinstall local Windows: install'
    if ($args.Count -gt 0) {
        & $installScript @args
    }
    else {
        & $installScript
    }
    if ($LASTEXITCODE -ne 0) { throw 'install.ps1 failed' }

    Write-Ok 'Local Windows reinstall complete'
}
catch {
    Write-Fail $_.Exception.Message
    exit 1
}
