---
name: end-to-end-cycle-redesign-webpage-ui-ux
description: Runs the full webpage engineering loop on one or more pages: audit (webpage-debugging), agent solutions (webpage-debugging-to-solutions), implement (webpage-solutions-to-implement), then compliance check (webpage-compliance-check). Passes artifact paths between stages and writes the loop report to ./.argent/webpage-engineering-loop/<UUID>.md. Use when the user asks to run the webpage loop, full UI fix pipeline, debug-to-compliance flow, or "loop engineering" on a page/URL.
---

# Webpage Engineering Loop

## Overview

- Scope: orchestrate the four webpage skills **in order** for each supplied page, hand off `.argent` artifact paths, optionally retry when compliance finds open issues, then write one loop report and reply to the human
- Input: page URL(s), route(s), or source path(s); optional max loop iterations (default `1`)
- Output: artifacts from each stage under `./.argent/`; loop report at `./.argent/webpage-engineering-loop/<UUID>.md`; short reply with that path
- Exclusions: do not invent issues; do not skip stages when a messy-page file exists; do not start a time-based `/loop` schedule — this is a sequential engineering pipeline
- Related skills (must run by reading each `SKILL.md` fully before that stage):
  1. `webpage-debugging`
  2. `webpage-debugging-to-solutions`
  3. `webpage-solutions-to-implement`
  4. `webpage-compliance-check`

## Objectives

1. Accept one or more page targets from the user
2. Run stages 1→4 in order, reading each skill’s `SKILL.md` and following it completely
3. Pass produced UUID file paths into the next stage (never invent paths)
4. Stop early when stage 1 marks the page **clean** (no debugging file)
5. Optionally re-run stages 1→4 when compliance returns NOT FIXED issues (up to max iterations)
6. Write the loop report to `./.argent/webpage-engineering-loop/<UUID>.md` and reply with its path plus a short verdict

## Workflow

### Step 0: Collect inputs

Accept:

| Input | Examples |
|-------|----------|
| Page targets | `http://localhost:3010/Pages/...`, `/orders`, `~/Admin/Users.aspx` |
| Max iterations | Number; default `1` (single pass). Use `2`+ only when user asks to retry until fixed |

Normalize each page to `page_label` + `open_target`.

Track per page (update after each stage):

| Field | Set by |
|-------|--------|
| `debug_path` | Stage 1 → `./.argent/webpage-debugging/<UUID>.md` or empty if clean |
| `solution_path` | Stage 2 → `./.argent/webpage-debugging-to-solutions/<UUID>.md` |
| `implement_path` | Stage 3 → `./.argent/webpage-solutions-to-implement/<UUID>.md` |
| `compliance_path` | Stage 4 → `./.argent/webpage-compliance-check/<UUID>.md` |
| `verdict` | Stage 4 → `PASS` / `PARTIAL` / `FAIL` / `SKIPPED_CLEAN` |

```text
Loop progress (copy and update):
- [ ] Inputs collected
- [ ] Stage 1: webpage-debugging
- [ ] Stage 2: webpage-debugging-to-solutions (if messy)
- [ ] Stage 3: webpage-solutions-to-implement (if solution exists)
- [ ] Stage 4: webpage-compliance-check
- [ ] Retry decision (if iterations remain and NOT FIXED)
- [ ] Write ./.argent/webpage-engineering-loop/<UUID>.md
- [ ] Reply to human with report path
```

### Step 1: Stage — webpage-debugging

1. Read `.cursor/skills/webpage-debugging/SKILL.md` and follow it fully
2. Pass the current page list as that skill’s page list
3. Capture every written `./.argent/webpage-debugging/<UUID>.md` into `debug_path` per page
4. If a page is **clean** (no file): set `verdict = SKIPPED_CLEAN`, skip stages 2–4 for that page

### Step 2: Stage — webpage-debugging-to-solutions

For each page with a `debug_path`:

1. Read `.cursor/skills/webpage-debugging-to-solutions/SKILL.md` and follow it fully
2. Input: the `debug_path` file (treat as the upstream prompt path)
3. Capture `./.argent/webpage-debugging-to-solutions/<UUID>.md` → `solution_path`
4. Do not paste long solution bodies to the human mid-pipeline

### Step 3: Stage — webpage-solutions-to-implement

For each page with a `solution_path`:

1. Read `.cursor/skills/webpage-solutions-to-implement/SKILL.md` and follow it fully
2. Inputs: page `open_target` + `solution_path`
3. Capture `./.argent/webpage-solutions-to-implement/<UUID>.md` → `implement_path`

### Step 4: Stage — webpage-compliance-check

For each page that ran stage 3 (or that has `debug_path` when implementation was blocked):

