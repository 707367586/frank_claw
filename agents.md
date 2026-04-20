# ClawX - Web Frontend for picoclaw

> This file is the entry point for all AI coding agents (Claude, Cursor, Windsurf, Copilot, etc.)

## Project Overview

ClawX is a React web frontend for [picoclaw](https://github.com/sipeed/picoclaw), the open-source personal AI agent runtime. Picoclaw's Go source is **vendored** into `backend/` at a pinned commit; the frontend lives in `apps/clawx-gui/`. All data processing runs locally for maximum privacy and security.

## Tech Stack

- **Frontend**: React 19 + Vite 6 + TypeScript 5 (in `apps/clawx-gui/`)
- **Backend**: vendored [picoclaw](https://github.com/sipeed/picoclaw) Go source under `backend/`. We freely modify it; every local change is recorded in `backend/PATCHES.md`. The pinned upstream SHA lives in `backend/UPSTREAM.md`.
- **Runtime tooling**: Go ≥ 1.25, Node ≥ 22, pnpm ≥ 10
- **Test**: Go's `go test` for backend, Vitest 4 + @testing-library/react + jsdom for frontend
- **Process glue**: `concurrently` at the repo root (`pnpm dev` runs both)

## Project Structure

```
frank_claw/
├── agents.md              # ← You are here. AI agent entry point.
├── workflow.md            # Development workflow (AI must follow this)
├── rules/                 # Coding rules & constraints
│   └── general.md         # Global rules (security, errors, testing)
├── docs/
│   └── arch/              # Architecture Design Documents
│       ├── README.md              # Architecture doc index
│       ├── architecture.md        # System architecture overview (v5.0)
│       ├── api-design.md          # API design
│       ├── data-model.md          # Data model
│       ├── memory-architecture.md # Memory architecture
│       ├── security-architecture.md # Security architecture
│       ├── autonomy-architecture.md # Autonomy architecture
│       └── decisions.md           # Architecture Decision Records (ADRs)
├── apps/
│   └── clawx-gui/         # React + Vite + TypeScript frontend
├── backend/               # Vendored picoclaw (Go source)
│   ├── PATCHES.md         # Local modifications we maintain
│   └── UPSTREAM.md        # Vendor SHA + sync procedure
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
| One-time: build embedded launcher frontend | `pnpm dev:backend:setup` |
| One-time: bootstrap `~/.picoclaw/config.json` | `cd backend && go run ./scripts/init-config && cd ..` |

See [`README.md`](./README.md) for the full quick-start.

## Modifying the backend

`backend/` is a vendored copy of upstream picoclaw at the SHA recorded in `backend/UPSTREAM.md`. We **own** this code now — feel free to edit it. But:

1. Every local change MUST be recorded in `backend/PATCHES.md` (one entry per logical patch, with **Why**, **Files**, **Local commit SHA**, and an **Upstream PR** field if you intend to push it back).
2. Use Go's `go test` against the modified file before committing — `go test ./backend/...` ideally green.
3. Don't refactor surrounding upstream code beyond what your patch needs. Keep the diff minimal so future upstream syncs are tractable.
4. To pull updates from upstream, follow the procedure in `backend/UPSTREAM.md` — never `git pull` upstream history into our branch.

If a patch grows large, consider whether it should be filed upstream first.

## How to Start (Read Order)

```
1. agents.md                    → Project overview & tech stack (you are here)
2. workflow.md                  → Step-by-step dev workflow (MUST follow)
3. docs/arch/architecture.md    → System architecture overview (v5.0)
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
feat: add memory hub trait definitions
fix: handle empty config.json on first run
docs(agents): switch toolchain refs from Rust → pnpm/go/picoclaw
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
