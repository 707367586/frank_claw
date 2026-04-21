# ClawX Web

ClawX Web is a thin React frontend (`apps/clawx-gui/`) for [hermes-agent](https://github.com/NousResearch/hermes-agent), the open-source autonomous AI agent framework from Nous Research. hermes-agent is embedded as a Python library inside `backend/hermes_bridge/`, a FastAPI adapter that exposes a REST + WebSocket surface the frontend consumes.

For the architecture + decision history see [`docs/arch/architecture.md`](./docs/arch/architecture.md) (v6.0) and [`docs/arch/decisions.md`](./docs/arch/decisions.md) (ADR-038).

## Prerequisites

- **Python** ≥ 3.11 — `brew install python@3.11` (macOS)
- **uv** — `curl -LsSf https://astral.sh/uv/install.sh | sh`
- **Node** ≥ 22 with **pnpm** ≥ 10 — `corepack enable && corepack prepare pnpm@latest --activate`
- An **LLM provider** that hermes-agent supports (Anthropic / OpenAI / OpenRouter / Nous / Ollama / …). Put the API key in `~/.hermes/.env`.

## Quick start

```bash
# 1. Install JS deps
pnpm install

# 2. One-time: install backend Python deps + bootstrap ~/.hermes/
pnpm dev:backend:setup

# 3. Start everything
pnpm dev
```

This brings up:
- **Backend** (`hermes_bridge`, Python/FastAPI/uvicorn) on `http://127.0.0.1:18800`
- **Frontend** (Vite dev server) on `http://localhost:1420`

Open `http://localhost:1420`.

### First-visit auth

The backend prints `dashboardToken: <…>` to its stdout. Copy it; the app's Settings page prompts you once; after that it lives in `localStorage`.

If `~/.hermes/config.yaml` lacks a usable provider, `/api/hermes/info` returns `enabled: false` and ChatPage instructs you to fix config.

## Repo layout

```
frank_claw/
├── apps/clawx-gui/   React + Vite + TypeScript frontend
├── backend/          Python (uv) — FastAPI adapter embedding hermes-agent
│   ├── hermes_bridge/            adapter package
│   ├── scripts/init_config.py    bootstrap ~/.hermes/
│   ├── tests/                    pytest suite
│   ├── pyproject.toml
│   └── uv.lock
├── docs/arch/        Architecture docs (current = v6.0)
├── docs/superpowers/ Migration plan
└── package.json      Root scripts (concurrently dev/test)
```

## Common tasks

```bash
pnpm dev               # backend + frontend in parallel
pnpm test              # uv run pytest + vitest run
pnpm build             # frontend bundle only
pnpm test:frontend     # just vitest
pnpm test:backend      # just pytest
pnpm build:frontend    # apps/clawx-gui/dist/
```

## Production single-process mode

```bash
pnpm build
uv run --project backend python -m hermes_bridge \
    --webroot ./apps/clawx-gui/dist \
    --no-browser
```

The backend serves both the API and the static frontend.

## Architecture documents

- [`docs/arch/architecture.md`](./docs/arch/architecture.md) — current (v6.0)
- [`docs/arch/api-design.md`](./docs/arch/api-design.md) — protocol contract
- [`docs/arch/decisions.md`](./docs/arch/decisions.md) — full ADR log; ADR-038 is the migration decision
- [`docs/superpowers/plans/2026-04-21-hermes-agent-migration.md`](./docs/superpowers/plans/2026-04-21-hermes-agent-migration.md) — this migration plan

## License

Frontend code in `apps/clawx-gui/` retains its original license. `backend/` is our original FastAPI glue under the same license; `hermes-agent` (Python dep) is MIT.
