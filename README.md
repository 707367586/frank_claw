# ClawX Web

ClawX Web is a thin React frontend for [picoclaw](https://github.com/sipeed/picoclaw), the open-source personal AI agent runtime. Picoclaw's source is **vendored** into [`backend/`](./backend/) at a pinned commit, with a small set of local patches we maintain ourselves. The frontend lives in [`apps/clawx-gui/`](./apps/clawx-gui/).

For the architectural rationale and decision history see [`docs/arch/decisions.md`](./docs/arch/decisions.md) (especially **ADR-037 v2**) and [`docs/arch/architecture.md`](./docs/arch/architecture.md).

## Prerequisites

- **Go** ≥ 1.25 — `brew install go` (macOS) or your platform's package manager
- **Node** ≥ 22 with **pnpm** ≥ 10 — `corepack enable && corepack prepare pnpm@latest --activate`
- An **LLM provider** for picoclaw to use. The simplest setup: install [Ollama](https://ollama.com) and pull a small model:

  ```bash
  brew install ollama
  ollama pull llama3.2
  ollama serve   # leave running
  ```

  You can also use any of the 30+ providers picoclaw supports via API key — see picoclaw's docs.

## Quick start

```bash
# 1. Install JS deps (root + workspace)
pnpm install

# 2. One-time: build the launcher's embedded frontend (used by Go embed.FS)
pnpm dev:backend:setup

# 3. One-time: bootstrap ~/.picoclaw/config.json
cd backend && go run ./scripts/init-config && cd ..

# 4. Start everything
pnpm dev
```

This brings up:
- **Backend** (`picoclaw-launcher`) on `http://127.0.0.1:18800`
- **Frontend** (Vite dev server) on `http://localhost:1420`

Open `http://localhost:1420` in your browser.

### First-visit auth

The launcher prints `dashboardToken: <…>` to its stdout. Copy that token; the app's Settings page will prompt you to paste it once. After that the token persists in your browser's `localStorage`.

If `~/.picoclaw/config.json` doesn't yet have an LLM model wired up, the chat WebSocket won't fully open (gateway can't start without a provider). The Settings page surfaces the connection state. Configure a model in `~/.picoclaw/config.json` + `~/.picoclaw/.security.yml`, restart the launcher, and refresh.

## Repo layout

```
frank_claw/
├── apps/clawx-gui/   React + Vite + TypeScript frontend
├── backend/          Vendored picoclaw (Go); see backend/UPSTREAM.md
│   ├── PATCHES.md    Local modifications we maintain
│   └── UPSTREAM.md   Vendor SHA + sync procedure
├── docs/arch/        Architecture docs (ADRs, current = v5.0)
├── docs/superpowers/ Migration plan + audit
└── package.json      Root scripts (concurrently dev/build/test)
```

## Common tasks

```bash
pnpm dev               # backend + frontend in parallel
pnpm test              # go test ./... + vitest run
pnpm build             # both binaries + dist/
pnpm test:frontend     # just vitest
pnpm test:backend      # just go test
pnpm build:backend     # build/picoclaw-launcher
pnpm build:frontend    # apps/clawx-gui/dist/
```

## Production single-binary mode

```bash
pnpm build
./backend/build/picoclaw-launcher -webroot ./apps/clawx-gui/dist -no-browser
```

The launcher serves both the API and the static frontend. No nginx, no docker.

## Architecture documents

- [`docs/arch/architecture.md`](./docs/arch/architecture.md) — current architecture (v5.0)
- [`docs/arch/api-design.md`](./docs/arch/api-design.md) — protocol contract
- [`docs/arch/decisions.md`](./docs/arch/decisions.md) — full ADR log; ADR-037 v2 is the migration decision
- [`backend/UPSTREAM.md`](./backend/UPSTREAM.md) — vendor SHA + how to sync upstream
- [`backend/PATCHES.md`](./backend/PATCHES.md) — every local change we've made to vendored picoclaw
- [`docs/superpowers/plans/2026-04-20-picoclaw-migration.md`](./docs/superpowers/plans/2026-04-20-picoclaw-migration.md) — migration plan
- [`docs/superpowers/plans/2026-04-20-picoclaw-surface-audit.md`](./docs/superpowers/plans/2026-04-20-picoclaw-surface-audit.md) — empirical API audit

Documents in `docs/arch/{autonomy,memory,security,data-model,crate-dependency-graph}-architecture.md` are **DEPRECATED** historical references from the Rust era.

## License

The frontend code in `apps/clawx-gui/` is under the same license as before this migration (no change). The vendored Go source in `backend/` is MIT-licensed (see [`backend/LICENSE`](./backend/LICENSE)) per upstream picoclaw.
