# ClawX - Web Frontend for hermes-agent

> This file is the entry point for all AI coding agents (Claude, Cursor, Windsurf, Copilot, etc.)

## Project Overview

ClawX is a React web frontend for [hermes-agent](https://github.com/NousResearch/hermes-agent), the open-source autonomous AI agent framework from Nous Research. hermes-agent is embedded as a Python library inside `backend/hermes_bridge/`, a FastAPI adapter that exposes a REST + WebSocket surface the frontend consumes. All data processing runs locally for maximum privacy and security.

## Tech Stack

- **Frontend**: React 19 + Vite 6 + TypeScript 5 (in `apps/clawx-gui/`)
- **Backend**: Python ≥ 3.11 FastAPI adapter (`backend/hermes_bridge/`) that imports [hermes-agent](https://github.com/NousResearch/hermes-agent) as a pinned git dependency (SHA recorded in `backend/pyproject.toml`)
- **Runtime tooling**: Python ≥ 3.11 + `uv`, Node ≥ 22, pnpm ≥ 10
- **Test**: pytest + pytest-asyncio + httpx for backend, Vitest 4 + @testing-library/react + jsdom for frontend
- **Process glue**: `concurrently` at the repo root (`pnpm dev` runs both)

## Project Structure

```
frank_claw/
├── agents.md              # ← You are here. AI agent entry point.
├── workflow.md            # Development workflow (AI must follow this)
├── rules/                 # Coding rules & constraints
├── docs/
│   └── arch/              # Architecture Design Documents (v6.0 current)
│       ├── README.md              # Architecture doc index
│       ├── architecture.md        # System architecture overview (v6.0)
│       ├── api-design.md          # API design (v6.0)
│       └── decisions.md           # Architecture Decision Records
├── apps/
│   └── clawx-gui/         # React + Vite + TypeScript frontend
├── backend/               # Python FastAPI adapter embedding hermes-agent
│   ├── hermes_bridge/     # Adapter package (api/, ws/, bridge/)
│   ├── scripts/init_config.py
│   ├── tests/             # pytest suite
│   ├── pyproject.toml
│   └── uv.lock
└── package.json           # Root scripts (concurrently dev/build/test)
```

## Build / test / run

| Task | Command |
|---|---|
| Run dev (backend + frontend) | `pnpm dev` |
| Frontend tests | `pnpm test:frontend` |
| Backend tests | `pnpm test:backend` |
| All tests | `pnpm test` |
| Production build | `pnpm build` |
| One-time: install deps + bootstrap `~/.hermes/` | `pnpm dev:backend:setup` |

See [`README.md`](./README.md) for the full quick-start.

## Modifying the backend

`backend/` is **our own** Python FastAPI glue code that embeds `hermes-agent` as a library. Two parts:

1. **Files we own freely** (`backend/hermes_bridge/**`): edit normally — this is original code with a thorough pytest suite.
2. **The hermes-agent upstream interface** (`backend/hermes_bridge/bridge/hermes_factory.py` is the only file that imports hermes-agent internals). When upstream moves the pinned SHA in `backend/pyproject.toml`, that file absorbs any symbol renames. Keep the brittleness localised there.

The hermes-agent internal surface we depend on is documented in `backend/docs/hermes-internal-surface.md`. Update that doc whenever the SHA moves.

## How to Start (Read Order)

```
1. agents.md                    → Project overview & tech stack (you are here)
2. workflow.md                  → Step-by-step dev workflow (MUST follow)
3. docs/arch/architecture.md    → System architecture overview (v6.0)
4. docs/arch/README.md          → Architecture doc index, dive deeper as needed
5. rules/general.md             → Coding constraints
```

## Core Rules (Quick Reference)

- **Local-first**: Never send data externally without user consent
- **Test coverage >= 80%** for core modules
- **One logical change per commit**: imperative mood, English
- **Security by default**: network whitelist, DLP scanning
- All public APIs must have doc comments
- Full rules in `rules/general.md`

## Collaboration Conventions

### Branch naming

- `feat/<slug>` — new feature
- `fix/<slug>` — bug fix
- `refactor/<slug>` — code cleanup without behavior change
- `docs/<slug>` — documentation only
- `test/<slug>` — tests only

### Commit messages

```
feat: add skill install endpoint
fix: handle empty config.yaml on first run
docs(agents): switch toolchain refs to uv/Python/hermes-agent
```

- Imperative mood, English, concise
- One logical change per commit
- Never `git add -A` — stage specific files by name

### PR style

- Keep PRs focused: one feature or fix per PR
- Include a short summary of **why**, not just **what**
- Link relevant ADRs from `docs/arch/decisions.md` if the change touches architecture
- All tests must pass before requesting review

### Code review etiquette

- Assume good intent; ask clarifying questions rather than asserting mistakes
- Block on correctness and security; suggest (don't block) on style
- Resolve all open comments before merging
