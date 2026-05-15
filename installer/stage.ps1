# Stage files for Inno Setup (run on Windows after: cargo build --release --bins)
$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Staging = Join-Path $PSScriptRoot 'staging'
$Release = Join-Path $RepoRoot 'target\release'
$WinDivertZipUrl = 'https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip'

if (-not (Test-Path (Join-Path $Release 'roust.exe'))) {
    throw "Build roust first: cargo build --release --bins (expected $($Release)\roust.exe)"
}

if (Test-Path $Staging) {
    Remove-Item -Recurse -Force $Staging
}
New-Item -ItemType Directory -Path $Staging | Out-Null

Copy-Item (Join-Path $Release 'roust.exe') $Staging
Copy-Item (Join-Path $Release 'roust-setup.exe') $Staging
Copy-Item (Join-Path $RepoRoot 'private_ips.json') $Staging

$zipPath = Join-Path $env:TEMP 'WinDivert-2.2.2-A.zip'
Write-Host "Downloading WinDivert from $WinDivertZipUrl ..."
Invoke-WebRequest -Uri $WinDivertZipUrl -OutFile $zipPath -UseBasicParsing
Expand-Archive -Path $zipPath -DestinationPath $Staging -Force

$dll = Join-Path $Staging 'WinDivert-2.2.2-A\x64\WinDivert.dll'
if (-not (Test-Path $dll)) {
    throw "WinDivert x64 DLL not found after extract: $dll"
}

Write-Host "Staging complete: $Staging"
Get-ChildItem -Recurse $Staging | Select-Object FullName
