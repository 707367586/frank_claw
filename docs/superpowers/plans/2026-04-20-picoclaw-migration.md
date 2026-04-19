# PicoClaw Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete the entire Rust workspace + Tauri shell, repoint the React frontend at upstream [picoclaw](https://github.com/sipeed/picoclaw) via its Pico WebSocket + REST APIs, and ship a slim "ClawX Web" that is purely a frontend for picoclaw.

**Architecture:** Per [ADR-037](../../arch/decisions.md#adr-037-2026-04-20-全面迁移至-picoclaw-后端删除全部-rust-代码) and [architecture.md v5.0](../../arch/architecture.md): browser → picoclaw gateway (`:18790`) over `wss://…/pico/ws?session_id=` (subprotocol auth) for chat, and `https://…/api/{pico/token,sessions,skills,tools}` for management. No own backend, no Tauri, no SSE.

**Tech Stack:** React 19 + TypeScript 5 + Vite 6 + react-router-dom 7 + vitest 4 (unchanged). picoclaw (Go ≥ 1.25) as external runtime via docker-compose.

**Out of scope (deleted, not migrated):** agents/memories/knowledge/vault/tasks/channels/skills-as-domain UIs, tool-approval UI, model-router UI, Tauri shell, Cargo workspace.

**Spec source:** This plan is the spec. Architecture context: `docs/arch/architecture.md` and `docs/arch/api-design.md` (both v5.0, written 2026-04-20 in the same session).

---

## Phase Map

| Phase | Theme | Reversible? | Blast radius |
|---|---|---|---|
| 1 | Stand up picoclaw locally + dev workflow | Yes | Local only |
| 2 | Delete all Rust + Tauri files | **No** (use git) | Whole repo |
| 3 | Build new TS protocol layer (TDD) | Yes | gui only |
| 4 | Rewire ChatPage to new protocol | Yes | gui only |
| 5 | Rebuild Connectors + Settings as REST shells | Yes | gui only |
| 6 | Delete obsolete pages, components, lib files | Yes (git) | gui only |
| 7 | README / AGENTS.md / CI / final verification | Yes | repo metadata |

> Phase 2 is destructive. Commit after Phase 1 so you have a clean restore point. Do all Phase 2 deletions in **one** commit so a single `git revert` restores everything.

---

## Phase 1 — Stand up picoclaw + dev workflow

### Task 1.1: Add `docker-compose.yml` pinning picoclaw

**Files:**
- Create: `docker-compose.yml`

- [ ] **Step 1: Look up the latest picoclaw release tag**

Open https://github.com/sipeed/picoclaw/releases in a browser, copy the latest stable tag (expected format `v0.2.x`). For this plan use `v0.2.4` as the placeholder; replace below if newer.

- [ ] **Step 2: Write docker-compose.yml**

```yaml
services:
  picoclaw:
    image: ghcr.io/sipeed/picoclaw:v0.2.4
    container_name: clawx-picoclaw
    restart: unless-stopped
    ports:
      - "127.0.0.1:18790:18790"   # gateway (frontend talks here)
      - "127.0.0.1:18800:18800"   # launcher webui (config / token mgmt)
    environment:
      PICOCLAW_GATEWAY_HOST: "0.0.0.0"
      PICOCLAW_GATEWAY_PORT: "18790"
    volumes:
      - ./.picoclaw:/root/.picoclaw
```

- [ ] **Step 3: Start it and verify**

Run: `docker compose up -d picoclaw && sleep 3 && curl -sS http://127.0.0.1:18800/`
Expected: HTTP 200 returning the launcher HTML (contains `<title>` mentioning picoclaw).

- [ ] **Step 4: Manually create Pico token via launcher**

Open `http://127.0.0.1:18800/` in browser, follow the launcher to enable the Pico channel and generate a token. Note: do **not** commit `.picoclaw/` — add to `.gitignore` next.

- [ ] **Step 5: Verify token endpoint**

Run: `curl -sS http://127.0.0.1:18790/api/pico/token`
Expected: JSON `{ "token": "<…>", "ws_url": "ws://…/pico/ws", "enabled": true }`. If `enabled: false`, redo Step 4.

- [ ] **Step 6: Update `.gitignore`**

Append to `/Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/.gitignore`:

```
# picoclaw runtime data (per-developer)
.picoclaw/
```

- [ ] **Step 7: Commit**

```bash
git add docker-compose.yml .gitignore
git commit -m "feat: docker-compose for local picoclaw runtime"
```

---

### Task 1.2: Add Vite proxy so dev server can reach picoclaw

**Files:**
- Modify: `apps/clawx-gui/vite.config.ts`

- [ ] **Step 1: Read current vite.config.ts**

Open the file. We are about to add a `server.proxy` block.

- [ ] **Step 2: Add proxy entries**

Inside `defineConfig({...})` add:

```ts
server: {
  proxy: {
    "/api": {
      target: "http://127.0.0.1:18790",
      changeOrigin: false,
    },
    "/pico/ws": {
      target: "ws://127.0.0.1:18790",
      ws: true,
      changeOrigin: false,
    },
  },
},
```

- [ ] **Step 3: Verify dev server can fetch token through proxy**

Run: `cd apps/clawx-gui && pnpm dev` (background). Then:
`curl -sS http://localhost:5173/api/pico/token`
Expected: same JSON as Task 1.1 Step 5.

- [ ] **Step 4: Stop dev server, commit**

```bash
git add apps/clawx-gui/vite.config.ts
git commit -m "feat(gui): vite proxy /api + /pico/ws to picoclaw gateway"
```

---

## Phase 2 — Delete Rust + Tauri (one atomic commit)

> **STOP AND CONFIRM** with user before this phase. Once committed, restoration requires `git revert <hash>` which is fine, but make sure Phase 1 left a working dev loop first.

### Task 2.1: Delete Rust workspace

**Files (delete):**
- `crates/` (entire directory, all 17 sub-crates)
- `apps/clawx-service/`
- `apps/clawx-cli/`
- `Cargo.toml`
- `Cargo.lock`
- `clippy.toml`
- `rust-toolchain.toml`
- `rustfmt.toml`
- `target/` (regenerated artifacts; don't `git rm` if already gitignored — verify)

- [ ] **Step 1: Verify `target/` is gitignored**

Run: `git check-ignore target` — expected output: `target`. If not ignored, add to `.gitignore` first.

- [ ] **Step 2: Stage all deletions**

```bash
git rm -r crates apps/clawx-service apps/clawx-cli
git rm Cargo.toml Cargo.lock clippy.toml rust-toolchain.toml rustfmt.toml
```

- [ ] **Step 3: Verify nothing else still references the Rust workspace**

Run: `grep -RIl "clawx-service\|clawx-runtime\|clawx-controlplane-client\|clawx-cli" -- ':!docs/arch' ':!docs/superpowers' ':!.git'`
Expected: empty output (only the architecture/plan docs may legitimately mention them).

- [ ] **Step 4: Commit**

```bash
git commit -m "refactor!: delete Rust workspace (superseded by picoclaw, ADR-037)"
```

---

### Task 2.2: Delete Tauri shell from clawx-gui

**Files (delete):**
- `apps/clawx-gui/src-tauri/` (entire directory)

**Files (modify):**
- `apps/clawx-gui/package.json` — remove `@tauri-apps/cli` devDep + `tauri` script
- `apps/clawx-gui/pnpm-lock.yaml` — regenerate
- `apps/clawx-gui/package-lock.json` — delete (project standardizes on pnpm; lock file mixing is a footgun — verify `pnpm-lock.yaml` is the canonical one before deleting `package-lock.json`. If unsure, ask the user.)

- [ ] **Step 1: Confirm pnpm vs npm canonical lockfile with user**

Stop and ask: "I see both `pnpm-lock.yaml` and `package-lock.json` in `apps/clawx-gui/`. Which is canonical? I'll delete the other."

- [ ] **Step 2: Delete src-tauri**

```bash
git rm -r apps/clawx-gui/src-tauri
```

- [ ] **Step 3: Update package.json**

Open `apps/clawx-gui/package.json`. Remove the line `"tauri": "tauri",` from `scripts`. Remove `"@tauri-apps/cli": "^2",` from `devDependencies`. Bump `"version": "0.2.0"` → `"version": "0.4.0"`. (v0.3 was the Rust era; v0.4 marks the picoclaw migration.)

- [ ] **Step 4: Reinstall to refresh lockfile**

```bash
cd apps/clawx-gui && pnpm install
```

Expected: lockfile updated, `node_modules` no longer contains `@tauri-apps/cli`. Verify with: `ls node_modules/@tauri-apps 2>/dev/null` — expected: not found.

- [ ] **Step 5: Verify build still passes**

```bash
cd apps/clawx-gui && pnpm build
```

Expected: TypeScript compile may fail because `lib/api.ts` still imports `Agent`, `Memory`, etc. — that's fine, we'll fix in Phase 3. **What we're checking:** Vite/Tauri removal didn't break the toolchain itself. If the failure is *only* in `src/lib/` and `src/pages/` source files (not in build config), proceed. If there's a config-level error, debug before continuing.

- [ ] **Step 6: Commit**

```bash
git add apps/clawx-gui/package.json apps/clawx-gui/pnpm-lock.yaml
git commit -m "refactor!: remove Tauri shell from clawx-gui; bump to v0.4.0"
```

---

## Phase 3 — Build new TS protocol layer (TDD)

> Each task in this phase is **test-first**. Write the failing test, watch it fail, write minimal code, watch it pass, commit. Do not skip the "watch it fail" step — that confirms the test is actually exercising the new code, not an old import.

### Task 3.1: Define PicoMessage types

**Files:**
- Create: `apps/clawx-gui/src/lib/pico-types.ts`
- Test: `apps/clawx-gui/src/lib/__tests__/pico-types.test.ts`

- [ ] **Step 1: Write failing test**

`apps/clawx-gui/src/lib/__tests__/pico-types.test.ts`:

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

- [ ] **Step 2: Run test, watch it fail**

Run: `cd apps/clawx-gui && pnpm vitest run src/lib/__tests__/pico-types.test.ts`
Expected: FAIL with "Cannot find module '../pico-types'".

- [ ] **Step 3: Implement minimal types**

`apps/clawx-gui/src/lib/pico-types.ts`:

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

- [ ] **Step 4: Run test, watch it pass**

Run: `pnpm vitest run src/lib/__tests__/pico-types.test.ts`
Expected: PASS, 3 tests.

- [ ] **Step 5: Commit**

```bash
git add apps/clawx-gui/src/lib/pico-types.ts apps/clawx-gui/src/lib/__tests__/pico-types.test.ts
git commit -m "feat(gui): pico-types — protocol envelope + payload types"
```

---

### Task 3.2: REST client (`pico-rest.ts`)

**Files:**
- Create: `apps/clawx-gui/src/lib/pico-rest.ts`
- Test: `apps/clawx-gui/src/lib/__tests__/pico-rest.test.ts`

- [ ] **Step 1: Write failing test**

`apps/clawx-gui/src/lib/__tests__/pico-rest.test.ts`:

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
    fetchMock.mockResolvedValue(
      ok({ token: "T", ws_url: "ws://x/pico/ws", enabled: true }),
    );
    const t = await fetchPicoToken();
    expect(t.token).toBe("T");
    expect(t.enabled).toBe(true);
    expect(fetchMock).toHaveBeenCalledWith("/api/pico/token", expect.any(Object));
  });

  it("listSessions passes offset/limit", async () => {
    fetchMock.mockResolvedValue(ok([]));
    await listSessions({ offset: 10, limit: 20, token: "T" });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/sessions?offset=10&limit=20",
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer T" }),
      }),
    );
  });

  it("getSession includes Authorization header", async () => {
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

  it("listSkills + listTools + setToolEnabled work", async () => {
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
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ message: "nope" }), { status: 401 }),
    );
    await expect(fetchPicoToken()).rejects.toBeInstanceOf(PicoApiError);
  });
});
```

- [ ] **Step 2: Run test, watch it fail**

Run: `pnpm vitest run src/lib/__tests__/pico-rest.test.ts`
Expected: FAIL with module-not-found.

- [ ] **Step 3: Implement `pico-rest.ts`**

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

- [ ] **Step 4: Run test, watch it pass**

Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/clawx-gui/src/lib/pico-rest.ts apps/clawx-gui/src/lib/__tests__/pico-rest.test.ts
git commit -m "feat(gui): pico-rest — REST client for picoclaw /api/*"
```

