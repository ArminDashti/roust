---
name: haproxy-letsencrypt
description: Issues and renews Let's Encrypt certificates for Docker HAProxy SNI on Irancell-T3 (xaigrok.ir hostnames). Use when adding TLS certs, replacing self-signed PEMs, Certbot/ACME, or LE renewals for dogan/exar/lexmora (or similar) behind reverse-proxy haproxy.
---

# HAProxy + Let's Encrypt (t3)

## Prerequisites

Follow [irancell-t3-server](../irancell-t3-server/SKILL.md) for SSH (`ssh t3` / SSH MCP). Ask before firewall changes that are unclear; opening `80/tcp` for ACME is expected when missing.

## Stack facts (do not assume Nginx/systemd HAProxy)

| Item | Value |
|---|---|
| Container | `haproxy` (compose project `reverse-proxy`) |
| Listens | **:443 only** (TCP SNI + SSL offload on `127.0.0.1:8443`) |
| PEM dir (host) | `/cloud-admin/docker-volumes/reverse-proxy/haproxy/certs/` |
| PEM in container | `/usr/local/etc/haproxy/certs/` (`crt` directory / SNI) |
| PEM format | `fullchain.pem` + `privkey.pem` concatenated → `{domain}.pem` |
| Reload | `docker kill -s HUP haproxy` (not `systemctl`) |
| Self-signed fallback | `reverse-proxy.sh` `ensure_certs()` — skips existing PEMs; **deleting** a PEM recreates self-signed |
| SoftEther | SNI `ovpbackup.xaigrok.ir` TCP passthrough — must keep working |

Do **not** stop HAProxy for HTTP-01 standalone: `:80` is free for Certbot.

## Issue certs (HTTP-01 standalone)

### 1. Preflight

```bash
getent hosts <domain>   # must be 2.144.27.74
ss -tlnp | grep -E ':80 |:443 '
sudo ufw status         # allow 80/tcp if missing
docker ps --filter name=haproxy
ls -la /cloud-admin/docker-volumes/reverse-proxy/haproxy/certs/
```

Backup existing PEMs before overwrite:

```bash
CERTS=/cloud-admin/docker-volumes/reverse-proxy/haproxy/certs
BACKUP=/cloud-admin/docker-volumes/reverse-proxy/haproxy/certs-backup-$(date +%Y%m%d)
mkdir -p "$BACKUP"
cp -a "$CERTS/<domain>.pem" "$BACKUP/"
```

### 2. Certbot

```bash
sudo apt install -y certbot   # if missing
sudo ufw allow 80/tcp comment "ACME HTTP-01 Certbot"   # if not already allowed

sudo certbot certonly --standalone \
  -d <domain> \
  --agree-tos --non-interactive --register-unsafely-without-email
# Or: -m <ops-email> instead of --register-unsafely-without-email
```

One Certbot lineage per hostname (matches `{domain}.pem` naming).

### 3. Install PEM + reload

```bash
CERTS=/cloud-admin/docker-volumes/reverse-proxy/haproxy/certs
sudo cat /etc/letsencrypt/live/<domain>/fullchain.pem \
         /etc/letsencrypt/live/<domain>/privkey.pem \
  > "$CERTS/<domain>.pem"
chmod 644 "$CERTS/<domain>.pem"
docker kill -s HUP haproxy
```

No HAProxy config edit needed if the hostname already has an SNI/`Host` route in `reverse-proxy.sh`.

### 4. Renew hook

Ensure `/etc/letsencrypt/renewal-hooks/deploy/haproxy-pem.sh` exists (executable). It should:

1. For each renewed lineage arg, if basename is a managed `*.xaigrok.ir` app host, rebuild `$CERTS/<domain>.pem`
2. `docker kill -s HUP haproxy` if the container is running

Extend the `case` list when issuing certs for new hostnames.

Dry-run (avoid long random sleep):

```bash
sudo certbot renew --dry-run --no-random-sleep-on-renew
```

## Verify

```bash
echo | openssl s_client -connect 127.0.0.1:443 -servername <domain> 2>/dev/null \
  | openssl x509 -noout -issuer -subject -dates
# issuer should be Let's Encrypt

curl -fsS -o /dev/null -w '%{http_code}\n' \
  --resolve <domain>:443:127.0.0.1 https://<domain>/

# SoftEther still passthrough
echo | openssl s_client -connect 127.0.0.1:443 -servername ovpbackup.xaigrok.ir 2>/dev/null \
  | openssl x509 -noout -subject
```

## After the task

- Append `./logs/<brief-task-title>.mdc` (per irancell-t3-server skill).
- Leave `80/tcp` open for renewals unless the user asks to close it.

## Gotchas

- Chatbot “stop HAProxy + `/etc/haproxy/certs` + systemctl” advice does **not** apply here.
- Windows SSH heredocs often corrupt remote scripts — prefer base64/`tee` of a local string, or scp.
- `ensure_certs()` will not overwrite LE PEMs while files exist; never delete LE PEMs casually.
