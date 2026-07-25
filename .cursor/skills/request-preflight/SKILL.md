---
name: request-preflight
description: >-
  Runs a pre-flight checklist on a user request before execution: verifies
  feasibility, measures risk (0–10 rate + Low/Medium/High/Critical), and always
  informs the user when risk is High or Critical. Use when starting any
  non-trivial job, when the user asks to validate or sanity-check a request, or
  when the user may not know a task is impossible, partial, or high-risk.
---

# Request Pre-flight

## Overview

- Scope: pre-flight checklist before acting — like checks before flying
- Measures risk (per-item + overall: level and **rate 0–10**) and **must** tell the user when overall risk is High or Critical (rate ≥ 6)
- Always ends with a **Pre-flight Report** (mandatory, last section of the response)
- Exclusions: does not implement the task; domain skills run after checklist passes
- Related skills: user Safety rules, domain skills (`code-removal`, `github-sync`), `exprience` for logging blocked work

## Objectives

1. Run every checklist item before or while scoping the work
2. Decide whether the task **can be done** — Yes, Partially, No, or Unknown
3. **Measure risk** — list every risk with level + **rate 0–10**, then set **overall risk** = highest level and highest rate
4. **Inform the user** when overall risk is High or Critical (rate ≥ 6) — prominent warning before any action
5. Surface what the user may not know (missing access, wrong assumptions, irreversible effects)
6. **Always** append the Pre-flight Report at the end of the response

## Workflow

### Step 1: Pre-flight checklist

Copy and run every item before executing the request:

```
Pre-flight checklist:
- [ ] 1. Parse request (goal, scope, constraints, success criteria)
- [ ] 2. Tools & access (MCP, shell, network, credentials, VPN)
- [ ] 3. Environment (OS, paths, services, read-only refs)
- [ ] 4. Permissions (git, admin, DB, remote hosts)
- [ ] 5. Codebase & scope (targets exist, no contradictions)
- [ ] 6. Policy & safety (user rules, AGENTS.md, skill guardrails)
- [ ] 7. Risk inventory & measure (level + rate 0–10 each; overall = max level and max rate)
- [ ] 8. If overall High/Critical or rate ≥ 6 — warn user first; wait for confirmation
- [ ] 9. Pre-flight Report (mandatory — last section of response)
```

### Step 2: Parse the request

| Field | Question |
|-------|----------|
| Goal | What outcome does the user want? |
| Scope | Which repos, files, systems, environments? |
| Constraints | "Don't touch X", read-only, no commits, etc. |
| Success | How would we know it worked? |

If vague, note **Unknown** and add a clarifying risk row — do not guess hidden intent.

### Step 3: Feasibility checks

| Check | Blocker examples |
|-------|------------------|
| Tools & access | MCP down, no shell, no network, missing credentials, VPN blocks target |
| Environment | Wrong OS, path missing, service not running |
| Permissions | No git push, no admin, no DB access |
| Codebase | Target missing, symbol not found, live references |
| Logic | Contradictory requirements, depends on unavailable data |
| Policy | Violates Safety rules, AGENTS.md, or skill guardrails |

**Can this be done?**

| Answer | Meaning |
|--------|---------|
| **Yes** | All critical checks pass |
| **Partially** | Doable with reduced scope, extra steps, or user input |
| **No** | Hard blocker — cannot complete as asked |
| **Unknown** | Insufficient info — must clarify before acting |

### Step 4: Measure risk

Identify **every** risk — do not collapse into one vague sentence. Assign both a **level** and a **rate 0–10**.

| Rate | Level | When |
|------|-------|------|
| 0–2 | Low | Read-only, reversible, local, no secrets |
| 3–5 | Medium | File writes, config changes, multi-step side effects |
| 6–8 | High | Production, deletes, force push, installs, firewall, credentials, bulk DB writes |
| 9–10 | Critical | Irreversible damage, policy violation, data loss, or unsafe without explicit approval |

Within a band, pick the rate by severity (e.g. one local file edit → 3; many files / hard rollback → 5).

**Overall risk level** = highest level among all rows (Critical > High > Medium > Low).  
**Overall risk rate** = highest rate among all rows (0–10). Level and rate must stay consistent with the table above.

Common blind spots to check:

- Looks simple but touches production or shared infra
- "Just delete it" but references still exist
- Assumes credentials or VPN that are not present
- Rollback is costly (migrations, mass refactors, batch git ops)

### Step 5: Inform user when risk is high

If **overall risk** is **High** or **Critical**, or **overall rate ≥ 6** (or any single row meets that):

