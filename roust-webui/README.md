# roust-webui

Vue 3 + Vite + Tailwind + shadcn-vue management UI for [`roust-api`](https://github.com/ArminDashti/roust-api).

## Features

- Live **service status** (`GET /api/status`)
- **Route list** with add / edit / delete (`/api/routes`)

## Local development

1. Start the API (default bind `127.0.0.1:8787`):

```powershell
cd ..\roust-api
cargo run --bin roust-api
```

2. Copy env and point the Vite proxy at the API if the port differs:

```powershell
copy .env.example .env
# edit VITE_API_PROXY_TARGET if needed, e.g. http://127.0.0.1:8788
```

3. Install and run the UI:

```powershell
npm install
npm run dev
```

Open http://127.0.0.1:5173 — requests to `/api/*` are proxied to `VITE_API_PROXY_TARGET`.

## Scripts

| Command | Description |
|---------|-------------|
| `npm run dev` | Vite hot reload |
| `npm run build` | Typecheck + production build |
| `npm run preview` | Preview production build |
