#Requires -Version 5.1
<#
.SYNOPSIS
  Commit and push local changes for all Git repos (Windows 11 only).
.DESCRIPTION
  Reads config.json, discovers repos, ensures .gitignore and .gitkeep for empty folders,
  handles UTF-8/CRLF, auto-resolves simple conflicts, escalates critical conflicts to user.
#>

param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# ── defaults (overridden by config.json and CLI) ─────────────────────────────
$Script:ConfigPath         = $null
$Script:DirsFile           = 'useful-dirs.md'
$Script:MaxDepth            = 5
$Script:DryRun              = $false
$Script:RepoTimeoutSec      = 120
$Script:MaxFileSizeMB       = 5
$Script:MaxFileSizeBytes    = 5MB
$Script:EmptyFolderMarker   = '.gitkeep'
$Script:LineEnding          = 'crlf'
$Script:Utf8Bom             = $false
$Script:GitAutoCrlf         = $true
$Script:MaxAutoResolveFiles = 3
$Script:CriticalPatterns    = @('*.sln', '*.csproj', 'package.json', 'package-lock.json')
$Script:GitIgnoreAutoStart  = '# --- auto: github-commit-push-all-repos ---'
$Script:GitIgnoreAutoEnd    = '# --- end auto ---'
$Script:SkipDirs            = @('node_modules', '.cache', '.npm', '.yarn', 'vendor', '__pycache__', '.venv', 'venv', '.tox', 'dist', 'build')
$Script:SecretPat           = '(?i)(\.env$|\.pem$|secret|credential|password|token\.json)'

function Write-Color([string]$Text, [string]$Color = 'White') {
    Write-Host $Text -ForegroundColor $Color
}

function Show-Help {
    Write-Color @"

GitHub Commit Push All Repos (Windows 11)
=========================================

Usage:
  commit-push-all-repos.ps1 [--config=<path>] [--dirs-file=<path>] [--max-depth=<n>]
                            [--repo-timeout=<n>] [--max-file-size-mb=<n>] [--dry-run] [--help]

Parameters:
  --config=<path>        Config JSON (default: ../config.json next to scripts/)
  --dirs-file=<path>     Override config dirsFile
  --max-depth=<n>        Override config maxDepth
  --repo-timeout=<n>     Override config repoTimeoutSec
  --max-file-size-mb=<n> Override config maxFileSizeMB
  --dry-run              Preview without commit or push
  --help                 Show this help

User environment variable:
  GITHUB_TOKEN_PAT       GitHub PAT — Windows User environment variables

"@ 'Cyan'
}

foreach ($arg in $args) {
    switch -Regex ($arg) {
        '^--help$'              { Show-Help; exit 0 }
        '^--dry-run$'           { $Script:DryRun = $true; continue }
        '^--config=(.+)$'       { $Script:ConfigPath = $Matches[1]; continue }
        '^--dirs-file=(.+)$'    { $Script:DirsFile = $Matches[1]; continue }
        '^--max-depth=(\d+)$'   { $Script:MaxDepth = [int]$Matches[1]; continue }
        '^--repo-timeout=(\d+)$'{ $Script:RepoTimeoutSec = [int]$Matches[1]; continue }
        '^--max-file-size-mb=(\d+)$' {
            $Script:MaxFileSizeMB = [int]$Matches[1]
            $Script:MaxFileSizeBytes = $Script:MaxFileSizeMB * 1MB
            continue
        }
        default                 { Write-Color "Unknown argument: $arg (use --help)" 'Red'; exit 1 }
    }
}

function Get-SkillConfigPath {
    if ($Script:ConfigPath) { return $Script:ConfigPath }
    return Join-Path (Split-Path $PSScriptRoot -Parent) 'config.json'
}

