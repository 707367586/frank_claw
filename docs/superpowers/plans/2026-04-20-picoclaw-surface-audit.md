# picoclaw vendor SHA `8461c996` — endpoint surface audit

Probed 2026-04-20 from `backend/build/picoclaw-launcher -console -no-browser` (vendor SHA in `backend/UPSTREAM.md`).

Auth model: all `/api/*` endpoints require `Authorization: Bearer <dashboardToken>`. The dashboard token is printed to launcher stdout at startup as `dashboardToken: <token>`. WS endpoint accepts the Bearer token via `Authorization` header (not subprotocol).

## Endpoints

| Method | Path | Auth? | HTTP | Notes |
|---|---|---|---|---|
| GET | `/` | no | 302 | Redirects to `/launcher-login` (SPA shell). No unauthenticated health route at this path. |
| GET | `/health` | no | 302 | Also redirects to `/launcher-login`. No dedicated health endpoint unauthenticated. |
| GET | `/launcher-login` | no | 200 | Returns SPA HTML shell (same bundle). |
| GET | `/api/pico/info` | no | 401 | `{"error":"unauthorized"}` — auth is required. |
| GET | `/api/pico/info` | yes | 200 | `{"configured":true,"enabled":true,"ws_url":"ws://127.0.0.1:18800/pico/ws"}` |
| GET | `/api/sessions` | yes | 200 | Returns `[]` (empty array). Endpoint EXISTS — gap may be in population, not route. |
| GET | `/api/sessions?offset=0&limit=10` | yes | 200 | Returns `[]`. Same as above; pagination params accepted but no sessions yet. |
| GET | `/api/skills` | yes | 200 | `{"skills":[]}` — endpoint exists, no skills installed. |
| GET | `/api/tools` | yes | 200 | Returns array of 20 built-in tools with name/description/category/status. Full shape below. |
| GET | `/api/models` | yes | 200 | Returns `{"default_model":"","models":[...29 model entries...]}`. All unconfigured. |
| GET | `/api/config` | yes | 200 | Returns full config JSON (agents defaults, channels, etc.). |
| GET | `/api/gateway/status` | yes | 200 | `{"gateway_restart_required":false,"gateway_start_allowed":false,"gateway_start_reason":"no default model configured","gateway_status":"stopped"}` |
| GET | `/api/launcher/health` | yes | 404 | Not implemented upstream. |
| GET | `/api/launcher/info` | yes | 404 | Not implemented upstream. |
| GET | `/api/pico/token` | yes | 404 | GET method not routed — POST-only (not probed to avoid token invalidation). |
| WS  | `/pico/ws?session_id=…` | subprotocol token | 401 | `token.<dashboardToken>` subprotocol rejected with 401. |
| WS  | `/pico/ws?session_id=…` | Bearer header | 503 | Auth accepted but gateway not running → `Gateway not available`. Expected 101 once gateway starts. |
| WS  | `/pico/ws` | Bearer header | 503 | Same — auth passes, gateway absent. |

### `/api/tools` response shape (representative)

```json
{
  "tools": [
    {"name":"read_file","description":"Read file content...","category":"filesystem","config_key":"read_file","status":"enabled"},
    {"name":"write_file","description":"Create or overwrite files...","category":"filesystem","config_key":"write_file","status":"enabled"},
    {"name":"exec","description":"Run shell commands...","category":"filesystem","config_key":"exec","status":"enabled"},
    {"name":"cron","description":"Schedule one-time or recurring...","category":"automation","config_key":"cron","status":"enabled"},
    {"name":"web_search","description":"Search the web...","category":"web","config_key":"web","status":"enabled"},
    {"name":"spawn","description":"Launch a background subagent...","category":"agents","config_key":"spawn","status":"enabled"}
  ]
}
```

Fields: `name` (string), `description` (string), `category` (string), `config_key` (string), `status` ("enabled"|"disabled").
Total: 20 tools (14 enabled, 6 disabled — hardware and discovery tools disabled by default).

### `/api/models` response shape (abbreviated)

```json
{
  "default_model": "",
  "models": [
    {"index":0,"model_name":"glm-4.7","model":"zhipu/glm-4.7","api_base":"...","api_key":"","enabled":false,"available":false,"status":"unconfigured","is_default":false,"is_virtual":false},
    ...
  ],
  "total": 29
}
```

## Gaps to fill in Phase 2

The following endpoints are **missing** (404) but will be needed by the frontend:

- **`GET /api/launcher/health`** — a lightweight health check the frontend dashboard can poll to verify the launcher is alive. Suggested response: `{"status":"ok","pid":<int>,"uptime_seconds":<int>}`.
- **`GET /api/launcher/info`** — static metadata the UI shows in the "about" pane. Suggested response: `{"version":"<string>","build_sha":"<string>","config_path":"<string>"}`.

The following endpoints **exist and work**, so no gap-fill needed:
- `/api/sessions` (exists, returns empty array — population depends on gateway running)
- `/api/skills` (exists)
- `/api/tools` (exists, richly populated)
- `/api/models` (exists, discovered during probe — bonus find)
- `/api/config` (exists — returns full config blob)
- `/api/gateway/status` (exists)
- `/api/pico/info` (exists)

## Notes

- **Gateway (port 18790) was NOT running** during probe — the launcher requires `agents.defaults.model_name` to be set before it will spawn the gateway subprocess. `gateway_start_reason: "no default model configured"`.
- **WS auth works via `Authorization: Bearer` header**, not the `Sec-WebSocket-Protocol: token.<TOKEN>` subprotocol. Subprotocol form returns 401. Bearer header returns 503 (gateway down), which means auth passed — once a model is configured and gateway starts, this should upgrade to 101.
- **`GET /` returns 302**, not 200. The prior `-L` (follow redirect) in the first probe caused a misleading 200 (was from `/launcher-login`). True behavior: `/ → 302 → /launcher-login → 200`.
- **`GET /health` also 302** — there is no unauthenticated health route. Frontend must either use `/api/gateway/status` (auth required) or accept that health is implicit from the token-issuing startup sequence.
- **No 401/403 surprises** — all `/api/*` routes without a Bearer token return 401 cleanly with `{"error":"unauthorized"}` JSON body.
- **`POST /api/pico/token`** was intentionally skipped (would invalidate the active dashboard token).
- **Bonus discovered routes**: `/api/models` and `/api/config` — not in the original probe matrix but found during enumeration. These are potentially useful for the settings UI.