---

### Task 3.3: WebSocket client (`pico-socket.ts`)

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
  send(data: string) {
    this.sent.push(data);
  }
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
    const s = new PicoSocket({
      wsBase: "ws://h/pico/ws",
      sessionId: "S1",
      token: "TKN",
    });
    s.connect();
    const ws = FakeWS.instances[0]!;
    expect(ws.url).toBe("ws://h/pico/ws?session_id=S1");
    expect(ws.protocols).toEqual(["token.TKN"]);
  });

  it("dispatches parsed server messages to onMessage", () => {
    const onMsg = vi.fn();
    const s = new PicoSocket({
      wsBase: "ws://h/pico/ws",
      sessionId: "S1",
      token: "TKN",
      onMessage: onMsg,
    });
    s.connect();
    const ws = FakeWS.instances[0]!;
    ws.open();
    ws.emit({
      type: "message.create",
      payload: { message_id: "m1", content: "hi" },
    });
    expect(onMsg).toHaveBeenCalledWith(
      expect.objectContaining({ type: "message.create" }),
    );
  });

  it("send wraps client message into envelope JSON", () => {
    const s = new PicoSocket({
      wsBase: "ws://h/pico/ws",
      sessionId: "S1",
      token: "TKN",
    });
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
    const s = new PicoSocket({
      wsBase: "ws://h/pico/ws",
      sessionId: "S1",
      token: "TKN",
    });
    s.connect();
    const ws = FakeWS.instances[0]!;
    s.send({ type: "message.send", payload: { content: "queued" } });
    expect(ws.sent).toHaveLength(0);
    ws.open();
    expect(ws.sent).toHaveLength(1);
  });

  it("close stops further reconnects", () => {
    const s = new PicoSocket({
      wsBase: "ws://h/pico/ws",
      sessionId: "S1",
      token: "TKN",
    });
    s.connect();
    s.close();
    const ws = FakeWS.instances[0]!;
    ws.close(1006);
    expect(FakeWS.instances).toHaveLength(1);
  });
});
```

- [ ] **Step 2: Run test, watch it fail**

Expected: FAIL, module not found.

- [ ] **Step 3: Implement `pico-socket.ts`**

```ts
import type { PicoMessage } from "./pico-types";