function Import-SkillConfig {
    $path = Get-SkillConfigPath
    if (-not (Test-Path -LiteralPath $path)) {
        Write-Color "Config not found: $path (using built-in defaults)" 'Yellow'
        return
    }
    $cfg = Get-Content -LiteralPath $path -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($cfg.dirsFile)           { $Script:DirsFile = [string]$cfg.dirsFile }
    if ($cfg.maxDepth)           { $Script:MaxDepth = [int]$cfg.maxDepth }
    if ($cfg.repoTimeoutSec)     { $Script:RepoTimeoutSec = [int]$cfg.repoTimeoutSec }
    if ($cfg.maxFileSizeMB) {
        $Script:MaxFileSizeMB = [int]$cfg.maxFileSizeMB
        $Script:MaxFileSizeBytes = $Script:MaxFileSizeMB * 1MB
    }
    if ($cfg.emptyFolderMarker)  { $Script:EmptyFolderMarker = [string]$cfg.emptyFolderMarker }
    if ($cfg.lineEnding)         { $Script:LineEnding = [string]$cfg.lineEnding.ToLower() }
    if ($null -ne $cfg.utf8Bom)  { $Script:Utf8Bom = [bool]$cfg.utf8Bom }
    if ($null -ne $cfg.gitAutoCrlf) { $Script:GitAutoCrlf = [bool]$cfg.gitAutoCrlf }
    if ($cfg.maxAutoResolveConflictFiles) { $Script:MaxAutoResolveFiles = [int]$cfg.maxAutoResolveConflictFiles }
    if ($cfg.criticalConflictPatterns) { $Script:CriticalPatterns = @($cfg.criticalConflictPatterns) }
    if ($cfg.skipDirs)           { $Script:SkipDirs = @($cfg.skipDirs) }
    if ($cfg.secretPatterns)     { $Script:SecretPat = [string]$cfg.secretPatterns }
    Write-Color "Config: $path" 'DarkGray'
}

Import-SkillConfig

# ── encoding / line endings (Win 11) ─────────────────────────────────────────
function Get-Eol {
    if ($Script:LineEnding -eq 'lf') { return "`n" }
    return "`r`n"
}

function Write-RepoTextFile {
    param([string]$Path, [string]$Content)
    $eol = Get-Eol
    $normalized = ($Content -replace "`r`n", "`n" -replace "`n", $eol).TrimEnd($eol.ToCharArray()) + $eol
    $enc = New-Object System.Text.UTF8Encoding $Script:Utf8Bom
    $dir = Split-Path -LiteralPath $Path -Parent
    if ($dir -and -not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    [System.IO.File]::WriteAllText($Path, $normalized, $enc)
}

function Read-RepoTextFile {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return $null }
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $enc = New-Object System.Text.UTF8Encoding $false
    return $enc.GetString($bytes)
}

function Set-RepoGitEolConfig {
    param([string]$RepoPath)
    if (-not $Script:GitAutoCrlf) { return }
    $null = Invoke-Git $RepoPath @('config', 'core.autocrlf', 'true')
    $null = Invoke-Git $RepoPath @('config', 'core.safecrlf', 'warn')
}

# ── git helpers ───────────────────────────────────────────────────────────────
function Invoke-Git {
    param([string]$RepoPath, [string[]]$GitArgs)
    try {
        $out = & git -C $RepoPath @GitArgs 2>&1
        return @{ Code = $LASTEXITCODE; Out = ($out -join "`n").Trim(); TimedOut = $false }
    }
    catch {
        return @{ Code = -1; Out = $_.Exception.Message; TimedOut = $false }
    }
}

function Invoke-GitWithTimeout {
    param([string]$RepoPath, [string[]]$GitArgs, [int]$TimeoutSec = $Script:RepoTimeoutSec)
    $argList = @('-C', $RepoPath) + $GitArgs
    $quoted = $argList | ForEach-Object {
        if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ }
    }
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = 'git'
    $psi.Arguments = $quoted -join ' '
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    try {
        $proc = [System.Diagnostics.Process]::Start($psi)
        if (-not $proc.WaitForExit($TimeoutSec * 1000)) {
            try { $proc.Kill() } catch {}
            return @{ Code = -1; Out = "git $($GitArgs -join ' ') timed out after ${TimeoutSec}s"; TimedOut = $true }
        }
        $out = ($proc.StandardOutput.ReadToEnd() + "`n" + $proc.StandardError.ReadToEnd()).Trim()
        return @{ Code = $proc.ExitCode; Out = $out; TimedOut = $false }
    }
    catch {
        return @{ Code = -1; Out = $_.Exception.Message; TimedOut = $false }
    }
}

function Reset-RepoState {
    param([string]$RepoPath)
    foreach ($gitArgs in @(@('rebase','--abort'), @('merge','--abort'), @('cherry-pick','--abort'))) {
        $null = Invoke-Git $RepoPath $gitArgs
    }
}

function Get-GitHubToken {
    $token = [Environment]::GetEnvironmentVariable('GITHUB_TOKEN_PAT', 'User')
    if (-not $token) { $token = [Environment]::GetEnvironmentVariable('GITHUB_TOKEN_PAT', 'Machine') }
    if (-not $token) { $token = [Environment]::GetEnvironmentVariable('GITHUB_TOKEN_PAT', 'Process') }
    if (-not $token) {
        Write-Color 'GITHUB_TOKEN_PAT User environment variable is not set. Aborting.' 'Red'
        Write-Color 'Set: [Environment]::SetEnvironmentVariable(''GITHUB_TOKEN_PAT'',''<pat>'',''User'')' 'Yellow'
        exit 1
    }
    $env:GH_TOKEN = $token
    return $token
}

