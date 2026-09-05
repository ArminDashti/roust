#Requires -Version 5.1
<#
.SYNOPSIS
  First-time native Windows install. Errors if already installed.
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$DeployDir = $PSScriptRoot
$ConfigPath = Join-Path $DeployDir 'install-on-local-windows.yaml'

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

function Test-Placeholder([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) { return $true }
    return $Value -match '<[^>]+>'
}

function Test-PidAlive([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return $false }
    $pidText = Get-Content -LiteralPath $Path -ErrorAction SilentlyContinue
    if (-not $pidText) { return $false }
    return [bool](Get-Process -Id ([int]$pidText) -ErrorAction SilentlyContinue)
}

function Test-PortInUse([int]$Port) {
    $listeners = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    return [bool]$listeners
}

function Test-TcpPortFree([int]$Port) { return -not (Test-PortInUse $Port) }

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
    # Always launch via cmd.exe so .cmd shims work and Start-Process never opens the
    # extensionless Node "npm" bash script in the default editor (often VS Code).
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

function Set-WebuiApiProxy([string]$WebuiDir, [int]$ApiPort) {
    $envFile = Join-Path $WebuiDir '.env'
    $envExample = Join-Path $WebuiDir '.env.example'
    if (-not (Test-Path -LiteralPath $envFile)) {
        if (Test-Path -LiteralPath $envExample) {
            Copy-Item -LiteralPath $envExample -Destination $envFile
        }
        else {
            Set-Content -LiteralPath $envFile -Value "VITE_API_PROXY_TARGET=http://127.0.0.1:$ApiPort"
            return
        }
    }
    $lines = Get-Content -LiteralPath $envFile
    $found = $false
    $updated = foreach ($line in $lines) {
        if ($line -match '^\s*VITE_API_PROXY_TARGET\s*=') {
            $found = $true
            "VITE_API_PROXY_TARGET=http://127.0.0.1:$ApiPort"
        }
        else { $line }
    }
    if (-not $found) {
        $updated = @($updated) + "VITE_API_PROXY_TARGET=http://127.0.0.1:$ApiPort"
    }
    Set-Content -LiteralPath $envFile -Value $updated
}

function Start-ApiProcess([string]$ApiDir, [int]$ApiPort, [string]$StateDir, [string]$ApiPidFile) {
    $stdoutLog = Join-Path $StateDir 'api.out.log'
    $stderrLog = Join-Path $StateDir 'api.err.log'

    if (Test-Path -LiteralPath (Join-Path $ApiDir 'Cargo.toml')) {
        Write-Step 'cargo build --bin roust-api (must run elevated)'
        $cargo = Resolve-CargoExe
        Push-Location $ApiDir
        try {
            & $cargo build --bin roust-api
            if ($LASTEXITCODE -ne 0) { throw 'cargo build --bin roust-api failed' }
        }
        finally { Pop-Location }
        $exe = Join-Path $ApiDir 'target\debug\roust-api.exe'
        if (-not (Test-Path -LiteralPath $exe)) { throw "Expected API binary missing: $exe" }
        $bind = "127.0.0.1:$ApiPort"
        Start-LoggedCmd -ExePath $exe `
            -ExeArgs @('--bind', $bind) `
            -WorkingDirectory $ApiDir -StdoutLog $stdoutLog -StderrLog $stderrLog -PidFile $ApiPidFile | Out-Null
        Wait-TcpPortListen -Port $ApiPort -TimeoutSec 180 -Label 'API (roust-api)' -ErrLog $stderrLog
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
    Write-Fail 'This script accepts no CLI arguments. Edit install-on-local-windows.yaml instead.'
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
    $postgresPort = [int](Require-Key $cfg 'postgres_port')
    $stateDirRel = Require-Key $cfg 'state_dir'
    $postgresContainer = Require-Key $cfg 'postgres_container'
    $postgresVolume = Require-Key $cfg 'postgres_volume'

    if (Test-Placeholder $repoRoot) { throw 'target_repo is still a placeholder.' }
    if (Test-Placeholder $stackName) { throw 'stack_name is still a placeholder.' }

    $apiDir = Join-Path $repoRoot $apiDirRel
    $webuiDir = Join-Path $repoRoot $webuiDirRel
    $stateDir = Join-Path $repoRoot $stateDirRel
    $apiPidFile = Join-Path $stateDir 'api.pid'
    $webuiPidFile = Join-Path $stateDir 'webui.pid'

    if ((Test-PidAlive $apiPidFile) -or (Test-PidAlive $webuiPidFile)) {
        throw "Already installed: stack '$stackName' appears running. Use update or reinstall."
    }
    if (-not (Test-TcpPortFree $apiPort)) {
        throw "api_port $apiPort is already in use. Re-author scripts with a free port (see create-ps-script-port-selection)."
    }
    if (-not (Test-TcpPortFree $webuiPort)) {
        throw "webui_port $webuiPort is already in use. Re-author scripts with a free port (see create-ps-script-port-selection)."
    }
    if (-not (Test-TcpPortFree $postgresPort)) {
        throw "postgres_port $postgresPort is already in use. Re-author scripts with a free port (see create-ps-script-port-selection)."
    }

    Write-Step "Install local Windows stack=$stackName"

    docker version *> $null
    if ($LASTEXITCODE -ne 0) { throw 'Docker is required for native Postgres. Start Docker Desktop.' }

    $existing = docker ps -a --filter "name=^/${postgresContainer}$" --format '{{.Names}}'
    if ($existing -eq $postgresContainer) {
        throw "Already installed: Postgres container '$postgresContainer' exists. Use update or reinstall."
    }

    Write-Step "Creating Postgres container $postgresContainer on port $postgresPort"
    docker run -d `
        --name $postgresContainer `
        -e POSTGRES_USER=localapps `
        -e POSTGRES_PASSWORD=localapps `
        -e POSTGRES_DB=localapps `
        -p "${postgresPort}:5432" `
        -v "${postgresVolume}:/var/lib/postgresql/data" `
        postgres:16-alpine | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Failed to create Postgres container' }

    New-Item -ItemType Directory -Force -Path $stateDir | Out-Null

    $envExample = Join-Path $apiDir '.env.example'
    $envFile = Join-Path $apiDir '.env'
    if (-not (Test-Path -LiteralPath $envFile) -and (Test-Path -LiteralPath $envExample)) {
        Copy-Item -LiteralPath $envExample -Destination $envFile
    }

    Start-ApiProcess -ApiDir $apiDir -ApiPort $apiPort -StateDir $stateDir -ApiPidFile $apiPidFile

    if (Test-Path -LiteralPath (Join-Path $webuiDir 'package.json')) {
        $npmCmd = Resolve-NpmCmd
        Push-Location $webuiDir
        try {
            Set-WebuiApiProxy -WebuiDir $webuiDir -ApiPort $apiPort
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

    Write-Ok "Local Windows install complete (api:$apiPort webui:$webuiPort postgres:$postgresPort)"
    Write-Host "Open http://127.0.0.1:$webuiPort/  (API http://127.0.0.1:$apiPort/)"
}
catch {
    Write-Fail $_.Exception.Message
    exit 1
}