1. Read `.cursor/skills/webpage-compliance-check/SKILL.md` and follow it fully
2. Inputs: page target + `debug_path` (required for issue FIXED/NOT FIXED table) + `implement_path` when available
3. Capture `./.argent/webpage-compliance-check/<UUID>.md` → `compliance_path` and the verdict

### Step 5: Retry loop (only if max iterations > 1)

After stage 4, if any debugging issue is **NOT FIXED** and `iteration < max_iterations`:

1. Increment iteration
2. Re-run stages 1→4 for that page only
3. Keep prior UUID paths in the final report history; always use new UUIDs for new files
4. Stop when verdict is PASS, all issues FIXED/BLOCKED with no NOT FIXED, or max iterations reached

Default `max_iterations = 1` — one full pipeline, no automatic retry.

### Step 6: Write loop report and reply

After all pages finish (and retries end):

1. Ensure `./.argent/webpage-engineering-loop/` exists at the project root
2. Generate a new UUID (PowerShell: `[guid]::NewGuid().ToString()`, or equivalent)
3. Write exactly one new file: `./.argent/webpage-engineering-loop/<UUID>.md`
4. Never overwrite an existing UUID file — always generate a new UUID

**Report template** — use this structure:

```markdown
# Webpage engineering loop report

## Pages
| Page | Verdict | Debug | Solution | Implement | Compliance | Issues | Iterations |
|------|---------|-------|----------|-----------|------------|--------|------------|
| <page_label> | SKIPPED_CLEAN / PASS / PARTIAL / FAIL | path or (clean — none) | path or (skipped) | path or (skipped) | path or (skipped) | <fixed>F / <not_fixed>NF / <blocked>B or n/a | <n> |

## Summary
- Clean (no work): <list>
- Passed: <list>
- Partial/Fail: <list>
- Blocked: <list with reason>

## Notes
- <optional blockers or skipped stages>
```

Then reply to the human with only:

```text
Webpage engineering loop complete:
- Report: ./.argent/webpage-engineering-loop/<UUID>.md
- Verdicts: <page_label → SKIPPED_CLEAN|PASS|PARTIAL|FAIL>
```

Do not dump full stage artifact bodies unless asked.

## Safety rules

1. **Always** read and follow each stage skill’s `SKILL.md` before running that stage — this skill only orchestrates; it does not replace stage rules
2. **Never** skip stage 2 or 3 when a `debug_path` exists, unless the user explicitly aborts
3. **Never** invent issues, solutions, or artifact paths
4. **Never** overwrite existing `.argent/**/<UUID>.md` files — each stage and the loop report use new UUIDs
5. **Never** change business logic, bindings, field names, or run unapproved DB writes (stage skills own these limits)
6. **Never** start a Cursor time `/loop` for this pipeline unless the user separately asks for a schedule
7. **Always** pass real `debug_path` into compliance when verifying UI-audit fixes
8. **Always** write the loop report under `./.argent/webpage-engineering-loop/` at the project root
9. **Always** reply with the loop report path after the file is written

## Key facts & reference

| Item | Value |
|------|-------|
| Skill path | `.cursor/skills/webpage-engineering-loop/SKILL.md` |
| Loop report dir | `./.argent/webpage-engineering-loop/` |
| Loop report file | `./.argent/webpage-engineering-loop/<UUID>.md` |
| UUID | New GUID per loop run; never reuse |
| Stage 1 | `.cursor/skills/webpage-debugging/SKILL.md` → `./.argent/webpage-debugging/` |
| Stage 2 | `.cursor/skills/webpage-debugging-to-solutions/SKILL.md` → `./.argent/webpage-debugging-to-solutions/` |
| Stage 3 | `.cursor/skills/webpage-solutions-to-implement/SKILL.md` → `./.argent/webpage-solutions-to-implement/` |
| Stage 4 | `.cursor/skills/webpage-compliance-check/SKILL.md` → `./.argent/webpage-compliance-check/` |
| Default iterations | `1` |
| Early exit | Stage 1 clean → skip 2–4 for that page |

### Trigger phrases

- "webpage engineering loop"
- "loop engineering on this page"
- "run debug → solutions → implement → compliance"
- "full UI fix pipeline"
- "run all webpage skills on"

### Handoff map

| From | Artifact | Into |
|------|----------|------|
| Stage 1 | `./.argent/webpage-debugging/<UUID>.md` | Stage 2 input; Stage 4 issue checklist |
| Stage 2 | `./.argent/webpage-debugging-to-solutions/<UUID>.md` | Stage 3 input |
| Stage 3 | `./.argent/webpage-solutions-to-implement/<UUID>.md` | Stage 4 optional context |
| Stage 4 | `./.argent/webpage-compliance-check/<UUID>.md` | Loop report + human reply |
| Loop | `./.argent/webpage-engineering-loop/<UUID>.md` | Stored final report for the human |
