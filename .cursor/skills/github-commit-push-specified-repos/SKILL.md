---
name: github-commit-push-specified-repos
description: Windows 11 only. Discovers Git repos under paths listed in markdown files, verifies each exists on GitHub, commits and pushes with short messages. Creates .gitignore and .gitkeep for empty folders; handles UTF-8 and CRLF. Auto-resolves simple conflicts; asks the user on critical conflicts. Never blocks on one repo. Reads GITHUB_TOKEN_PAT from User environment variables. Config in config.json. Use for batch commit/push all repos.
---

# GitHub Commit Push All Repos

## Overview

- **Windows 11 only** — PowerShell 5.1+, User environment variables, CRLF line endings
- Reads scan roots from markdown files (default from `config.json`: `useful-dirs.md`)
- Finds every folder containing `.git` under those roots
- Creates `.gitkeep` in empty leaf folders so Git can include them
- **Auto `.gitignore`** — creates if missing; always ignores `*.exe` and files **> maxFileSizeMB**
- **Encoding** — writes text files as UTF-8 (optional BOM per config); normalizes to CRLF on Win 11
- **Conflicts** — auto-resolve simple ones (newer wins); **ask user** on critical conflicts
- **Never blocks on one repo** — timeout, skip, or fail fast; report all failures at end
- Related: `github-sync` (full bidirectional sync); this skill is commit-and-push only

## Objectives

1. Load user settings from `config.json` in the skill directory
2. Parse directory paths from `.md` files and discover local Git repos
3. Read `GITHUB_TOKEN_PAT` from Windows User environment variables before asking the user
4. Add `.gitkeep` (or configured marker) to empty leaf folders before commit
5. Commit with short messages; push; never commit `.exe` or files over size limit
6. Auto-resolve non-critical conflicts (newer wins); pause and ask user on critical conflicts
7. Never stop the full run because of one repo — report failures and user-needed conflicts at end

## Workflow

### Step 1: Configure

Edit `.cursor/skills/github-commit-push-all-repos/config.json`:

| Key | Default | Purpose |
|-----|---------|---------|
| `dirsFile` | `useful-dirs.md` | Markdown file listing scan roots |
| `maxDepth` | `5` | Max folder depth under each root |
| `repoTimeoutSec` | `120` | Git network command timeout |
| `maxFileSizeMB` | `5` | Files larger than this are gitignored |
| `emptyFolderMarker` | `.gitkeep` | File created in empty leaf folders |
| `lineEnding` | `crlf` | `crlf` (Win 11) or `lf` |
| `fileEncoding` | `utf8` | Text file encoding |
| `utf8Bom` | `false` | UTF-8 BOM when writing files |
| `gitAutoCrlf` | `true` | Set `core.autocrlf` in each repo |
| `maxAutoResolveConflictFiles` | `3` | Above this count → critical, ask user |
| `criticalConflictPatterns` | see file | Glob patterns → always ask user |
| `skipDirs` | see file | Directories excluded from scan |
| `secretPatterns` | see file | Regex — skip repo if matched in changes |

CLI flags override `config.json` when passed.

### Step 2: Authenticate

```powershell
[Environment]::GetEnvironmentVariable('GITHUB_TOKEN_PAT', 'User')
[Environment]::SetEnvironmentVariable('GITHUB_TOKEN_PAT', '<your-pat>', 'User')
```

Script reads User → Machine → Process scope.

### Step 3: Run

```powershell
.\.cursor\skills\github-commit-push-all-repos\scripts\commit-push-all-repos.ps1
.\.cursor\skills\github-commit-push-all-repos\scripts\commit-push-all-repos.ps1 --dry-run
.\.cursor\skills\github-commit-push-all-repos\scripts\commit-push-all-repos.ps1 --config=path\to\config.json
```

### Step 4: Per-repo logic

| Step | Action |
|------|--------|
| 1 | Parse `origin` → verify repo exists on GitHub |
| 2 | `git fetch origin`; configure `core.autocrlf` per config |
| 3 | Ensure `.gitignore`; untrack exe/large files |
| 4 | **Empty folders** — create `emptyFolderMarker` in each empty leaf folder |
| 5 | Skip if clean and nothing unpushed |
| 6 | `git add -A` → unstage excluded → commit if dirty → push |
| 7 | Push rejected → pull/rebase → resolve or escalate |

**Empty folder rule:** leaf directory with no files and no subdirectories gets `<emptyFolderMarker>` (default `.gitkeep`), written UTF-8 CRLF.

**`.gitignore` rules:**

| Rule | Action |
|------|--------|
| No `.gitignore` | Create with `*.exe` and auto-managed section |
| `*.exe` | Always ignored |
| Files > `maxFileSizeMB` | Added to auto section; never staged |

**Conflict resolution:**

| Type | Action |
|------|--------|
| Simple (≤ `maxAutoResolveConflictFiles`, text, not critical pattern) | Auto-resolve — newer timestamp wins |
| **Critical** (binary, critical pattern, too many files, unresolvable) | Abort rebase/merge; record in **NEEDS USER** list; **ask user** before retry |
| Agent | Use AskQuestion with repo path, conflicted files, and options (keep local / keep remote / skip) |

### Step 5: Review summary

Script prints `PUSH`, `SKIP`, `FAIL`, and **NEEDS USER** sections. Relay both failed and needs-user lists to the user.

## Safety rules

1. **Never** commit `.env`, credentials, or files matching `secretPatterns` — skip repo and warn
2. **Never** commit `*.exe` or files over `maxFileSizeMB`
3. **Never** echo or log `GITHUB_TOKEN_PAT`
4. **Never** force-push
5. **Never** auto-resolve critical conflicts — always ask the user
6. **Always** write text files with configured encoding and CRLF on Win 11
7. **Always** create `.gitignore` and empty-folder markers before commit
8. **Always** finish all repos; report failed and needs-user lists at the end

## Key facts & reference

| Item | Value |
|------|-------|
| Platform | Windows 11 only |
| Config | `.cursor/skills/github-commit-push-all-repos/config.json` |
| Script | `.cursor/skills/github-commit-push-all-repos/scripts/commit-push-all-repos.ps1` |
| Token | User environment variable `GITHUB_TOKEN_PAT` |
| Default repos root | From `useful-dirs.md` |
| Empty folder file | `.gitkeep` (override in config) |

### Script parameters

| Parameter | Overrides config key |
|-----------|---------------------|
| `--config=<path>` | entire config file path |
| `--dirs-file=<path>` | `dirsFile` |
| `--max-depth=<n>` | `maxDepth` |
| `--repo-timeout=<n>` | `repoTimeoutSec` |
| `--max-file-size-mb=<n>` | `maxFileSizeMB` |
| `--dry-run` | preview only |
| `--help` | usage |
