---
name: code-removal
description: |
  Safely removes dead or orphan code — symbols, imports, files, or blocks with no connections to live code. Use when the user asks to remove unused, dead, unreachable, or orphaned code; clean up unused imports; or delete files/modules with zero references.
---

# Code Removal (Dead / Orphan Only)

**Scope:** Remove only code that is provably dead or orphaned — not referenced, imported, called, exported, registered, or reachable from any live code path.

**Out of scope:** Do not remove code that is still connected, even if the user names it explicitly. Refuse and explain what still references it.

## Eligibility (all must pass)

A target is removable only when every check below is satisfied:

| Check | Requirement |
| ----- | ----------- |
| References | Zero call sites, imports, exports, re-exports, type usages, or string/symbol references in the repo |
| Reachability | Not reachable from entry points (`main`, `index`, routes, CLI, jobs, hooks, DI registration, config manifests) |
| Tests | Not referenced by any test (unless the test itself is also dead/orphan) |
| Runtime wiring | Not registered in routing, DI, plugins, feature flags, build config, or deployment manifests |
| Public API | Not part of a public/exported surface consumed elsewhere |

If **any** connection exists → **do not remove**. Report the connection(s) and stop.

## Workflow

Copy this checklist and track progress:

```
Task Progress:
- [ ] 1. Identify candidate targets
- [ ] 2. Prove dead/orphan status
- [ ] 3. Confirm no hidden connections
- [ ] 4. Execute removal
- [ ] 5. Validate and report
```

### 1. Identify candidate targets

Look for dead or orphan artifacts:

| Target | Examples |
| ------ | -------- |
| Unused function / method | Never called anywhere |
| Unused class / type | Never instantiated or referenced |
| Unused variable / constant | Declared but never read |
| Unused import | Imported but never used |
| Unreachable code block | After `return`/`throw`, disabled branch, or provably false condition |
| Orphan file / module | No imports from other project files |
| Stale comment / empty stub | Left behind by prior removal; documents nothing live |
| Unused config entry | Key present but nothing reads it |

When the user names a specific symbol, treat it as a **candidate** — eligibility still requires proof in step 2.

### 2. Prove dead/orphan status (required)

Before deleting anything:

1. Search the codebase for the symbol name, file path, and export names (Grep, symbol search).
2. Search imports, re-exports, dynamic references (`require`, `import()`, reflection, string literals).
3. Check tests, configs, routing, DI, build files, and manifests.
4. Trace reachability from known entry points when removing files or modules.

**Evidence required:** List every search performed and confirm zero live connections. If evidence is incomplete, search more or ask before proceeding.

### 3. Confirm no hidden connections

Block removal when:

- Any caller, importer, or exporter exists (including cross-package)
- Symbol is used only in tests that still run live code paths
- Symbol is an entry point or registered externally
- Usage is dynamic/reflective and cannot be ruled out safely
- Generated or vendor code (unless provably unused within that boundary)

Do **not** offer to "remove and fix callers." Connected code is out of scope for this skill.

### 4. Execute removal

- Edit only files required for confirmed dead/orphan targets.
- For whole orphan files, prefer `git rm` when the repo is under git.
- Clean up artifacts left behind:
  - Unused imports made orphan by the removal
  - Empty blocks or trailing commas
  - Orphaned comments that only described removed code
  - Config / manifest entries that became unused as a direct result
- Match surrounding code style; do not reformat unrelated lines.

### 5. Validate and report

- Re-search for dangling references after edits.
- Run linters or tests when available and relevant.
- Apply **doc-sync** if project docs exist and the removal changes structure or APIs.
- End with a completion report per **report-end-task** (Mode A).

## Safety rules

- **Never remove connected code** — even when the user requests it. Explain blockers and what would need to change outside this skill's scope.
- Never delete without proof when impact is unclear.
- Warn when a file becomes empty after removal; remove the file only if it is itself orphan.
- Respect `.gitignore` and do not delete ignored secrets or local-only files without explicit intent.
- For bulk cleanup, list all confirmed dead targets in a plan before editing.

## Edge cases

| Case | Handling |
| ---- | -------- |
| User asks to remove live code | Refuse; list all connections found |
| Used only in dead tests | Remove test and target together if both are orphan |
| Last export in file | Remove file only if the whole file is orphan |
| Entry point (`main`, `index`, route root) | Never remove |
| Generated / vendor code | Do not remove unless provably unused and user explicitly targets it |
| Feature flag branches | Remove only branches provably unreachable and unreferenced |
| Ambiguous dynamic usage | Do not remove; report uncertainty |

## Removal report template

```markdown
### Code Removal Report

**Removed:** [name and type]
**Location:** [file:line or path]
**Orphan proof:** [searches run; zero references confirmed]

**Dependencies checked:** yes
**Connections found:** none (required to proceed)
**Cleanup performed:** [unused imports, config, orphan files, etc.]

**Skipped (still connected):** [list or none]
**Suggested next steps:** [run tests, manual review of skipped items]
```

## Examples

### Example 1 — Remove orphan helper

**User:** Clean up unused helpers.

**Agent:**

1. Search for private helpers with zero references across src/ and tests/.
2. Find `_formatLegacyDate()` in `utils/dates.js` — no imports, calls, or exports.
3. Remove function and any imports that became unused.
4. Report: removed 1 dead function; 0 connections found.

### Example 2 — Refuse connected code

**User:** Remove `validateEmail`.

**Agent:**

1. Found `validateEmail()` in `utils/validators.js`.
2. Connections: `signup.js:23`, `profile.js:67`; exported from `validators.js`.
3. **Do not remove** — code is live. Report connections and stop.

### Example 3 — Orphan file

**User:** Delete `legacy_auth.py`.

**Agent:**

1. Search imports and references to `legacy_auth` — zero matches outside the file.
2. Not listed in routes, DI, or config.
3. Remove file with `git rm`.
4. Report: removed 1 orphan file; re-search shows no dangling references.
