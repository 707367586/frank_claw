# Python Coding Rules

Scope: `backend/hermes_bridge/` + `backend/scripts/` + `backend/tests/`. Python ≥ 3.11.

## Tooling

- `uv` is the only package manager. Run commands via `uv run …` from `backend/`.
- `ruff` for lint + format. No black, no isort, no flake8.
- `pytest` + `pytest-asyncio` + `httpx` for tests. `asyncio_mode = "auto"` in `pyproject.toml`.
- No Poetry, no pip install, no pip-tools.

## Typing

- Type hints everywhere in new code. Use built-in generics (`list[str]`, not `List[str]`).
- `from __future__ import annotations` at the top of every file to get deferred eval — avoids runtime import cycles.
- Pydantic v2 for schemas. Prefer `BaseModel` with default field values over dataclasses when the shape crosses an API boundary.
- Use `Protocol` classes (like `HermesAgentLike`) for structural subtyping instead of inheritance.

## Structure

- One responsibility per module. `api/` is routing + Pydantic schemas only; `bridge/` is domain logic; `ws/` is framing. `bridge/` **never** imports from `api/` or `ws/`.
- Every subpackage has an empty `__init__.py`.
- Router factories (`make_router(settings)`) return `APIRouter` — never module-level app wiring.
- Service factories (`_store_factory`, `_svc_factory`) are module-level functions so tests can monkeypatch them.

## FastAPI

- Dependency injection via `Depends(require_bearer_token(settings))` for auth. Apply at router level (`dependencies=[dep]`) not per-endpoint when uniform.
- Return Pydantic models or `dict` / `list`; let FastAPI serialise.
- Use `status` constants (`status.HTTP_404_NOT_FOUND`) not bare integers.
- 204 responses use `Response(status_code=status.HTTP_204_NO_CONTENT)`.
- Validation errors become 4xx via `HTTPException`; map domain exceptions (`KeyError` → 404, `ValueError` → 409) at the handler boundary, not in services.

## WebSocket

- `websocket.accept(subprotocol=…)` must echo the matched client subprotocol — required by browsers.
- Never `await websocket.send_json({...})` with non-JSON types (datetime, Path). Convert first.
- Validate inbound frames with `HermesMessage.model_validate`. Bad frames → send `error` payload with `code: "bad_frame"`, do NOT close the socket.
- Wrap the whole receive loop in `try/except WebSocketDisconnect`. Any other exception = server bug; let it propagate so it's visible in logs.

## Async Discipline

- Don't mix blocking I/O with `async def`. Use `anyio.to_thread.run_sync(...)` to wrap hermes-agent's sync calls.
- Don't create a new `asyncio.new_event_loop()` manually; FastAPI / uvicorn / TestClient manage it.
- Never call `asyncio.run()` inside library code — only at true entrypoints.

## Tests

- Each module gets a `tests/test_<module>.py`.
- Use `monkeypatch.setenv` for `Settings()` overrides rather than constructing `Settings(**kwargs)` — pydantic-settings parses aliases from env, not kwargs.
- Inject service fakes via `monkeypatch.setattr("hermes_bridge.api.X._svc_factory", lambda _s: FakeSvc())`. The factory indirection exists precisely so tests can do this.
- Live/integration tests are `@pytest.mark.skipif(not os.environ.get("HERMES_BRIDGE_LIVE"), …)`.

## Hermes-Agent Adapter

- **Only** `bridge/hermes_factory.py` imports hermes-agent internals (`run_agent.AIAgent`, `hermes_state.SessionDB`, `toolsets`, `agent.skill_commands`).
- If you need a hermes symbol from another file, route the call through an adapter class in `bridge/` instead of importing directly.
- Update `backend/docs/hermes-internal-surface.md` any time you touch an import from hermes-agent.

## Errors

- Domain exceptions are bare `KeyError` / `ValueError` / `FileNotFoundError` — no custom hierarchy until one is justified.
- Translate at the HTTP/WS layer, not in services.
- User-facing error messages never leak stack traces or internal paths.

## Imports

- Stdlib → third-party → local, separated by blank lines. Ruff enforces.
- No wildcard imports (`from x import *`).
- Local imports use explicit relative form inside the package (`from ..config import Settings`).
