#Requires -Version 5.1
<#
.SYNOPSIS
  Remove local Roust stack: stop API/WebUI, uninstall Windows service Roust, clear state and install dir.
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module NetTCPIP -Force -ErrorAction SilentlyContinue | Out-Null

$DeployDir = $PSScriptRoot
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $DeployDir '..\..\..'))
$StateDir = Join-Path $RepoRoot '.armin\state'
$InstallDir = Join-Path $RepoRoot 'Roust'
$StateJson = Join-Path $StateDir 'state.json'
$ApiPidFile = Join-Path $StateDir 'api.pid'
$WebuiPidFile = Join-Path $StateDir 'webui.pid'
$ServiceName = 'Roust'
$DefaultApiPort = 8787
$DefaultWebUiPort = 5173

function Write-Step([string]$Message) { Write-Host ">> $Message" -ForegroundColor Cyan }
function Write-Ok([string]$Message) { Write-Host "OK  $Message" -ForegroundColor Green }
function Write-Fail([string]$Message) { Write-Host "ERR $Message" -ForegroundColor Red }

function Test-IsAdmin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    $prin = New-Object Security.Principal.WindowsPrincipal($id)
    return $prin.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Read-State {
    if (-not (Test-Path -LiteralPath $StateJson)) { return $null }
    try {
        return Get-Content -LiteralPath $StateJson -Raw | ConvertFrom-Json
    }
    catch {
        return $null
    }
}

function Stop-PidFile([string]$Path, [string]$Label) {
    if (-not (Test-Path -LiteralPath $Path)) { return }
    $pidText = Get-Content -LiteralPath $Path -ErrorAction SilentlyContinue
    if ($pidText) {
        $procId = [int]$pidText
        if (Get-Process -Id $procId -ErrorAction SilentlyContinue) {
            Write-Step "Stopping $Label (pid $procId)"
            & taskkill.exe /PID $procId /T /F 2>$null | Out-Null
            Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
        }
    }
    Remove-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
}

function Get-ListeningOwnerPids([int]$Port) {
    if ($Port -le 0) { return @() }
    try {
        $owners = @(Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction Stop |
            Select-Object -ExpandProperty OwningProcess -Unique)
        return @($owners | Where-Object { $_ -and $_ -gt 0 })
    }
    catch {
        $pids = @()
        $lines = & netstat.exe -ano -p tcp 2>$null
        foreach ($line in $lines) {
            if ($line -match "^\s*TCP\s+\S+:$Port\s+\S+\s+LISTENING\s+(\d+)\s*$") {
                $pids += [int]$Matches[1]
            }
        }
        return @($pids | Select-Object -Unique)
    }
}

function Stop-ListenersOnPort([int]$Port, [string]$Label) {
    if ($Port -le 0) { return }
    $owningPids = @(Get-ListeningOwnerPids $Port)
    foreach ($procId in $owningPids) {
        $proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
        if (-not $proc) { continue }
        Write-Step "Releasing $Label port $Port (pid $procId / $($proc.ProcessName))"
        & taskkill.exe /PID $procId /T /F 2>$null | Out-Null
        Stop-Process -Id $procId -Force -ErrorAction SilentlyContinue
    }
}

function Wait-PortFree([int]$Port, [int]$TimeoutSec = 15) {
    if ($Port -le 0) { return }
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        if (-not (Get-ListeningOwnerPids $Port)) { return }
        Start-Sleep -Milliseconds 400
    }
    throw "Port $Port is still listening after remove; cannot release it."
}

function Test-FileLocked([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return $false }
    try {
        $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
        $stream.Close()
        return $false
    }
    catch {
        return $true
    }
}

