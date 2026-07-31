---
name: create-edit-skill
description: >-
  Enforce a fixed layout and writing standard when creating or editing other skills.
disable-model-invocation: false
metadata:
  version: "1.1.0"
  author: Armin Dashti
  category: governance
  tags: [skills, format, layout, frontmatter]
  last_updated: "2026-07-31 16:16:21"
  uuid: dd2dae87-38d6-41f9-8560-ef9e9dd75f27
---

# Skills Style & Format

## When to use

- User asks to create a new skill
- User asks to edit, refactor, or standardize an existing skill
- Agent is about to write any `SKILL.md` under `.cursor/skills/`

**Do not** reformat this governance file unless the user explicitly asks to change the standard itself.

Every target skill path (project root only): `.cursor/skills/<skill-name>/SKILL.md`

## Objective

When authoring or refactoring **another** skill, ensure it is:

1. **Simple** — plain language, bullets, short steps.
2. **Comprehensive** — all facts and actions the agent needs; nothing critical missing.
3. **Free of noise** — no filler, no basics the agent already knows, no repeated points.

Also enforce: fixed section layout below, valid frontmatter, under 500 lines.

## Workflow

### Step 0 — Edit order (when changing an existing skill)

If the user asks to edit a skill that does **not** already follow this standard:

1. **First** — refactor the target skill to match the standard (full frontmatter including metadata, five sections, no Overview, correct third-section name, short Objective `description`). Preserve all domain facts; use the legacy heading map. Keep existing `uuid` if present; generate one only if missing.
2. **Then** — apply the user's requested changes on the standardized skill.

Do not apply user edits onto a non-standard layout and leave it non-compliant. If the skill already follows the standard, skip step 1 and apply the user's changes only.

### Step 1 — Frontmatter (on the target skill)

Always define all of these fields:

```yaml
---
name: skill-name
description: >-
  [Short description of the Objective — third person.]
disable-model-invocation: false
metadata:
  version: "1.0.0"
  author: Armin Dashti
  category: [category]
  tags: [tag1, tag2]
  last_updated: "YYYY-MM-DD HH:MM:SS"
  uuid: [uuid-v4]
---
```


| Field | Rule |
|-------|------|
| `name` | Lowercase, hyphens, max 64 chars, matches directory |
| `description` | Short summary of Objective only; third person; max 1024 chars; no triggers |
| `disable-model-invocation` | Always `false` |
| `metadata.version` | Semver `"MAJOR.MINOR.PATCH"`. **Must** change on every create/edit that alters the skill. New skill → `"1.0.0"`. Then bump by change size (see below). |
| `metadata.author` | Always `Armin Dashti` |
| `metadata.category` | One short category label (e.g. `governance`, `devops`, `database`) |
| `metadata.tags` | List of short keywords |
| `metadata.last_updated` | Datetime `YYYY-MM-DD HH:MM:SS` (set to now on create/edit) |
| `metadata.uuid` | Stable UUID v4; generate once on create; **never** change on edit |

**Version bumps (required on every content change):**

| Change | Bump |
|--------|------|
| Fix typos, clarify wording, small safety/checklist tweaks | PATCH (`1.0.0` → `1.0.1`) |
| New steps, sections content, metadata rules, behavior changes | MINOR (`1.0.1` → `1.1.0`) |
| Incompatible redesign of the skill’s purpose or required layout | MAJOR (`1.1.0` → `2.0.0`) |

Do **not** leave `version` unchanged when the skill body or required frontmatter rules change.




### Step 2 — Body layout (on the target skill)

One H1 title after frontmatter. Fixed H2 order — do not skip or reorder:


| Section      | Content                                                                 |
| ------------ | ----------------------------------------------------------------------- |
| When to use  | Triggers, scope, exclusions, related skills                             |
| Objective    | Numbered outcomes (singular heading)                                    |
| Workflow **or** Guidelines | Linear steps → `Workflow`; non-linear rules/decisions → `Guidelines` |
| Safety rules | Never / Always constraints                                              |
| Examples     | Concrete input → output or before/after samples                         |


**Third-section name (fixed — do not invent other titles):**


| Situation                                  | H2 title     |
| ------------------------------------------ | ------------ |
| Linear, ordered steps                      | `Workflow`   |
| Non-linear rules, decisions, or guidelines | `Guidelines` |


Do **not** include an `Overview` section. Put scope and exclusions under **When to use**.

Optional extra H2 sections (`Edge cases`, `Key facts`, links to `reference.md`) only **after** Examples.

### Step 3 — Quality pass (on the target skill)

**Simple**

- Bullets and tables over paragraphs
- One idea per line
- One default path; no option lists unless truly needed

**Comprehensive**

- Every action, path, command, and constraint the agent needs is present
- Triggers and exclusions stated in When to use
- Steps live in Workflow or Guidelines, not scattered

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
  [Short description of the Objective.]
disable-model-invocation: false
metadata:
  version: "1.0.0"
  author: Armin Dashti
  category: [category]
  tags: [tag1, tag2]
  last_updated: "YYYY-MM-DD HH:MM:SS"
  uuid: [uuid-v4]
---

# Title

## When to use

- [Trigger]
- [Exclusion]

## Objective

1. [Outcome]

## Workflow

### Step 1: [Phase]

- [ ] [Action]

## Safety rules

