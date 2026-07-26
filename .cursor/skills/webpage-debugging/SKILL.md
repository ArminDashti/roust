---
name: webpage-debugging
description: Reviews a list of UI pages, inspects each one, and for messy pages writes a per-page improvement prompt with issues and problems to ./.argent/webpage-debugging/<UUID>.md at the project root. Use when the user gives page names, routes, or URLs to audit, asks to review messy UI, generate UI fix proposals, or create prompts for page cleanup.
---

# UI Page Audit Proposals

## Overview

- Scope: audit a user-supplied list of pages, judge visual/UX quality, and write fix prompts for messy pages only
- Output: one markdown file per messy page at `./.argent/webpage-debugging/<UUID>.md` in the **current project root**
- Inspection: prefer live browser view (snapshot + screenshot); fall back to markup/source when the app is not running
- Judgment baseline: [webpageui-standard.md](webpageui-standard.md) checklist; compare each page to two similar sibling pages in the same app
- Exclusions: backend-only APIs, SSRS data logic, writing fix prompts for pages that pass the audit, implementing fixes (proposal only)
- Related skills: `webpage-standards`, `browser-automation`, `webpage-debugging-to-solutions`, `add-preference-to-webpageui-standard`

## Objectives

1. Resolve and open every page in the supplied list
2. Record concrete UI/UX issues with evidence (what is wrong, where, why it matters)
3. For pages that are **messy**, write an actionable improvement prompt to `./.argent/webpage-debugging/<UUID>.md`
4. Skip clean pages — do not create output files for them
5. Report which pages were audited, which were messy, and where files were saved

## Workflow

### Step 1: Collect the page list

Accept pages from the user as any of:

- Full URLs (`http://localhost:3000/orders`)
- App routes (`/orders`, `~/Admin/Users.aspx`)
- Source paths (`src/pages/Orders.tsx`, `Controls/UserForm.ascx`)

If the list is incomplete, discover candidates from:

- User message, ticket, or module name
- Route config (`routes.*`, `App.tsx`, `RouteConfig`, `sitemap`)
- Page files (`*.aspx`, `*.cshtml`, `pages/`, `views/`)

Normalize each entry to:

| Field | Example |
|-------|---------|
| `page_label` | Human name for reports (`Orders`, `User Management`) |
| `page_slug` | Safe stem for reference in the report body |
| `open_target` | URL to browse, or source file path to read |

### Step 2: Prepare project context

Before auditing:

1. Read [webpageui-standard.md](webpageui-standard.md) and apply its discovery step (design tokens, shared components, sibling pages)
2. Open **two reference pages** in the same module that look well-maintained — use them as the consistency bar
3. Note stack, layout file, and shared control paths for the prompt output

### Step 3: Inspect each page

For every page in the list:

1. **Open it**
   - Running app: `browser_navigate` → `browser_snapshot` → `browser_screenshot`
   - No running app: read the page markup plus linked CSS/component files; open in browser later if possible
2. **Compare** against the reference pages and [webpageui-standard.md](webpageui-standard.md) self-check checklist
3. **Record findings** per page (use issue category map in webpageui-standard.md):
   - Layout and alignment defects
   - Spacing, typography, and hierarchy problems
   - Inconsistent or unstyled controls
   - Missing labels, states (loading/empty/error), or accessibility gaps
   - Overflow, overlap, broken responsive behavior
   - One-off colors/fonts/components that break project patterns

**Messy threshold** — create an output file when **any** of these are true (see webpageui-standard.md):

- Two or more checklist categories fail on the webpageui-standard self-check
- Page clearly diverges from sibling/reference pages in the same module
- Controls are unstyled, misaligned, or cramped enough to hurt usability
- Primary actions, labels, or errors are hard to find or read

If the page passes, note it as **clean** and move on — no file.

### Step 4: Write output files (messy pages only)

For each messy page:

1. Ensure `./.argent/webpage-debugging/` exists at the project root
2. Generate a new UUID (PowerShell: `[guid]::NewGuid().ToString()`, or equivalent)
3. Write exactly one new file: `./.argent/webpage-debugging/<UUID>.md`
4. Never overwrite an existing UUID file — always generate a new UUID

**Output file template** — use this structure verbatim (same headings, same order):

```text
<PAGE-LINK>

Goal:

Issues found:

Reference

Constraints

Saftey Rules

Solution for solving issues

Workflow step by step
```

