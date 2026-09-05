#Requires -Version 5.1
<#
.SYNOPSIS
  Update native Windows stack: refresh code/deps; keep database and .env.
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$DeployDir = $PSScriptRoot
$ConfigPath = Join-Path $DeployDir 'update-on-local-windows.yaml'

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

function Test-PortInUse([int]$Port) {
    $listeners = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    return [bool]$listeners
}

function Resolve-NpmCmd {
    $cmd = Get-Command npm.cmd -ErrorAction SilentlyContinue
    if (-not $cmd) { throw 'npm.cmd not found on PATH. Install Node.js.' }
    return $cmd.Source
}

function Resolve-CargoExe {
    $cmd = Get-Command cargo.exe -ErrorAction SilentlyContinue
    if (-not $cmd) { throw 'cargo.exe not found on PATH. Install Rust (rustup).' }
    return $cmd.Source
}

function Resolve-GoExe {
    $cmd = Get-Command go.exe -ErrorAction SilentlyContinue
    if (-not $cmd) { throw 'go.exe not found on PATH. Install Go.' }
    return $cmd.Source
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
    $owners = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty OwningProcess -Unique
    foreach ($procId in $owners) {
        if ($procId -and (Get-Process -Id $procId -ErrorAction SilentlyContinue)) {
            Write-Step "Stopping $Label listener on port $Port (pid $procId)"
            & taskkill.exe /PID $procId /T /F 2>$null | Out-Null
        }
    }
}

function Wait-TcpPortListen([int]$Port, [int]$TimeoutSec, [string]$Label, [string]$ErrLog) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        if (Test-PortInUse $Port) { return }
        Start-Sleep -Seconds 1
    }
    $tail = ''
    if ($ErrLog -and (Test-Path -LiteralPath $ErrLog)) {
        $tail = (Get-Content -LiteralPath $ErrLog -Tail 40) -join "`n"
    }
    throw "$Label did not listen on port $Port within ${TimeoutSec}s.`n$tail"
}

function Start-LoggedCmd([string]$ExePath, [string[]]$ExeArgs, [string]$WorkingDirectory, [string]$StdoutLog, [string]$StderrLog, [string]$PidFile) {
    $quoted = @('"' + $ExePath + '"') + @($ExeArgs | ForEach-Object {
        if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ }
    })
    $inner = $quoted -join ' '
    $proc = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/d', '/c', $inner) `
        -WorkingDirectory $WorkingDirectory -PassThru `
        -RedirectStandardOutput $StdoutLog -RedirectStandardError $StderrLog `
        -WindowStyle Hidden
    $proc.Id | Set-Content -LiteralPath $PidFile
    return $proc
}

function Start-ApiProcess([string]$ApiDir, [int]$ApiPort, [string]$StateDir, [string]$ApiPidFile) {
    $stdoutLog = Join-Path $StateDir 'api.out.log'
    $stderrLog = Join-Path $StateDir 'api.err.log'

    if (Test-Path -LiteralPath (Join-Path $ApiDir 'Cargo.toml')) {
        Write-Step 'cargo fetch for roust-api'
        $cargo = Resolve-CargoExe
        Push-Location $ApiDir
        try {
            & $cargo fetch
            if ($LASTEXITCODE -ne 0) { throw 'cargo fetch failed' }
        }
        finally { Pop-Location }
        $bind = "127.0.0.1:$ApiPort"
        Start-LoggedCmd -ExePath $cargo `
            -ExeArgs @('run', '--bin', 'roust-api', '--', '--bind', $bind) `
            -WorkingDirectory $ApiDir -StdoutLog $stdoutLog -StderrLog $stderrLog -PidFile $ApiPidFile | Out-Null
        Wait-TcpPortListen -Port $ApiPort -TimeoutSec 180 -Label 'API (cargo)' -ErrLog $stderrLog
        return
    }

    if (Test-Path -LiteralPath (Join-Path $ApiDir 'go.mod')) {
        $go = Resolve-GoExe
        Push-Location $ApiDir
        try {
            & $go mod download
            if ($LASTEXITCODE -ne 0) { throw 'go mod download failed' }
        }
        finally { Pop-Location }
        Start-LoggedCmd -ExePath $go `
            -ExeArgs @('run', './cmd/server') `
            -WorkingDirectory $ApiDir -StdoutLog $stdoutLog -StderrLog $stderrLog -PidFile $ApiPidFile | Out-Null
        Wait-TcpPortListen -Port $ApiPort -TimeoutSec 120 -Label 'API (go)' -ErrLog $stderrLog
        return
    }

    throw "Unsupported API project in $ApiDir (expected Cargo.toml or go.mod)"
}

