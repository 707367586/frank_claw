# PicoClaw Migration Implementation Plan v2

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete the entire Rust workspace + Tauri shell, **vendor [picoclaw](https://github.com/sipeed/picoclaw) source code into `backend/`** at a pinned SHA so we can freely modify it, then rewire the React frontend to talk to the Go-built `picoclaw-launcher` over its Pico WebSocket + REST API. No docker.

**Architecture:** Per [ADR-037 v2](../../arch/decisions.md#adr-037-2026-04-20-删除-rust-后端将-picoclaw-源码-vendor-进本仓库作为新后端) and [architecture.md v5.0](../../arch/architecture.md): single repo, two subprojects — `apps/clawx-gui/` (frontend) + `backend/` (vendored Go picoclaw, owned/forked).

**Tech Stack (new):** React 19 + TypeScript 5 + Vite 6 (unchanged) + **Go ≥ 1.25** (newly required for backend).

**Out of scope:** agents/memories/knowledge/vault/tasks/channels/skills-as-domain UIs, tool-approval UI, model-router UI, Tauri shell, Cargo workspace, docker.

**v1 → v2 changes (why this rewrite):** v1 assumed we could `docker pull` a stable picoclaw image. Empirical test of the latest tagged release `:v0.2.6` proved that the `/api/*` endpoints, the launcher subcommand, and the Pico WS path all live in main-branch source but are absent from the released binary. v2 vendors source at a specific SHA and runs `picoclaw-launcher` directly with `go run`. Also: user authorized us to fork/modify picoclaw freely.

**Spec source:** This plan + `docs/arch/architecture.md` v5.0 + `docs/arch/api-design.md` v5.0.

**Working directory for all tasks:** `/Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/.worktrees/picoclaw-migration` (worktree on branch `feat/picoclaw-migration`).

---

## Phase Map

| Phase | Theme | Reversible? | Blast radius |
|---|---|---|---|
| 1 | Vendor picoclaw source into `backend/`; verify build + run | Yes (delete dir) | new dir only |
| 2 | (Conditional) Fill protocol gaps in Go | Yes | backend/ only |
| 3 | Delete entire Rust workspace | No (use git revert) | repo-wide |
| 4 | Delete Tauri shell from `apps/clawx-gui/` | Yes (git) | gui only |
| 5 | TS protocol layer (TDD): pico-types, pico-rest, pico-socket, chat-store, store.tsx | Yes | gui only |
| 6 | Rewire ChatPage to new protocol | Yes | gui only |
| 7 | SettingsPage + ConnectorsPage as REST shells | Yes | gui only |
| 8 | Delete obsolete pages/components/routes | Yes (git) | gui only |
| 9 | Root dev workflow (no docker): concurrently scripts, Vite proxy, Makefile, README, AGENTS.md | Yes | repo metadata |
| 10 | Final verification + tag `v0.4.0` | n/a | none |

> Phase 3 is the destructive one; commit Phase 1+2 first.

---

## Phase 1 — Vendor picoclaw into `backend/`

### Task 1.1: Clone + copy picoclaw source at pinned SHA

**Pinned SHA:** `8461c996e5ad2f20801622a8eeec931f8966a066` (sipeed/picoclaw `main` HEAD as of 2026-04-20T03:18:42Z, PR #2592).

**Files (create):**
- `backend/` (entire directory tree from upstream, see exclusions)
- `backend/UPSTREAM.md`
- `backend/PATCHES.md`

- [ ] **Step 1: Clone upstream into a scratch dir**

```bash
TMP=$(mktemp -d)
git clone --depth 50 https://github.com/sipeed/picoclaw.git "$TMP/picoclaw"
cd "$TMP/picoclaw"
git checkout 8461c996e5ad2f20801622a8eeec931f8966a066
```

If checkout fails because depth-50 didn't reach the SHA, redo with `--depth 200` or `git fetch origin 8461c996...`.

- [ ] **Step 2: Copy upstream into `backend/`**

```bash
cd /Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/.worktrees/picoclaw-migration
mkdir backend
# Copy everything except git metadata, CI configs that conflict, and obvious pruning candidates
rsync -a --exclude='.git' \
        --exclude='.github' \
        --exclude='.gitignore' \
        --exclude='docker' \
        --exclude='.goreleaser*' \
        --exclude='dist' \
        --exclude='build' \
        "$TMP/picoclaw/" backend/
```

> **Why exclude `docker/` and `.goreleaser*`**: ADR-037 v2 explicitly removed docker. We don't ship containers from this repo; the upstream Dockerfiles are dead weight here.
>
> **Why keep `web/frontend/`**: the Go code uses `embed.FS` to bundle it into `picoclaw-launcher`. Removing it now would break compilation. We keep it as compile-time dead weight; Phase 9 task 9.6 will revisit pruning it.

- [ ] **Step 3: Write `backend/UPSTREAM.md`**

```markdown
# picoclaw upstream provenance

This `backend/` tree is a flat copy of [sipeed/picoclaw](https://github.com/sipeed/picoclaw) source at:

| Field | Value |
|---|---|
| Source | https://github.com/sipeed/picoclaw |
| Branch | main |
| Commit SHA | `8461c996e5ad2f20801622a8eeec931f8966a066` |
| Commit date | 2026-04-20T03:18:42Z |
| Commit subject | chore(web): update linting and router dependencies (#2592) |
| License | MIT (see `backend/LICENSE`) |
| Imported on | 2026-04-20 |
| Importer | ClawX migration v0.4.0 (ADR-037 v2) |

## Excluded from import

- `.git`, `.github` — separate version control / CI flows
- `.gitignore` — replaced by repo root `.gitignore`
- `docker/` — ADR-037 v2 doesn't deploy via docker
- `.goreleaser*` — release pipeline is owned by this repo, not upstream

## Manual upstream sync (when needed)

1. Fetch new SHA: `git -C /tmp/picoclaw fetch && git -C /tmp/picoclaw checkout <new-sha>`
2. Three-way diff: `diff -ru /tmp/picoclaw/ backend/ > /tmp/upstream.diff`
3. Apply selectively, **always preserving the local changes recorded in `PATCHES.md`**.
4. Run full test suite (`go test ./backend/...` + `pnpm --filter clawx-gui vitest run`).
5. If green, update `Commit SHA` and `Commit date` above.

We do **not** use `git subtree` / `submodule`. Maintaining a clean linear history matters more than automatic upstream pulls.
```

- [ ] **Step 4: Write `backend/PATCHES.md`**

```markdown
# Local patches to vendored picoclaw

Each entry: short subject + commit SHA + rationale + (optional) upstream-PR link.

When upstream syncs happen, every entry here must be re-applied or explicitly retired.

## (none yet)
```

- [ ] **Step 5: Update repo `.gitignore`**

Append to `/Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/.gitignore`:

```
# Go build artifacts
/backend/build/
/backend/dist/
*.test
*.out
```

- [ ] **Step 6: Sanity-check the layout**

```bash
ls backend/cmd/        # Expected: picoclaw, picoclaw-launcher (and maybe picoclaw-launcher-tui)
ls backend/pkg/        # Expected: many subdirs incl. channels/, gateway/, config/
test -f backend/go.mod && echo "go.mod OK"
test -f backend/LICENSE && echo "LICENSE OK"
```

If any directory is missing (especially `cmd/picoclaw-launcher`), STOP — the SHA may be wrong or the rsync excludes too aggressive. Re-do.

- [ ] **Step 7: Commit (one big commit; intentional)**

```bash
cd /Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/.worktrees/picoclaw-migration
git add backend .gitignore
git commit -m "feat(backend): vendor picoclaw source @ 8461c996"
```

The commit will be very large (probably 50–200 MB of tracked files). Don't squeeze it into multiple commits — one atomic vendor commit makes future syncs sane.

---

### Task 1.2: Verify `backend/` builds with `make build-launcher`

**Files:** none modified (build artifacts only)

> **Discovery from Task 1.1:** the launcher binary is **not** built from `cmd/picoclaw-launcher` (that dir doesn't exist). The actual Go entry point is `backend/web/backend/main.go` (comment in file: "PicoClaw Web Console - Web-based chat and management interface"). It depends on a pre-built frontend bundle in `backend/web/frontend/dist/` which `make build-launcher-frontend` produces via pnpm. The upstream Makefile target `build-launcher` orchestrates both — frontend first, then go build.

- [ ] **Step 1: `go mod download`**

```bash
cd backend
go mod download
```

Expected: prints nothing on success. If it fails, STOP — likely Go version mismatch.

- [ ] **Step 2: Build the embedded frontend (pnpm)**

```bash
make build-launcher-frontend
```

Expected: pnpm installs into `backend/web/frontend/node_modules/`, then builds `backend/web/frontend/dist/`. First run takes minutes. Subsequent runs are cached via the `frontend.installed` stamp file.

If pnpm isn't installed at the right version, STOP and report.

- [ ] **Step 3: Build the launcher binary**

```bash
make build-launcher
```

Expected: produces `backend/build/picoclaw-launcher` (a symlink to the platform-specific binary, e.g. `picoclaw-launcher-darwin-arm64`). Exit 0.

If build fails, capture the FIRST compile error verbatim and STOP.

- [ ] **Step 4: Verify the binary runs `--help`**

```bash
./build/picoclaw-launcher --help 2>&1 | head -40
```

Expected: prints flags. Per upstream `Dockerfile.goreleaser.launcher`, supported flags include `-console`, `-public`, `-no-browser`, plus a `-webroot` (or similar) for serving custom static assets. Record the actual flag list in your DONE report.

- [ ] **Step 5: Commit (only if you authored a helper script)**

If you wrote `scripts/build.sh` or similar to wrap these commands, commit it. Otherwise no commit (build artifacts are gitignored under `/backend/build/`).

---

### Task 1.3: First-run launcher locally + minimal config

**Files (config — outside repo):** `~/.picoclaw/config.json` and `~/.picoclaw/.security.yml` will be auto-generated.

> **Note:** This task touches the user's home directory. The state at `~/.picoclaw/` is per-developer and not committed.

- [ ] **Step 1: First-run onboarding via the `picoclaw` (gateway) binary**

```bash
cd backend
go run ./cmd/picoclaw onboard 2>&1 | tail -10
```

(The launcher binary delegates to `picoclaw onboard` internally on first launch — see `cmd/picoclaw-launcher-tui/main.go` for the same pattern. Running it directly is faster.)

Expected: prints "First-run setup complete" and exits 0. Files created: `~/.picoclaw/config.json`, `~/.picoclaw/.security.yml`, `~/.picoclaw/workspace/`.

- [ ] **Step 2: Patch config to enable Pico channel + bind to 0.0.0.0**

Edit `~/.picoclaw/config.json`:
- Set `gateway.host = "0.0.0.0"` (only if you'll reach it from a non-loopback ip; for pure localhost dev the default `127.0.0.1` is fine since Vite dev server is also on localhost)
- Set `channels.pico.enabled = true`
- Leave `agents.defaults.provider` and `model_name` empty (we'll start with `--allow-empty`)

Snippet to do it scriptably:

```bash
python3 - <<'PY'
import json
p = "/Users/zhoulingfeng/.picoclaw/config.json"
c = json.load(open(p))
c["channels"]["pico"]["enabled"] = True
json.dump(c, open(p, "w"), indent=2)
print("ok")
PY
```

- [ ] **Step 3: Start the launcher in foreground (one terminal tab)**

```bash
./build/picoclaw-launcher --allow-empty 2>&1 | tee /tmp/picoclaw.log
```

Expected: log shows `Gateway started on 127.0.0.1:18790`. Leave it running; Step 4 probes from another shell.

- [ ] **Step 4: From a second shell, verify `/health` works**

```bash
curl -sS http://127.0.0.1:18790/health
```

Expected: `{"status":"ok","uptime":"...","pid":...}`.

If it returns "Empty reply from server", the gateway bound to localhost INSIDE the binary's loopback (rare for native binary, but possible). Re-check `gateway.host` in config.

- [ ] **Step 5: Stop the launcher (Ctrl+C in the launcher terminal)**

We've proven build + run + health works. Onward.

- [ ] **Step 6: Commit nothing**

This task touches per-developer state only. If you wrote a helper script (e.g. `scripts/setup-dev-config.sh`) to automate Steps 1–2, commit *that* — but do NOT commit anything from `~/.picoclaw/`.

---

### Task 1.4: Probe the actual `/api/*` + `/pico/ws` surface

**Goal:** Discover empirically which endpoints `picoclaw-launcher` (built from our vendor SHA) actually serves. Output is a Phase-2 audit.

**Files (create):**
- `docs/superpowers/plans/2026-04-20-picoclaw-surface-audit.md`

- [ ] **Step 1: Start launcher again**

```bash
cd backend
./build/picoclaw-launcher --allow-empty 2>&1 > /tmp/picoclaw.log &
LAUNCHER_PID=$!
sleep 3
```

- [ ] **Step 2: Probe every endpoint the frontend will rely on**

```bash
for path in /health /ready /api/pico/token /api/sessions /api/skills /api/tools /pico/ws; do
  code=$(curl -sS -o /dev/null -w "%{http_code}" "http://127.0.0.1:18790$path")
  echo "$code  GET $path"
done
# WS upgrade probe
code=$(curl -sS -o /dev/null -w "%{http_code}" \
  -H "Upgrade: websocket" -H "Connection: Upgrade" \
  -H "Sec-WebSocket-Version: 13" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  "http://127.0.0.1:18790/pico/ws?session_id=probe")
echo "$code  WS  /pico/ws"
```

- [ ] **Step 3: Probe with token if any 401s appear**

```bash
TOKEN=$(curl -sS http://127.0.0.1:18790/api/pico/token | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])' 2>/dev/null)
echo "TOKEN=$TOKEN"
# Re-probe authed endpoints with Authorization header
for path in /api/sessions /api/skills /api/tools; do
  code=$(curl -sS -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:18790$path")
  echo "$code  GET $path  (with token)"
done
```

- [ ] **Step 4: Stop launcher, write the audit**

```bash
kill $LAUNCHER_PID
```

Create `docs/superpowers/plans/2026-04-20-picoclaw-surface-audit.md` with:

```markdown
# picoclaw vendor SHA `8461c996…` — endpoint surface audit

Probed on 2026-04-20 from `backend/build/picoclaw-launcher --allow-empty` (vendor SHA from `backend/UPSTREAM.md`).

| Endpoint | Expected by frontend | Actual response | Action |
|---|---|---|---|
| `GET /health` | 200 JSON | <fill in> | (none if 200) |
| `GET /api/pico/token` | 200 JSON | <fill in> | <gap-fill / OK> |
| `GET /api/sessions` | 200 JSON | <fill in> | <gap-fill / OK> |
| `GET /api/sessions/:id` | 200 JSON | (probe with a real id from list) | … |
| `DELETE /api/sessions/:id` | 204 | … | … |
| `GET /api/skills` | 200 JSON | … | … |
| `POST /api/skills/install` | 200 / 202 | … | … |
| `DELETE /api/skills/:name` | 204 | … | … |
| `GET /api/tools` | 200 JSON | … | … |
| `PUT /api/tools/:name/state` | 204 | … | … |
| `WS /pico/ws` | 101 Switching Protocols | … | … |

## Gaps to fill in Phase 2

- `<endpoint>` — current behavior `<…>`, needed behavior `<…>`. Add Go handler in `backend/pkg/<…>/<…>.go`.

## Notes

(…anything surprising you noticed in launcher logs, e.g. routes only register when `channels.pico.enabled=true`…)
```

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/2026-04-20-picoclaw-surface-audit.md
git commit -m "docs: empirical audit of picoclaw vendor surface"
```

---

## Phase 2 — Fill protocol gaps in Go (CONDITIONAL on Phase 1.4 audit)

This phase is **conditional**. If Phase 1.4 audit shows zero gaps, skip Phase 2 entirely. If gaps exist, create one task per gap, each:

- Following Go's table-driven test pattern
- Adding the handler in the right `backend/pkg/api/...` file (or wherever the launcher's HTTP routes live — discover from `backend/cmd/picoclaw-launcher/main.go`)
- Updating `backend/PATCHES.md` with a one-line entry
- Committing with message `feat(backend): add <endpoint> to launcher (gap from upstream)`

**Template task** (one per gap):

### Task 2.x: Add `<METHOD> <PATH>` to launcher

**Files:**
- Modify: `backend/pkg/api/<file>.go` (or new file)
- Test: `backend/pkg/api/<file>_test.go`
- Modify: `backend/PATCHES.md`

- [ ] **Step 1: Locate the existing route registration**

```bash
grep -RIn "HandleFunc\|router\.\|mux\." backend/cmd/picoclaw-launcher backend/pkg/api 2>/dev/null | head -20
```

Identify the file where existing `/api/*` routes are wired.

- [ ] **Step 2: Write failing test**

Use Go's `httptest.NewServer` + `net/http/httptest`. Table-driven:

```go
func TestNewEndpoint(t *testing.T) {
    tests := []struct {
        name     string
        body     string
        wantCode int
        wantBody string
    }{
        {"happy path", `{...}`, 200, `{...}`},
        {"bad input", `{}`, 400, `{...}`},
    }
    for _, tt := range tests {
        t.Run(tt.name, func(t *testing.T) { ... })
    }
}
```

- [ ] **Step 3: Run, watch fail.**

```bash
cd backend && go test ./pkg/api/... -run TestNewEndpoint -v
```

- [ ] **Step 4: Implement minimal handler.**
- [ ] **Step 5: Run, watch pass.**
- [ ] **Step 6: Update `backend/PATCHES.md`**
- [ ] **Step 7: Commit**

```bash
git add backend/pkg/api backend/PATCHES.md
git commit -m "feat(backend): add <METHOD> <PATH> to launcher (gap from upstream)"
```

> **Cap on this phase:** if Phase 1.4 reveals more than 5 gaps, STOP and escalate — at that point the upstream surface diverges enough from the plan that we should re-evaluate (e.g. maybe a different upstream commit has more of these routes already).

---

## Phase 3 — Delete Rust workspace (atomic commit)

> **STOP AND CONFIRM** with user before this phase. Once committed, restoration is `git revert <hash>`.

### Task 3.1: Delete Rust workspace

**Files (delete):**
- `crates/` (entire, all 17 sub-crates)
- `apps/clawx-service/`
- `apps/clawx-cli/`
- `Cargo.toml`
- `Cargo.lock`
- `clippy.toml`
- `rust-toolchain.toml`
- `rustfmt.toml`

- [ ] **Step 1: Verify `target/` is gitignored**

`git check-ignore target` — expected: `target`. If not, fix `.gitignore` first.

- [ ] **Step 2: git rm**

```bash
git rm -r crates apps/clawx-service apps/clawx-cli
git rm Cargo.toml Cargo.lock clippy.toml rust-toolchain.toml rustfmt.toml
```

- [ ] **Step 3: Verify nothing in repo (outside docs/arch + docs/superpowers + backend/) references the deleted Rust modules**

```bash
grep -RIl "clawx-service\|clawx-runtime\|clawx-controlplane-client\|clawx-cli" \
    --exclude-dir=docs \
    --exclude-dir=backend \
    --exclude-dir=node_modules .
```

Expected: no matches.

- [ ] **Step 4: Commit**

```bash
git commit -m "refactor!: delete Rust workspace (superseded by vendored picoclaw, ADR-037 v2)"
```

---

## Phase 4 — Delete Tauri shell from clawx-gui

### Task 4.1: Remove src-tauri + Tauri devDeps

**Files:**
- Delete: `apps/clawx-gui/src-tauri/`
- Modify: `apps/clawx-gui/package.json` — drop `@tauri-apps/cli` + `tauri` script; bump `version` to `0.4.0`
- Delete: `apps/clawx-gui/package-lock.json` (controller pre-resolved: pnpm is canonical)
- Modify: `apps/clawx-gui/pnpm-lock.yaml` (regenerated by `pnpm install`)

- [ ] **Step 1: git rm src-tauri**

```bash
git rm -r apps/clawx-gui/src-tauri
```

- [ ] **Step 2: Update package.json**

In `apps/clawx-gui/package.json`:
- Remove `"tauri": "tauri",` from `scripts`
- Remove `"@tauri-apps/cli": "^2",` from `devDependencies`
- Change `"version": "0.2.0"` → `"version": "0.4.0"`

- [ ] **Step 3: Drop the npm lockfile (controller pre-resolved: pnpm is canonical)**

```bash
git rm apps/clawx-gui/package-lock.json
```

- [ ] **Step 4: Refresh pnpm lockfile**

```bash
cd apps/clawx-gui && pnpm install
```

Verify: `ls node_modules/@tauri-apps 2>/dev/null` returns nothing.

- [ ] **Step 5: Verify config-level toolchain still works (TS errors in source files are expected and fine here)**

```bash
pnpm vitest run 2>&1 | tail -10
```

Expected: tests still pass. If TypeScript errors appear in `src/lib/api.ts` etc., that's fine — Phase 6 fixes them. If Vite/Vitest themselves can't load (config-level), STOP.

- [ ] **Step 6: Commit**

```bash
cd /Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/.worktrees/picoclaw-migration
git add apps/clawx-gui/package.json apps/clawx-gui/pnpm-lock.yaml
git commit -m "refactor!: remove Tauri shell from clawx-gui; bump to v0.4.0; drop npm lockfile"
```

---

## Phase 5 — TS protocol layer (TDD)

> Each task in this phase is **test-first**. Write failing test, watch fail, implement, watch pass, commit. Don't skip the "watch fail" step.

### Task 5.1: pico-types.ts (TDD)

**Files:**
- Create: `apps/clawx-gui/src/lib/pico-types.ts`
- Test: `apps/clawx-gui/src/lib/__tests__/pico-types.test.ts`

- [ ] **Step 1: Write failing test**

```ts
import { describe, it, expect, expectTypeOf } from "vitest";
import {
  isServerMessage,
  type PicoMessage,
  type ServerMessageType,
} from "../pico-types";

describe("pico-types", () => {
  it("recognises every server-to-client message type", () => {
    const types: ServerMessageType[] = [
      "message.create",
      "message.update",
      "media.create",
      "typing.start",
      "typing.stop",
      "error",
      "pong",
    ];
    for (const t of types) {
      const msg: PicoMessage = { type: t, payload: {} };
      expect(isServerMessage(msg)).toBe(true);
    }
  });

  it("rejects unknown server types", () => {
    expect(isServerMessage({ type: "wat", payload: {} } as PicoMessage)).toBe(false);
  });

  it("PicoMessage envelope shape", () => {
    expectTypeOf<PicoMessage>().toHaveProperty("type");
    expectTypeOf<PicoMessage>().toHaveProperty("payload");
  });
});
```

- [ ] **Step 2: Run test, watch fail**

```bash
cd apps/clawx-gui && pnpm vitest run src/lib/__tests__/pico-types.test.ts
```

Expected: FAIL "Cannot find module '../pico-types'".

- [ ] **Step 3: Implement `pico-types.ts`**

```ts
export type ClientMessageType = "message.send" | "media.send" | "ping";

export type ServerMessageType =
  | "message.create"
  | "message.update"
  | "media.create"
  | "typing.start"
  | "typing.stop"
  | "error"
  | "pong";

export type PicoMessageType = ClientMessageType | ServerMessageType;

export interface PicoMessage<P = Record<string, unknown>> {
  type: PicoMessageType;
  id?: string;
  session_id?: string;
  timestamp?: number;
  payload?: P;
}

export interface MessageCreatePayload {
  message_id: string;
  content: string;
  thought?: boolean;
}

export interface MessageUpdatePayload extends MessageCreatePayload {}

export interface MessageSendPayload {
  content: string;
  media?: string | object | unknown[];
}

export interface ErrorPayload {
  code: string;
  message: string;
  request_id?: string;
}

const SERVER_TYPES = new Set<ServerMessageType>([
  "message.create",
  "message.update",
  "media.create",
  "typing.start",
  "typing.stop",
  "error",
  "pong",
]);

export function isServerMessage(m: PicoMessage): boolean {
  return SERVER_TYPES.has(m.type as ServerMessageType);
}
```

- [ ] **Step 4: Run, watch pass.**
- [ ] **Step 5: Commit**

```bash
git add apps/clawx-gui/src/lib/pico-types.ts apps/clawx-gui/src/lib/__tests__/pico-types.test.ts
git commit -m "feat(gui): pico-types — protocol envelope + payload types"
```

---

### Task 5.2: pico-rest.ts (TDD)

**Files:**
- Create: `apps/clawx-gui/src/lib/pico-rest.ts`
- Test: `apps/clawx-gui/src/lib/__tests__/pico-rest.test.ts`

- [ ] **Step 1: Write failing test**

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  fetchPicoToken,
  listSessions,
  getSession,
  deleteSession,
  listSkills,
  listTools,
  setToolEnabled,
  PicoApiError,
} from "../pico-rest";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

function ok(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

describe("pico-rest", () => {
  it("fetchPicoToken returns token info", async () => {
    fetchMock.mockResolvedValue(ok({ token: "T", ws_url: "ws://x/pico/ws", enabled: true }));
    const t = await fetchPicoToken();
    expect(t.token).toBe("T");
    expect(t.enabled).toBe(true);
    expect(fetchMock).toHaveBeenCalledWith("/api/pico/token", expect.any(Object));
  });

  it("listSessions passes offset/limit + auth header", async () => {
    fetchMock.mockResolvedValue(ok([]));
    await listSessions({ offset: 10, limit: 20, token: "T" });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/sessions?offset=10&limit=20",
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer T" }),
      }),
    );
  });

  it("getSession resolves a single session", async () => {
    fetchMock.mockResolvedValue(ok({ id: "s1", messages: [] }));
    const s = await getSession("s1", "T");
    expect(s.id).toBe("s1");
  });

  it("deleteSession sends DELETE", async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }));
    await deleteSession("s1", "T");
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/sessions/s1",
      expect.objectContaining({ method: "DELETE" }),
    );
  });

  it("listSkills + listTools + setToolEnabled", async () => {
    fetchMock.mockResolvedValue(ok([]));
    await listSkills("T");
    await listTools("T");
    fetchMock.mockResolvedValue(ok({ ok: true }));
    await setToolEnabled("web_search", false, "T");
    expect(fetchMock).toHaveBeenLastCalledWith(
      "/api/tools/web_search/state",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ enabled: false }),
      }),
    );
  });

  it("throws PicoApiError on non-2xx", async () => {
    fetchMock.mockResolvedValue(new Response(JSON.stringify({ message: "nope" }), { status: 401 }));
    await expect(fetchPicoToken()).rejects.toBeInstanceOf(PicoApiError);
  });
});
```

- [ ] **Step 2: Run, watch fail.**

- [ ] **Step 3: Implement `pico-rest.ts`** (full code below — copy verbatim, do not abbreviate)

```ts
export interface PicoTokenInfo {
  token: string;
  ws_url: string;
  enabled: boolean;
}

export interface SessionSummary {
  id: string;
  title: string;
  preview: string;
  message_count: number;
  created: number;
  updated: number;
}

export interface SessionMessage {
  role: "user" | "assistant" | "system";
  content: string;
  media?: unknown;
}

export interface SessionDetail extends SessionSummary {
  messages: SessionMessage[];
  summary: string;
}

export interface SkillInfo {
  name: string;
  description?: string;
  installed?: boolean;
}

export interface ToolInfo {
  name: string;
  enabled: boolean;
  description?: string;
}

export class PicoApiError extends Error {
  constructor(public readonly status: number, message: string) {
    super(message);
  }
}

async function call<T>(
  path: string,
  init: RequestInit & { token?: string } = {},
): Promise<T> {
  const { token, ...rest } = init;
  const headers: Record<string, string> = {
    ...(rest.headers as Record<string, string> | undefined),
  };
  if (token) headers.Authorization = `Bearer ${token}`;
  if (rest.body && !headers["Content-Type"])
    headers["Content-Type"] = "application/json";
  const res = await fetch(path, { ...rest, headers });
  if (!res.ok) {
    let msg = `${res.status} ${res.statusText}`;
    try {
      const body = await res.json();
      if (body?.message) msg = body.message;
    } catch {
      /* ignore */
    }
    throw new PicoApiError(res.status, msg);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export function fetchPicoToken(): Promise<PicoTokenInfo> {
  return call<PicoTokenInfo>("/api/pico/token", {});
}

export function listSessions(opts: {
  offset?: number;
  limit?: number;
  token: string;
}): Promise<SessionSummary[]> {
  const params = new URLSearchParams();
  if (opts.offset != null) params.set("offset", String(opts.offset));
  if (opts.limit != null) params.set("limit", String(opts.limit));
  const q = params.toString() ? `?${params}` : "";
  return call<SessionSummary[]>(`/api/sessions${q}`, { token: opts.token });
}

export function getSession(id: string, token: string): Promise<SessionDetail> {
  return call<SessionDetail>(`/api/sessions/${encodeURIComponent(id)}`, { token });
}

export function deleteSession(id: string, token: string): Promise<void> {
  return call<void>(`/api/sessions/${encodeURIComponent(id)}`, {
    method: "DELETE",
    token,
  });
}

export function listSkills(token: string): Promise<SkillInfo[]> {
  return call<SkillInfo[]>("/api/skills", { token });
}

export function listTools(token: string): Promise<ToolInfo[]> {
  return call<ToolInfo[]>("/api/tools", { token });
}

export function setToolEnabled(
  name: string,
  enabled: boolean,
  token: string,
): Promise<void> {
  return call<void>(`/api/tools/${encodeURIComponent(name)}/state`, {
    method: "PUT",
    token,
    body: JSON.stringify({ enabled }),
  });
}
```

- [ ] **Step 4: Run, watch pass. Commit.**

```bash
git add apps/clawx-gui/src/lib/pico-rest.ts apps/clawx-gui/src/lib/__tests__/pico-rest.test.ts
git commit -m "feat(gui): pico-rest — REST client for picoclaw /api/*"
```

---

### Task 5.3: pico-socket.ts (TDD)

**Files:**
- Create: `apps/clawx-gui/src/lib/pico-socket.ts`
- Test: `apps/clawx-gui/src/lib/__tests__/pico-socket.test.ts`

- [ ] **Step 1: Write failing test**

```ts
import { describe, it, expect, beforeEach, vi } from "vitest";
import { PicoSocket } from "../pico-socket";
import type { PicoMessage } from "../pico-types";

class FakeWS {
  static instances: FakeWS[] = [];
  url: string;
  protocols?: string | string[];
  readyState = 0;
  onopen?: () => void;
  onclose?: (e: { code: number; reason: string }) => void;
  onerror?: (e: unknown) => void;
  onmessage?: (e: { data: string }) => void;
  sent: string[] = [];

  constructor(url: string, protocols?: string | string[]) {
    this.url = url;
    this.protocols = protocols;
    FakeWS.instances.push(this);
  }
  send(data: string) { this.sent.push(data); }
  close(code = 1000) {
    this.readyState = 3;
    this.onclose?.({ code, reason: "" });
  }
  open() {
    this.readyState = 1;
    this.onopen?.();
  }
  emit(msg: PicoMessage) {
    this.onmessage?.({ data: JSON.stringify(msg) });
  }
}

beforeEach(() => {
  FakeWS.instances = [];
  vi.stubGlobal("WebSocket", FakeWS as unknown as typeof WebSocket);
});

describe("PicoSocket", () => {
  it("connects with token subprotocol and session_id query", () => {
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN" });
    s.connect();
    const ws = FakeWS.instances[0]!;
    expect(ws.url).toBe("ws://h/pico/ws?session_id=S1");
    expect(ws.protocols).toEqual(["token.TKN"]);
  });

  it("dispatches parsed server messages to onMessage", () => {
    const onMsg = vi.fn();
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN", onMessage: onMsg });
    s.connect();
    const ws = FakeWS.instances[0]!;
    ws.open();
    ws.emit({ type: "message.create", payload: { message_id: "m1", content: "hi" } });
    expect(onMsg).toHaveBeenCalledWith(expect.objectContaining({ type: "message.create" }));
  });

  it("send wraps client message into envelope JSON", () => {
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN" });
    s.connect();
    const ws = FakeWS.instances[0]!;
    ws.open();
    s.send({ type: "message.send", payload: { content: "hello" } });
    expect(ws.sent).toHaveLength(1);
    const sent = JSON.parse(ws.sent[0]!);
    expect(sent.type).toBe("message.send");
    expect(sent.session_id).toBe("S1");
    expect(typeof sent.id).toBe("string");
    expect(typeof sent.timestamp).toBe("number");
  });

  it("queues sends until socket open, flushes on open", () => {
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN" });
    s.connect();
    const ws = FakeWS.instances[0]!;
    s.send({ type: "message.send", payload: { content: "queued" } });
    expect(ws.sent).toHaveLength(0);
    ws.open();
    expect(ws.sent).toHaveLength(1);
  });

  it("close stops further reconnects", () => {
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN" });
    s.connect();
    s.close();
    const ws = FakeWS.instances[0]!;
    ws.close(1006);
    expect(FakeWS.instances).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run, watch fail.**

- [ ] **Step 3: Implement `pico-socket.ts`**

```ts
import type { PicoMessage } from "./pico-types";

export interface PicoSocketOptions {
  wsBase: string;
  sessionId: string;
  token: string;
  onMessage?: (msg: PicoMessage) => void;
  onOpen?: () => void;
  onClose?: (code: number) => void;
  onError?: (err: unknown) => void;
}

const RECONNECT_INITIAL_MS = 500;
const RECONNECT_MAX_MS = 30_000;

export class PicoSocket {
  private ws: WebSocket | null = null;
  private queue: PicoMessage[] = [];
  private reconnectMs = RECONNECT_INITIAL_MS;
  private closedByUser = false;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly opts: PicoSocketOptions) {}

  connect(): void {
    this.closedByUser = false;
    const url = `${this.opts.wsBase}?session_id=${encodeURIComponent(this.opts.sessionId)}`;
    const ws = new WebSocket(url, [`token.${this.opts.token}`]);
    this.ws = ws;
    ws.onopen = () => {
      this.reconnectMs = RECONNECT_INITIAL_MS;
      while (this.queue.length) {
        const m = this.queue.shift()!;
        ws.send(JSON.stringify(m));
      }
      this.opts.onOpen?.();
    };
    ws.onmessage = (ev) => {
      let parsed: PicoMessage;
      try {
        parsed = JSON.parse(typeof ev.data === "string" ? ev.data : "") as PicoMessage;
      } catch {
        return;
      }
      this.opts.onMessage?.(parsed);
    };
    ws.onerror = (err) => this.opts.onError?.(err);
    ws.onclose = (ev) => {
      this.opts.onClose?.(ev.code);
      if (this.closedByUser) return;
      this.timer = setTimeout(() => this.connect(), this.reconnectMs);
      this.reconnectMs = Math.min(this.reconnectMs * 2, RECONNECT_MAX_MS);
    };
  }

  send(msg: PicoMessage): void {
    const enriched: PicoMessage = {
      ...msg,
      id: msg.id ?? crypto.randomUUID(),
      session_id: msg.session_id ?? this.opts.sessionId,
      timestamp: msg.timestamp ?? Date.now(),
    };
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(enriched));
    } else {
      this.queue.push(enriched);
    }
  }

  close(): void {
    this.closedByUser = true;
    if (this.timer) clearTimeout(this.timer);
    this.ws?.close(1000);
  }
}
```

- [ ] **Step 4: Run, watch pass. Commit.**

```bash
git add apps/clawx-gui/src/lib/pico-socket.ts apps/clawx-gui/src/lib/__tests__/pico-socket.test.ts
git commit -m "feat(gui): pico-socket — WS client w/ subprotocol auth, queue, reconnect"
```

---

### Task 5.4: chat-store.ts (TDD)

**Files:**
- Create: `apps/clawx-gui/src/lib/chat-store.ts`
- Test: `apps/clawx-gui/src/lib/__tests__/chat-store.test.ts`

- [ ] **Step 1: Write failing test**

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { ChatStore } from "../chat-store";

describe("ChatStore", () => {
  let s: ChatStore;
  beforeEach(() => { s = new ChatStore(); });

  it("addUser optimistically appends user message", () => {
    const id = s.addUser("hi");
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]).toMatchObject({ id, role: "user", content: "hi" });
  });

  it("applyServer message.create appends assistant message", () => {
    s.applyServer({ type: "message.create", payload: { message_id: "m1", content: "hello" } });
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]).toMatchObject({
      id: "m1", role: "assistant", content: "hello", thought: false,
    });
  });

  it("applyServer message.update merges by message_id", () => {
    s.applyServer({ type: "message.create", payload: { message_id: "m1", content: "hel" } });
    s.applyServer({ type: "message.update", payload: { message_id: "m1", content: "hello world" } });
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]!.content).toBe("hello world");
  });

  it("thought:true messages tagged as thought", () => {
    s.applyServer({ type: "message.create", payload: { message_id: "t1", content: "thinking…", thought: true } });
    expect(s.messages[0]!.thought).toBe(true);
  });

  it("typing.start / typing.stop toggle typing flag", () => {
    s.applyServer({ type: "typing.start", payload: {} });
    expect(s.typing).toBe(true);
    s.applyServer({ type: "typing.stop", payload: {} });
    expect(s.typing).toBe(false);
  });

  it("error with request_id rolls back optimistic user message", () => {
    const id = s.addUser("oops", "REQ1");
    expect(s.messages).toHaveLength(1);
    s.applyServer({ type: "error", payload: { code: "RATE_LIMIT", message: "slow down", request_id: "REQ1" } });
    expect(s.messages.find((m) => m.id === id)).toBeUndefined();
    expect(s.lastError?.code).toBe("RATE_LIMIT");
  });

  it("subscribers fire on every state change", () => {
    let calls = 0;
    s.subscribe(() => calls++);
    s.addUser("a");
    s.applyServer({ type: "message.create", payload: { message_id: "m", content: "b" } });
    expect(calls).toBeGreaterThanOrEqual(2);
  });
});
```

- [ ] **Step 2: Run, watch fail.**

- [ ] **Step 3: Implement `chat-store.ts`**

```ts
import type {
  ErrorPayload,
  MessageCreatePayload,
  PicoMessage,
} from "./pico-types";

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  thought: boolean;
  requestId?: string;
  ts: number;
}

export class ChatStore {
  messages: ChatMessage[] = [];
  typing = false;
  lastError: ErrorPayload | null = null;
  private subs = new Set<() => void>();

  subscribe(fn: () => void): () => void {
    this.subs.add(fn);
    return () => this.subs.delete(fn);
  }

  private emit(): void {
    this.subs.forEach((f) => f());
  }

  addUser(content: string, requestId?: string): string {
    const id = requestId ?? crypto.randomUUID();
    this.messages = [
      ...this.messages,
      { id, role: "user", content, thought: false, requestId, ts: Date.now() },
    ];
    this.emit();
    return id;
  }

  applyServer(msg: PicoMessage): void {
    switch (msg.type) {
      case "message.create": {
        const p = msg.payload as MessageCreatePayload;
        this.messages = [
          ...this.messages,
          {
            id: p.message_id,
            role: "assistant",
            content: p.content,
            thought: !!p.thought,
            ts: Date.now(),
          },
        ];
        break;
      }
      case "message.update": {
        const p = msg.payload as MessageCreatePayload;
        this.messages = this.messages.map((m) =>
          m.id === p.message_id
            ? { ...m, content: p.content, thought: !!p.thought }
            : m,
        );
        break;
      }
      case "typing.start": this.typing = true; break;
      case "typing.stop": this.typing = false; break;
      case "error": {
        const p = msg.payload as ErrorPayload;
        this.lastError = p;
        if (p.request_id) {
          this.messages = this.messages.filter((m) => m.requestId !== p.request_id);
        }
        break;
      }
      default: return;
    }
    this.emit();
  }
}
```

- [ ] **Step 4: Run, watch pass. Commit.**

```bash
git add apps/clawx-gui/src/lib/chat-store.ts apps/clawx-gui/src/lib/__tests__/chat-store.test.ts
git commit -m "feat(gui): chat-store — message-id merge store + optimistic rollback"
```

---

### Task 5.5: store.tsx React provider (TDD)

**Files:**
- Modify (rewrite): `apps/clawx-gui/src/lib/store.tsx`
- Test: `apps/clawx-gui/src/lib/__tests__/store.test.tsx`

- [ ] **Step 1: Write failing test**

```tsx
import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { ClawProvider, useClaw, type ClawContextValue } from "../store";

vi.mock("../pico-rest", () => ({
  fetchPicoToken: vi.fn().mockResolvedValue({
    token: "T",
    ws_url: "ws://localhost:18790/pico/ws",
    enabled: true,
  }),
  listSessions: vi.fn().mockResolvedValue([]),
}));

describe("ClawProvider / useClaw", () => {
  it("loads token on mount and exposes session controls", async () => {
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => {});
    const v: ClawContextValue = result.current;
    expect(v.token).toBe("T");
    expect(typeof v.startNewSession).toBe("function");
    expect(typeof v.sendUserMessage).toBe("function");
    expect(v.chat.messages).toEqual([]);
  });
});
```

- [ ] **Step 2: Run, watch fail.**

- [ ] **Step 3: Rewrite `store.tsx`**

```tsx
import {
  createContext, useContext, useEffect, useMemo, useRef, useState,
  type ReactNode,
} from "react";
import { fetchPicoToken, type PicoTokenInfo } from "./pico-rest";
import { PicoSocket } from "./pico-socket";
import { ChatStore } from "./chat-store";

export interface ClawContextValue {
  token: string | null;
  wsUrl: string | null;
  enabled: boolean;
  sessionId: string | null;
  chat: ChatStore;
  startNewSession: () => void;
  sendUserMessage: (content: string) => void;
  refreshToken: () => Promise<PicoTokenInfo>;
}

const Ctx = createContext<ClawContextValue | null>(null);

export function ClawProvider({ children }: { children: ReactNode }) {
  const [token, setToken] = useState<string | null>(null);
  const [wsUrl, setWsUrl] = useState<string | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const chatRef = useRef(new ChatStore());
  const sockRef = useRef<PicoSocket | null>(null);
  const [, forceRender] = useState(0);

  useEffect(() => chatRef.current.subscribe(() => forceRender((n) => n + 1)), []);

  const refreshToken = async () => {
    const info = await fetchPicoToken();
    setToken(info.token);
    setWsUrl(info.ws_url);
    setEnabled(info.enabled);
    return info;
  };

  useEffect(() => { refreshToken().catch(() => undefined); }, []);

  useEffect(() => {
    if (!token || !wsUrl || !sessionId) return;
    sockRef.current?.close();
    const s = new PicoSocket({
      wsBase: wsUrl, sessionId, token,
      onMessage: (m) => chatRef.current.applyServer(m),
    });
    s.connect();
    sockRef.current = s;
    return () => s.close();
  }, [token, wsUrl, sessionId]);

  const startNewSession = () => setSessionId(crypto.randomUUID());

  const sendUserMessage = (content: string) => {
    if (!sockRef.current || !sessionId) return;
    const reqId = chatRef.current.addUser(content);
    sockRef.current.send({ type: "message.send", id: reqId, payload: { content } });
  };

  const value = useMemo<ClawContextValue>(
    () => ({
      token, wsUrl, enabled, sessionId,
      chat: chatRef.current,
      startNewSession, sendUserMessage, refreshToken,
    }),
    [token, wsUrl, enabled, sessionId],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useClaw(): ClawContextValue {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useClaw must be used inside <ClawProvider>");
  return ctx;
}
```

- [ ] **Step 4: Run, watch pass. Commit.**

```bash
git add apps/clawx-gui/src/lib/store.tsx apps/clawx-gui/src/lib/__tests__/store.test.tsx
git commit -m "feat(gui): rewrite store.tsx around picoclaw protocol"
```

---

## Phase 6 — Rewire ChatPage

### Task 6.1: Delete legacy api/types/chat-stream-store/agent-conv-memory

**Files (delete):** `apps/clawx-gui/src/lib/{api,types,chat-stream-store,agent-conv-memory}.ts` and matching `__tests__/{api,sse,chat-stream-store}.test.ts`.

- [ ] **Step 1: List all importers**

```bash
grep -RIn "from .*lib/\(api\|types\|chat-stream-store\|agent-conv-memory\)" apps/clawx-gui/src
```

Record the list — Task 6.2 + Phase 8 will fix them.

- [ ] **Step 2: git rm**

```bash
git rm apps/clawx-gui/src/lib/api.ts \
        apps/clawx-gui/src/lib/types.ts \
        apps/clawx-gui/src/lib/chat-stream-store.ts \
        apps/clawx-gui/src/lib/agent-conv-memory.ts \
        apps/clawx-gui/src/lib/__tests__/api.test.ts \
        apps/clawx-gui/src/lib/__tests__/sse.test.ts \
        apps/clawx-gui/src/lib/__tests__/chat-stream-store.test.ts
```

- [ ] **Step 3: Acknowledge break**

`pnpm build` will fail. Subsequent tasks restore green.

- [ ] **Step 4: Commit**

```bash
git commit -m "refactor!: drop legacy ClawX REST/SSE client (breaks build, fixed in subsequent commits)"
```

---

### Task 6.2: Rewire ChatPage to useClaw()

**Files:**
- Modify: `apps/clawx-gui/src/pages/ChatPage.tsx`
- Test: `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx`

- [ ] **Step 1: Read current ChatPage.tsx**

Note its imports and JSX structure for `MessageBubble`, `ChatInput`, `ChatWelcome`. Plan the new prop signatures.

- [ ] **Step 2: Write failing test**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import ChatPage from "../ChatPage";
import { ClawProvider } from "../../lib/store";

vi.mock("../../lib/pico-rest", () => ({
  fetchPicoToken: vi.fn().mockResolvedValue({
    token: "T",
    ws_url: "ws://localhost:18790/pico/ws",
    enabled: true,
  }),
}));

class FakeWS {
  static last: FakeWS;
  readyState = 1;
  onopen?: () => void;
  onmessage?: (e: { data: string }) => void;
  onclose?: () => void;
  onerror?: () => void;
  sent: string[] = [];
  constructor(public url: string, public protocols?: string | string[]) {
    FakeWS.last = this;
    queueMicrotask(() => this.onopen?.());
  }
  send(d: string) { this.sent.push(d); }
  close() { this.onclose?.(); }
}
vi.stubGlobal("WebSocket", FakeWS as unknown as typeof WebSocket);

describe("ChatPage", () => {
  it("welcome → user send → assistant reply renders", async () => {
    render(
      <MemoryRouter>
        <ClawProvider>
          <ChatPage />
        </ClawProvider>
      </MemoryRouter>,
    );
    await act(async () => {});
    expect(screen.getByTestId("chat-welcome")).toBeInTheDocument();

    const input = screen.getByRole("textbox");
    await act(async () => {
      fireEvent.change(input, { target: { value: "hello" } });
      fireEvent.submit(input.closest("form")!);
    });
    expect(screen.getByText("hello")).toBeInTheDocument();

    await act(async () => {
      FakeWS.last.onmessage?.({
        data: JSON.stringify({
          type: "message.create",
          payload: { message_id: "a1", content: "hi back" },
        }),
      });
    });
    expect(screen.getByText("hi back")).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Run, watch fail.**

- [ ] **Step 4: Rewrite ChatPage.tsx**

```tsx
import { useEffect } from "react";
import ChatInput from "../components/ChatInput";
import ChatWelcome from "../components/ChatWelcome";
import MessageBubble from "../components/MessageBubble";
import { useClaw } from "../lib/store";

export default function ChatPage() {
  const claw = useClaw();

  useEffect(() => {
    if (!claw.sessionId && claw.enabled) claw.startNewSession();
  }, [claw.enabled, claw.sessionId]);

  if (!claw.enabled) {
    return (
      <div className="p-8 text-center text-sm text-neutral-500">
        Pico channel disabled. Edit <code>~/.picoclaw/config.json</code>:
        set <code>channels.pico.enabled = true</code> and restart the launcher.
      </div>
    );
  }

  const { messages, typing } = claw.chat;

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 overflow-auto p-4 space-y-3">
        {messages.length === 0 ? (
          <ChatWelcome />
        ) : (
          messages.map((m) => (
            <MessageBubble
              key={m.id}
              role={m.role}
              content={m.content}
              thought={m.thought}
            />
          ))
        )}
        {typing && (
          <div className="text-xs text-neutral-400" data-testid="typing">…</div>
        )}
      </div>
      <ChatInput onSubmit={(text) => claw.sendUserMessage(text)} />
    </div>
  );
}
```

- [ ] **Step 5: Adapt `ChatWelcome` (root must have `data-testid="chat-welcome"`).**

- [ ] **Step 6: Adapt `MessageBubble` props**

```tsx
interface MessageBubbleProps {
  role: "user" | "assistant";
  content: string;
  thought?: boolean;
}
```

`thought` variant: muted opacity / "thinking" label. Keep markdown rendering for `content` if it was there before.

- [ ] **Step 7: Adapt `ChatInput`**

Signature: `{ onSubmit: (text: string) => void }`. Wrap in `<form>` so test's `fireEvent.submit` works. Clear input on submit.

- [ ] **Step 8: Run, watch pass. Commit.**

```bash
git add apps/clawx-gui/src/pages/ChatPage.tsx \
        apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx \
        apps/clawx-gui/src/components/ChatWelcome.tsx \
        apps/clawx-gui/src/components/MessageBubble.tsx \
        apps/clawx-gui/src/components/ChatInput.tsx
git commit -m "feat(gui): ChatPage uses Pico WS; bubble + input adapted"
```

---

## Phase 7 — Settings + Connectors

### Task 7.1: SettingsPage

**Files:**
- Modify: `apps/clawx-gui/src/pages/SettingsPage.tsx`
- Test: `apps/clawx-gui/src/pages/__tests__/SettingsPage.test.tsx`

- [ ] **Step 1: Failing test**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import SettingsPage from "../SettingsPage";
import { ClawProvider } from "../../lib/store";

const refresh = vi.fn().mockResolvedValue({
  token: "ABC",
  ws_url: "ws://localhost:18790/pico/ws",
  enabled: true,
});
vi.mock("../../lib/pico-rest", () => ({ fetchPicoToken: refresh }));

describe("SettingsPage", () => {
  it("renders token + ws_url + enabled, supports refresh", async () => {
    render(
      <MemoryRouter><ClawProvider><SettingsPage /></ClawProvider></MemoryRouter>,
    );
    await act(async () => {});
    expect(screen.getByText(/ABC/)).toBeInTheDocument();
    expect(screen.getByText(/ws:\/\/localhost:18790/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /refresh/i }));
    await act(async () => {});
    expect(refresh).toHaveBeenCalledTimes(2);
  });
});
```

- [ ] **Step 2: Run, watch fail.**

- [ ] **Step 3: Implement SettingsPage**

```tsx
import { useClaw } from "../lib/store";

export default function SettingsPage() {
  const claw = useClaw();
  return (
    <div className="p-6 space-y-4">
      <h1 className="text-xl font-semibold">Settings</h1>
      <section>
        <h2 className="font-medium">Pico Connection</h2>
        <dl className="mt-2 grid grid-cols-[140px_1fr] gap-x-4 gap-y-1 text-sm">
          <dt>Token</dt><dd className="font-mono break-all">{claw.token ?? "(none)"}</dd>
          <dt>WebSocket URL</dt><dd className="font-mono">{claw.wsUrl ?? "(none)"}</dd>
          <dt>Enabled</dt><dd>{claw.enabled ? "yes" : "no"}</dd>
        </dl>
        <button className="mt-3 rounded bg-neutral-200 px-3 py-1 text-sm"
                onClick={() => claw.refreshToken()}>
          Refresh
        </button>
      </section>
      <section className="text-xs text-neutral-500">
        To change the token or enable / disable the channel, edit
        <code className="mx-1">~/.picoclaw/config.json</code>
        and restart the launcher.
      </section>
    </div>
  );
}
```

- [ ] **Step 4: Run, watch pass. Commit.**

```bash
git add apps/clawx-gui/src/pages/SettingsPage.tsx apps/clawx-gui/src/pages/__tests__/SettingsPage.test.tsx
git commit -m "feat(gui): SettingsPage shows Pico token / ws URL / enabled"
```

---

### Task 7.2: ConnectorsPage = skills + tools browser

**Files:**
- Modify: `apps/clawx-gui/src/pages/ConnectorsPage.tsx`
- Test: `apps/clawx-gui/src/pages/__tests__/ConnectorsPage.test.tsx`

- [ ] **Step 1: Failing test**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import ConnectorsPage from "../ConnectorsPage";
import { ClawProvider } from "../../lib/store";

vi.mock("../../lib/pico-rest", () => ({
  fetchPicoToken: vi.fn().mockResolvedValue({ token: "T", ws_url: "ws://x", enabled: true }),
  listSkills: vi.fn().mockResolvedValue([{ name: "weather" }, { name: "code-runner" }]),
  listTools: vi.fn().mockResolvedValue([
    { name: "web_search", enabled: true },
    { name: "fs_read", enabled: false },
  ]),
  setToolEnabled: vi.fn().mockResolvedValue(undefined),
}));

describe("ConnectorsPage", () => {
  it("lists skills + tools; toggling a tool calls API", async () => {
    render(
      <MemoryRouter><ClawProvider><ConnectorsPage /></ClawProvider></MemoryRouter>,
    );
    await act(async () => {});
    expect(await screen.findByText("weather")).toBeInTheDocument();
    expect(screen.getByText("code-runner")).toBeInTheDocument();
    expect(screen.getByText("web_search")).toBeInTheDocument();

    const toggle = screen.getByRole("checkbox", { name: /web_search/i });
    await act(async () => { fireEvent.click(toggle); });
    const { setToolEnabled } = await import("../../lib/pico-rest");
    expect(setToolEnabled).toHaveBeenCalledWith("web_search", false, "T");
  });
});
```

- [ ] **Step 2: Run, watch fail.**

- [ ] **Step 3: Implement ConnectorsPage**

```tsx
import { useEffect, useState } from "react";
import { listSkills, listTools, setToolEnabled, type SkillInfo, type ToolInfo } from "../lib/pico-rest";
import { useClaw } from "../lib/store";

export default function ConnectorsPage() {
  const claw = useClaw();
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [tools, setTools] = useState<ToolInfo[]>([]);

  useEffect(() => {
    if (!claw.token) return;
    listSkills(claw.token).then(setSkills).catch(() => undefined);
    listTools(claw.token).then(setTools).catch(() => undefined);
  }, [claw.token]);

  const toggle = async (t: ToolInfo) => {
    if (!claw.token) return;
    const next = !t.enabled;
    setTools((arr) => arr.map((x) => (x.name === t.name ? { ...x, enabled: next } : x)));
    try { await setToolEnabled(t.name, next, claw.token); }
    catch {
      setTools((arr) => arr.map((x) => (x.name === t.name ? { ...x, enabled: t.enabled } : x)));
    }
  };

  return (
    <div className="p-6 space-y-6">
      <section>
        <h2 className="text-lg font-semibold">Skills</h2>
        <ul className="mt-2 space-y-1 text-sm">
          {skills.map((s) => (
            <li key={s.name}>
              <span className="font-mono">{s.name}</span>
              {s.description && <span className="text-neutral-500"> — {s.description}</span>}
            </li>
          ))}
          {skills.length === 0 && <li className="text-neutral-400">(no skills installed)</li>}
        </ul>
      </section>
      <section>
        <h2 className="text-lg font-semibold">Tools</h2>
        <ul className="mt-2 space-y-1 text-sm">
          {tools.map((t) => (
            <li key={t.name} className="flex items-center gap-2">
              <input id={`tool-${t.name}`} type="checkbox" aria-label={t.name}
                     checked={t.enabled} onChange={() => toggle(t)} />
              <label htmlFor={`tool-${t.name}`} className="font-mono">{t.name}</label>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
```

- [ ] **Step 4: Run, watch pass. Commit.**

```bash
git add apps/clawx-gui/src/pages/ConnectorsPage.tsx apps/clawx-gui/src/pages/__tests__/ConnectorsPage.test.tsx
git commit -m "feat(gui): ConnectorsPage = picoclaw skills + tools browser"
```

---

## Phase 8 — Delete obsolete pages, components, routes

### Task 8.1: Delete obsolete pages

**Files (delete):** `AgentsPage.tsx`, `TasksPage.tsx`, `KnowledgePage.tsx`, `ContactsPage.tsx`, and matching tests.

- [ ] **Step 1: List matching tests**

```bash
ls apps/clawx-gui/src/pages/__tests__/*.test.tsx
```

- [ ] **Step 2: git rm**

```bash
git rm apps/clawx-gui/src/pages/{AgentsPage,TasksPage,KnowledgePage,ContactsPage}.tsx
# plus matching tests
```

- [ ] **Step 3: Commit**

```bash
git commit -m "refactor!: remove agents/tasks/knowledge/contacts pages (no picoclaw equivalent)"
```

---

### Task 8.2: Delete obsolete components

**Files (delete, 14 components):** `AddProviderModal`, `AgentGridCard`, `AgentModelAssignTable`, `AgentSidebar`, `AgentTemplateModal`, `ArtifactsPanel`, `AvailableChannelChip`, `ConnectorCard`, `KnowledgeSearchPanel`, `KnowledgeSourceList`, `ModelProviderCard`, `SkillStore`, `SourceReferences`, `TaskCard` — and any matching `__tests__/`.

- [ ] **Step 1: List matching tests**

```bash
ls apps/clawx-gui/src/components/__tests__/*.test.tsx
```

- [ ] **Step 2: git rm them all**

(Batch the 14 paths into one `git rm` call.)

- [ ] **Step 3: Commit**

```bash
git commit -m "refactor!: remove components tied to deleted domain pages"
```

---

### Task 8.3: App.tsx + NavBar reduced to 3 routes

**Files:**
- Modify: `apps/clawx-gui/src/App.tsx`
- Modify: `apps/clawx-gui/src/components/NavBar.tsx`
- Modify or delete: `apps/clawx-gui/src/components/SettingsNav.tsx`

- [ ] **Step 1: Update App.tsx**

```tsx
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import ChatPage from "./pages/ChatPage";
import ConnectorsPage from "./pages/ConnectorsPage";
import SettingsPage from "./pages/SettingsPage";
import NavBar from "./components/NavBar";
import { ClawProvider } from "./lib/store";

export default function App() {
  return (
    <ClawProvider>
      <BrowserRouter>
        <div className="flex h-screen">
          <NavBar />
          <main className="flex-1 overflow-hidden">
            <Routes>
              <Route path="/" element={<ChatPage />} />
              <Route path="/connectors" element={<ConnectorsPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </main>
        </div>
      </BrowserRouter>
    </ClawProvider>
  );
}
```

- [ ] **Step 2: Reduce NavBar to 3 links + drop dead imports.**
- [ ] **Step 3: SettingsNav: delete if no longer needed; otherwise prune.**

- [ ] **Step 4: Run vitest + build**

```bash
cd apps/clawx-gui && pnpm vitest run && pnpm build
```

Expected: all green, dist emitted, zero TS errors.

- [ ] **Step 5: Commit**

```bash
git commit -m "refactor: routes + nav reduced to chat/connectors/settings"
```

---

## Phase 9 — Dev workflow (no docker)

### Task 9.1: Root `package.json` for concurrent dev

**Files:** Create `/Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/package.json`.

- [ ] **Step 1: Write root package.json**

```json
{
  "name": "frank-claw-root",
  "version": "0.4.0",
  "private": true,
  "scripts": {
    "dev": "concurrently -k -n backend,frontend -c blue,green \"pnpm dev:backend\" \"pnpm dev:frontend\"",
    "dev:backend": "cd backend/web && go run ./backend/ --allow-empty",
    "dev:backend:setup": "cd backend && make build-launcher-frontend",
    "dev:frontend": "pnpm --filter clawx-gui dev",
    "build": "pnpm build:backend && pnpm build:frontend",
    "build:backend": "cd backend && make build-launcher",
    "build:frontend": "pnpm --filter clawx-gui build",
    "test": "pnpm test:backend && pnpm test:frontend",
    "test:backend": "cd backend && go test ./...",
    "test:frontend": "pnpm --filter clawx-gui vitest run"
  },
  "devDependencies": {
    "concurrently": "^9.0.0"
  }
}
```

- [ ] **Step 2: Add root pnpm-workspace.yaml**

```yaml
packages:
  - "apps/*"
```

- [ ] **Step 3: Install at root**

```bash
cd /Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/.worktrees/picoclaw-migration
pnpm install
```

Expected: `concurrently` installed at root; `apps/clawx-gui` linked into workspace.

- [ ] **Step 4: Smoke-test the dev script (optional, manual)**

```bash
pnpm dev
```

Expected: backend prints `Gateway started on 127.0.0.1:18790`; frontend prints `Local: http://localhost:5173/`. Ctrl+C terminates both.

- [ ] **Step 5: Commit**

```bash
git add package.json pnpm-workspace.yaml pnpm-lock.yaml
git commit -m "feat: root pnpm workspace + concurrently dev script"
```

---

### Task 9.2: Vite proxy for /api + /pico/ws

**Files:** Modify `apps/clawx-gui/vite.config.ts`.

- [ ] **Step 1: Add server.proxy block**

Inside `defineConfig({...})`:

```ts
server: {
  proxy: {
    "/api": { target: "http://127.0.0.1:18790", changeOrigin: false },
    "/pico/ws": { target: "ws://127.0.0.1:18790", ws: true, changeOrigin: false },
  },
},
```

- [ ] **Step 2: Verify (with launcher running) that `curl http://localhost:5173/api/pico/token` returns the same JSON as `curl http://127.0.0.1:18790/api/pico/token`.**

- [ ] **Step 3: Commit**

```bash
git add apps/clawx-gui/vite.config.ts
git commit -m "feat(gui): vite proxy /api + /pico/ws → :18790"
```

---

### Task 9.3: Rewrite README.md

**Files:** Modify or create `/Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/README.md`.

- [ ] **Step 1: Write README**

Sections:
1. **What this is** — "ClawX Web is a thin React frontend for [picoclaw](https://github.com/sipeed/picoclaw), whose source we vendor in `backend/` (see [ADR-037 v2](docs/arch/decisions.md))."
2. **Prerequisites** — Go ≥ 1.25, Node ≥ 22 + pnpm, at least one LLM provider configured in `~/.picoclaw/.security.yml`.
3. **Quick start**:
   ```bash
   pnpm install
   pnpm dev   # starts backend on :18790 + frontend on :5173
   ```
   then open `http://localhost:5173`.
4. **Configuring picoclaw** — `~/.picoclaw/config.json`, `~/.picoclaw/.security.yml`, the `enabled = true` requirement on the Pico channel.
5. **Repo layout** — link to `docs/arch/architecture.md`.
6. **Architecture** — link to ADR-037 v2 + architecture.md.
7. **Production build** — `pnpm build` then `./backend/build/picoclaw-launcher -webroot ./apps/clawx-gui/dist -no-browser`.
8. **License** — your existing.

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: README for vendored-picoclaw architecture"
```

---

### Task 9.4: Update AGENTS.md

**Files:** Modify `AGENTS.md`.

- [ ] **Step 1: Strip Rust + Tauri sections.**
- [ ] **Step 2: Replace tech-stack section** with the v5.0 stack (React + Vite + TypeScript + Go + pnpm).
- [ ] **Step 3: Replace build / test / run section** with `pnpm dev` / `pnpm build` / `pnpm test`.
- [ ] **Step 4: Add "Modifying the backend" subsection** noting that `backend/` is a vendored picoclaw fork; any local change must be recorded in `backend/PATCHES.md`.
- [ ] **Step 5: Keep all collaboration / commit-message conventions intact.**
- [ ] **Step 6: Commit**

```bash
git add AGENTS.md
git commit -m "docs(agents): switch toolchain refs from Rust → pnpm/go/picoclaw"
```

---

## Phase 10 — Final verification + tag v0.4.0

### Task 10.1: Repo-wide hygiene check

- [ ] **Step 1: No stray Rust / Tauri / Cargo references**

```bash
grep -RIn "cargo \|crate ::\|clawx-runtime\|clawx-service\|clawx-controlplane-client\|@tauri-apps\|src-tauri" \
    --exclude-dir=docs \
    --exclude-dir=node_modules \
    --exclude-dir=backend \
    .
```

Expected: zero matches.

- [ ] **Step 2: Backend tests green**

```bash
pnpm test:backend
```

Expected: `go test ./backend/...` exits 0. (If picoclaw's own tests have flakiness, identify which test, mark `t.Skip` with a `// upstream-flake` comment, record in `backend/PATCHES.md`. Don't ignore.)

- [ ] **Step 3: Frontend tests green**

```bash
pnpm test:frontend
```

Expected: all vitest suites pass.

- [ ] **Step 4: Production build clean**

```bash
pnpm build
```

Expected: `backend/build/picoclaw-launcher` produced, `apps/clawx-gui/dist/` produced.

---

### Task 10.2: Manual browser smoke test

- [ ] **Step 1: Start everything**

```bash
pnpm dev
```

- [ ] **Step 2: In browser at `http://localhost:5173`:**

- [ ] Chat page loads. If "Pico channel disabled", apply config patch from Task 1.3 Step 2 + restart launcher (Ctrl+C the dev script and re-run).
- [ ] Send "hello" — user bubble appears immediately.
- [ ] Assistant replies (assuming a provider is configured). Bubble renders. (If no provider, expect a `error` event in DevTools console — that's a test of the error path, not a bug.)
- [ ] `/connectors` lists skills + tools; toggling a tool round-trips (DevTools → Network → `PUT /api/tools/:name/state`).
- [ ] `/settings` shows the token, ws URL, enabled.

---

### Task 10.3: Tag v0.4.0

- [ ] **Step 1: From the worktree, after PR merge to main:**

```bash
git tag v0.4.0
# user pushes when they want
```

(The plan executor does NOT push tags or PRs without explicit user approval per overall safety norms.)

---

## Done.

What you should have on disk after Phase 10:
- `apps/clawx-gui/` — React + Vite + TS, no Tauri, talks to backend via Vite proxy
- `backend/` — vendored picoclaw Go source at SHA `8461c996…`, compiled by `pnpm build:backend`
- `docs/arch/{architecture,api-design,decisions}.md` — v5.0 + ADR-037 v2
- `docs/arch/{autonomy,memory,security,data-model,crate-dependency-graph}-architecture.md` — historical, deprecation banner
- `backend/UPSTREAM.md`, `backend/PATCHES.md` — provenance + local-change ledger
- Root `package.json` + `pnpm-workspace.yaml` — `pnpm dev` runs everything
- `README.md` — quick-start for the new shape
- Tag: `v0.4.0`

What's gone: 17 Rust crates, 2 Rust app binaries, Tauri shell, custom REST/SSE backend, every domain UI not directly tied to chatting with an LLM, all docker artifacts.
