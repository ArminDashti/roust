---
name: docker-deploy
description: >-
  Creates Docker deploy files under .deploy/docker from skill samples
  (run-on-docker-local/server .ps1 + .yaml). Use when adding or editing
  local/server Docker deploy scripts or YAML under .deploy/docker for any
  containerized app, service, or stack.
---

# Docker Deploy

## Overview

- Owns create / edit of Docker deploy assets under `.deploy/docker/` for any project
- Copy and adapt the four files from `samples/` — do not invent a different contract
- Exclusions: app business logic; inventing real SSH credentials; domain-specific test or runtime steps (those belong in other skills)

## Objectives

1. Create the full `.deploy/docker/` set when missing, or edit the existing set
2. Base each of the four files on the matching sample
3. Leave `ssh` / `volume_dir` as placeholders unless the user provided values

## Workflow

### Step 1: Create these files under `.deploy/docker/`

Copy from this skill's `samples/`, then adapt names, ports, and paths for the project:

| Path | Role | Sample |
|------|------|--------|
| `run-on-docker-local.ps1` | Local Docker daemon deploy | [samples/run-on-docker-local.ps1](samples/run-on-docker-local.ps1) |
| `run-on-docker-local.yaml` | Local settings | [samples/run-on-docker-local.yaml](samples/run-on-docker-local.yaml) |
| `run-on-docker-server.ps1` | Remote SSH deploy | [samples/run-on-docker-server.ps1](samples/run-on-docker-server.ps1) |
| `run-on-docker-server.yaml` | Remote settings | [samples/run-on-docker-server.yaml](samples/run-on-docker-server.yaml) |

**Read samples as templates** (adapt and write into the target repo). Do not execute sample scripts from the skill folder.

### Step 2: Adapt YAML for the project

| Key | Set to |
|-----|--------|
| `stack_name` | Compose project name (e.g. app or service slug) |
| `image_tag` | Image tag (e.g. `myapp:latest`) |
| `compose_file` | Path relative to `.deploy/docker/` (e.g. `../../docker-compose.yml`) |
| `dockerfile` | Path relative to `.deploy/docker/` (e.g. `../../dockerfile`) |
| `docker_network` | External network name |
| `internal_port` | Container listen port; non-empty overrides compose via `INTERNAL_PORT` |
| `publish_port` | Server YAML only — host bind port; omit or empty when behind a reverse proxy |

Ensure `compose_file` and `dockerfile` paths resolve from `.deploy/docker/`. Compose may use override env vars: `IMAGE_TAG`, `DOCKER_NETWORK`, `INTERNAL_PORT`, `PUBLISH_PORT` (server).

### Step 3: Run deploy

| Target | Command |
|--------|---------|
| Local | `.\.deploy\docker\run-on-docker-local.ps1` |
| Remote | `.\.deploy\docker\run-on-docker-server.ps1` (fill `ssh` and `volume_dir` first) |

Fix build or compose errors before any follow-on work in other skills.

## Safety rules

1. **Never** invent hosts, aliases, passwords, or key paths
2. **Never** print the password segment of `host@user@password` — log `user@host` or `ssh <alias>` only
3. **Never** add CLI `--` flags; change behavior only via YAML
4. **Never** execute scripts from `.cursor/skills/docker-deploy/samples/` — copy into `.deploy/docker/` first

## Key facts & reference

| Item | Value |
|------|-------|
| Deploy root | `.deploy/docker/` |
| Samples dir | `.cursor/skills/docker-deploy/samples/` |
| Local pair | [run-on-docker-local.ps1](samples/run-on-docker-local.ps1) + [run-on-docker-local.yaml](samples/run-on-docker-local.yaml) |
| Server pair | [run-on-docker-server.ps1](samples/run-on-docker-server.ps1) + [run-on-docker-server.yaml](samples/run-on-docker-server.yaml) |
| SSH placeholder | `ssh: "ssh <alias>"` |
| Build context | Repo root; Dockerfile path from YAML |
| Override env vars | `IMAGE_TAG`, `DOCKER_NETWORK`, `INTERNAL_PORT`, `PUBLISH_PORT` (server) |
| Server build modes | `build_image_on: local` (build + upload) or `server` (build on remote) |
