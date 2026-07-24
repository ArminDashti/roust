---
name: skill-style-format
description: >-
  Enforces layout and writing standards on other Cursor skills, including
  required metadata.json and create-skill precedence. Apply before creating or
  editing any SKILL.md under ~/.cursor/skills/ or .cursor/skills/ — not to this
  file itself unless the user asks to change the standard.
---

# Skills Style & Format

## Overview

- Governance skill. When the user asks to **create or edit another skill**, read this first and make **that target skill** conform
- Creation base: built-in Cursor skill `/create-skill` (`~/.cursor/skills-cursor/create-skill/`)
- **Priority:** this skill wins on any conflict with `/create-skill` (layout, metadata, paths, wording). Do **not** edit the built-in `/create-skill`
- Every target skill folder must include `SKILL.md` **and** `metadata.json`
- Exclusions: do not reformat this governance file unless the user explicitly asks to change the standard

## Objectives

1. **Simple** — plain language, bullets, short steps
2. **Comprehensive** — all facts and actions the agent needs; nothing critical missing
3. **Free of noise** — no filler, no basics the agent already knows, no repeated points
4. Enforce fixed five-section layout, valid frontmatter, under 500 lines, and required `metadata.json`

## Workflow

### When this applies

- User asks to create a new skill
- User asks to edit, refactor, or standardize an existing skill
- Agent is about to write any `SKILL.md` under `~/.cursor/skills/` or `.cursor/skills/`

### Step 0 — Create path (new skills only)

1. Read built-in `/create-skill` for discovery, directory layout, description rules, and anti-patterns
2. Apply **this** skill’s rules for body layout, quality, `metadata.json`, and checklist
3. If `/create-skill` and this skill disagree → follow **this** skill
4. **Never** modify `~/.cursor/skills-cursor/create-skill/` or any file under `~/.cursor/skills-cursor/`

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

### Step 3 — `metadata.json` (required in every skill folder)

Path: `<skill-folder>/metadata.json` (same folder as `SKILL.md`).

Required keys (exact spelling):

| Key | Rule | Default |
|-----|------|---------|
| `version` | Semver string, e.g. `1.0.0`. Bump on meaningful edits | `1.0.0` |
| `author` | Skill owner | `Armin Dashti` |
| `category` | Short label, e.g. `governance`, `deploy`, `database` | (derive from skill purpose) |
| `last-modified` | ISO datetime with time and offset, e.g. `2026-07-24T19:46:00+03:30` | now (update on every save) |
| `licence` | Spelling **licence**, not license | `MIT` |

Template:

```json
{
  "version": "1.0.0",
  "author": "Armin Dashti",
  "category": "",
  "last-modified": "YYYY-MM-DDTHH:mm:ss±HH:mm",
  "licence": "MIT"
}
```

- Create `metadata.json` when creating a skill
- When editing an existing skill that lacks it, add it before finishing
- On edit: set `last-modified` to current datetime with offset; bump `version` when behavior or contract changes
- Use defaults above unless the user overrides

### Step 4 — Quality pass (on the target skill)

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

### Step 5 — Skeleton (output for new skills)

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

Plus `metadata.json` in the same folder (see Step 3).

### Step 6 — Final checklist (target skill only)

- [ ] Five sections, correct order
- [ ] Simple, comprehensive, no noise
- [ ] `metadata.json` present with all five keys
- [ ] No duplicated content
- [ ] Forward slashes in paths
- [ ] Not under `~/.cursor/skills-cursor/`
- [ ] Built-in `/create-skill` was used as base for creation; this skill overrides conflicts

## Safety rules

1. **Always** apply this standard to **other** skills when creating or editing them.
2. **Always** use built-in `/create-skill` as the creation base; **this skill has priority** on conflicts.
3. **Never** modify built-in `/create-skill` or any file under `~/.cursor/skills-cursor/`.
4. **Never** deliver a new or updated skill without `metadata.json` (`version`, `author`, `category`, `last-modified`, `licence`).
5. **Never** deliver a new or updated skill that violates the five-section layout unless the user explicitly overrides.
6. **Never** create skills in `~/.cursor/skills-cursor/`.
7. **Never** drop domain facts when refactoring — compress into the right section.
8. **Always** preserve user verbatim wording in the target skill when supplied.

## Key facts & reference

| Item | Value |
|------|-------|
| Personal skills path | `~/.cursor/skills/<name>/SKILL.md` |
| Project skills path | `.cursor/skills/<name>/SKILL.md` |
| Metadata path | `<skill-folder>/metadata.json` |
| Create base | `/create-skill` → `~/.cursor/skills-cursor/create-skill/` (read only) |
| Priority | `skill-style-format` > `/create-skill` |
| Reserved path | `~/.cursor/skills-cursor/` (do not write here) |
| Max body length | 500 lines |
| Sample metadata | [samples/metadata.json](samples/metadata.json) |

### Legacy heading map (when refactoring old skills)

| Old | → New section |
|-----|---------------|
| About, Scope | Overview |
| Goal, Purpose | Objectives |
| Steps, Process | Workflow |
| Safety, Guardrails | Safety rules |
| Points, Facts | Key facts & reference |