export interface PicoSocketOptions {
  wsBase: string;        // e.g. "ws://127.0.0.1:18790/pico/ws"
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
    const url = `${this.opts.wsBase}?session_id=${encodeURIComponent(
      this.opts.sessionId,
    )}`;
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

- [ ] **Step 4: Run test, watch it pass**

Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/clawx-gui/src/lib/pico-socket.ts apps/clawx-gui/src/lib/__tests__/pico-socket.test.ts
git commit -m "feat(gui): pico-socket — WS client w/ subprotocol auth, queue, reconnect"
```

---

### Task 3.4: Chat store (message-id merge, no SSE)

**Files:**
- Create: `apps/clawx-gui/src/lib/chat-store.ts`
- Test: `apps/clawx-gui/src/lib/__tests__/chat-store.test.ts`

- [ ] **Step 1: Write failing test**

```ts
import { describe, it, expect, beforeEach } from "vitest";
import { ChatStore } from "../chat-store";

describe("ChatStore", () => {
  let s: ChatStore;
  beforeEach(() => {
    s = new ChatStore();
  });

  it("addUser optimistically appends user message", () => {
    const id = s.addUser("hi");
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]).toMatchObject({ id, role: "user", content: "hi" });
  });

  it("applyServer message.create appends assistant message", () => {
    s.applyServer({
      type: "message.create",
      payload: { message_id: "m1", content: "hello" },
    });
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]).toMatchObject({
      id: "m1",
      role: "assistant",
      content: "hello",
      thought: false,
    });
  });

  it("applyServer message.update merges by message_id", () => {
    s.applyServer({
      type: "message.create",
      payload: { message_id: "m1", content: "hel" },
    });
    s.applyServer({
      type: "message.update",
      payload: { message_id: "m1", content: "hello world" },
    });
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]!.content).toBe("hello world");
  });

  it("thought:true messages tagged as thought", () => {
    s.applyServer({
      type: "message.create",
      payload: { message_id: "t1", content: "thinking…", thought: true },
    });
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
    s.applyServer({
      type: "error",
      payload: { code: "RATE_LIMIT", message: "slow down", request_id: "REQ1" },
    });
    expect(s.messages.find((m) => m.id === id)).toBeUndefined();
    expect(s.lastError?.code).toBe("RATE_LIMIT");
  });

  it("subscribers fire on every state change", () => {
    let calls = 0;
    s.subscribe(() => calls++);
    s.addUser("a");
    s.applyServer({
      type: "message.create",
      payload: { message_id: "m", content: "b" },
    });
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
      case "typing.start":
        this.typing = true;
        break;
      case "typing.stop":
        this.typing = false;
        break;
      case "error": {
        const p = msg.payload as ErrorPayload;
        this.lastError = p;
        if (p.request_id) {
          this.messages = this.messages.filter(
            (m) => m.requestId !== p.request_id,
          );
        }
        break;
      }
      default:
        return;
    }
    this.emit();
  }
}
```

- [ ] **Step 4: Run, watch pass.**

- [ ] **Step 5: Commit**

```bash
git add apps/clawx-gui/src/lib/chat-store.ts apps/clawx-gui/src/lib/__tests__/chat-store.test.ts
git commit -m "feat(gui): chat-store — message-id merge store + optimistic rollback"
```

---

### Task 3.5: React store wrapper (`store.tsx`)

**Files:**
- Modify (rewrite): `apps/clawx-gui/src/lib/store.tsx`
- Test: `apps/clawx-gui/src/lib/__tests__/store.test.tsx`

- [ ] **Step 1: Read current store.tsx so we know what to delete**

Open the file. Catalogue every export so we can grep for callers later.

- [ ] **Step 2: Write failing test**

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

- [ ] **Step 3: Run, watch fail.**

- [ ] **Step 4: Implement `store.tsx`**

Replace the entire file:

```tsx
import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
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

