# ClawX Development Workflow

> AI agents must follow this workflow for every task. No shortcuts.

## Phase 1: Understand (读)

Before writing any code, read and understand the context:

```
1. Read agents.md                    → Project overview, tech stack, quick rules
2. Read docs/arch/architecture.md    → System architecture (v6.0)
3. Read docs/arch/api-design.md      → Wire protocol (REST + WS)
4. Read rules/                       → Coding constraints for the language you're working in
```

**Checkpoint**: Can you explain what you're about to build and why? If not, keep reading.

## Phase 2: Plan (想)

Design before you code:

1. Identify which modules/files are affected.
2. Define the public API (function signatures, Pydantic schemas, TypeScript types) first.
3. Consider security implications (token handling, input validation, no secrets in logs).
4. Check `docs/arch/decisions.md` for prior decisions that constrain your design.
5. If making a new architectural decision, log it in `docs/arch/decisions.md`.

**Checkpoint**: Write a brief plan and confirm with the user before proceeding.

## Phase 3: Test First (测)

Write tests before implementation:

1. Write failing unit tests that define the expected behavior.
2. Use parametrize / table-driven tests for combinatorial cases.
3. Include adversarial inputs (prompt injection, malformed data, edge cases).
4. Backend: pytest + pytest-asyncio + httpx (`uv run pytest -q` from `backend/`).
5. Frontend: vitest + @testing-library/react (`pnpm --filter clawx-gui test`).

**Checkpoint**: Tests exist and they fail (because implementation doesn't exist yet).

## Phase 4: Implement (写)

Write the minimum code to make tests pass:

1. Follow existing patterns in the codebase — don't restructure surrounding code.
2. Keep files focused; split when one module does too much.
3. Name clearly: function / variable names say what things do, not how they work.
4. Backend: ruff-clean (`uv run ruff check`) before committing.
5. Frontend: `pnpm --filter clawx-gui lint` clean.

**Checkpoint**: All tests pass. Lint clean. No dead code.

## Phase 5: Review (审)

Before claiming done:

1. Re-read the diff with fresh eyes.
2. Run the full test suite once more: `pnpm test`.
3. Dev-smoke any user-facing change in the browser (`pnpm dev`, open `http://localhost:1420`).
4. Write a commit message that explains **why**, not **what**.

## Commits

- Imperative mood, English, concise.
- One logical change per commit.
- Never `git add -A`; stage specific files by name.
- Co-authored-by footer when the commit was done with Claude.

## Hermes-agent upgrades

When bumping `backend/pyproject.toml` to a newer hermes-agent SHA:

1. `uv lock` inside `backend/`, run `uv run pytest -q` — should be green.
2. If a symbol used by `backend/hermes_bridge/bridge/hermes_factory.py` or `session_store.py` was renamed upstream, patch only those files. The rest of the adapter is insulated by design.
3. Update `backend/docs/hermes-internal-surface.md` with the new SHA + any changed symbol names.
4. Smoke-test an end-to-end chat in the browser before committing.
