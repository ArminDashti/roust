---
name: skills-style-format
description: >-
  Enforces layout and writing standards on other Cursor skills. Apply before
  creating or editing any SKILL.md in ~/.cursor/skills/ — not to this file itself.
---

# Skills Style & Format

## Overview

Governance skill. When the user asks to **create or edit another skill**, read this first and make **that target skill** conform.

This file defines the standard. **Other** `SKILL.md` files must follow it.

Every target skill: `~/.cursor/skills/<skill-name>/SKILL.md`

## Objectives

When authoring or refactoring **another** skill, ensure it is:

1. **Simple** — plain language, bullets, short steps.
2. **Comprehensive** — all facts and actions the agent needs; nothing critical missing.
3. **Free of noise** — no filler, no basics the agent already knows, no repeated points.

Also enforce: fixed five-section layout, valid frontmatter, under 500 lines.

## Workflow

### When this applies

- User asks to create a new skill
- User asks to edit, refactor, or standardize an existing skill
- Agent is about to write any `SKILL.md` under `~/.cursor/skills/`

**Do not** reformat or expand this governance file unless the user explicitly asks to change the standard itself.

### Step 1 — Frontmatter (on the target skill)

```yaml
---
name: skill-name
description: >-
  [WHAT — third person]. Use when [triggers].
---
```

| Field | Rule |
|-------|------|
| `name` | Lowercase, hyphens, max 64 chars, matches directory |
| `description` | WHAT + WHEN, third person, max 1024 chars |

### Step 2 — Body layout (on the target skill)

Fixed H2 order — do not rename, skip, or reorder:

| Section | Content |
|---------|---------|
| Overview | Scope, exclusions, related skills |
| Objectives | Numbered outcomes |
| Workflow | Steps, checklists, commands |
| Safety rules | Never / Always constraints |
| Key facts & reference | Tables, paths, links to `reference.md` / scripts |

One H1 title after frontmatter. Optional H2 sections (`Edge cases`, `Output template`) only **after** Key facts & reference.

### Step 3 — Quality pass (on the target skill)

Before saving the target skill, verify:

**Simple**
- Bullets and tables over paragraphs
- One idea per line
- One default path; no option lists unless truly needed

**Comprehensive**
- Every action, path, command, and constraint the agent needs is present
- Scope and exclusions stated in Overview
- Steps live in Workflow, not scattered

**No noise** — delete from the target skill:
- Obvious explanations
- Content duplicated across sections
- Meta-commentary ("this section covers…")
- Long examples where one line suffices
- Padding to look thorough

Move long material to `reference.md` or `scripts/`; keep the target `SKILL.md` lean.

### Step 4 — Skeleton (output for new skills)

```markdown
---
name: your-skill-name
description: >-
  [WHAT]. Use when [triggers].
---

# Title

## Overview

- [Scope]
- [Exclusions]

## Objectives

1. [Outcome]

## Workflow

### Step 1: [Phase]

- [ ] [Action]

## Safety rules

1. **Never** [constraint].

## Key facts & reference

| Item | Value |
|------|-------|
| ... | ... |
```

### Step 5 — Final checklist (target skill only)

- [ ] Five sections, correct order
- [ ] Simple, comprehensive, no noise
- [ ] No duplicated content
- [ ] Forward slashes in paths
- [ ] Not under `~/.cursor/skills-cursor/`

## Safety rules

1. **Always** apply this standard to **other** skills when creating or editing them.
2. **Never** deliver a new or updated skill that violates the five-section layout unless the user explicitly overrides.
3. **Never** create skills in `~/.cursor/skills-cursor/`.
4. **Never** drop domain facts when refactoring — compress into the right section.
5. **Always** preserve user verbatim wording in the target skill when supplied.

## Key facts & reference

| Item | Value |
|------|------|
| Personal skills path | `~/.cursor/skills/<name>/SKILL.md` |
| Project skills path | `.cursor/skills/<name>/SKILL.md` |
| Reserved path | `~/.cursor/skills-cursor/` (do not write here) |
| Max body length | 500 lines |

### Legacy heading map (when refactoring old skills)

| Old | → New section |
|-----|---------------|
| About, Scope | Overview |
| Goal, Purpose | Objectives |
| Steps, Process | Workflow |
| Safety, Guardrails | Safety rules |
| Points, Facts | Key facts & reference |
