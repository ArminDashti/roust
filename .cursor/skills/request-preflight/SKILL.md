---
name: request-preflight
description: >-
  Runs a pre-flight checklist on a user request before execution: verifies
  feasibility, inventories risks, and always reports at the end whether the task
  can be done. Use when starting any non-trivial job, when the user asks to
  validate or sanity-check a request, or when the user may not know a task is
  impossible, partial, or high-risk.
---

# Request Pre-flight

## Overview

- Scope: pre-flight checklist before acting — like checks before flying
- Always ends with a **Pre-flight Report** (mandatory, last section of the response)
- Exclusions: does not implement the task; domain skills run after checklist passes
- Related skills: user Safety rules, domain skills (`code-removal`, `github-sync`), `exprience` for logging blocked work

## Objectives

1. Run every checklist item before or while scoping the work
2. Decide whether the task **can be done** — Yes, Partially, No, or Unknown
3. List **all** identified risks in one table with level Low, Medium, High, or Blocked
4. Surface what the user may not know (missing access, wrong assumptions, irreversible effects)
5. **Always** append the Pre-flight Report at the end of the response

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
- [ ] 7. Risk inventory (list every risk, assign level)
- [ ] 8. Pre-flight Report (mandatory — last section of response)
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

### Step 4: Risk inventory

Identify **every** risk — do not collapse into one overall level.

| Level | When |
|-------|------|
| Low | Read-only, reversible, local, no secrets |
| Medium | File writes, config changes, multi-step side effects |
| High | Production, deletes, force push, installs, firewall, credentials, bulk DB writes |
| Blocked | Hard stop — infeasible, policy violation, or irreversible without approval |

Common blind spots to check:

- Looks simple but touches production or shared infra
- "Just delete it" but references still exist
- Assumes credentials or VPN that are not present
- Rollback is costly (migrations, mass refactors, batch git ops)

### Step 5: Act on findings

| Can be done? | Highest risk | Action before implementing |
|--------------|--------------|----------------------------|
| Yes | Low only | Proceed |
| Yes | Medium | Proceed; note mitigations |
| Yes / Partially | High | Wait for explicit user confirmation |
| Partially | Any | Propose reduced scope or list missing inputs |
| No | Blocked | Stop — do not proceed |
| Unknown | Any | Ask clarifying questions only |

### Step 6: Pre-flight Report (always last)

**Mandatory.** Append as the **final section** of every response where this skill applies — never skip, never bury mid-response.

Use the output template in **Key facts & reference**.

Report rules:

1. **First line:** **Can this be done?** — Yes | Partially | No | Unknown
2. **Second:** one sentence explaining why
3. **Then:** risks table — every risk, one row each, level Low | Medium | High | Blocked
4. If no risks found, one row: `No material risks identified` | Low
5. Optional short **Next step** after the table

## Safety rules

1. **Always** append the Pre-flight Report as the **last section** of the response.
2. **Never** omit the report because the task looks simple or urgent.
3. **Never** proceed when **Can this be done?** is **No** without user acknowledgment.
4. **Never** assume credentials, VPN, remote hosts, or production — verify or list as a risk.
5. **Always** ask for confirmation when any risk is **High** or **Blocked**.
6. **Never** validate by executing the risky operation — inspect and infer only.

## Key facts & reference

| Item | Value |
|------|-------|
| Skill path | `.cursor/skills/request-preflight/SKILL.md` |
| Former name | `task-cirtix` (renamed) |
| Trigger phrases | pre-flight, validate request, can this be done, sanity check, review task |
| Risk levels | Low, Medium, High, Blocked |
| Feasibility answers | Yes, Partially, No, Unknown |

### Feasibility signals (quick scan)

| Signal | Source |
|--------|--------|
| Workspace | List/read files, git status |
| Tools | MCP catalog, shell, skill list |
| Secrets | Env var **names** only — not `.env` contents unless user directs |
| Network / VPN | Failed reachability, `wsl-bypass-vpn` skill |
| Constraints | User rules, AGENTS.md, attached skills |

## Output template

```markdown
## Pre-flight Report

**Can this be done?** Yes | Partially | No | Unknown

[One sentence: why it can, cannot, or needs clarification.]

| Risk | Level | Notes |
|------|-------|-------|
| [Specific risk or blocker] | Low / Medium / High / Blocked | [Impact, mitigation, or what user must provide] |
| ... | ... | ... |

**Next step:** [Proceed / Confirm X / Clarify Y / Stop]
```
