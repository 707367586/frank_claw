# Local patches to vendored picoclaw

Each entry: short subject + commit SHA + rationale + (optional) upstream-PR link.

When upstream syncs happen, every entry here must be re-applied or explicitly retired.

## 2026-04-20 · Add `Sec-WebSocket-Protocol: token.<…>` auth to `/pico/ws`

**Why:** Browsers' WebSocket API cannot set arbitrary HTTP headers on the upgrade request, so upstream's Bearer-only auth path at `/pico/ws` is unreachable from `apps/clawx-gui/`. We added a subprotocol fallback in `web/backend/middleware/launcher_dashboard_auth.go` (functions `validLauncherDashboardSubprotocolAuth` and `parseSubprotocols`) that constant-time-compares `token.<value>` against the dashboard token. The original Bearer header path and cookie-based auth are unchanged and still work for non-browser clients.

**Files:** `backend/web/backend/middleware/launcher_dashboard_auth.go`, `backend/web/backend/middleware/launcher_dashboard_auth_test.go`
**Local commit:** `<fill after committing>`
**Upstream PR:** TODO — file when stable.
