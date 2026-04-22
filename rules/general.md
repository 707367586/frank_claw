# General Coding Rules

## Principles

- **Local-first**: Never send data externally without explicit user consent. All LLM traffic is hermes-agent's responsibility; the frontend never talks to a provider directly.
- **Security by default**: All traffic loopback (`127.0.0.1:18800`) by default. Dashboard token is the only auth. API keys live in `~/.hermes/.env` (chmod 0600) — never in code, never in logs.
- **Minimal footprint**: Cold start < 2s for `hermes_bridge`. Idle RSS < 200MB excluding hermes-agent itself.
- **YAGNI**: Don't add features, abstractions, or config the task doesn't require.

## Code Quality

- All public APIs have docstrings / JSDoc.
- Functions < 50 lines; decompose when longer.
- No magic numbers — named constants or settings fields.
- Don't write comments that restate the code. Only comment the *why* when non-obvious.
- Prefer editing existing files over creating new ones.

## Error Handling

- Errors carry context: what operation failed + why.
- Never silently swallow — log at minimum.
- Validate at system boundaries (HTTP, WS, filesystem). Trust internal calls.
- User-facing errors never leak secrets, tokens, or stack traces.

## Security Rules

- Never log secrets, API keys, or PII.
- Dashboard tokens masked in any UI display (`XXXX…XXXX`).
- All external input is untrusted — validate at boundaries.
- File operations: never escape `~/.hermes/` without explicit user scope.
- REST: `Authorization: Bearer` only, never `?token=` query.
- WS: `Sec-WebSocket-Protocol: token.<…>` only.

## Testing

- Unit test coverage ≥ 80% for `hermes_bridge/bridge/**` and `apps/clawx-gui/src/lib/**`.
- Integration tests for every user-facing flow (send message, toggle tool, paste token, refresh info).
- TDD where practical: failing test → minimal impl → green → refactor.
- Live hermes-agent tests are gated behind `HERMES_BRIDGE_LIVE=1` and skipped by default.

## Adapter Insulation

- `backend/hermes_bridge/bridge/hermes_factory.py` is the **only** file that imports hermes-agent internals. When upstream renames symbols, patch this file — nothing else should change.
- `backend/docs/hermes-internal-surface.md` tracks which upstream symbols we depend on. Update it whenever you change the pinned SHA.

## Git

- Branch: `feat/`, `fix/`, `refactor/`, `docs/`, `test/`, `chore/`
- Commits: imperative mood, English, concise. One logical change per commit.
- Never `git add -A` / `git add .` — stage specific files by name.
- Never `--no-verify` to skip hooks.
- Commit messages include `Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>` when the change was AI-assisted.

## Wire Protocol Invariants (frontend ↔ backend)

These cannot be changed unilaterally — any change must hit both sides in one PR:

- Message type strings: `message.send` / `message.create` / `message.update` / `media.send` / `media.create` / `typing.start` / `typing.stop` / `ping` / `pong` / `error`.
- Envelope fields: `type`, `id`, `session_id`, `timestamp`, `payload`.
- Same `payload.message_id` merges in `chat-store.ts` — servers must emit stable ids across `message.create` + subsequent `message.update` frames.
- `payload.thought: true` renders as a secondary bubble; `false` or absent = final reply.
- Authentication: REST `Authorization: Bearer <token>`; WS `Sec-WebSocket-Protocol: token.<token>`. No other auth mechanisms.
