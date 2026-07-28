---
name: docker-deploy
description: >-
  Creates Docker deploy files under .armin/docker-scripts from skill samples
  (run-on-docker-local/server .ps1 + .yaml). Use when adding or editing
  local/server Docker deploy scripts or YAML under .armin/docker-scripts for any
  containerized app, service, or stack.
---

# Docker Deploy

## Overview

- Owns create / edit of Docker deploy assets under `.armin/docker-scripts/` for any project
- Copy and adapt the four files from `samples/` — do not invent a different contract
- Exclusions: app business logic; inventing real SSH credentials; domain-specific test or runtime steps (those belong in other skills)

## Objectives

1. Create the full `.armin/docker-scripts/` set when missing, or relocate + update when the four files already exist elsewhere
2. Base each of the four files on the matching sample
3. Agent always must look at local docker and make sure which port is ok
4. Delete image always must be yes
5. Leave `ssh` / `volume_dir` as placeholders unless the user provided values

## Workflow

### Step 1: Locate or create the four files under `.armin/docker-scripts/`

Target set:

| Path | Role | Sample |
|------|------|--------|
| `run-on-docker-local.ps1` | Local Docker daemon deploy | [samples/run-on-docker-local.ps1](samples/run-on-docker-local.ps1) |
| `run-on-docker-local.yaml` | Local settings | [samples/run-on-docker-local.yaml](samples/run-on-docker-local.yaml) |
| `run-on-docker-server.ps1` | Remote SSH deploy | [samples/run-on-docker-server.ps1](samples/run-on-docker-server.ps1) |
| `run-on-docker-server.yaml` | Remote settings | [samples/run-on-docker-server.yaml](samples/run-on-docker-server.yaml) |

**If already the four files exists** (repo root, `docker-scripts/`, `.argent/docker-scripts/`, `scripts/`, or elsewhere): **Cut them to `./.armin/docker-scripts/`** and compatible them with latest standard of the skill (keys, defaults, relative paths from the new folder).

**If missing:** copy from this skill's `samples/`, then adapt names, ports, and paths for the project.

**Read samples as templates** (adapt and write into the target repo). Do not execute sample scripts from the skill folder.

### Step 2: Pick an OK host port from local Docker

Agent always must look at local docker and make sure which port is ok before writing YAML:

1. List published ports: `docker ps --format "{{.Names}}\t{{.Ports}}"`
2. Optionally cross-check listeners: `netstat -ano` / `Get-NetTCPConnection` (Windows) or `ss -tlnp` (Linux)
3. Choose a free `publish_port` (prefer the app’s usual port if free; otherwise next free in the same range)
4. Set `publish_port` in local YAML (and server YAML only when host bind is required)
5. Set `internal_port` to the container listen port (app default); leave empty only when compose already defines it and no override is needed

Never reuse a port already published by another running container.

### Step 3: Adapt YAML for the project

| Key | Set to |
|-----|--------|
| `stack_name` | Compose project name (e.g. app or service slug) |
| `image_tag` | Image tag (e.g. `myapp:latest`) |
| `compose_file` | Path relative to `.armin/docker-scripts/` (e.g. `../../docker-compose.yml`) |
| `dockerfile` | Path relative to `.armin/docker-scripts/` (e.g. `../../dockerfile`) |
| `docker_network` | External network name |
| `internal_port` | Container listen port; non-empty overrides compose via `INTERNAL_PORT` |
| `publish_port` | Host bind port (local required after Step 2; server — omit or empty when behind a reverse proxy) |
| `delete_image` | **Always** `"yes"` |
| `delete_volume` | `"no"` unless a clean volume wipe is required |

Ensure `compose_file` and `dockerfile` paths resolve from `.armin/docker-scripts/`. Compose may use override env vars: `IMAGE_TAG`, `DOCKER_NETWORK`, `INTERNAL_PORT`, `PUBLISH_PORT`.

### Step 4: Run deploy

| Target | Command |
|--------|---------|
| Local | `.\.armin\docker-scripts\run-on-docker-local.ps1` |
| Remote | `.\.armin\docker-scripts\run-on-docker-server.ps1` (fill `ssh` and `volume_dir` first) |

Fix build or compose errors before any follow-on work in other skills.

## Safety rules

1. **Always** look at local Docker published ports and pick an OK free `publish_port` before writing or editing YAML
2. **Always** set `delete_image: "yes"` in both local and server YAML
3. **Always** cut existing four-file sets into `./.armin/docker-scripts/` and align them to the current samples
4. **Never** invent hosts, aliases, passwords, or key paths
5. **Never** print the password segment of `host@user@password` — log `user@host` or `ssh <alias>` only
6. **Never** add CLI `--` flags; change behavior only via YAML
7. **Never** execute scripts from `.cursor/skills/docker-deploy/samples/` — copy into `.armin/docker-scripts/` first

## Key facts & reference

| Item | Value |
|------|-------|
| Deploy root | `.armin/docker-scripts/` |
| Samples dir | `.cursor/skills/docker-deploy/samples/` |
| Local pair | [run-on-docker-local.ps1](samples/run-on-docker-local.ps1) + [run-on-docker-local.yaml](samples/run-on-docker-local.yaml) |
| Server pair | [run-on-docker-server.ps1](samples/run-on-docker-server.ps1) + [run-on-docker-server.yaml](samples/run-on-docker-server.yaml) |
| Port check | `docker ps --format "{{.Names}}\t{{.Ports}}"` before setting `publish_port` |
| `delete_image` | Always `"yes"` |
| SSH placeholder | `ssh: "ssh <alias>"` |
| Build context | Repo root; Dockerfile path from YAML |
| Override env vars | `IMAGE_TAG`, `DOCKER_NETWORK`, `INTERNAL_PORT`, `PUBLISH_PORT` |
| Server build modes | `build_image_on: local` (build + upload) or `server` (build on remote) |
