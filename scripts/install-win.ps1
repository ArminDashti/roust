#Requires -Version 5.1
<#
.SYNOPSIS
  Install or update local Roust: Windows service Roust (roust.exe) + API + WebUI processes.

.DESCRIPTION
  Fresh install when not present.
  When already installed: remove the app (service + binaries + API/WebUI processes) then reinstall;
  keep data (routes.json, app-binds.json, and any other non-app files under the install dir).
  Default ports: API 8787, WebUI 5173.
  Override API port:  .\install.ps1 port set <NUMBER>
#>
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# PowerShell 7 often fails NetTCPIP auto-load (alias conflicts); import explicitly when possible.
Import-Module NetTCPIP -Force -ErrorAction SilentlyContinue | Out-Null

$DeployDir = $PSScriptRoot
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $DeployDir '..\..\..'))
$ApiDir = Join-Path $RepoRoot 'roust-api'
$WebuiDir = Join-Path $RepoRoot 'roust-webui'
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

function Require-Admin([string]$Why) {
    if (-not (Test-IsAdmin)) {
        throw "Administrator elevation required: $Why. Re-run in elevated PowerShell."
    }
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

function Test-PortInUse([int]$Port) {
    return [bool](Get-ListeningOwnerPids $Port)
}

function Test-TcpPortFree([int]$Port) { return -not (Test-PortInUse $Port) }

function Test-PidAlive([string]$Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return $false }
    $pidText = Get-Content -LiteralPath $Path -ErrorAction SilentlyContinue
    if (-not $pidText) { return $false }
    return [bool](Get-Process -Id ([int]$pidText) -ErrorAction SilentlyContinue)
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

function Write-State([int]$ApiPort, [int]$WebUiPort, [string]$InstallPath) {
    New-Item -ItemType Directory -Force -Path $StateDir | Out-Null
    $obj = [ordered]@{
        stack_name    = 'roust'
        service_name  = $ServiceName
        installed     = $true
        ports         = @{ api = $ApiPort; webui = $WebUiPort }
        install_dir   = $InstallPath
        binary_path   = (Join-Path $InstallPath 'roust.exe')
        api_binary    = (Join-Path $InstallPath 'roust-api.exe')
        updated_at    = (Get-Date).ToString('o')
    }
    ($obj | ConvertTo-Json -Depth 5) | Set-Content -LiteralPath $StateJson -Encoding UTF8
}

function Test-AlreadyInstalled {
    $state = Read-State
    if ($state -and $state.installed) { return $true }
    if (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue) { return $true }
    if ((Test-PidAlive $ApiPidFile) -or (Test-PidAlive $WebuiPidFile)) { return $true }
    if (Test-Path -LiteralPath (Join-Path $InstallDir 'roust.exe')) { return $true }
    return $false
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
    $owners = @(Get-ListeningOwnerPids $Port)
    foreach ($procId in $owners) {
        $proc = Get-Process -Id $procId -ErrorAction SilentlyContinue
        if (-not $proc) { continue }
        Write-Step "Stopping $Label listener on port $Port (pid $procId)"
        & taskkill.exe /PID $procId /T /F 2>$null | Out-Null
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

function Resolve-WinDivertX64 {
    $candidates = @(
        (Join-Path $ApiDir 'WinDivert-2.2.2-A\x64'),
        (Join-Path $ApiDir 'assets\WinDivert-2.2.2-A\x64'),
        (Join-Path $ApiDir 'assets\WinDivert-2.2.2-A\WinDivert-2.2.2-A\x64')
    )
    foreach ($dir in $candidates) {
        if (Test-Path -LiteralPath (Join-Path $dir 'WinDivert.dll')) { return $dir }
    }
    throw 'WinDivert x64 runtime not found under roust-api (expected WinDivert-2.2.2-A/x64).'
}

function Set-WebuiApiProxy([string]$Dir, [int]$ApiPort) {
    $envFile = Join-Path $Dir '.env'
    $envExample = Join-Path $Dir '.env.example'
    $line = "VITE_API_PROXY_TARGET=http://127.0.0.1:$ApiPort"
    if (-not (Test-Path -LiteralPath $envFile)) {
        if (Test-Path -LiteralPath $envExample) {
            Copy-Item -LiteralPath $envExample -Destination $envFile
        }
        else {
            Set-Content -LiteralPath $envFile -Value $line
            return
        }
    }
    $lines = Get-Content -LiteralPath $envFile
    $found = $false
    $updated = foreach ($l in $lines) {
        if ($l -match '^\s*VITE_API_PROXY_TARGET\s*=') {
            $found = $true
            $line
        }
        else { $l }
    }
    if (-not $found) { $updated = @($updated) + $line }
    Set-Content -LiteralPath $envFile -Value $updated
}

function Stop-AppProcesses {
    Stop-PidFile -Path $WebuiPidFile -Label 'WebUI'
    Stop-PidFile -Path $ApiPidFile -Label 'API'
    $state = Read-State
    $apiPort = $DefaultApiPort
    $webPort = $DefaultWebUiPort
    if ($state -and $state.ports) {
        if ($state.ports.api) { $apiPort = [int]$state.ports.api }
        if ($state.ports.webui) { $webPort = [int]$state.ports.webui }
    }
    Stop-ListenersOnPort -Port $webPort -Label 'WebUI'
    Stop-ListenersOnPort -Port $apiPort -Label 'API'
}

function Stop-RoustServiceIfPresent {
    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if (-not $svc) { return }
    if ($svc.Status -in @('Running', 'StartPending', 'PausePending', 'ContinuePending')) {
        Write-Step "Stopping Windows service '$ServiceName'"
        Stop-Service -Name $ServiceName -Force -ErrorAction Stop
        $svc.WaitForStatus([System.ServiceProcess.ServiceControllerStatus]::Stopped, (New-TimeSpan -Seconds 45))
    }
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

function Uninstall-RoustServiceKeepData {
    $exeCandidates = @(
        (Join-Path $InstallDir 'roust.exe'),
        (Join-Path $ApiDir 'target\release\roust.exe')
    )
    $exe = $null
    foreach ($c in $exeCandidates) {
        if (Test-Path -LiteralPath $c) { $exe = $c; break }
    }

    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if (-not $svc) { return }

    Require-Admin "uninstall Windows service '$ServiceName' before update"
    Stop-RoustServiceIfPresent

    if ($exe) {
        Write-Step "Uninstalling Windows service via $exe --uninstall-service"
        & $exe --uninstall-service 2>&1 | ForEach-Object { Write-Host $_ }
    }
    else {
        Write-Step "roust.exe not found; removing service '$ServiceName' with sc.exe"
        & sc.exe delete $ServiceName | Out-Null
    }
}

function Remove-AppKeepData {
    Write-Step 'Removing installed app (keeping routes.json / app-binds.json and other data)'
    Stop-AppProcesses
    Uninstall-RoustServiceKeepData
    Stop-WinDivertDriver
    Assert-WinDivertSysUnlocked

    if (-not (Test-Path -LiteralPath $InstallDir)) { return }

    $appNames = @('roust.exe', 'roust-api.exe', 'WinDivert.dll', 'WinDivert64.sys')
    foreach ($name in $appNames) {
        $path = Join-Path $InstallDir $name
        if (Test-Path -LiteralPath $path) {
            Write-Step "Removing app file $name"
            Remove-Item -LiteralPath $path -Force -ErrorAction Stop
        }
    }

    $sysLeft = Join-Path $InstallDir 'WinDivert64.sys'
    if (Test-Path -LiteralPath $sysLeft) {
        throw "Failed to remove locked or leftover WinDivert64.sys at $sysLeft"
    }
}

function Build-ReleaseBinaries {
    $cargo = Resolve-CargoExe
    if (-not (Test-Path -LiteralPath (Join-Path $ApiDir 'Cargo.toml'))) {
        throw "Missing Cargo.toml at $ApiDir"
    }
    Write-Step 'cargo build --release --bin roust --bin roust-api'
    Push-Location $ApiDir
    try {
        & $cargo build --release --bin roust --bin roust-api
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
    }
    finally { Pop-Location }

    $roustBuilt = Join-Path $ApiDir 'target\release\roust.exe'
    $apiBuilt = Join-Path $ApiDir 'target\release\roust-api.exe'
    if (-not (Test-Path -LiteralPath $roustBuilt)) { throw "Missing build output: $roustBuilt" }
    if (-not (Test-Path -LiteralPath $apiBuilt)) { throw "Missing build output: $apiBuilt" }
}

function Install-BinariesToInstallDir {
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    $roustBuilt = Join-Path $ApiDir 'target\release\roust.exe'
    $apiBuilt = Join-Path $ApiDir 'target\release\roust-api.exe'
    Write-Step "Copying binaries to $InstallDir"
    Copy-Item -LiteralPath $roustBuilt -Destination (Join-Path $InstallDir 'roust.exe') -Force
    Copy-Item -LiteralPath $apiBuilt -Destination (Join-Path $InstallDir 'roust-api.exe') -Force

    Stop-WinDivertDriver
    Assert-WinDivertSysUnlocked

    $wd = Resolve-WinDivertX64
    foreach ($name in @('WinDivert.dll', 'WinDivert64.sys')) {
        $src = Join-Path $wd $name
        if (-not (Test-Path -LiteralPath $src)) { throw "Missing WinDivert file: $src" }
        Copy-Item -LiteralPath $src -Destination (Join-Path $InstallDir $name) -Force
    }

    foreach ($dataName in @('routes.json', 'app-binds.json')) {
        $dataInstall = Join-Path $InstallDir $dataName
        $dataSource = Join-Path $ApiDir $dataName
        if (-not (Test-Path -LiteralPath $dataInstall)) {
            if ($dataName -eq 'routes.json' -and -not (Test-Path -LiteralPath $dataSource)) {
                throw "routes.json missing at $dataSource"
            }
            if (Test-Path -LiteralPath $dataSource) {
                Write-Step "Seeding $dataName into install dir (first install only)"
                Copy-Item -LiteralPath $dataSource -Destination $dataInstall
            }
        }
        else {
            Write-Step "Keeping existing $dataName"
        }
    }
}

function Ensure-RoustService {
    Require-Admin 'install or start Windows service Roust'
    $exe = Join-Path $InstallDir 'roust.exe'
    if (-not (Test-Path -LiteralPath $exe)) { throw "Missing $exe" }

    Stop-RoustServiceIfPresent

    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($svc) {
        Write-Step 'Re-registering existing Roust service'
        & $exe --uninstall-service 2>&1 | ForEach-Object { Write-Host $_ }
    }

    Write-Step 'Installing Windows service Roust'
    & $exe --install-service 2>&1 | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) {
        throw 'roust.exe --install-service failed'
    }

    Write-Step 'Starting Windows service Roust'
    Start-Service -Name $ServiceName -ErrorAction Stop
}

function Start-Api([int]$ApiPort) {
    Require-Admin 'run roust-api (needs elevation for routes/WFP)'
    New-Item -ItemType Directory -Force -Path $StateDir | Out-Null
    $exe = Join-Path $InstallDir 'roust-api.exe'
    $config = Join-Path $InstallDir 'routes.json'
    $stdoutLog = Join-Path $StateDir 'api.out.log'
    $stderrLog = Join-Path $StateDir 'api.err.log'
    $bind = "127.0.0.1:$ApiPort"
    Write-Step "Starting roust-api on $bind"
    Start-LoggedCmd -ExePath $exe `
        -ExeArgs @('--bind', $bind, '--config', $config) `
        -WorkingDirectory $InstallDir `
        -StdoutLog $stdoutLog -StderrLog $stderrLog -PidFile $ApiPidFile | Out-Null
    Wait-TcpPortListen -Port $ApiPort -TimeoutSec 180 -Label 'API (roust-api)' -ErrLog $stderrLog
}

function Start-WebUi([int]$ApiPort, [int]$WebUiPort) {
    if (-not (Test-Path -LiteralPath (Join-Path $WebuiDir 'package.json'))) {
        throw "WebUI missing at $WebuiDir"
    }
    $npmCmd = Resolve-NpmCmd
    Set-WebuiApiProxy -Dir $WebuiDir -ApiPort $ApiPort
    Write-Step 'npm install (WebUI)'
    Push-Location $WebuiDir
    try {
        & $npmCmd install
        if ($LASTEXITCODE -ne 0) { throw 'npm install failed' }
        $env:VITE_API_PROXY_TARGET = "http://127.0.0.1:$ApiPort"
        $webOut = Join-Path $StateDir 'webui.out.log'
        $webErr = Join-Path $StateDir 'webui.err.log'
        Write-Step "Starting WebUI on 127.0.0.1:$WebUiPort"
        Start-LoggedCmd -ExePath $npmCmd `
            -ExeArgs @('run', 'dev', '--', '--host', '127.0.0.1', '--port', "$WebUiPort", '--strictPort') `
            -WorkingDirectory $WebuiDir `
            -StdoutLog $webOut -StderrLog $webErr -PidFile $WebuiPidFile | Out-Null
        Wait-TcpPortListen -Port $WebUiPort -TimeoutSec 120 -Label 'WebUI (vite)' -ErrLog $webErr
    }
    finally { Pop-Location }
}

function Parse-CliArgs([object[]]$CliArgs) {
    $apiPort = $DefaultApiPort
    $webUiPort = $DefaultWebUiPort
    $state = Read-State
    if ($state -and $state.ports -and $state.ports.api) {
        $apiPort = [int]$state.ports.api
    }
    if ($state -and $state.ports -and $state.ports.webui) {
        $webUiPort = [int]$state.ports.webui
    }

    if ($null -eq $CliArgs -or $CliArgs.Count -eq 0) {
        return @{ ApiPort = $apiPort; WebUiPort = $webUiPort }
    }

    if ($CliArgs.Count -eq 3 -and $CliArgs[0] -eq 'port' -and $CliArgs[1] -eq 'set') {
        $n = 0
        if (-not [int]::TryParse([string]$CliArgs[2], [ref]$n)) {
            throw "port set requires an integer NUMBER; got '$($CliArgs[2])'"
        }
        if ($n -lt 1024 -or $n -gt 65535) {
            throw "API port must be 1024-65535 (got $n)"
        }
        return @{ ApiPort = $n; WebUiPort = $DefaultWebUiPort }
    }

    throw "Unknown arguments. Usage: .\install.ps1   or   .\install.ps1 port set <NUMBER>"
}

try {
    $parsed = Parse-CliArgs -CliArgs $args
    $apiPort = [int]$parsed.ApiPort
    $webUiPort = [int]$parsed.WebUiPort
    $isUpdate = Test-AlreadyInstalled

    if ($isUpdate) {
        Write-Step "Update local Roust (remove app; keep data) api=$apiPort webui=$webUiPort"
        Remove-AppKeepData
    }
    else {
        Write-Step "Fresh install local Roust api=$apiPort webui=$webUiPort"
        if (-not (Test-TcpPortFree $apiPort)) {
            throw "API port $apiPort is already in use"
        }
        if (-not (Test-TcpPortFree $webUiPort)) {
            throw "WebUI port $webUiPort is already in use"
        }
    }

    Build-ReleaseBinaries
    Install-BinariesToInstallDir
    Ensure-RoustService
    Start-Api -ApiPort $apiPort
    Start-WebUi -ApiPort $apiPort -WebUiPort $webUiPort
    Write-State -ApiPort $apiPort -WebUiPort $webUiPort -InstallPath $InstallDir

    if ($isUpdate) {
        Write-Ok "Local Windows update complete (api:$apiPort webui:$webUiPort service:$ServiceName)"
    }
    else {
        Write-Ok "Local Windows install complete (api:$apiPort webui:$webUiPort service:$ServiceName)"
    }
    Write-Host "Open http://127.0.0.1:$webUiPort/  (API http://127.0.0.1:$apiPort/)"
}
catch {
    Write-Fail $_.Exception.Message
    exit 1
}