function Stop-WinDivertDriver {
    foreach ($name in @('WinDivert', 'WinDivert14')) {
        $queryOut = & sc.exe query $name 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0) { continue }
        if ($queryOut -notmatch 'RUNNING') { continue }

        Write-Step "Stopping kernel driver '$name'"
        & sc.exe stop $name 2>&1 | ForEach-Object { Write-Host $_ }
        $deadline = (Get-Date).AddSeconds(30)
        while ((Get-Date) -lt $deadline) {
            $again = & sc.exe query $name 2>&1 | Out-String
            if ($LASTEXITCODE -ne 0 -or $again -match 'STOPPED') { break }
            Start-Sleep -Milliseconds 400
        }
    }
}

function Assert-WinDivertSysUnlocked {
    $sys = Join-Path $InstallDir 'WinDivert64.sys'
    if ((Test-Path -LiteralPath $sys) -and (Test-FileLocked $sys)) {
        throw "WinDivert64.sys is still locked under $InstallDir after stopping WinDivert; cannot remove or replace it."
    }
}

function Uninstall-RoustService {
    $exeCandidates = @(
        (Join-Path $InstallDir 'roust.exe'),
        (Join-Path $RepoRoot 'roust-api\target\release\roust.exe')
    )
    $exe = $null
    foreach ($c in $exeCandidates) {
        if (Test-Path -LiteralPath $c) { $exe = $c; break }
    }

    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($svc) {
        if (-not (Test-IsAdmin)) {
            throw "Administrator elevation required to stop/uninstall Windows service '$ServiceName'."
        }
        if ($svc.Status -in @('Running', 'StartPending', 'PausePending', 'ContinuePending')) {
            Write-Step "Stopping Windows service '$ServiceName'"
            Stop-Service -Name $ServiceName -Force -ErrorAction SilentlyContinue
            try {
                $svc.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped, (New-TimeSpan -Seconds 45))
            }
            catch { }
        }
        if ($exe) {
            Write-Step "Uninstalling Windows service via $exe --uninstall-service"
            & $exe --uninstall-service 2>&1 | ForEach-Object { Write-Host $_ }
        }
        else {
            Write-Step "roust.exe not found; removing service '$ServiceName' with sc.exe"
            & sc.exe delete $ServiceName | Out-Null
        }
    }
    elseif ($exe) {
        Write-Step 'No Roust service registered; skipping uninstall'
    }
}

if ($args.Count -gt 0) {
    Write-Fail 'This script accepts no CLI arguments.'
    exit 1
}

try {
    Write-Step 'Removing local Windows Roust stack (API + WebUI + service)'

    $state = Read-State
    $apiPort = $DefaultApiPort
    $webUiPort = $DefaultWebUiPort
    if ($state -and $state.ports) {
        if ($state.ports.api) { $apiPort = [int]$state.ports.api }
        if ($state.ports.webui) { $webUiPort = [int]$state.ports.webui }
    }

    Stop-PidFile -Path $WebuiPidFile -Label 'WebUI'
    Stop-PidFile -Path $ApiPidFile -Label 'API'
    Stop-ListenersOnPort -Port $webUiPort -Label 'WebUI'
    Stop-ListenersOnPort -Port $apiPort -Label 'API'

    Uninstall-RoustService
    Stop-WinDivertDriver
    Assert-WinDivertSysUnlocked

    Wait-PortFree -Port $webUiPort
    Wait-PortFree -Port $apiPort

    if (Test-Path -LiteralPath $InstallDir) {
        Write-Step "Removing install directory $InstallDir"
        Remove-Item -LiteralPath $InstallDir -Recurse -Force -ErrorAction Stop
        if (Test-Path -LiteralPath $InstallDir) {
            $sys = Join-Path $InstallDir 'WinDivert64.sys'
            if (Test-Path -LiteralPath $sys) {
                throw "Install dir remove left WinDivert64.sys at $sys"
            }
            throw "Install directory still present after remove: $InstallDir"
        }
    }

    if (Test-Path -LiteralPath $StateDir) {
        Write-Step "Clearing state directory $StateDir"
        Remove-Item -LiteralPath $StateDir -Recurse -Force -ErrorAction Stop
    }

    Write-Ok 'Local Windows remove complete'
}
catch {
    Write-Fail $_.Exception.Message
    exit 1
}