1. Put a clear warning **near the top** of the response (before implementing anything):
   - `⚠️ High risk (N/10)` or `⛔ Critical risk (N/10)` + one sentence naming the main hazard(s)
2. Do **not** start the risky work until the user explicitly confirms
3. Repeat the same overall level and rate in the Pre-flight Report

If overall is Low or Medium only (rate ≤ 5) — no special banner; still list risks and rates in the report.

### Step 6: Act on findings

| Can be done? | Overall risk / rate | Action before implementing |
|--------------|---------------------|----------------------------|
| Yes | Low (0–2) | Proceed |
| Yes | Medium (3–5) | Proceed; note mitigations |
| Yes / Partially | High (6–8) or rate ≥ 6 | Stop — warn user; wait for explicit confirmation |
| Any | Critical (9–10) | Stop — warn user; do not proceed without explicit confirmation |
| Partially | Any | Propose reduced scope or list missing inputs |
| No | Any | Stop — do not proceed |
| Unknown | Any | Ask clarifying questions only |

### Step 7: Pre-flight Report (always last)

**Mandatory.** Append as the **final section** of every response where this skill applies — never skip, never bury mid-response.

Use the output template in **Key facts & reference**.

Report rules:

1. **First line:** **Can this be done?** — Yes | Partially | No | Unknown
2. **Second:** **Overall risk:** Low | Medium | High | Critical — **N/10** (= max level and max rate)
3. **Third:** one sentence explaining feasibility + why that overall risk
4. **Then:** risks table — every risk, one row each: level, **rate 0–10**, notes
5. If no risks found, one row: `No material risks identified` | Low | 0
6. Optional short **Next step** after the table

## Safety rules

1. **Always** measure risk (per-item + overall: level and rate 0–10) before acting.
2. **Always** inform the user with a clear warning when overall risk is **High** or **Critical**, or rate ≥ 6 — before any implementation.
3. **Always** append the Pre-flight Report as the **last section** of the response.
4. **Never** omit the report because the task looks simple or urgent.
5. **Never** proceed when **Can this be done?** is **No** without user acknowledgment.
6. **Never** proceed when overall risk is **High** or **Critical**, or rate ≥ 6, without explicit user confirmation.
7. **Always** include a **rate 0–10** for overall risk and for every risk row.
8. **Never** assume credentials, VPN, remote hosts, or production — verify or list as a risk.
9. **Never** validate by executing the risky operation — inspect and infer only.
10. **Never** assign a rate outside its level band (e.g. Low must be 0–2, High 6–8, Critical 9–10).

## Key facts & reference

| Item | Value |
|------|-------|
| Skill path | `.cursor/skills/request-preflight/SKILL.md` |
| Former name | `task-cirtix` (renamed) |
| Trigger phrases | pre-flight, validate request, can this be done, sanity check, review task, risk check |
| Risk levels | Low, Medium, High, Critical |
| Risk rate | Integer 0–10 (required overall + per row) |
| Rate bands | 0–2 Low, 3–5 Medium, 6–8 High, 9–10 Critical |
| Overall risk | Highest level and highest rate among all rows |
| Feasibility answers | Yes, Partially, No, Unknown |
| High-risk duty | Warn when High/Critical or rate ≥ 6; wait for confirmation |

### Feasibility signals (quick scan)

| Signal | Source |
|--------|--------|
| Workspace | List/read files, git status |
| Tools | MCP catalog, shell, skill list |
| Secrets | Env var **names** only — not `.env` contents unless user directs |
| Network / VPN | Failed reachability, related network skills |
| Constraints | User rules, AGENTS.md, attached skills |

## Output template

```markdown
## Pre-flight Report

**Can this be done?** Yes | Partially | No | Unknown

**Overall risk:** Low | Medium | High | Critical — N/10

[One sentence: why it can/cannot, and why that overall risk.]

| Risk | Level | Rate | Notes |
|------|-------|------|-------|
| [Specific risk or blocker] | Low / Medium / High / Critical | 0–10 | [Impact, mitigation, or what user must provide] |
| ... | ... | ... | ... |

**Next step:** [Proceed / Confirm high-risk X / Clarify Y / Stop]
```

### High-risk warning (put near top when High/Critical or rate ≥ 6)

```markdown
⚠️ **High risk (7/10)** — [one sentence naming the main hazard(s)]. Waiting for your confirmation before proceeding.

<!-- or -->

⛔ **Critical risk (10/10)** — [one sentence]. Cannot proceed until [what user must do].
```
