# hermes_bridge

Python FastAPI adapter that embeds [hermes-agent](https://github.com/NousResearch/hermes-agent) and exposes a Pico-compatible REST + WebSocket API consumed by `apps/clawx-gui/`.

## Dev

```bash
uv sync
uv run python -m hermes_bridge
```

## Test

```bash
uv run pytest
```

## Entrypoint

`python -m hermes_bridge` → uvicorn on `127.0.0.1:18800`.
