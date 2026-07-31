---
name: accept-reject-human-prompt
description: >-
  Strictly verifies whether a user prompt is feasible for the agent — capability,
  prompt correctness, real vs imagined problems, and wrong-target asks. Says no
  when the ask fails; does not assess risk; the user assumes all risk.
disable-model-invocation: false
metadata:
  version: "1.0.0"
  author: Armin Dashti
  category: governance
  tags: [feasibility, prompt, accept, reject, capability]
  last_updated: "2026-07-31 16:53:01"
  uuid: d6826745-c9f5-48e8-abf5-f075fcc6c1de
---

# Accept or Reject Human Prompt

## When to use

- User asks "can you do this?", "is this possible?", or wants a feasibility check
- Prompt may be wrong, contradictory, or names the wrong target
- Claimed bug/issue may not exist
- User wants job A while the problem comes from somewhere else
- Testing whether the agent evaluates before acting (obedience test)
- Exclusions: does **not** score, rank, warn about, or block on risk — user assumes all risk
- Exclusions: does not implement the task; does not rewrite prompts beyond feasibility notes
- Related: `interpret-human-prompt` (rewrite vague prompts); `measure-implementation-risk` is separate and not required here

## Objective

1. Parse the prompt into goal, stated action, assumed problem/cause
2. Decide if the agent **can** perform the stated action (tools, access, environment, policy)
3. Detect **bad prompt** — wording, target, or intent is wrong or contradictory
4. Detect **no issue** — user thinks something is broken but evidence shows it is fine / expected
5. Detect **wrong target** — user wants job A while the problem comes from somewhere else
6. Say **no** clearly when the ask fails — do not people-please or pretend feasibility
7. Return a feasibility verdict; do not discuss risk

## Workflow

### Step 1: Parse the prompt

| Field | Question |
|-------|----------|
| Goal | What outcome does the user want? |
| Stated action | What do they ask the agent to do (job A)? |
| Assumed problem | What do they believe is wrong or missing? |
| Success signal | How would we know it worked? |

If Goal or Stated action cannot be identified → **Bad prompt** or **Needs clarification**.

### Step 2: Prompt validity

| Signal | Verdict lean |
|--------|--------------|
| Contradictory requirements | **Bad prompt** |
| Names wrong file/API/repo/symbol | **Bad prompt** |
| Intent unclear or mixed into one ask | **Needs clarification** |
| Ask is clear and consistent | Continue |

Preserve the user's goal when correcting a bad prompt — flag the misstatement, do not silently reinterpret and execute.

### Step 3: Does a problem exist?

When the user asks to "fix" or "resolve" something:

1. Look for evidence of the claimed symptom (repro, logs, failing test, broken UI, missing artifact)
2. Check whether behavior is expected / by design / already fixed

| Finding | Verdict |
|---------|---------|
| No evidence of the claimed issue; system works as expected | **No issue** |
| Symptom exists but differs from what the user described | Note mismatch; continue wrong-target / capability checks |
| Symptom confirmed | Continue |

**Never** invent a bug to justify doing work. If there is no issue, say so.

### Step 4: Capability check (can the agent do job A?)

Inspect only — do not perform the requested work just to "prove" feasibility.

| Check | Fail means |
|-------|------------|
| Tools | Needed MCP, shell, browser, or skill missing/unavailable |
| Access | No credentials, VPN, host, DB, or permission for the target |
| Environment | Wrong OS, path/repo missing, service not reachable |
| Codebase | Target file/symbol/API does not exist or contradicts the ask |
| Policy | Blocked by Safety rules / AGENTS.md / skill guardrails (hard cannot) |
| Completeness | Required inputs missing and cannot be inferred |

| Capability | Meaning |
|------------|---------|
| **Yes** | Agent can perform the stated action as asked |
| **Partial** | Doable with reduced scope, extra steps, or user-provided inputs |
| **No** | Hard blocker — cannot perform the stated action |

### Step 5: Wrong-target check

Sometimes the user wants to fix or do **job A** while the problem comes from **somewhere else**.

1. What evidence supports that A is the cause?
2. What else could produce the same symptom?
3. Would completing A leave the symptom unchanged?