function newSessionId(): string {
  return crypto.randomUUID();
}

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

  useEffect(() => {
    refreshToken().catch(() => undefined);
  }, []);

  // (Re)connect when we have token + sessionId
  useEffect(() => {
    if (!token || !wsUrl || !sessionId) return;
    sockRef.current?.close();
    const s = new PicoSocket({
      wsBase: wsUrl,
      sessionId,
      token,
      onMessage: (m) => chatRef.current.applyServer(m),
    });
    s.connect();
    sockRef.current = s;
    return () => s.close();
  }, [token, wsUrl, sessionId]);

  const startNewSession = () => setSessionId(newSessionId());

  const sendUserMessage = (content: string) => {
    if (!sockRef.current || !sessionId) return;
    const reqId = chatRef.current.addUser(content);
    sockRef.current.send({
      type: "message.send",
      id: reqId,
      payload: { content },
    });
  };

  const value = useMemo<ClawContextValue>(
    () => ({
      token,
      wsUrl,
      enabled,
      sessionId,
      chat: chatRef.current,
      startNewSession,
      sendUserMessage,
      refreshToken,
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

- [ ] **Step 5: Run, watch pass.**

- [ ] **Step 6: Commit**

```bash
git add apps/clawx-gui/src/lib/store.tsx apps/clawx-gui/src/lib/__tests__/store.test.tsx
git commit -m "feat(gui): rewrite store.tsx around picoclaw protocol"
```

---

## Phase 4 — Rewire ChatPage

### Task 4.1: Strip old api.ts/types.ts/chat-stream-store.ts/agent-conv-memory.ts

**Files (delete):**
- `apps/clawx-gui/src/lib/api.ts`
- `apps/clawx-gui/src/lib/types.ts`
- `apps/clawx-gui/src/lib/chat-stream-store.ts`
- `apps/clawx-gui/src/lib/agent-conv-memory.ts`
- `apps/clawx-gui/src/lib/__tests__/api.test.ts`
- `apps/clawx-gui/src/lib/__tests__/sse.test.ts`
- `apps/clawx-gui/src/lib/__tests__/chat-stream-store.test.ts`

- [ ] **Step 1: List all files importing from these modules**

Run: `Grep` for `from "./api"`, `from "./types"`, `from "./chat-stream-store"`, `from "./agent-conv-memory"` (and `from "../lib/api"` etc. variants) under `apps/clawx-gui/src/`.

Expected: a list of files. Record it for Task 4.2 + 6.x.

- [ ] **Step 2: Delete the files**

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

`pnpm build` will fail with hundreds of TS errors at every callsite. That is **expected**. The next tasks (4.2 + Phase 6) systematically restore green.

- [ ] **Step 4: Commit (broken-build commit, intentional)**

```bash
git commit -m "refactor!: drop legacy ClawX REST/SSE client (breaks build, fixed in subsequent commits)"
```

---

### Task 4.2: Rewire `ChatPage.tsx` to `useClaw()`

**Files:**
- Modify: `apps/clawx-gui/src/pages/ChatPage.tsx`
- Test: `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx`

- [ ] **Step 1: Read current ChatPage.tsx**

Identify which imports came from the deleted modules. Note the JSX structure for `MessageBubble`, `ChatInput`, `ChatWelcome` so we can keep their existing prop contracts.

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
  it("renders welcome state, sends a message, renders assistant reply", async () => {
    render(
      <MemoryRouter>
        <ClawProvider>
          <ChatPage />
        </ClawProvider>
      </MemoryRouter>,
    );
    await act(async () => {});
    // Welcome visible (no messages)
    expect(screen.getByTestId("chat-welcome")).toBeInTheDocument();

    // Send a user message
    const input = screen.getByRole("textbox");
    await act(async () => {
      fireEvent.change(input, { target: { value: "hello" } });
      fireEvent.submit(input.closest("form")!);
    });
    expect(screen.getByText("hello")).toBeInTheDocument();

    // Simulate assistant message arriving from server
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

(Add `data-testid="chat-welcome"` to `ChatWelcome`'s root element if it doesn't already have one. If `ChatInput` doesn't render an actual `<form>`, wrap its onSubmit in one.)

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
        Pico channel disabled. Open the picoclaw launcher
        ({" "}<a href="http://127.0.0.1:18800" className="underline">:18800</a>{" "})
        and enable it, then refresh.
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
          <div className="text-xs text-neutral-400" data-testid="typing">
            …
          </div>
        )}
      </div>
      <ChatInput onSubmit={(text) => claw.sendUserMessage(text)} />
    </div>
  );
}
```

- [ ] **Step 5: Update ChatWelcome to expose testid**

`apps/clawx-gui/src/components/ChatWelcome.tsx`: ensure root element has `data-testid="chat-welcome"`. If absent, add it; otherwise skip.

- [ ] **Step 6: Update MessageBubble props**

If `MessageBubble` currently expects a `Message` from `lib/types`, change its prop signature to:

```tsx
interface MessageBubbleProps {
  role: "user" | "assistant";
  content: string;
  thought?: boolean;
}
```

Render `thought` variant with a subdued style (e.g. lower opacity, "thinking" label).

- [ ] **Step 7: Update ChatInput**

Ensure it has signature `{ onSubmit: (text: string) => void }` and submits a `<form>` so the test's `fireEvent.submit` works.

- [ ] **Step 8: Run, watch pass.**

- [ ] **Step 9: Commit**

```bash
git add apps/clawx-gui/src/pages/ChatPage.tsx \
        apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx \
        apps/clawx-gui/src/components/ChatWelcome.tsx \
        apps/clawx-gui/src/components/MessageBubble.tsx \
        apps/clawx-gui/src/components/ChatInput.tsx
git commit -m "feat(gui): ChatPage uses Pico WS; bubble + input adapted"
```

---

## Phase 5 — Connectors + Settings as REST shells

### Task 5.1: Rewrite SettingsPage

**Files:**
- Modify: `apps/clawx-gui/src/pages/SettingsPage.tsx`
- Test: `apps/clawx-gui/src/pages/__tests__/SettingsPage.test.tsx`

- [ ] **Step 1: Write failing test**

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
      <MemoryRouter>
        <ClawProvider>
          <SettingsPage />
        </ClawProvider>
      </MemoryRouter>,
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
        <button
          className="mt-3 rounded bg-neutral-200 px-3 py-1 text-sm"
          onClick={() => claw.refreshToken()}
        >
          Refresh
        </button>
      </section>
      <section className="text-xs text-neutral-500">
        To change the token or enable/disable the channel, use the picoclaw
        launcher at{" "}
        <a className="underline" href="http://127.0.0.1:18800" target="_blank" rel="noreferrer">
          http://127.0.0.1:18800
        </a>
        .
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

### Task 5.2: Rewrite ConnectorsPage as Skills + Tools browser

**Files:**
- Modify: `apps/clawx-gui/src/pages/ConnectorsPage.tsx`
- Test: `apps/clawx-gui/src/pages/__tests__/ConnectorsPage.test.tsx`

- [ ] **Step 1: Write failing test**

```tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import ConnectorsPage from "../ConnectorsPage";
import { ClawProvider } from "../../lib/store";

vi.mock("../../lib/pico-rest", () => ({
  fetchPicoToken: vi.fn().mockResolvedValue({
    token: "T",
    ws_url: "ws://x",
    enabled: true,
  }),
  listSkills: vi.fn().mockResolvedValue([{ name: "weather" }, { name: "code-runner" }]),
  listTools: vi.fn().mockResolvedValue([
    { name: "web_search", enabled: true },
    { name: "fs_read", enabled: false },
  ]),
  setToolEnabled: vi.fn().mockResolvedValue(undefined),
}));

describe("ConnectorsPage", () => {
  it("lists skills and tools; toggling a tool calls the API", async () => {
    render(
      <MemoryRouter>
        <ClawProvider>
          <ConnectorsPage />
        </ClawProvider>
      </MemoryRouter>,
    );
    await act(async () => {});
    expect(screen.getByText("weather")).toBeInTheDocument();
    expect(screen.getByText("code-runner")).toBeInTheDocument();
    expect(screen.getByText("web_search")).toBeInTheDocument();

    const toggle = screen.getByRole("checkbox", { name: /web_search/i });
    await act(async () => {
      fireEvent.click(toggle);
    });
    const { setToolEnabled } = await import("../../lib/pico-rest");
    expect(setToolEnabled).toHaveBeenCalledWith("web_search", false, "T");
  });
});
```

- [ ] **Step 2: Run, watch fail.**

- [ ] **Step 3: Implement ConnectorsPage**

```tsx
import { useEffect, useState } from "react";
import {
  listSkills,
  listTools,
  setToolEnabled,
  type SkillInfo,
  type ToolInfo,
} from "../lib/pico-rest";
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
    try {
      await setToolEnabled(t.name, next, claw.token);
    } catch {
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
              <input
                id={`tool-${t.name}`}
                type="checkbox"
                aria-label={t.name}
                checked={t.enabled}
                onChange={() => toggle(t)}
              />
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

## Phase 6 — Delete obsolete pages, components, and routes

### Task 6.1: Delete obsolete pages

**Files (delete):**
- `apps/clawx-gui/src/pages/AgentsPage.tsx`
- `apps/clawx-gui/src/pages/TasksPage.tsx`
- `apps/clawx-gui/src/pages/KnowledgePage.tsx`
- `apps/clawx-gui/src/pages/ContactsPage.tsx`
- All matching files under `apps/clawx-gui/src/pages/__tests__/` (e.g. `AgentsPage.test.tsx` if present)

- [ ] **Step 1: List which page tests exist**

Run: `Glob` `apps/clawx-gui/src/pages/__tests__/*.test.tsx`. Note any that match the pages above so we delete them too.

- [ ] **Step 2: git rm them**

```bash
git rm apps/clawx-gui/src/pages/AgentsPage.tsx \
        apps/clawx-gui/src/pages/TasksPage.tsx \
        apps/clawx-gui/src/pages/KnowledgePage.tsx \
        apps/clawx-gui/src/pages/ContactsPage.tsx
# plus any matching tests from Step 1
```

- [ ] **Step 3: Commit**

```bash
git commit -m "refactor!: remove agents/tasks/knowledge/contacts pages (no picoclaw equivalent)"
```

---

### Task 6.2: Delete obsolete components

**Files (delete):**
- `apps/clawx-gui/src/components/AddProviderModal.tsx`
- `apps/clawx-gui/src/components/AgentGridCard.tsx`
- `apps/clawx-gui/src/components/AgentModelAssignTable.tsx`
- `apps/clawx-gui/src/components/AgentSidebar.tsx`
- `apps/clawx-gui/src/components/AgentTemplateModal.tsx`
- `apps/clawx-gui/src/components/ArtifactsPanel.tsx`
- `apps/clawx-gui/src/components/AvailableChannelChip.tsx`
- `apps/clawx-gui/src/components/ConnectorCard.tsx`
- `apps/clawx-gui/src/components/KnowledgeSearchPanel.tsx`
- `apps/clawx-gui/src/components/KnowledgeSourceList.tsx`
- `apps/clawx-gui/src/components/ModelProviderCard.tsx`
- `apps/clawx-gui/src/components/SkillStore.tsx` (subsumed by ConnectorsPage)
- `apps/clawx-gui/src/components/SourceReferences.tsx`
- `apps/clawx-gui/src/components/TaskCard.tsx`
- All matching tests under `apps/clawx-gui/src/components/__tests__/`

- [ ] **Step 1: List matching component tests**

Run: `Glob` `apps/clawx-gui/src/components/__tests__/*.test.tsx`. Note matching ones for deletion.

- [ ] **Step 2: git rm them all**

```bash
git rm apps/clawx-gui/src/components/AddProviderModal.tsx \
        apps/clawx-gui/src/components/AgentGridCard.tsx \
        apps/clawx-gui/src/components/AgentModelAssignTable.tsx \
        apps/clawx-gui/src/components/AgentSidebar.tsx \
        apps/clawx-gui/src/components/AgentTemplateModal.tsx \
        apps/clawx-gui/src/components/ArtifactsPanel.tsx \
        apps/clawx-gui/src/components/AvailableChannelChip.tsx \
        apps/clawx-gui/src/components/ConnectorCard.tsx \
        apps/clawx-gui/src/components/KnowledgeSearchPanel.tsx \
        apps/clawx-gui/src/components/KnowledgeSourceList.tsx \
        apps/clawx-gui/src/components/ModelProviderCard.tsx \
        apps/clawx-gui/src/components/SkillStore.tsx \
        apps/clawx-gui/src/components/SourceReferences.tsx \
        apps/clawx-gui/src/components/TaskCard.tsx
# plus matching tests from Step 1
```

- [ ] **Step 3: Commit**

```bash
git commit -m "refactor!: remove components tied to deleted domain pages"
```

---

### Task 6.3: Update App.tsx + NavBar routes

**Files:**
- Modify: `apps/clawx-gui/src/App.tsx`
- Modify: `apps/clawx-gui/src/components/NavBar.tsx`
- Modify: `apps/clawx-gui/src/components/SettingsNav.tsx` (if it lists subroutes that no longer exist)

- [ ] **Step 1: Read App.tsx and identify routes**

Open `App.tsx`. The router should now expose only: `/` (chat), `/connectors`, `/settings`. Remove every `<Route>` for `/agents`, `/tasks`, `/knowledge`, `/contacts`.

- [ ] **Step 2: Update App.tsx**

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

- [ ] **Step 3: Update NavBar to expose only 3 routes**

Open `NavBar.tsx`. Reduce its links to: Chat (`/`), Connectors (`/connectors`), Settings (`/settings`). Delete any imports of removed icons/components.

- [ ] **Step 4: Update SettingsNav**

If it referenced old subpages, prune to only what SettingsPage actually has (currently a single section, so SettingsNav can be deleted entirely; if so `git rm` it and remove import sites).

- [ ] **Step 5: Run smoke test**

```bash
cd apps/clawx-gui && pnpm vitest run
```

Expected: all remaining tests pass. If `__smoke__.test.tsx` still references deleted modules, update or delete it.

- [ ] **Step 6: Verify build**

```bash
pnpm build
```

Expected: `tsc -b` clean, Vite build emits `dist/`. **All TypeScript errors must be zero.**

- [ ] **Step 7: Commit**

```bash
git add apps/clawx-gui/src/App.tsx apps/clawx-gui/src/components/NavBar.tsx
# and SettingsNav changes / deletion
git commit -m "refactor: routes + nav reduced to chat/connectors/settings"
```

---

## Phase 7 — Repo metadata + final verification

### Task 7.1: Rewrite README.md

**Files:**
- Modify: `/Users/zhoulingfeng/Desktop/code/makemoney/frank_claw/README.md` (or create if absent)

- [ ] **Step 1: Read current README**

If it exists, scan for Rust/Cargo/Tauri references — all must go.

- [ ] **Step 2: Write new README**

Sections to include:
1. **What this is** — "ClawX Web is a thin React frontend for [picoclaw](https://github.com/sipeed/picoclaw)."
2. **Quick start** — `docker compose up -d picoclaw`, browser to `http://127.0.0.1:18800` to grab a token, then `cd apps/clawx-gui && pnpm install && pnpm dev`, open `http://localhost:5173`.
3. **Repo layout** — link to `docs/arch/architecture.md`.
4. **Architecture** — link to ADR-037 + architecture.md.
5. **License** — keep whatever current is.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: README for picoclaw-backed ClawX Web"
```

---

### Task 7.2: Update AGENTS.md

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: Read current AGENTS.md**

It currently describes Rust workspace conventions, cargo commands, etc.

- [ ] **Step 2: Strip Rust-specific sections; keep collaboration norms**

Rewrite "Tech stack" and "How to build/test/run" sections to point at the new toolchain (pnpm + vitest + docker compose). Keep general collaboration / commit-message / branch conventions intact.

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs(agents): update toolchain refs from Rust → pnpm/picoclaw"
```

---

### Task 7.3: Final verification

- [ ] **Step 1: Confirm no stray Rust references remain in tracked files**

Run: `Grep` (case-insensitive) for `cargo `, `crate ::`, `clawx-runtime`, `clawx-service`, `clawx-controlplane-client`, `Tauri`, `tauri-apps`, `src-tauri`, scoped to all tracked files except `docs/arch/decisions.md` (which legitimately mentions them in ADR history) and `docs/superpowers/plans/` (this plan).
Expected: no hits.

- [ ] **Step 2: Run full test suite**

```bash
cd apps/clawx-gui && pnpm vitest run
```

Expected: all green.

- [ ] **Step 3: Build production bundle**

```bash
pnpm build
```

Expected: clean build, `dist/` populated.

- [ ] **Step 4: Manual smoke test**

```bash
docker compose up -d picoclaw
cd apps/clawx-gui && pnpm dev
```

In browser at `http://localhost:5173`:
- [ ] Chat page loads, shows "Pico channel disabled" if launcher token wasn't set up; otherwise welcome.
- [ ] Sending "hello" appears as a user bubble immediately.
- [ ] picoclaw replies (assuming an LLM provider is configured in the launcher) and the assistant bubble renders.
- [ ] Refreshing the browser, the chat history reloads from the same `session_id` (verify by re-mounting; picoclaw should remember).
- [ ] `/connectors` renders skills + tools list and a tool toggle round-trips (check Network tab for `PUT /api/tools/:name/state`).
- [ ] `/settings` shows the token + ws URL.

- [ ] **Step 5: Tag the migration**

```bash
git tag v0.4.0
git push origin v0.4.0
```

- [ ] **Step 6: Optional — open a PR**

If the work was done on a branch other than `main`, open a PR titled `feat!: migrate to picoclaw backend (drop Rust)` referencing ADR-037 in the body.

---

## Done.

What you should now have on disk:
- `apps/clawx-gui/` — React + Vite + TS, no Tauri, talking to picoclaw
- `docker-compose.yml` — pinned picoclaw runtime
- `docs/arch/{architecture,api-design,decisions}.md` — v5.0 + ADR-037
- `docs/arch/{autonomy,memory,security,data-model,crate-dependency-graph}-architecture.md` — historical, deprecation banner
- `README.md` — quick-start for the new shape
- Tag: `v0.4.0`

What's gone: 17 Rust crates, 2 Rust app binaries, Tauri shell, custom REST/SSE backend, every domain UI not directly tied to chatting with an LLM.