1. **Never** [constraint].

## Examples

**Example 1:** [short case]

- Input: …
- Output: …
```

If the third section is not linear, use `## Guidelines` (not Workflow) and use bullets or decision rules instead of numbered steps. Do not invent other third-section titles.

### Step 5 — Final checklist (target skill only)

- [ ] Frontmatter has `name`, `description`, `disable-model-invocation: false`, and full `metadata`
- [ ] `metadata.author` is `Armin Dashti`; `uuid` stable; `last_updated` set to now (`YYYY-MM-DD HH:MM:SS`)
- [ ] `metadata.version` bumped for this change (PATCH / MINOR / MAJOR as appropriate)
- [ ] Five required sections, correct order
- [ ] No Overview section
- [ ] `description` is a short Objective summary (no triggers)
- [ ] Third section is exactly `Workflow` (linear) or `Guidelines` (non-linear) — no other titles
- [ ] If editing: standardized first, then user changes applied
- [ ] Simple, comprehensive, no noise
- [ ] No duplicated content
- [ ] Forward slashes in paths
- [ ] Path is `.cursor/skills/<name>/` under the project root only
- [ ] Not under `~/` or any home-directory skills path
- [ ] Considered suggestions/flaws; reported only if real ones exist

### Step 6 — Suggestions and flaws (every create or edit)

After creating or editing the target skill — no matter what the user asked — **always consider** whether there are useful suggestions or real flaws (gaps, ambiguity, conflicts, missing safety, weak examples, etc.).

| Finding | Action |
|---------|--------|
| Agent found real suggestions or flaws | Point them out briefly after delivering the skill work |
| Agent found none | Say nothing about suggestions or flaws — do not invent filler |

Do not pad the response with empty "looks good" reviews or forced improvement lists.



## Safety rules

1. **Always** apply this standard to **other** skills when creating or editing them.
2. **Always** standardize a non-compliant skill **before** applying the user's edit requests.
3. **Always** include frontmatter: `name`, `description`, `disable-model-invocation: false`, and full `metadata` (`version`, `author: Armin Dashti`, `category`, `tags`, `last_updated`, `uuid`).
4. **Always** bump `metadata.version` when the skill changes (PATCH / MINOR / MAJOR by change size); set `last_updated` to now.
5. **Always** consider suggestions and flaws after every create or edit; report them only when they are real.
6. **Never** invent filler suggestions or forced "improvements" when nothing meaningful is wrong.
7. **Never** deliver a new or updated skill that violates the section layout unless the user explicitly overrides.
8. **Never** add an Overview section to a target skill.
9. **Never** invent a third-section title — use only `Workflow` or `Guidelines`.
10. **Never** put triggers or "Use when …" in `description` — only a short Objective summary.
11. **Never** change `metadata.uuid` after create; **never** set `disable-model-invocation` to anything but `false`.
12. **Never** leave `metadata.version` unchanged when skill content or rules change.
13. **Never** create or edit skills under `~/` (home directory) — project root `.cursor/skills/` only.
14. **Never** drop domain facts when refactoring — compress into the right section.
15. **Always** preserve user verbatim wording in the target skill when supplied.



## Examples

**Example 1 — Linear skill (use Workflow)**

```markdown
## When to use
- User asks to commit and push

## Objective
1. Create a clear commit and push to origin.

## Workflow
### Step 1: Inspect
- Run git status, diff, log
### Step 2: Commit
- Stage and commit
### Step 3: Push
- Push with tracking if needed

## Safety rules
1. **Never** force-push to main.

## Examples
**Example:** User says "commit and push" → status → commit → push → report URL.
```

**Example 2 — Non-linear skill (use Guidelines)**

```markdown
## When to use
- User asks for naming advice

## Objective
1. Pick a clear industry-standard name.

## Guidelines
- Prefer full words over abbreviations
- Match existing project conventions
- Avoid generic names (`helper`, `utils`)

## Safety rules
1. **Never** invent a new convention when the repo already has one.

## Examples
**Example:** `GetUserById` over `FetchData` for a user lookup method.
```

**Example 3 — Edit a non-standard skill**

User: "Add a safety rule to never skip tests" on a skill that still has Overview / Objectives / Key facts.

1. Refactor to When to use → Objective → Workflow **or** Guidelines → Safety rules → Examples (map old headings; keep all facts).
2. Then add the new safety rule under Safety rules.

**Example 4 — Suggestions only when real**

- After an edit, agent notices the skill has no Examples and a vague Objective → briefly point that out.
- After an edit that is already clear and complete → deliver the change only; no suggestion block.



### Legacy heading map (when refactoring old skills)


| Old                                  | → New section                                 |
| ------------------------------------ | --------------------------------------------- |
| Overview, About, Scope               | When to use                                   |
| Goal, Purpose, Objectives            | Objective                                     |
| Steps, Process (linear)              | Workflow                                      |
| Steps, Process (non-linear)          | Guidelines                                    |
| Safety, Guardrails                   | Safety rules                                  |
| Key facts & reference, Points, Facts | Examples (or optional section after Examples) |



| Item              | Value                                |
| ----------------- | ------------------------------------ |
| Skills path       | `.cursor/skills/<name>/SKILL.md`     |
| Scope             | Project root only — never `~/`       |
| Max body length   | 500 lines                            |


