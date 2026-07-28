---
name: openrouter-experience
description: >-
  Integrates apps with OpenRouter (chat completions, model search, credits/key
  usage, HTTP proxy egress, settings UX). Use when adding or changing OpenRouter
  clients, API keys, model pickers, credits/balance UI, or LLM transform calls
  via openrouter.ai.
---

# OpenRouter Experience

Proven patterns from Lexmora (`translator-api` / `translator-webui`). Prefer these over inventing a new OpenRouter client shape.

## Quick facts

| Item | Value |
|------|-------|
| Base URL | `https://openrouter.ai/api/v1` |
| Auth | `Authorization: Bearer <api_key>` |
| Model IDs | `provider/model` (e.g. `anthropic/claude-3.5-sonnet`) |
| Chat | `POST /chat/completions` (OpenAI-compatible messages) |
| Models | `GET /models?q=&limit=` |
| Account credits | `GET /credits` (needs **management** key) |
| Key usage | `GET /key` (normal API keys) |
| LLM timeout | ~120s (completions can be slow) |

## Architecture (preferred)

1. **Store key + model in app settings** (DB), not only env — UI can update without redeploy.
2. **Backend owns the OpenRouter HTTP client** — never call OpenRouter from the browser (CORS + key leak).
3. **Proxy settings endpoints** for models/credits so the SPA stays same-origin.
4. Map upstream failures to **502** with a stable code like `OPENROUTER_ERROR`.

```
SPA  →  your API (/settings, /transform)
            →  OpenRouter (/chat/completions, /models, /credits|/key)
```

## Client checklist

When implementing or editing an OpenRouter client:

- [ ] Base URL defaults to `https://openrouter.ai/api/v1` (trim trailing `/`; tolerate legacy `.../chat/completions` passed as base)
- [ ] Require non-empty API key and model before `Complete`
- [ ] Send `system` + `user` messages for prompt+input transforms
- [ ] On HTTP ≥400, include status + response body in the error
- [ ] Handle JSON `error.message` in an otherwise-200-shaped payload
- [ ] Reject empty `choices`
- [ ] Optionally strip wrapping markdown fences from model text before return
- [ ] Support optional HTTP proxy via env (see below)

## Credits vs key (important)

| Endpoint | Key type | What you get |
|----------|----------|--------------|
| `GET /credits` | Management key | `total_credits`, `total_usage` → remaining = credits − usage |
| `GET /key` | Normal API key | `usage`, optional `limit_remaining` |

**Do this:** try `/credits` first; on failure, fall back to `/key`.

**UI copy:** if source is `key` and `limit_remaining` is null, tell the user account balance needs an OpenRouter **management** key — do not pretend remaining credits exist.

Normalized response shape (API → SPA):

```json
{
  "source": "credits|key",
  "remaining": 1.23,
  "total_credits": 10,
  "total_usage": 8.77,
  "limit_remaining": null,
  "usage": null
}
```

## Models search UX

- Proxy `GET /models` with `q` + `limit` (cap ~50–200).
- Return `{ id, name, context_length }[]`; if `name` empty, use `id`.
- Settings UI: debounced search (~300ms), select by **id**, show selected id under the field.
- Persist the selected **model id** string in settings.

## Restricted-network egress

Some servers cannot reach OpenRouter directly. Pattern used on t3:

- Env: `OPENROUTER_HTTP_PROXY` (e.g. `http://mullvad-1:8778`)
- Optional Docker overlay joins VPN netns and sets that env
- Configure the HTTP client's `Transport.Proxy` from that URL at client construction time

Local/dev: leave proxy empty unless needed.

## Settings API surface (reference)

| Method | Path | Role |
|--------|------|------|
| GET/PATCH | `/api/v1/settings` | Read/update `openrouter_api_key`, `model_name` |
| GET | `/api/v1/settings/models?q=` | Model search proxy |
| GET | `/api/v1/settings/credits` | Credits/key usage proxy |

Never log or commit raw API keys. Mask in responses if the product is multi-user.

## Transform flow

1. Load settings (key + model)
2. Resolve system prompt from instruction key / DB
3. `Complete(ctx, apiKey, model, systemPrompt, userText)`
4. Persist history (input, output, model, instruction key)
5. On OpenRouter errors → 502 + `OPENROUTER_ERROR`

## Anti-patterns

- Calling OpenRouter from the SPA with the user key in localStorage
- Assuming `/credits` works for every key type
- Hardcoding a tiny model allow-list instead of searching `/models`
- Short HTTP timeouts (<60s) for chat completions
- Storing the key only in `.env` when the product has a Settings page

## Docs to re-check when APIs drift

- https://openrouter.ai/docs — chat completions, models, credits, API keys
