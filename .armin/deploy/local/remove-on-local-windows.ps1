#Requires -Version 5.1
<#
.SYNOPSIS
  Remove native Windows stack completely: stop app, free YAML ports, wipe DB if present.
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$DeployDir = $PSScriptRoot
$ConfigPath = Join-Path $DeployDir 'remove-on-local-windows.yaml'

function Write-Step([string]$Message) { Write-Host ">> $Message" -ForegroundColor Cyan }
function Write-Ok([string]$Message) { Write-Host "OK  $Message" -ForegroundColor Green }
function Write-Fail([string]$Message) { Write-Host "ERR $Message" -ForegroundColor Red }

function Read-FlatYaml([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { throw "Missing config: $Path" }
    $map = @{}
    foreach ($raw in Get-Content -LiteralPath $Path) {
        $line = $raw.Trim()
        if ($line -eq '' -or $line.StartsWith('#')) { continue }
        if ($line -notmatch '^(?<key>[^:#]+):\s*(?<val>.*)$') { continue }
        $map[$Matches['key'].Trim()] = $Matches['val'].Trim().Trim('"').Trim("'")
    }
    return $map
}

function Require-Key($Map, [string]$Key) {
    if (-not $Map.ContainsKey($Key) -or [string]::IsNullOrWhiteSpace([string]$Map[$Key])) {
        throw "YAML missing required key: $Key"
    }
    return [string]$Map[$Key]
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

function Stop-ListenersOnPort([int]$Port, [string]$Label) {
    if ($Port -le 0) { return }
    $listeners = @(Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue)
    if ($listeners.Count -eq 0) { return }
    $owningPids = @($listeners | Select-Object -ExpandProperty OwningProcess -Unique)
    foreach ($procId in $owningPids) {
        if ($procId -le 0) { continue }
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
        $stillListening = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
        if (-not $stillListening) { return }
        Start-Sleep -Milliseconds 400
    }
    throw "Port $Port is still listening after remove; cannot release it."
}

function Remove-PostgresIfExists([string]$ContainerName, [string]$VolumeName) {
    docker version *> $null
    if ($LASTEXITCODE -ne 0) {
        Write-Step 'Docker not available; skipping Postgres container/volume remove'
        return
    }

    $existing = docker ps -a --filter "name=^/${ContainerName}$" --format '{{.Names}}' 2>$null
    if ($existing -eq $ContainerName) {
        Write-Step "Removing Postgres container $ContainerName"
        docker rm -f $ContainerName | Out-Null
    }

    $volExists = docker volume ls --format '{{.Name}}' 2>$null | Where-Object { $_ -eq $VolumeName }
    if ($volExists) {
        Write-Step "Removing Postgres volume $VolumeName"
        docker volume rm -f $VolumeName | Out-Null
    }
}

if ($args.Count -gt 0) {
    Write-Fail 'This script accepts no CLI arguments. Edit remove-on-local-windows.yaml instead.'
    exit 1
}

try {
    $cfg = Read-FlatYaml $ConfigPath
    $repoRoot = Require-Key $cfg 'target_repo'
    $stackName = Require-Key $cfg 'stack_name'
    $apiPort = [int](Require-Key $cfg 'api_port')
    $webuiPort = [int](Require-Key $cfg 'webui_port')
    $postgresPort = [int](Require-Key $cfg 'postgres_port')
    $stateDirRel = Require-Key $cfg 'state_dir'
    $postgresContainer = Require-Key $cfg 'postgres_container'
    $postgresVolume = Require-Key $cfg 'postgres_volume'

    $stateDir = Join-Path $repoRoot $stateDirRel
    $apiPidFile = Join-Path $stateDir 'api.pid'
    $webuiPidFile = Join-Path $stateDir 'webui.pid'

    Write-Step "Removing local Windows stack=$stackName (app + ports + DB if present)"
    Stop-PidFile -Path $webuiPidFile -Label 'WebUI'
    Stop-PidFile -Path $apiPidFile -Label 'API'

    Stop-ListenersOnPort -Port $webuiPort -Label 'WebUI'
    Stop-ListenersOnPort -Port $apiPort -Label 'API'
    Stop-ListenersOnPort -Port $postgresPort -Label 'Postgres'

    Remove-PostgresIfExists -ContainerName $postgresContainer -VolumeName $postgresVolume

    Wait-PortFree -Port $webuiPort
    Wait-PortFree -Port $apiPort
    Wait-PortFree -Port $postgresPort

    if (Test-Path -LiteralPath $stateDir) {
        Remove-Item -LiteralPath $stateDir -Recurse -Force -ErrorAction SilentlyContinue
    }

    Write-Ok 'Local Windows remove complete (ports free; DB wiped if it existed)'
}
catch {
    Write-Fail $_.Exception.Message
    exit 1
}
