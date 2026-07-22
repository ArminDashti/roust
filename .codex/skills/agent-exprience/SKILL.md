---
name: agent-exprience
description: Records problems, issues, and project learnings as question-and-answer entries in ./exprience/exprience.md at the project root. Use when the user asks to save, record, log, or capture an experience, problem, issue, solution, or lesson learned for the current project.
---

# Project Experience Log

## Overview

- Scope: persist problems, issues, fixes, workflows, and other useful project knowledge as Q&A entries
- Storage: `./exprience/exprience.md` in the **current project root** (create the folder if missing)
- Exclusions: personal knowledge base (`remember-experience` / `KNOWLEDGE_DIR`), unrelated project files, secrets or credentials

## Objectives

1. Turn the current problem, issue, or learning into a clear question-and-answer entry
2. Append the entry to `./exprience/exprience.md` without losing existing content
3. Confirm the saved location and entry summary to the user

## Workflow

### Step 1: Identify what to record

Source may be:

- Explicit user request ("record this issue", "save this experience")
- A problem just solved, error fixed, or workflow completed in the current chat

Extract:

- **Question**: what problem, issue, or situation does this address?
- **Answer**: solution, steps, commands, decisions, and context needed to reuse it later

### Step 2: Prepare the file

1. Resolve the project root (workspace root or git root)
2. Ensure `./exprience/` exists
3. Read `./exprience/exprience.md` if it already exists

If the file is new, create it with:

```markdown
# Project Experience Log

Recorded problems, issues, and learnings as question-and-answer entries.
```

### Step 3: Append the Q&A entry

Append **one** entry at the end of `exprience.md` using this template:

```markdown
---

## Q: [Clear question describing the problem, issue, or learning]

**Date:** YYYY-MM-DD  
**Tags:** [2-4 lowercase keywords]

### A:

[Concrete answer: what happened, root cause if known, fix or workflow, commands, paths, code snippets, and caveats. Enough detail for a future session to act without reconstructing context.]
```

Rules:

- Use today's date
- One entry per save request
- Keep the question specific; keep the answer actionable
- Include commands, file paths, config values, or snippets when they were part of the experience

### Step 4: Confirm

Tell the user:

> Saved to `./exprience/exprience.md`

Include a one-line summary of the Q&A recorded.

## Safety rules

1. **Never** overwrite or truncate `./exprience/exprience.md` — append only
2. **Never** store passwords, API keys, tokens, connection strings with secrets, or other credentials
3. **Never** write to `KNOWLEDGE_DIR` or personal knowledge paths — that is `remember-experience`
4. **Always** use `./exprience/exprience.md` at the project root, not a global or user home path
5. **Always** read the existing file before appending to avoid duplicate Q&A for the same issue

## Key facts & reference

| Item | Value |
|------|-------|
| Skill path | `~/.cursor/skills/exprience/SKILL.md` |
| Project log file | `./exprience/exprience.md` |
| Project log folder | `./exprience/` |
| Entry format | Question (`## Q:`) + Answer (`### A:`) |
| Related skill | `remember-experience` (personal `KNOWLEDGE_DIR`, not project) |

### Trigger phrases

- "record this experience"
- "save this issue"
- "log this problem"
- "capture this learning"
- "remember what we fixed"
- "add to project experience"

### Example entry

```markdown
---

## Q: Why did SSRS report export fail with "database login failed" on the test server?

**Date:** 2026-07-14  
**Tags:** ssrs, sql-server, authentication

### A:

The report data source pointed to `Pakhsh_Data_New` with a stale SQL login after the test DB restore.

Fix: open Report Manager → Data Sources → update credentials to the test service account, then re-run export.

Verify with: browse `http://test-server/Reports` and run the report in HTML before PDF export.
```
