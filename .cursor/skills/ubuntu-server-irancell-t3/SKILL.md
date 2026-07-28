---
name: ubuntu-server-irancell-t3
description: Connects to and manages the Irancell-T3 Ubuntu server (2.144.27.74) via SSH MCP or fallback SSH. Use when deploying, inspecting, configuring, or troubleshooting the Irancell-T3 datacenter server, t3-new host, or cloud-admin remote operations in Iran.
---

# Irancell-T3 Server

## Server reference

Read [server.md](../server.md) for host inventory, credentials, and live system state.

## Connection workflow

### 1. Prefer SSH MCP

Use **SSH MCP** (`user-ssh-mcp`) for all remote operations. Reuse active sessions when possible.

If SSH MCP is unavailable or errors, fall back to shell SSH (step 2).

### 2. Fallback: shell SSH

Try authentication in this order:

**A. SSH key (try first)**

```bash
ssh -i ~/.ssh/id_ed25519_irancell -o IdentitiesOnly=yes cloud-admin@2.144.27.74
```

**B. Password (if key fails)**

Read credentials from [server.md](../server.md). On Windows when non-interactive auth is required, use a short-lived Python `paramiko` one-liner — delete any helper script immediately after use.

Suggested `~/.ssh/config` entry:

```
Host t3
    HostName 2.144.27.74
    User cloud-admin
    Port 22
    IdentityFile ~/.ssh/id_ed25519_irancell
    IdentitiesOnly yes
```

Then: `ssh t3`

### 3. Verify connectivity

Run safe read-only checks before changes:

```bash
hostname && uname -a
uptime && free -h && df -h /
```

## Core rules

### Use SSH MCP first

Always attempt SSH MCP before shell SSH. Reuse sessions across commands in the same task.

### Protect SSH connectivity

Never run commands that could break the active SSH session without explicit user approval.

**Forbidden without approval:**

```bash
sudo systemctl stop ssh
sudo systemctl restart networking
sudo iptables -F
sudo reboot
sudo shutdown now
```

Prefer `sudo systemctl reload ssh` over restart.

### Low-impact operations only

Server has **2 CPU cores, 2 GB RAM, 30 GB SSD**. Avoid heavy workloads, large downloads, or concurrent intensive tasks.

**Avoid without approval:**

- Full system upgrades (`apt upgrade` on all packages)
- Building large projects on-server
- Stress tests or benchmarks
- Mass file transfers
- Service restarts on production workloads

**Safe defaults:** read-only inspection, small config edits, targeted package installs.

### Ask before critical commands

Ask the user before:

- Reboot / shutdown
- Restarting services
- Firewall or networking changes
- Deleting important files
- Replacing configs
- Destructive or security-sensitive operations
- Anything that could affect uptime or performance

### Privileged commands

`cloud-admin` has passwordless sudo. Still prefix privileged commands with `sudo` and inspect state first.

### Cleanup

Delete all temporary scripts, configs, and archives created during a task:

```bash
rm -f /tmp/temp_script.sh
```

Never leave helper scripts on the server or in the repo after the task.

### Modern commands

| Deprecated | Preferred |
|---|---|
| `ifconfig` | `ip` |
| `netstat` | `ss` |
| `service` | `systemctl` |
| `apt-get` | `apt` |

## Pre-change inspection

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

## Config edits

Back up before editing:

```bash
sudo cp /etc/some/config /etc/some/config.bak
```

## Security

- Do not expose credentials in logs, commits, or chat output when avoidable.
- Do not disable firewalls unless explicitly required.
- Apply least privilege; validate permissions after changes.

## Task logging

After every server task, append or create a log in `./logs/<brief-task-title>.mdc` with:

- Task title and timestamp
- Commands executed
- Important outputs
- Final result and warnings

Create `./logs/` if missing.

## Downloads

Save files fetched from the server to `./downloaded-files/`.

## Update server inventory

After major system changes, update [server.md](../server.md) with new OS details, ports, services, disk usage, and installed software.