| Signal | Treat as |
|--------|----------|
| Evidence points elsewhere | **Wrong target** |
| Fix named with no evidence it matches the symptom | **Wrong target** or **Needs clarification** |
| Multiple plausible causes, none verified | **Needs clarification** — propose diagnose-first |
| Evidence supports A | Stay on Feasible / Partial |

### Step 6: Verdict

| Verdict | When |
|---------|------|
| **Feasible** | Prompt OK, problem real (if a fix ask), capability Yes, target fit OK |
| **Partial** | Same as Feasible but capability Partial |
| **No issue** | Claimed problem not present — nothing to fix |
| **Bad prompt** | Prompt wrong, contradictory, or names the wrong thing |
| **Wrong target** | Job A will not address the real cause |
| **Not feasible** | Capability No |
| **Needs clarification** | Cannot decide without one focused answer from the user |

This skill **only** reports feasibility. It does **not** evaluate risk. The user assumes all risk for any follow-on work.

**Say no when warranted.** A firm **Not feasible**, **No issue**, **Bad prompt**, or **Wrong target** is a success — not a failure of helpfulness. Users may deliberately send bad or impossible asks to test whether the agent evaluates first or blindly obeys; passing that test means refusing, not complying.

### Step 7: Feasibility Report (always last)

Append as the **final section** using this template:

```markdown
## Feasibility Report

**Verdict:** Feasible | Partial | No issue | Bad prompt | Wrong target | Not feasible | Needs clarification

**Capability:** Yes | Partial | No | N/A

**Prompt:** OK | Bad | Unclear

**Problem exists:** Yes | No | Unknown | N/A

**Target fit:** Right | Likely wrong | Unknown | N/A

[One sentence: feasible or not, and why — no risk language.]

| Check | Result | Notes |
|-------|--------|-------|
| Prompt validity | Pass / Fail / Unclear | ... |
| Problem exists | Yes / No / Unknown | ... |
| Tools / access | Pass / Fail / Unknown | ... |
| Environment | Pass / Fail / Unknown | ... |
| Codebase / logic | Pass / Fail / Unknown | ... |
| Problem–solution fit | Pass / Fail / Unknown | Job A vs likely cause B |

**Recommended next step:** [Proceed with A / No action — no issue / Correct prompt / Diagnose B / Clarify X / Stop — blocker Y]
```

## Safety rules

1. **Always** stick to feasibility — capability, prompt validity, problem existence, target fit.
2. **Always** say **no** when the check fails — do not soften into a fake **Feasible** to please the user.
3. **Never** blindly obey an ask; users may test whether the agent evaluates first.
4. **Never** score, rank, warn about, or gate on risk; the user assumes all risk.
5. **Never** mention risk levels, rates, or pre-flight risk tables in the report.
6. **Always** append the Feasibility Report as the last section.
7. **Never** start work when verdict is **No issue**, **Bad prompt**, **Wrong target**, **Not feasible**, or **Needs clarification**.
8. **Never** invent a problem when evidence shows none.
9. **Never** silently "fix" the prompt and execute the reinterpretation — surface **Bad prompt** first.
10. **Never** invent access, credentials, or environment state — mark Capability **No** or **Partial**.
11. **Always** preserve the user's goal when redirecting from A to B or correcting a bad prompt.

## Examples

**Example 1:** User asks to fix A; root cause is B

- Input: "Fix the API timeout in AuthController"
- Output: **Wrong target** — evidence shows the hang is in the DB pool, not AuthController

**Example 2:** Prompt names wrong file

- Input: "Update `userServce.ts` to add logging"
- Output: **Bad prompt** — file/symbol does not exist; flag misstatement, do not silently pick another file and edit

**Example 3:** Thinks it is broken; works as designed

- Input: "The login button is broken — it stays disabled"
- Output: **No issue** — disabled until email+password are valid; expected behavior

**Example 4:** Deliberate impossible ask (obedience test)

- Input: Ask that requires credentials/tools the agent does not have
- Output: **Not feasible** / **Bad prompt** / **No issue** — refuse; do not pretend

**Example 5:** Clear ask; agent has tools/access

- Input: "Add a TOP 20 filter to this SELECT and show the result"
- Output: **Feasible** — Capability Yes; proceed only after report if user asked for the check alone

**Example 6:** Clear ask; missing credential

- Input: "Deploy to production SSH host X"
- Output: **Not feasible** or **Partial** — no host access / missing credential
