---
name: ubuntu-server-irancell-t3
description: >-
  Connects to and manages the Irancell-T3 Ubuntu server (2.144.27.74) via shell
  SSH. Use when deploying, inspecting, configuring, or troubleshooting the
  Irancell-T3 datacenter server, t3 host, or cloud-admin remote operations in Iran.
---

# Irancell-T3 Server

## Overview

- Scope: remote ops on Irancell-T3 (`2.144.27.74`, host alias `t3`) as `cloud-admin`
- Connection: shell SSH only — never use SSH MCP
- Inventory: read [server.md](../server.md) for credentials and live system state

## Objectives

1. Connect with `ssh t3`, falling back to `ssh t3 -p 80` if that fails
2. Inspect safely before changes; keep load low on a small VPS
3. Ask before critical or destructive commands
4. Log the task and update inventory after major changes

## Workflow

### Step 1: Connect

1. Try:

```bash
ssh t3
```

2. If that fails (timeout, connection refused, or unreachable), use:

```bash
ssh t3 -p 80
```

Suggested `~/.ssh/config` entry:

```
Host t3
    HostName 2.144.27.74
    User cloud-admin
    Port 22
    IdentityFile ~/.ssh/id_ed25519_irancell
    IdentitiesOnly yes
```

If key auth fails, read credentials from [server.md](../server.md). On Windows when non-interactive auth is required, use a short-lived Python `paramiko` one-liner — delete any helper script immediately after use.

### Step 2: Verify connectivity

```bash
hostname && uname -a
uptime && free -h && df -h /
```

### Step 3: Pre-change inspection

**Firewall:**

```bash
sudo ufw status
sudo iptables -S
```

**Network:**

```bash
ip addr
ip route
```

**Services:**

```bash
systemctl is-active ssh
ss -tlnp
```

### Step 4: Make changes

- Back up before editing: `sudo cp /etc/some/config /etc/some/config.bak`
- Prefer `sudo systemctl reload ssh` over restart
- Use modern commands: `ip` (not `ifconfig`), `ss` (not `netstat`), `systemctl` (not `service`), `apt` (not `apt-get`)
- Prefix privileged commands with `sudo` (`cloud-admin` has passwordless sudo)

### Step 5: Cleanup and log

- Delete temp scripts/archives on the server and locally when done
- Save downloads to `./downloaded-files/`
- Append or create `./logs/<brief-task-title>.mdc` with title, timestamp, commands, outputs, result, warnings
- After major system changes, update [server.md](../server.md)

## Safety rules

1. **Never** use SSH MCP — shell SSH only (`ssh t3`, then `ssh t3 -p 80`).
2. **Never** run session-breaking commands without explicit user approval: `systemctl stop ssh`, restart networking, `iptables -F`, `reboot`, `shutdown`.
3. **Never** run heavy workloads without approval: full `apt upgrade`, large builds, stress tests, mass transfers, production service restarts.
4. **Always** ask before reboot/shutdown, service restarts, firewall/network changes, deleting important files, replacing configs, or anything that affects uptime.
5. **Always** avoid exposing credentials in logs, commits, or chat when avoidable.
6. **Never** disable firewalls unless explicitly required; apply least privilege and validate permissions after changes.
7. **Always** clean up temporary helpers after the task.

## Key facts & reference

| Item | Value |
|------|-------|
| Host | `2.144.27.74` |
| Alias | `t3` |
| User | `cloud-admin` |
| Primary connect | `ssh t3` |
| Fallback connect | `ssh t3 -p 80` |
| Identity file | `~/.ssh/id_ed25519_irancell` |
| Resources | 2 CPU, 2 GB RAM, 30 GB SSD |
| Inventory | [server.md](../server.md) |
| Task logs | `./logs/<brief-task-title>.mdc` |
| Downloads | `./downloaded-files/` |
