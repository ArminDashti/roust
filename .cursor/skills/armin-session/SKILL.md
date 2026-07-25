---
name: armin-session
description: >-
  Creates and updates session markdown logs under .armin/sessions/ with metadata
  (ID, Agent, Date, Time, Device), Skills, Rules, and a prompt/response transcript.
  Use when the user asks to save a session, write a session title.md, log a chat
  under .armin/sessions, or record agent conversation turns without reasoning.
---

# Armin Session Log

## Overview

- Scope: write or append session logs at `.armin/sessions/<title>.md`
- Each file captures session metadata plus a clean user/agent transcript
- Exclusions: does not rename Cursor chat tabs; does not store chain-of-thought / reasoning; does not commit or push
- Related: `exprience` for Q&A learnings (different store and format)

## Objectives

1. Create `.armin/sessions/` if missing
2. Write a well-formed session `.md` using the fixed template
3. Fill metadata (GUID, agent, date, time, device) and list Skills / Rules used
4. Append turns as **User prompt** then **Agent response** (final answer only)
5. Confirm path and filename to the user

## Workflow

### Step 1: Resolve the session file

1. Project root = workspace / git root
2. Ensure `.armin/sessions/` exists
3. Filename = kebab-case slug from the session title + `.md`  
   Example: title `Fix login API` → `.armin/sessions/fix-login-api.md`
4. If the user gives an exact path or name, use that under `.armin/sessions/`

### Step 2: Collect metadata

| Field | Source |
|-------|--------|
| ID | New GUID (e.g. `New-Guid` on PowerShell / `uuidgen`) |
| Agent | Current agent / model name (e.g. `Auto`, `Composer`) |
| Date | Local date `YYYY-MM-DD` |
| Time | Local time `HH:MM:SS` (24-hour) |
| Device | Machine hostname |
| Skills | Skills attached or applied in the session, slash-separated |
| Rules | Rules applied in the session, slash-separated |

If Skills or Rules are none: write `None`.

### Step 3: Write or update the file

**New file** — write the full template from Key facts & reference.

**Existing file** — keep the header block unchanged; append new transcript turns before the end of the file (after the last turn, still under `## Transcript`).

For each turn:

1. User prompt — verbatim user message (trim only leading/trailing blank lines)
2. Agent response — final user-facing answer only  
   - Strip thinking / reasoning / tool traces / internal checklists  
   - Keep code blocks, links, and conclusions the user saw

### Step 4: Confirm

Tell the user:

- Full relative path (e.g. `.armin/sessions/fix-login-api.md`)
- ID
- Whether created or appended

## Safety rules

1. **Never** include agent reasoning, chain-of-thought, or hidden tool narratives in the transcript.
2. **Never** write secrets, tokens, passwords, or full connection strings into the session file.
3. **Never** overwrite an existing session file’s header or earlier turns unless the user explicitly asks to replace the file.
4. **Always** use forward slashes in paths.
5. **Always** use the markdown template in Key facts & reference (proper headings, table, separators).
6. **Always** preserve user prompt wording; only normalize whitespace.

## Key facts & reference

| Item | Value |
|------|-------|
| Root dir | `.armin/sessions/` |
| File name | `<title-slug>.md` |
| Date format | `YYYY-MM-DD` |
| Time format | `HH:MM:SS` |
| Skills / Rules | `name1` / `name2` / `name3` or `None` |

### Session file template

```markdown
# <Session Title>

| Field | Value |
|-------|-------|
| ID | `<GUID>` |
| Agent | `<AGENT-NAME>` |
| Date | `<YYYY-MM-DD>` |
| Time | `<HH:MM:SS>` |
| Device | `<DEVICE-NAME>` |

## Skills

`<SKILL>` / `<SKILL>` / ...

## Rules

`<RULE>` / `<RULE>` / ...

---

## Transcript

### User

<user prompt>

### Agent

<agent response without reasoning>
```

### Append turn template

```markdown
---

### User

<user prompt>

### Agent

<agent response without reasoning>
```

## Example

**Input**

- Title: `Docker deploy smoke test`
- Agent: `Composer`
- Device: `ARMIN-LAPTOP`
- Skills: `docker-deploy` / `test-api-apps`
- Rules: `.NET Development Rules`
- One turn: user asks how to smoke-test; agent gives a short answer

**Output file:** `.armin/sessions/docker-deploy-smoke-test.md`

```markdown
# Docker deploy smoke test

| Field | Value |
|-------|-------|
| ID | `a3f1c2e4-9b8d-4e2a-91f0-7c6d5b4a3210` |
| Agent | `Composer` |
| Date | `2026-07-23` |
| Time | `14:53:00` |
| Device | `ARMIN-LAPTOP` |

## Skills

`docker-deploy` / `test-api-apps`

## Rules

`.NET Development Rules`

---

## Transcript

### User

How do I smoke-test the API after a Docker deploy?

### Agent

Deploy with the project Docker skill, then hit the health endpoint and one read-only route. If both return 200, the smoke test passed.
```