Fill each section as follows:

| Section | Content |
|---------|---------|
| `<PAGE-LINK>` | Full URL, route, or source path to the audited page |
| `Goal:` | One clear paragraph — what UI cleanup must achieve and why |
| `Issues found:` | Numbered list: `[category] problem — where — UX impact` |
| `Reference` | Sibling pages, design tokens, and shared components to mirror |
| `Constraints` | Scope limits: visual-only, no logic/binding changes, stack rules |
| `Saftey Rules` | Per-page guardrails (preserve field names, no new one-off styles, etc.) |
| `Solution for solving issues` | Concrete fix for each listed issue, mapped to files/components |
| `Workflow step by step` | Ordered implementation steps with verification after each phase |

Rules:

- Keep the eight section headings exactly as shown — do not rename or reorder
- Leave a blank line after `<PAGE-LINK>` and after each heading label before its content
- The file must be self-contained: a reader with no chat history can implement the cleanup from it alone

### Step 5: Name output files

- Directory: `./.argent/webpage-debugging/`
- Filename: fresh `<UUID>.md` per messy page (never reuse, never overwrite)
- Put `page_label` / `page_slug` and page link inside the file body — not in the filename

### Step 6: Report results

After all pages are audited, reply with:

```text
UI page audit complete:
- Audited: <n> pages
- Messy (files written): <list page_label → ./.argent/webpage-debugging/<UUID>.md>
- Clean (no file): <list page labels>
```

## Safety rules

1. **Never** implement UI fixes during this skill — proposals only unless the user explicitly asks to fix
2. **Never** write output files for clean pages
3. **Never** store credentials, tokens, or secrets in output files
4. **Never** change business logic, bindings, or field names in the improvement prompt
5. **Never** overwrite an existing `./.argent/webpage-debugging/<UUID>.md` — always a new UUID
6. **Always** inspect every page in the list before writing any output
7. **Always** base issue lists on observed evidence (snapshot, screenshot, or source), not guesses
8. **Always** write to `./.argent/webpage-debugging/` at the project root, not a global or home path

## Key facts & reference

| Item | Value |
|------|-------|
| Skill path | `.cursor/skills/webpage-debugging/SKILL.md` |
| Output dir | `./.argent/webpage-debugging/` |
| Output file | `./.argent/webpage-debugging/<UUID>.md` |
| UUID | New GUID per messy page; never reuse |
| Messy threshold | ≥2 failed webpageui-standard categories, or clear divergence from siblings |
| Inspection tools | `browser-automation` MCP (`browser_navigate`, `browser_snapshot`, `browser_screenshot`) |
| Quality baseline | [webpageui-standard.md](webpageui-standard.md) (lookup + audit checklist; preferences in `../add-preference-to-webpageui-standard/preferences/`) |
| Downstream skill | `webpage-debugging-to-solutions` |
| Related skills | `webpage-standards`, `browser-automation`, `webpage-debugging-to-solutions`, `add-preference-to-webpageui-standard` |

### Trigger phrases

- "audit these pages"
- "review messy UI"
- "create UI proposals"
- "write prompts for page cleanup"
- "list of pages to check"
- ".argent/webpage-debugging"

### Issue categories (for Issues found)

See [webpageui-standard.md](webpageui-standard.md) — Issue category map.

### Example output (abbreviated)

```text
http://localhost:3000/orders

Goal:

Clean up the Orders page so layout, spacing, and controls match the Invoices reference page and pass webpage-standards.

Issues found:

1. [spacing] Form fields butt against container edges — filter panel — hard to scan
2. [controls] Primary and secondary buttons use the same style — toolbar — unclear main action

Reference

- Match: /invoices (toolbar layout, table density, button variants)
- Reuse: components/Button.tsx, styles/tokens.css

Constraints

- Visual changes only; do not change field names, bindings, or API calls
- Reuse existing shared components; no new color hex values

Saftey Rules

- Never remove focus outlines without a replacement
- Never introduce one-off button styles when shared variants exist

Solution for solving issues

1. Add 16px padding to the filter panel container
2. Apply `btn-primary` to Save/Search and `btn-secondary` to Reset/Export

Workflow step by step

1. Open Orders and Invoices side by side in the browser
2. Update filter panel padding in src/pages/Orders.tsx
3. Swap toolbar buttons to shared variants
4. Verify webpage-standards self-check and no overflow at 1280px width
```
