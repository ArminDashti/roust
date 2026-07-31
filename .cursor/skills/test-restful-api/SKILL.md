---
name: test-restful-api
description: Tests RESTful APIs end-to-end by first generating Docker deploy scripts with `docker-deploy`, deploying locally, then validating every endpoint across positive, negative, edge, auth, and error scenarios at user-chosen depth (basic, standard, or comprehensive). Use when the user asks to test API endpoints in Docker or requests REST API scenario coverage. Writes report to `./.argent/api-test/YYYY-MM-DD-HH-MM-<depth>.md`.
---

# RESTful API Testing

## Overview

- Scope: containerized REST API test workflow with strict order: deploy scripts -> local Docker deploy -> endpoint scenario testing.
- Uses `docker-deploy` only to create `.argent/docker-scripts/` scripts before tests run.
- User chooses test depth: **basic**, **standard**, or **comprehensive** (default **standard** when unspecified).
- Exclusions: production deploys, load testing at scale, security penetration testing, and changing product requirements.

## Objectives

1. Ensure deploy scripts exist and are generated through `.cursor/skills/docker-deploy`.
2. Deploy the API stack on local Docker successfully before test execution.
3. Test all REST endpoints at the chosen depth level and record pass/fail evidence.
4. Deliver an endpoint summary table to the user and write the full report to `./.argent/api-test/YYYY-MM-DD-HH-MM-<depth>.md`.

## Workflow

### Step 1: Prepare deploy scripts using docker-deploy skill

- [ ] Use `docker-deploy` to create/update `.argent/docker-scripts/run-on-docker-local.ps1` and `.argent/docker-scripts/run-on-docker-local.yaml`.
- [ ] Do not continue to tests until `.argent/docker-scripts/` deploy files are valid.

### Step 2: Deploy locally with Docker

- [ ] Run local deploy: `.\.argent\docker-scripts\run-on-docker-local.ps1`.
- [ ] Verify containers are healthy and API base URL responds.
- [ ] If deploy fails, fix build/compose/config issues first, then redeploy.
- [ ] Capture final base URL for test execution (example: `http://localhost:8080`).

### Step 3: Confirm test depth

- [ ] Ask the user for depth unless they already set it in the request.
- [ ] Use **standard** when the user does not specify a level.
- [ ] Record the chosen level in the report header.

| Level | Aliases | Use when |
|-------|---------|----------|
| **basic** | simple, smoke, quick | API is up; happy path only |
| **standard** | normal, medium | Routine regression before merge |
| **comprehensive** | full, deep, big | Release, major change, or high-risk API |

### Step 4: Build endpoint inventory

- [ ] Discover all endpoints from OpenAPI/Swagger, route files, or controller definitions.
- [ ] Build a test matrix sized to the chosen depth (method, path, auth, scenarios, expected status).
- [ ] Include health, version, and metadata endpoints if present.

### Step 5: Execute scenarios per depth level

**basic** — per endpoint:

- [ ] One positive request with valid body, params, and headers.
- [ ] Expected status code and one key response field.

**standard** — per endpoint (basic + below):

- [ ] Negative: missing one required field; one wrong data type or invalid format.
- [ ] Auth (protected endpoints only): no token; valid token with access.
- [ ] Query (list/search endpoints only): default page and one filter or sort.

**comprehensive** — per endpoint (standard + below):

- [ ] Negative: out-of-range values and constraint violations.
- [ ] Auth: expired or malformed token; valid token with insufficient role.
- [ ] Edge: empty results; boundary dates, numbers, string lengths; idempotent duplicate requests.
- [ ] Query: pagination boundaries; filter combinations; sort by allowed fields and direction.
- [ ] Error-path: not-found and conflict semantics; stable error shape (`code`, `message`, optional `details`).
- [ ] Create/update/delete: follow-up read checks for data integrity.

### Step 6: Validate responses

- [ ] **basic**: status code and key field only.
- [ ] **standard**: status, response schema, and primary business fields.
- [ ] **comprehensive**: full contract, headers, timestamps/timezones, nullable fields, and actionable errors without secret leakage.

### Step 7: Report results

- [ ] Create `./.argent/api-test/` if missing.
- [ ] Write full report to `./.argent/api-test/YYYY-MM-DD-HH-MM-<depth>.md` (local run timestamp).
- [ ] Show the user the endpoint summary table (same as report header).
- [ ] Include full details for every failed test in the report and in the user response.

**Summary table** (user response + report):

| Endpoint | Pass |
|----------|------|
| `example.com/api/example` | `5/10` |

- `Pass` = `passed/total` scenarios for that endpoint.
- One row per endpoint (include method when paths collide, e.g. `GET example.com/api/example`).

**Failed-test details** (required for each failure):

- Scenario name and type (positive, negative, auth, edge, query, error-path).
- Request: method, URL, headers (redact secrets), body.
- Expected vs actual status code and response body.
- Assertion or validation error message.
- Repro steps to rerun the single scenario.

## Safety rules

1. **Always** use `docker-deploy` first to create/update `.argent/docker-scripts/` scripts when they are missing or outdated.
2. **Never** test against production endpoints unless the user explicitly requests it.
3. **Never** log secrets, tokens, or credentials in reports or terminal output.
4. **Always** stop and fix local Docker deploy failures before continuing test scenarios.
5. **Never** run scenarios above the chosen depth level.
6. **Never** mark coverage complete below the required bar for that level:
   - **basic**: all endpoints have at least one passing positive scenario.
   - **standard**: positive and negative scenarios ran on every applicable endpoint.
   - **comprehensive**: every scenario group in Step 5 ran where applicable.

## Key facts & reference

| Item | Value |
|------|-------|
| Skill path | `.cursor/skills/restful-api-testing/SKILL.md` |
| Required upstream skill | `.cursor/skills/docker-deploy/SKILL.md` |
| Deploy scripts root | `.argent/docker-scripts/` |
| Local deploy command | `.\.argent\docker-scripts\run-on-docker-local.ps1` |
| Test depth levels | `basic`, `standard` (default), `comprehensive` |
| Level aliases | basic: simple, smoke, quick — standard: normal, medium — comprehensive: full, deep, big |
| Preferred endpoint sources | OpenAPI/Swagger, route definitions, controllers |
| Completion gate | Local deploy success + endpoint matrix executed at chosen depth |
| Report directory | `./.argent/api-test/` |
| Report filename | `YYYY-MM-DD-HH-MM-<depth>.md` (local run time + chosen depth) |
| User summary columns | `Endpoint`, `Pass` (`passed/total`) |

## Output template

```markdown
# API Test Report — YYYY-MM-DD HH:MM

**Depth:** standard

## Summary

| Endpoint | Pass |
|----------|------|
| example.com/api/example | 5/10 |

## Failed tests

### GET example.com/api/example — missing required field

- Scenario: negative — missing required field
- Request: `POST example.com/api/example` — body: `{ ... }`
- Expected: `400` — Actual: `200`
- Error: required field `name` accepted when absent
- Repro: send POST without `name` against local deploy base URL
```