if ($args.Count -gt 0) {
    Write-Fail 'This script accepts no CLI arguments. Edit update-on-local-windows.yaml instead.'
    exit 1
}

try {
    $cfg = Read-FlatYaml $ConfigPath
    $repoRoot = Require-Key $cfg 'target_repo'
    $stackName = Require-Key $cfg 'stack_name'
    $apiDirRel = Require-Key $cfg 'api_dir'
    $webuiDirRel = Require-Key $cfg 'webui_dir'
    $apiPort = [int](Require-Key $cfg 'api_port')
    $webuiPort = [int](Require-Key $cfg 'webui_port')
    $stateDirRel = Require-Key $cfg 'state_dir'
    $postgresContainer = Require-Key $cfg 'postgres_container'

    $apiDir = Join-Path $repoRoot $apiDirRel
    $webuiDir = Join-Path $repoRoot $webuiDirRel
    $stateDir = Join-Path $repoRoot $stateDirRel
    $apiPidFile = Join-Path $stateDir 'api.pid'
    $webuiPidFile = Join-Path $stateDir 'webui.pid'

    Write-Step "Update local Windows stack=$stackName (data preserved)"
    Stop-PidFile -Path $webuiPidFile -Label 'WebUI'
    Stop-PidFile -Path $apiPidFile -Label 'API'
    Stop-ListenersOnPort -Port $webuiPort -Label 'WebUI'
    Stop-ListenersOnPort -Port $apiPort -Label 'API'

    docker version *> $null
    if ($LASTEXITCODE -eq 0) {
        $existing = docker ps -a --filter "name=^/${postgresContainer}$" --format '{{.Names}}'
        if ($existing -eq $postgresContainer) {
            $running = docker ps --filter "name=^/${postgresContainer}$" --format '{{.Names}}'
            if ($running -ne $postgresContainer) {
                docker start $postgresContainer | Out-Null
            }
        }
    }

    New-Item -ItemType Directory -Force -Path $stateDir | Out-Null

    Start-ApiProcess -ApiDir $apiDir -ApiPort $apiPort -StateDir $stateDir -ApiPidFile $apiPidFile

    if (Test-Path -LiteralPath (Join-Path $webuiDir 'package.json')) {
        $npmCmd = Resolve-NpmCmd
        Push-Location $webuiDir
        try {
            & $npmCmd install
            if ($LASTEXITCODE -ne 0) { throw 'npm install failed' }
            $env:VITE_API_PROXY_TARGET = "http://127.0.0.1:$apiPort"
            $webOut = Join-Path $stateDir 'webui.out.log'
            $webErr = Join-Path $stateDir 'webui.err.log'
            Start-LoggedCmd -ExePath $npmCmd `
                -ExeArgs @('run', 'dev', '--', '--host', '127.0.0.1', '--port', "$webuiPort", '--strictPort') `
                -WorkingDirectory $webuiDir -StdoutLog $webOut -StderrLog $webErr -PidFile $webuiPidFile | Out-Null
            Wait-TcpPortListen -Port $webuiPort -TimeoutSec 120 -Label 'WebUI (vite)' -ErrLog $webErr
        }
        finally { Pop-Location }
    }

    Write-Ok "Local Windows update complete (api:$apiPort webui:$webuiPort)"
    Write-Host "Open http://127.0.0.1:$webuiPort/  (API http://127.0.0.1:$apiPort/)"
}
catch {
    Write-Fail $_.Exception.Message
    exit 1
}
