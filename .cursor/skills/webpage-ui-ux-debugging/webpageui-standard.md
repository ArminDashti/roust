# Webpage UI Standard (Audit Baseline)

Companion reference for `webpage-debugging`. Use this checklist when judging whether a page is messy and when writing Issues found categories.

Project design system, tokens, and sibling pages override these defaults when present.

## Discovery before audit

1. Find design tokens / theme: `tokens.*`, `theme.*`, `variables.css`, `tailwind.config.*`, `_variables.scss`
2. Find shared components: `components/`, `Controls/`, `Shared/`
3. Find global styles: `global.css`, `site.css`, `App.css`, master page / layout
4. Open **two similar sibling pages** in the same module — use them as the consistency bar
5. When project rules conflict with defaults below, **project rules win**

## Layout and structure

- [ ] One primary action per section; secondary actions visually subordinate
- [ ] Related fields and actions grouped (fieldset, card, panel, section heading)
- [ ] Aligned to a grid — columns and gutters match sibling pages
- [ ] Full width reserved for data-heavy content (tables, wide forms); no arbitrary max-width on operational screens

## Spacing

- [ ] Project spacing scale used when one exists; otherwise 4px or 8px base (4, 8, 12, 16, 24, 32, 48)
- [ ] Section padding: 16–24px; gap between related controls: 8–12px; gap between sections: 24–32px
- [ ] Controls not butted against container edges without padding
- [ ] Labels consistently above or beside inputs — match the app, do not mix

## Typography and hierarchy

- [ ] Page title largest and distinct; section headings smaller; body readable (typically 14–16px)
- [ ] One font family stack unless the project defines more
- [ ] Muted text only for hints and secondary metadata — not primary labels or required fields
- [ ] Long text truncated or wrapped intentionally; no overflow clipping without ellipsis or wrap

## Color and surfaces

- [ ] Project tokens or existing classes for primary, secondary, danger, success, borders, backgrounds
- [ ] No one-off hex/rgb when a shared variable or class exists
- [ ] Error and validation states use the app's danger color and placement
- [ ] No harsh 1px black borders or default unstyled browser controls on production screens

## Controls and consistency

- [ ] Existing button, input, select, table, modal, and badge components reused
- [ ] One primary button style on the page; destructive actions use danger variant
- [ ] Same control heights within a form row; aligned baselines in toolbars
- [ ] Icons match the app set and size; text labels when icons alone are ambiguous
- [ ] Loading, empty, and error states present where data is fetched or lists can be empty

## Forms

- [ ] Every input has a visible label (or established `aria-label` / `aria-labelledby` pattern)
- [ ] Required fields marked consistently with the rest of the app
- [ ] Tab order follows visual order
- [ ] Validation messages specific and adjacent to the field

## Tables and dense data

- [ ] Column headers align with data type (text left, numbers right when appropriate)
- [ ] Row hover and selection styles match sibling tables
- [ ] Sticky header or pagination when the app already uses that pattern
- [ ] Readable density — row height and cell padding match existing grids

## Accessibility (baseline)

- [ ] Contrast ≥ 4.5:1 normal text; ≥ 3:1 large text and UI boundaries
- [ ] Interactive targets ≥ 44×44px or padded to equivalent click area
- [ ] Focus visible on keyboard navigation — outline not removed without replacement
- [ ] Meaningful images and icons have accessible names
- [ ] Status not conveyed by color alone — text, icon, or pattern added

## Responsive behavior

- [ ] Layout works at supported breakpoints; no unintended horizontal scroll
- [ ] Touch-friendly spacing on mobile when the app is responsive

## Self-check (messy threshold)

Use after inspecting each page. Failures here feed Issues found in the output prompt.

```text
UI standards verification:
- [ ] Matches project design system / sibling pages
- [ ] Clear visual hierarchy (title → sections → actions)
- [ ] Consistent spacing and alignment
- [ ] Reused shared components and tokens (no one-off styles)
- [ ] All inputs labeled; errors clear and localized
- [ ] Primary vs secondary actions distinct
- [ ] Loading / empty / error states handled
- [ ] Contrast and focus acceptable
- [ ] No obvious layout defects at target viewport
```

**Messy** when any of:

- Two or more self-check categories fail
- Page clearly diverges from sibling/reference pages
- Controls unstyled, misaligned, or cramped enough to hurt usability
- Primary actions, labels, or errors hard to find or read

## Issue category map

| Category | Map to when |
|----------|-------------|
| layout | misalignment, no grid, cramped sections |
| spacing | no padding, inconsistent gaps |
| typography | weak hierarchy, unreadable sizes |
| controls | mixed button styles, default browser inputs |
| forms | missing labels, unclear required markers |
| tables | dense rows, misaligned columns |
| accessibility | low contrast, no focus, color-only status |
| consistency | one-off colors/fonts vs sibling pages |
| responsive | horizontal scroll, broken breakpoints |
| states | missing loading, empty, or error UI |

## Defaults (when no project scale)

| Item | Value |
|------|-------|
| Spacing base | 4px or 8px: 4, 8, 12, 16, 24, 32, 48 |
| Section padding | 16–24px |
| Control gap | 8–12px within groups; 24–32px between sections |
| Body text | 14–16px equivalent |
| Min contrast | 4.5:1 normal; 3:1 large / chrome |
| Min touch target | 44×44px |

## Conflict rules

| Situation | Rule |
|-----------|------|
| Project design system exists | Follow it |
| No design system | Mirror the two most similar existing pages |
| User asks for explicit style | User request wins if accessibility baseline holds |
| User preference recorded below | User preference wins over defaults in this file if accessibility baseline holds |
| Legacy page with mixed patterns | Match the dominant pattern on that module's other pages |

## User preferences

Lookup table for captured likes/dislikes. Each preference lives in its own file under `.cursor/skills/add-webpageui-preference/preferences/`. These override defaults in this file when they conflict (accessibility baseline still holds). Maintain with `add-webpageui-preference`.

| Title | Category | Standard | File |
|-------|----------|----------|------|

## Related

| Item | Path |
|------|------|
| Parent skill | `.cursor/skills/webpage-debugging/SKILL.md` |
| Capture preference skill | `.cursor/skills/add-webpageui-preference/SKILL.md` |
| Preferences directory | `.cursor/skills/add-webpageui-preference/preferences/` |
| Full standards skill | `.cursor/skills/webpage-standards/SKILL.md` |
