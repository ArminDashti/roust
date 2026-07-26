---
name: software-versioning
description: >-
  Assigns and bumps software version numbers using SemVer, CalVer, sequential
  builds, and pre-release tags. Use when versioning a release, choosing the next
  version, writing Git tags, updating CHANGELOG version headers, or when the
  user asks about SemVer, CalVer, alpha/beta/RC, or build numbers. Focus only
  on versioning — not packaging, publishing, or deploy pipelines beyond version
  bump automation.
---

# Software Versioning

## Overview

- Scope: assigning unique names or numbers to specific releases of a software program so developers, users, and package managers can track changes, understand compatibility, and know when to update
- Default scheme: Semantic Versioning (SemVer) unless the project already uses another scheme
- Exclusions: package publishing, marketplace listings, installers, and full CI/CD design — only the version number, tag, and CHANGELOG version entry
- Related: none required; pair with project release docs if present

## Objectives

1. Pick the correct scheme for the project (SemVer, CalVer, sequential, or hybrid)
2. Compute the next version from the change type (breaking / feature / fix / pre-release)
3. Apply naming conventions (`v` prefix, pre-release tags, build pairing)
4. Keep released versions immutable and documented in CHANGELOG

## Workflow

### Step 1: Detect current scheme

- [ ] Read the latest version from the project source of truth (e.g. `package.json`, `.csproj`, `VERSION`, latest Git tag)
- [ ] Identify the scheme:

| Scheme | Pattern | When to use |
|--------|---------|-------------|
| SemVer | `MAJOR.MINOR.PATCH` (e.g. `v2.4.1`) | Default / industry standard |
| CalVer | `YYYY.MM.PATCH` or `YY.MM` | Time-scheduled releases |
| Sequential | Single increasing number (e.g. Build 1405) | Internal builds; often paired with SemVer |
| Pre-release | SemVer + hyphen tag | Alpha / beta / RC before a full release |

### Step 2: Choose next SemVer (default path)

Each number is incremented based on the type of changes made:

| Segment | Example | Increment when |
|---------|---------|----------------|
| MAJOR | `2.x.x` | Incompatible API changes or breaking changes. If a user updates to a new major version, they might need to change their own code. |
| MINOR | `x.4.x` | New functionality in a backwards-compatible manner. The user gets new features, but their existing setup won't break. |
| PATCH | `x.x.1` | Backwards-compatible bug fixes |

Examples:

- Current `1.4.2` + typo fix → `1.4.3`
- Current `1.4.2` + new login feature → `1.5.0`
- Brand new / initial development → start at `0.1.0`; keep MAJOR at `0` until stable, then release `1.0.0`

### Step 3: Apply CalVer when the project uses dates

- Format usually `YYYY.MM.PATCH` or `YY.MM`
- Examples: Ubuntu `22.04` / `24.04`; JetBrains `2023.1`; Unity `2022.3.1f1`
- Bump the date segment on the scheduled release; bump PATCH for fixes within that period

### Step 4: Attach pre-release or build metadata when needed

Pre-release tags (attached with a hyphen):

| Tag | Form | Meaning |
|-----|------|---------|
| Alpha | `v1.0.0-alpha` | Internal testing, very buggy, incomplete features |
| Beta | `v1.0.0-beta.1` | Public testing, mostly feature-complete but likely contains bugs |
| RC | `v1.0.0-rc.1` | Believed to be the final version; last check before official release |

Sequential / build numbers:

- Use a continuously increasing number for compiled builds (e.g. Build 1405, Build 1406)
- Pair with SemVer when useful: `v1.2.0 (Build 403)`

### Step 5: Record and tag

- [ ] Prefer a `v` prefix for Git tags and release artifacts (`v1.0.3` is clearer than `1.0.3`)
- [ ] Append an entry to `CHANGELOG.md` for that version listing what changed
- [ ] Create an immutable Git tag for the release
- [ ] If CI can bump versions / generate notes on merge, prefer automation over manual edits

## Safety rules

1. **Never** alter a released version. Once `v1.2.3` is published (e.g. to npm, pip, or GitHub), never overwrite it. If you need to fix a mistake, release `v1.2.4`.
2. **Never** skip SemVer meaning: do not bump only PATCH for a breaking change, or MAJOR for a pure bug fix.
3. **Never** invent a second parallel versioning scheme in the same product surface without documenting which is canonical.
4. **Always** start new unstable software at `0.1.0` and only move to `1.0.0` when production-ready.
5. **Always** maintain a CHANGELOG that lists exactly what changed in every version.
6. **Always** keep this skill focused on versioning only — do not expand into packaging, deploy, or marketing release notes beyond version identity.

## Key facts & reference

| Item | Value |
|------|-------|
| Industry default | SemVer `MAJOR.MINOR.PATCH` |
| Recommended tag form | `vMAJOR.MINOR.PATCH` |
| Initial development | `0.1.0` → `1.0.0` at stability |
| Pre-release separator | Hyphen (`-alpha`, `-beta.N`, `-rc.N`) |
| Changelog file | `CHANGELOG.md` |
| Build hybrid example | `v1.2.0 (Build 403)` |
| CalVer examples | `YYYY.MM.PATCH`, `YY.MM` |