function Enable-GitHubAuth {
    param([string]$Token)
    if (Get-Command gh -ErrorAction SilentlyContinue) {
        try { $null = & gh auth setup-git 2>&1 } catch {}
    }
    $env:GIT_HTTP_EXTRAHEADER = "AUTHORIZATION: bearer $Token"
}

function Get-RemoteOwnerRepo {
    param([string]$RepoPath)
    $r = Invoke-Git $RepoPath @('remote', 'get-url', 'origin')
    if ($r.Code -ne 0 -or -not $r.Out) { return $null }
    if ($r.Out -match 'github\.com[:/]([^/]+)/([^/.]+)') {
        return @{ Owner = $Matches[1]; Repo = $Matches[2] -replace '\.git$','' }
    }
    return $null
}

function Test-GitHubRepoExists {
    param([string]$Owner, [string]$Repo, [string]$Token)
    $headers = @{ Authorization = "Bearer $Token"; Accept = 'application/vnd.github+json'; 'User-Agent' = 'dopagent-commit-push' }
    try {
        Invoke-RestMethod -Uri "https://api.github.com/repos/$Owner/$Repo" -Headers $headers -Method Get -TimeoutSec 30 | Out-Null
        return @{ Exists = $true; Error = $null }
    }
    catch {
        if ($_.Exception.Response -and [int]$_.Exception.Response.StatusCode -eq 404) {
            return @{ Exists = $false; Error = 'GitHub repo not found (404)' }
        }
        return @{ Exists = $false; Error = "GitHub API error: $($_.Exception.Message)" }
    }
}

