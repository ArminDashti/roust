---
name: add-webpageui-preference
description: >-
  Organizes a UI preference into its own markdown file and registers it in the
  webpageui-standard.md lookup table. Takes a screenshot when the user does not
  provide one. Use when the user likes a webpage UI, asks to save a preference,
  add a UI rule to webpageui-standard, or capture a design preference from a
  page or screenshot.
---

# Add Preference to Webpage UI Standard

## Overview

- Scope: **organize** one UI preference — screenshot if needed, write one preference `.md`, register it in the lookup table
- Lookup table: `.cursor/skills/webpage-debugging/webpageui-standard.md` (`## User preferences`)
- Preference files: `.cursor/skills/add-webpageui-preference/preferences/<slug>.md` (one file per preference)
- Input: screenshot, page URL, and/or short explanation
- Exclusions: implementing UI in app code, rewriting baseline audit sections, expanding preferences into long essays
- Related skills: `webpage-debugging`, `webpage-standards`

## Objectives

1. Ensure there is a screenshot (use the user's, or take one)
2. Organize the preference into a single `.md` file under `preferences/`
3. Add or update one row in the `webpageui-standard.md` lookup table
4. Confirm the slug, file path, and lookup row

## Workflow

### Step 1: Collect input

Accept any of:

- Screenshot / image attachment
- Page URL or open browser tab
- Short explanation of what to prefer or avoid

### Step 2: Screenshot (required when missing)

| User gave | Action |
|-----------|--------|
| Screenshot / image | Use it — do not re-capture unless they ask |
| URL, no screenshot | `browser_navigate` → `browser_snapshot` → `browser_take_screenshot` |
| Explanation + known page/tab, no screenshot | Open that page/tab and take a screenshot |
| Explanation only, no page | Skip screenshot; organize from text |

Save evidence path in the preference file when a screenshot was taken or provided (path or note). Do **not** embed base64 in markdown.

### Step 3: Organize the preference

Read `.cursor/skills/webpage-debugging/webpageui-standard.md` lookup table first.

Distill **one** preference:

| Field | Rule |
|-------|------|
| `preference_title` | Short name (e.g. `Dense filter toolbar`) |
| `slug` | kebab-case filename stem, unique under `preferences/` |
| `category` | One lookup category: `layout`, `spacing`, `typography`, `color`, `controls`, `forms`, `tables`, `accessibility`, `responsive`, `consistency`, `states` |
| `standard` | One imperative line — Prefer / Use / Avoid … |
| `note` | Optional one line from the user explanation |

Keep it thin — organize only; do not invent extra rules.

If the same standard already exists in the lookup table or a preference file, do not duplicate — report the existing row/file.

### Step 4: Write the preference file

1. Ensure `.cursor/skills/add-webpageui-preference/preferences/` exists
2. Create `.cursor/skills/add-webpageui-preference/preferences/<slug>.md` using this template:

```markdown
# <preference_title>

| Field | Value |
|-------|-------|
| Category | <category> |
| Standard | <standard> |
| Source | screenshot \| explanation \| both |
| Screenshot | <path or none> |

## Note

<note or "—">
```

3. Never overwrite an existing `<slug>.md` — pick a new slug or update only if the user asked to revise that preference

### Step 5: Update the lookup table

In `.cursor/skills/webpage-debugging/webpageui-standard.md`, under `## User preferences`, add one table row:

| Title | Category | Standard | File |
|-------|----------|----------|------|
| `<preference_title>` | `<category>` | `<standard>` | [../add-webpageui-preference/preferences/`<slug>`.md](../add-webpageui-preference/preferences/`<slug>`.md) |

- Create the table header if the section has no rows yet
- Do not paste the full preference body into the lookup file
- Do not change baseline checklist sections unless the user asks

### Step 6: Confirm

```text
Preference organized:
- File: .cursor/skills/add-webpageui-preference/preferences/<slug>.md
- Lookup: webpageui-standard.md → User preferences → <preference_title>
- Screenshot: provided | captured | skipped (text-only)
```

## Safety rules

1. **Never** implement app UI changes for this skill — organize files only
2. **Never** embed image base64 in preference or lookup markdown
3. **Never** invent preferences the user did not show or state
4. **Never** duplicate an existing lookup row or preference file
5. **Never** overwrite an existing preference file unless the user asks to revise it
6. **Always** take a screenshot when the user did not provide one and a page/URL/tab is available
7. **Always** keep one preference per `.md` file
8. **Always** register every new preference in the `webpageui-standard.md` lookup table

## Key facts & reference

| Item | Value |
|------|-------|
| Skill path | `.cursor/skills/add-webpageui-preference/SKILL.md` |
| Lookup table | `.cursor/skills/webpage-debugging/webpageui-standard.md` |
| Lookup section | `## User preferences` |
| Preferences dir | `.cursor/skills/add-webpageui-preference/preferences/` |
| Preference file | `.cursor/skills/add-webpageui-preference/preferences/<slug>.md` |
| Screenshot tools | `browser_navigate`, `browser_snapshot`, `browser_take_screenshot` |
| Parent audit skill | `webpage-debugging` |

### Trigger phrases

- "add this preference to webpageui-standard"
- "I like this UI — save it as a standard"
- "organize this UI preference"
- "capture this as a webpageui preference"
- "add-webpageui-preference"
