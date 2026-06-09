# Shared MSVC linker detection for publish.ps1 and installer.ps1.

function Write-MsvcStep {
    param([string]$Message)
    Write-Host "==> $Message"
}

function Test-LinkExeOnPath {
    return $null -ne (Get-Command -Name 'link.exe' -ErrorAction SilentlyContinue)
}

function Get-VsWherePath {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
        return $vswhere
    }
    return $null
}

function Get-VsInstallations {
    $vswhere = Get-VsWherePath
    if (-not $vswhere) {
        return @()
    }

    $json = & $vswhere -all -format json 2>$null
    if ([string]::IsNullOrWhiteSpace($json)) {
        return @()
    }

    return @($json | ConvertFrom-Json) | Where-Object {
        $_.installationPath -match '\\Microsoft Visual Studio\\' -or
        ($_.productPath -and $_.productPath -match 'devenv\.exe$')
    }
}

function Test-VsInstallationHasComponent {
    param(
        [string]$InstallationPath,
        [string[]]$Requires
    )

    if ($Requires.Count -eq 0) {
        return $true
    }

    $vswhere = Get-VsWherePath
    if (-not $vswhere) {
        return $false
    }

    $args = @(
        '-products', '*',
        '-version', '[17.0,18.0)',
        '-property', 'installationPath'
    )
    foreach ($component in $Requires) {
        $args += '-requires'
        $args += $component
    }

    $matches = @(& $vswhere @args 2>$null)
    foreach ($path in $matches) {
        if ([string]::IsNullOrWhiteSpace($path)) { continue }
        if ([System.IO.Path]::GetFullPath($path.Trim()) -ieq [System.IO.Path]::GetFullPath($InstallationPath)) {
            return $true
        }
    }
    return $false
}

function Get-VsInstallPath {
    param([string[]]$Requires = @())

    $installations = @(Get-VsInstallations)
    if ($installations.Count -eq 0) {
        return $null
    }

    if ($Requires.Count -gt 0) {
        foreach ($install in ($installations | Sort-Object { [version]$_.installationVersion } -Descending)) {
            if (Test-VsInstallationHasComponent -InstallationPath $install.installationPath -Requires $Requires) {
                return $install.installationPath.Trim()
            }
        }
        return $null
    }

    $preferred = $installations | Sort-Object { [version]$_.installationVersion } -Descending | Select-Object -First 1
    return $preferred.installationPath.Trim()
}

function Get-VsDisplayName {
    param([string]$VsInstallPath)

    $install = @(Get-VsInstallations) | Where-Object {
        [System.IO.Path]::GetFullPath($_.installationPath) -ieq [System.IO.Path]::GetFullPath($VsInstallPath)
    } | Select-Object -First 1

    if ($install -and -not [string]::IsNullOrWhiteSpace($install.displayName)) {
        return $install.displayName.Trim()
    }
    return $VsInstallPath
}

function Import-MsvcDevEnvironment {
    param([string]$VsInstallPath)

    $devShell = Join-Path $VsInstallPath 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll'
    if (-not (Test-Path -LiteralPath $devShell)) {
        return $false
    }

    Import-Module -Name $devShell -ErrorAction Stop
    Enter-VsDevShell -VsInstallPath $VsInstallPath -SkipAutomaticLocation -Arch amd64 -HostArch amd64 | Out-Null
    return (Test-LinkExeOnPath)
}

function Add-MsvcToolchainToPath {
    param([string]$VsInstallPath)

    $msvcRoot = Join-Path $VsInstallPath 'VC\Tools\MSVC'
    if (-not (Test-Path -LiteralPath $msvcRoot)) {
        return $false
    }

    $toolchain = Get-ChildItem -LiteralPath $msvcRoot -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending |
        Select-Object -First 1
    if (-not $toolchain) {
        return $false
    }

    $linkDir = Join-Path $toolchain.FullName 'bin\Hostx64\x64'
    if (-not (Test-Path -LiteralPath (Join-Path $linkDir 'link.exe'))) {
        return $false
    }

    $env:Path = "$linkDir;$env:Path"
    return (Test-LinkExeOnPath)
}

function Get-MsvcMissingHint {
    param([string]$ScriptName)

    $vsInstallations = @(Get-VsInstallations)
    $vsWithVc = Get-VsInstallPath -Requires @(
        'Microsoft.VisualStudio.Component.VC.Tools.x86.x64'
    )

    if ($vsInstallations.Count -gt 0 -and -not $vsWithVc) {
        $vsInstall = ($vsInstallations | Sort-Object { [version]$_.installationVersion } -Descending | Select-Object -First 1).installationPath
        $displayName = Get-VsDisplayName -VsInstallPath $vsInstall
        return @(
            "Visual Studio is installed ($displayName) but the C++ build tools are missing."
            ''
            'Add the C++ workload, then open a new PowerShell window and re-run {0}:' -f $ScriptName
            '  1. Open "Visual Studio Installer" from the Start menu'
            '  2. Click Modify on your Visual Studio installation'
            '  3. Enable workload: "Desktop development with C++"'
            '  4. Install, then restart PowerShell'
            ''
            'Or install Build Tools only:'
            '  https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022'
            '  (select "Desktop development with C++")'
        ) -join [Environment]::NewLine
    }

    return @(
        'The MSVC linker (link.exe) is required to build roust.exe but was not found.'
        ''
        "Install one of the following, then open a new PowerShell window and re-run $ScriptName`:"
        '  - Visual Studio Build Tools: https://visualstudio.microsoft.com/downloads/'
        '    (Workload: "Desktop development with C++")'
        '  - Visual Studio with the same C++ workload'
        ''
        'Rust is already using the x86_64-pc-windows-msvc toolchain; VS Code alone is not enough.'
        'See core/README.md → Build Windows `.exe` files yourself → Prerequisites.'
    ) -join [Environment]::NewLine
}

function Ensure-MsvcLinker {
    param([string]$ScriptName = 'this script')

    if (Test-LinkExeOnPath) {
        return
    }

    $candidates = @(
        Get-VsInstallPath -Requires @('Microsoft.VisualStudio.Component.VC.Tools.x86.x64')
    )
    if ($candidates.Count -eq 0 -or [string]::IsNullOrWhiteSpace($candidates[0])) {
        $candidates = @(Get-VsInstallations | Sort-Object { [version]$_.installationVersion } -Descending |
            ForEach-Object { $_.installationPath })
    }

    foreach ($vsInstall in $candidates) {
        if ([string]::IsNullOrWhiteSpace($vsInstall)) { continue }
        if (Import-MsvcDevEnvironment -VsInstallPath $vsInstall) {
            Write-MsvcStep 'Loaded MSVC environment from Visual Studio (link.exe was not on PATH).'
            return
        }
        if (Add-MsvcToolchainToPath -VsInstallPath $vsInstall) {
            Write-MsvcStep 'Added MSVC toolchain directory to PATH for this session.'
            return
        }
    }

    throw (Get-MsvcMissingHint -ScriptName $ScriptName)
}