function Get-ScanRootsFromMarkdown {
    param([string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        Write-Color "Dirs file not found: $Path" 'Red'
        exit 1
    }
    $roots = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($line in (Read-RepoTextFile $Path) -split "`r?`n") {
        if ($line -notmatch '^\s*-\s+') { continue }
        $text = ($line -replace '^\s*-\s+', '').Trim().Trim('`').Trim()
        if ($text -match '^~[/\\]') { $text = Join-Path $env:USERPROFILE ($text.Substring(1).TrimStart('\','/')) }
        if ($text -match '^[A-Za-z]:\\' -or $text -match '^/') {
            $expanded = [System.IO.Path]::GetFullPath($text)
            if (Test-Path -LiteralPath $expanded -PathType Container) { [void]$roots.Add($expanded) }
        }
    }
    if ($roots.Count -eq 0) { Write-Color "No directory paths in $Path" 'Red'; exit 1 }
    return @($roots)
}

function Find-GitRepos {
    param([string[]]$Roots, [int]$Depth)
    $repos = [System.Collections.Generic.List[string]]::new()
    foreach ($root in $Roots) {
        Write-Color "Scanning $root ..." 'DarkGray'
        $queue = [System.Collections.Generic.Queue[object]]::new()
        $queue.Enqueue(@{ Path = $root; Depth = 0 })
        while ($queue.Count -gt 0) {
            $item = $queue.Dequeue()
            if (Test-Path -LiteralPath (Join-Path $item.Path '.git') -PathType Container) {
                $repos.Add($item.Path); continue
            }
            if ($item.Depth -ge $Depth) { continue }
            Get-ChildItem -LiteralPath $item.Path -Directory -ErrorAction SilentlyContinue | ForEach-Object {
                if ($Script:SkipDirs -contains $_.Name -or $_.Name.StartsWith('.')) { return }
                $queue.Enqueue(@{ Path = $_.FullName; Depth = $item.Depth + 1 })
            }
        }
    }
    return $repos
}

function Test-ShouldSkipDirRel {
    param([string]$RelPath)
    foreach ($skip in $Script:SkipDirs) {
        if ($RelPath -match ('(^|[/\\])' + [regex]::Escape($skip) + '([/\\]|$)')) { return $true }
    }
    return $false
}

function Get-RelativeRepoPath {
    param([string]$RepoPath, [string]$FullPath)
    return ($FullPath.Substring($RepoPath.Length).TrimStart('\','/') -replace '\\', '/')
}

# ── empty folders ─────────────────────────────────────────────────────────────
function Ensure-EmptyFolderMarkers {
    param([string]$RepoPath)
    $marker = $Script:EmptyFolderMarker
    $gitDir = Join-Path $RepoPath '.git'
    $created = 0
    Get-ChildItem -LiteralPath $RepoPath -Directory -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object {
        if ($_.FullName.StartsWith($gitDir, [StringComparison]::OrdinalIgnoreCase)) { return }
        $rel = Get-RelativeRepoPath $RepoPath $_.FullName
        if (Test-ShouldSkipDirRel $rel) { return }
        $hasFiles = Get-ChildItem -LiteralPath $_.FullName -File -Force -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -ne $marker }
        $hasSubDirs = Get-ChildItem -LiteralPath $_.FullName -Directory -Force -ErrorAction SilentlyContinue
        if ($hasFiles -or $hasSubDirs) { return }
        $keepPath = Join-Path $_.FullName $marker
        if (-not (Test-Path -LiteralPath $keepPath)) {
            Write-RepoTextFile $keepPath ''
            $created++
        }
    }
    if ($created -gt 0) { Write-Color "  empty folders: added $created x $marker" 'Green' }
}

# ── gitignore ─────────────────────────────────────────────────────────────────
function Test-IsExcludedFile {
    param([string]$RepoPath, [string]$RelPath)
    if ($RelPath -match '(?i)\.exe$') { return $true }
    $full = Join-Path $RepoPath ($RelPath -replace '/', [IO.Path]::DirectorySeparatorChar)
    if ((Test-Path -LiteralPath $full -PathType Leaf) -and (Get-Item -LiteralPath $full).Length -gt $Script:MaxFileSizeBytes) {
        return $true
    }
    return $false
}

function Find-LargeFilesInRepo {
    param([string]$RepoPath)
    $large = [System.Collections.Generic.List[string]]::new()
    $gitDir = Join-Path $RepoPath '.git'
    Get-ChildItem -LiteralPath $RepoPath -Recurse -File -Force -ErrorAction SilentlyContinue | ForEach-Object {
        if ($_.FullName.StartsWith($gitDir, [StringComparison]::OrdinalIgnoreCase)) { return }
        $rel = Get-RelativeRepoPath $RepoPath $_.FullName
        if (Test-ShouldSkipDirRel $rel) { return }
        if ($_.Length -gt $Script:MaxFileSizeBytes) { [void]$large.Add($rel) }
    }
    return @($large | Sort-Object -Unique)
}

function Ensure-GitIgnore {
    param([string]$RepoPath)
    $giPath = Join-Path $RepoPath '.gitignore'
    if (-not (Test-Path -LiteralPath $giPath)) {
        Write-RepoTextFile $giPath @"
# Auto-managed by github-commit-push-all-repos
*.exe
*.EXE

$($Script:GitIgnoreAutoStart)
$($Script:GitIgnoreAutoEnd)
"@
        Write-Color '  created .gitignore' 'Green'
        return
    }
    $content = Read-RepoTextFile $giPath
    $updated = $false
    if ($content -notmatch '(?m)^\*\.exe\s*$') { $content = "*.exe`n*.EXE`n`n" + $content; $updated = $true }
    if ($content -notmatch [regex]::Escape($Script:GitIgnoreAutoStart)) {
        $content = $content.TrimEnd() + "`n`n$($Script:GitIgnoreAutoStart)`n$($Script:GitIgnoreAutoEnd)`n"
        $updated = $true
    }
    if ($updated) { Write-RepoTextFile $giPath $content; Write-Color '  updated .gitignore' 'Green' }
}

function Update-GitIgnoreAutoSection {
    param([string]$RepoPath, [string[]]$ExtraPaths)
    if ($ExtraPaths.Count -eq 0) { return }
    Ensure-GitIgnore $RepoPath
    $giPath = Join-Path $RepoPath '.gitignore'
    $content = Read-RepoTextFile $giPath
    $existing = [System.Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $startPat = [regex]::Escape($Script:GitIgnoreAutoStart)
    $endPat   = [regex]::Escape($Script:GitIgnoreAutoEnd)
    if ($content -match "(?s)$startPat\r?\n(.*?)\r?\n$endPat") {
        foreach ($line in ($Matches[1] -split "`r?`n")) {
            $line = $line.Trim()
            if ($line -and -not $line.StartsWith('#')) { [void]$existing.Add($line) }
        }
    }
    foreach ($p in $ExtraPaths) { [void]$existing.Add(($p -replace '\\', '/')) }
    $newSection = "$($Script:GitIgnoreAutoStart)`n$(($existing | Sort-Object) -join "`n")`n$($Script:GitIgnoreAutoEnd)"
    if ($content -match "(?s)$startPat.*?$endPat") {
        $content = [regex]::Replace($content, "(?s)$startPat.*?$endPat", $newSection)
    }
    else { $content = $content.TrimEnd() + "`n`n$newSection`n" }
    Write-RepoTextFile $giPath $content
}

function Remove-ExcludedFromGitIndex {
    param([string]$RepoPath)
    $removed = 0
    foreach ($rel in @((Invoke-Git $RepoPath @('ls-files')).Out -split "`n" | Where-Object { $_ })) {
        if (Test-IsExcludedFile $RepoPath $rel) {
            if ((Invoke-Git $RepoPath @('rm', '--cached', '--', $rel)).Code -eq 0) { $removed++ }
        }
    }
    if ($removed -gt 0) { Write-Color "  untracked $removed exe/large file(s)" 'Yellow' }
}

function Unstage-ExcludedFiles {
    param([string]$RepoPath)
    foreach ($rel in @((Invoke-Git $RepoPath @('diff', '--cached', '--name-only')).Out -split "`n" | Where-Object { $_ })) {
        if (Test-IsExcludedFile $RepoPath $rel) { $null = Invoke-Git $RepoPath @('reset', 'HEAD', '--', $rel) }
    }
}

function Prepare-RepoGitIgnore {
    param([string]$RepoPath)
    Ensure-GitIgnore $RepoPath
    $large = Find-LargeFilesInRepo $RepoPath
    if ($large.Count -gt 0) {
        Update-GitIgnoreAutoSection $RepoPath $large
        Write-Color "  gitignore: $($large.Count) large file(s)" 'DarkGray'
    }
    Remove-ExcludedFromGitIndex $RepoPath
}

function Test-SensitiveFiles {
    param([string[]]$Files)
    foreach ($f in $Files) { if ($f -match $Script:SecretPat) { return $true } }
    return $false
}

function Get-CommitPrefix {
    param([string[]]$Files, [string]$Stat)
    $blob = (($Files -join ' ') + ' ' + $Stat).ToLower()
    if ($blob -match 'fix|bug|patch|error|crash') { return 'fix' }
    if ($blob -match 'feat|add|new|implement') { return 'feat' }
    if ($blob -match 'doc|readme|\.md') { return 'docs' }
    return 'chore'
}

function New-CommitMessage {
    param([string]$RepoPath)
    $stat = (Invoke-Git $RepoPath @('diff', '--cached', '--stat')).Out
    $files = @((Invoke-Git $RepoPath @('diff', '--cached', '--name-only')).Out -split "`n" | Where-Object { $_ })
    if ($files.Count -eq 0) { return "chore: sync $(Get-Date -Format 'yyyy-MM-dd HH:mm')" }
    $exts = ($files | ForEach-Object { if ($_ -match '\.([^.]+)$') { $Matches[1] } } | Sort-Object -Unique)[0..3] -join '/'
    $dirs = ($files | ForEach-Object { ($_ -split '[/\\]')[0] } | Sort-Object -Unique)[0..2] -join ', '
    return "$(Get-CommitPrefix $files $stat): $($files.Count) file(s) [$exts] in $dirs"
}

# ── conflicts ─────────────────────────────────────────────────────────────────
function Get-ConflictedFiles {
    param([string]$RepoPath)
    return @((Invoke-Git $RepoPath @('diff', '--name-only', '--diff-filter=U')).Out -split "`n" | Where-Object { $_ })
}

function Test-IsBinaryFile {
    param([string]$FullPath)
    try {
        $bytes = [System.IO.File]::ReadAllBytes($FullPath)
        if ($bytes.Length -gt 8000) { $sample = $bytes[0..7999] } else { $sample = $bytes }
        foreach ($b in $sample) { if ($b -eq 0) { return $true } }
    }
    catch {}
    return $false
}

function Test-IsCriticalConflictFile {
    param([string]$RelPath, [string]$FullPath)
    $name = Split-Path $RelPath -Leaf
    foreach ($pat in $Script:CriticalPatterns) {
        if ($name -like $pat) { return $true }
    }
    if (Test-IsBinaryFile $FullPath) { return $true }
    return $false
}

function Test-IsCriticalConflict {
    param([string]$RepoPath, [string[]]$Files)
    if ($Files.Count -gt $Script:MaxAutoResolveFiles) { return $true }
    foreach ($rel in $Files) {
        $full = Join-Path $RepoPath ($rel -replace '/', [IO.Path]::DirectorySeparatorChar)
        if (Test-IsCriticalConflictFile $rel $full) { return $true }
    }
    return $false
}

function Get-FileSideTimestamp {
    param([string]$RepoPath, [string]$Ref, [string]$RelPath)
    $r = Invoke-Git $RepoPath @('log', '-1', '--format=%ct', $Ref, '--', $RelPath)
    if ($r.Code -ne 0 -or -not $r.Out) { return 0 }
    return [long]$r.Out
}

function Resolve-ConflictFile {
    param([string]$RepoPath, [string]$RelPath, [string]$TheirsRef)
    $full = Join-Path $RepoPath ($RelPath -replace '/', [IO.Path]::DirectorySeparatorChar)
    $content = Read-RepoTextFile $full
    if ($null -eq $content -or $content -notmatch '<<<<<<< ') { return $true }

    $oursTs = Get-FileSideTimestamp $RepoPath 'HEAD' $RelPath
    $theirsTs = Get-FileSideTimestamp $RepoPath $TheirsRef $RelPath
    $preferOurs = $oursTs -gt $theirsTs
    $lines = $content -split "`r?`n", -1
    $resolved = [System.Collections.Generic.List[string]]::new()
    $i = 0
    while ($i -lt $lines.Count) {
        if ($lines[$i] -match '^<<<<<<< ') {
            $ours = [System.Collections.Generic.List[string]]::new()
            $theirs = [System.Collections.Generic.List[string]]::new()
            $i++; $section = 'ours'
            while ($i -lt $lines.Count) {
                $l = $lines[$i]
                if ($l -match '^=======$') { $section = 'theirs' }
                elseif ($l -match '^>>>>>>> ') { break }
                elseif ($section -eq 'ours') { $ours.Add($l) } else { $theirs.Add($l) }
                $i++
            }
            $chosen = if ($preferOurs) { $ours } elseif ($oursTs -eq $theirsTs) {
                if ($theirs.Count -ge $ours.Count) { $theirs } else { $ours }
            } else { $theirs }
            $resolved.AddRange($chosen)
        }
        else { $resolved.Add($lines[$i]) }
        $i++
    }
    $text = $resolved -join "`n"
    if ($text -match '<<<<<<< |>>>>>>> ') { return $false }
    Write-RepoTextFile $full $text
    return $true
}

function Resolve-AllConflicts {
    param([string]$RepoPath, [string]$TheirsRef)
    $files = Get-ConflictedFiles $RepoPath
    if ($files.Count -eq 0) { return @{ Ok = $true; Critical = $false; Files = @() } }
    if (Test-IsCriticalConflict $RepoPath $files) {
        return @{ Ok = $false; Critical = $true; Files = $files }
    }
    foreach ($f in $files) {
        if (-not (Resolve-ConflictFile $RepoPath $f $TheirsRef)) {
            return @{ Ok = $false; Critical = $true; Files = $files }
        }
        $null = Invoke-Git $RepoPath @('add', '--', $f)
        Write-Color "  resolved: $f" 'Green'
    }
    return @{ Ok = $true; Critical = $false; Files = @() }
}

function Register-Failure {
    param([string]$RepoPath, [string]$Reason, $FailedList, [hashtable]$Stats)
    Reset-RepoState $RepoPath
    Write-Color "  FAIL: $Reason" 'Red'
    $FailedList.Add([pscustomobject]@{ Path = $RepoPath; Name = Split-Path $RepoPath -Leaf; Reason = $Reason })
    $Stats['FAIL']++
}

function Register-NeedsUser {
    param([string]$RepoPath, [string]$Reason, [string[]]$Files, $NeedsUserList, [hashtable]$Stats)
    Reset-RepoState $RepoPath
    Write-Color "  NEEDS USER: $Reason" 'Magenta'
    $NeedsUserList.Add([pscustomobject]@{
        Path   = $RepoPath
        Name   = Split-Path $RepoPath -Leaf
        Reason = $Reason
        Files  = ($Files -join ', ')
    })
    $Stats['USER']++
}

function Handle-PullConflicts {
    param([string]$RepoPath, [string]$Branch, $FailedList, $NeedsUserList, [hashtable]$Stats)
    $theirsRef = "origin/$Branch"
    $files = Get-ConflictedFiles $RepoPath
    if ($files.Count -eq 0) { return $true }

    $result = Resolve-AllConflicts $RepoPath $theirsRef
    if ($result.Critical) {
        Register-NeedsUser $RepoPath 'critical conflict — user must choose resolution' $result.Files $NeedsUserList $Stats
        return $false
    }
    if (-not $result.Ok) {
        Register-Failure $RepoPath 'unresolvable conflict' $FailedList $Stats
        return $false
    }
    return $true
}

function Process-Repo {
    param([string]$RepoPath, [string]$Token, [hashtable]$Stats, $FailedList, $NeedsUserList)

    Write-Color "`n▶ $(Split-Path $RepoPath -Leaf)" 'Cyan'
    try {
        $remote = Get-RemoteOwnerRepo $RepoPath
        if (-not $remote) { Write-Color '  SKIP: no github origin' 'Yellow'; $Stats['SKIP']++; return }

        $exists = Test-GitHubRepoExists $remote.Owner $remote.Repo $Token
        if (-not $exists.Exists) {
            if ($exists.Error -match '404') { Write-Color '  SKIP: not on GitHub' 'Yellow'; $Stats['SKIP']++ }
            else { Register-Failure $RepoPath $exists.Error $FailedList $Stats }
            return
        }

        $fetch = Invoke-GitWithTimeout $RepoPath @('fetch', 'origin')
        if ($fetch.TimedOut -or $fetch.Code -ne 0) {
            Register-Failure $RepoPath $(if ($fetch.TimedOut) { $fetch.Out } else { "fetch — $($fetch.Out)" }) $FailedList $Stats
            return
        }

        Set-RepoGitEolConfig $RepoPath
        Prepare-RepoGitIgnore $RepoPath
        Ensure-EmptyFolderMarkers $RepoPath

        $branch = (Invoke-Git $RepoPath @('branch', '--show-current')).Out
        if (-not $branch) { Write-Color '  SKIP: detached HEAD' 'Yellow'; $Stats['SKIP']++; return }

        $porcelain = (Invoke-Git $RepoPath @('status', '--porcelain')).Out
        $dirty = [bool]$porcelain
        $hasPush = [bool](Invoke-Git $RepoPath @('log', "@{u}..@", '--oneline')).Out

        if (-not $dirty -and -not $hasPush) { Write-Color '  SKIP: clean' 'DarkGray'; $Stats['SKIP']++; return }

        if ($dirty -and (Test-SensitiveFiles @($porcelain -split "`n" | ForEach-Object { ($_ -replace '^\S+\s+','').Trim('"') }))) {
            Write-Color '  SKIP: sensitive files' 'Yellow'; $Stats['SKIP']++; return
        }

        if ($Script:DryRun) {
            Write-Color "  DRY-RUN: commit=$dirty push=$($dirty -or $hasPush)" 'Magenta'
            $Stats['PUSH']++; return
        }

        if ($dirty) {
            $null = Invoke-Git $RepoPath @('add', '-A')
            Unstage-ExcludedFiles $RepoPath
            if (-not (Invoke-Git $RepoPath @('diff', '--cached', '--name-only')).Out) {
                Write-Color '  SKIP: nothing to commit' 'Yellow'; $Stats['SKIP']++; return
            }
            $commit = Invoke-Git $RepoPath @('commit', '-m', (New-CommitMessage $RepoPath))
            if ($commit.Code -ne 0) { Register-Failure $RepoPath "commit — $($commit.Out)" $FailedList $Stats; return }
            Write-Color '  committed' 'Green'
        }

        $push = Invoke-GitWithTimeout $RepoPath @('push', 'origin', $branch)
        if ($push.TimedOut) { Register-Failure $RepoPath $push.Out $FailedList $Stats; return }
        if ($push.Code -eq 0) { Write-Color '  pushed ✓' 'Green'; $Stats['PUSH']++; return }

        Write-Color '  push rejected — pull/rebase...' 'Yellow'
        $pull = Invoke-GitWithTimeout $RepoPath @('pull', '--rebase', 'origin', $branch)
        if ($pull.TimedOut) { Register-Failure $RepoPath $pull.Out $FailedList $Stats; return }

        if ($pull.Code -ne 0) {
            $inRebase = (Invoke-Git $RepoPath @('diff', '--name-only', '--diff-filter=U')).Out
            if ($inRebase) {
                if (-not (Handle-PullConflicts $RepoPath $branch $FailedList $NeedsUserList $Stats)) { return }
                $cont = Invoke-Git $RepoPath @('rebase', '--continue')
                if ($cont.Code -ne 0) {
                    if (Get-ConflictedFiles $RepoPath) {
                        Register-NeedsUser $RepoPath 'rebase conflict after continue' (Get-ConflictedFiles $RepoPath) $NeedsUserList $Stats
                    }
                    else { Register-Failure $RepoPath "rebase continue — $($cont.Out)" $FailedList $Stats }
                    return
                }
            }
            else {
                $null = Invoke-Git $RepoPath @('rebase', '--abort')
                $pull = Invoke-GitWithTimeout $RepoPath @('pull', '--no-rebase', 'origin', $branch)
                if ($pull.TimedOut -or $pull.Code -ne 0) {
                    if (Get-ConflictedFiles $RepoPath) {
                        if (-not (Handle-PullConflicts $RepoPath $branch $FailedList $NeedsUserList $Stats)) { return }
                        $null = Invoke-Git $RepoPath @('commit', '-m', "merge: resolve conflicts $(Get-Date -Format 'yyyy-MM-dd')")
                    }
                    else {
                        Register-Failure $RepoPath "pull failed — $($pull.Out)" $FailedList $Stats; return
                    }
                }
            }
        }
        elseif (Get-ConflictedFiles $RepoPath) {
            if (-not (Handle-PullConflicts $RepoPath $branch $FailedList $NeedsUserList $Stats)) { return }
            $cont = Invoke-Git $RepoPath @('rebase', '--continue')
            if ($cont.Code -ne 0) {
                Register-NeedsUser $RepoPath 'rebase conflict' (Get-ConflictedFiles $RepoPath) $NeedsUserList $Stats
                return
            }
        }

        $push2 = Invoke-GitWithTimeout $RepoPath @('push', 'origin', $branch)
        if ($push2.TimedOut -or $push2.Code -ne 0) {
            Register-Failure $RepoPath $(if ($push2.TimedOut) { $push2.Out } else { "push — $($push2.Out)" }) $FailedList $Stats
            return
        }
        Write-Color '  pushed ✓' 'Green'
        $Stats['PUSH']++
    }
    catch {
        Register-Failure $RepoPath "unexpected — $($_.Exception.Message)" $FailedList $Stats
    }
    finally {
        Reset-RepoState $RepoPath
    }
}

# ── main ──────────────────────────────────────────────────────────────────────
$token = Get-GitHubToken
Enable-GitHubAuth $token

$dirsPath = $Script:DirsFile
if (-not [System.IO.Path]::IsPathRooted($dirsPath)) {
    $walk = $PSScriptRoot
    for ($i = 0; $i -lt 6; $i++) {
        $candidate = Join-Path $walk $Script:DirsFile
        if (Test-Path -LiteralPath $candidate) { $dirsPath = $candidate; break }
        $parent = Split-Path $walk -Parent
        if ($parent -eq $walk) { break }
        $walk = $parent
    }
}

Write-Color "`nGitHub Commit Push All Repos (Windows 11)" 'Cyan'
Write-Color "Dirs file: $dirsPath" 'DarkGray'
Write-Color "EOL: $($Script:LineEnding) | Encoding: UTF-8 (BOM=$($Script:Utf8Bom))" 'DarkGray'
if ($Script:DryRun) { Write-Color 'DRY-RUN' 'Magenta' }

$repos = Find-GitRepos (Get-ScanRootsFromMarkdown $dirsPath) $Script:MaxDepth
Write-Color "Found $($repos.Count) repositories`n" 'White'
Write-Host ('─' * 60)

$stats = @{ PUSH = 0; SKIP = 0; FAIL = 0; USER = 0 }
$failed = [System.Collections.Generic.List[object]]::new()
$needsUser = [System.Collections.Generic.List[object]]::new()
$i = 0
foreach ($repo in $repos) {
    $i++
    Write-Progress -Activity 'Processing repos' -Status "$i / $($repos.Count)" -PercentComplete ([math]::Round(100 * $i / [math]::Max($repos.Count, 1)))
    Process-Repo $repo $token $stats $failed $needsUser
}
Write-Progress -Activity 'Processing repos' -Completed

Write-Host "`n$('=' * 60)"
Write-Color 'SUMMARY' 'Cyan'
Write-Color "  Pushed : $($stats['PUSH'])" 'Green'
Write-Color "  Skipped: $($stats['SKIP'])" 'Yellow'
Write-Color "  Failed : $($stats['FAIL'])" 'Red'
Write-Color "  Needs user: $($stats['USER'])" 'Magenta'
Write-Host ('─' * 60)

if ($needsUser.Count -gt 0) {
    Write-Host ""
    Write-Color 'NEEDS USER (critical conflicts — ask before retry)' 'Magenta'
    Write-Host ('─' * 60)
    foreach ($item in $needsUser) {
        Write-Color "  $($item.Name)" 'Magenta'
        Write-Color "    Path  : $($item.Path)" 'DarkGray'
        Write-Color "    Files : $($item.Files)" 'Yellow'
        Write-Color "    Reason: $($item.Reason)" 'Yellow'
    }
    Write-Host ('─' * 60)
}

if ($failed.Count -gt 0) {
    Write-Host ""
    Write-Color 'FAILED REPOS' 'Red'
    Write-Host ('─' * 60)
    foreach ($item in $failed) {
        Write-Color "  $($item.Name)" 'Red'
        Write-Color "    Path  : $($item.Path)" 'DarkGray'
        Write-Color "    Reason: $($item.Reason)" 'Yellow'
    }
    Write-Host ('─' * 60)
}

if ($stats['FAIL'] -gt 0 -or $stats['USER'] -gt 0) { exit 1 }
