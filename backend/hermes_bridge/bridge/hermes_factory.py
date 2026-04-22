"""The ONLY file in hermes_bridge that imports hermes-agent's internals.

On upstream version bumps, this file absorbs the change. All other code
depends on `HermesRunner` + `HermesAgentLike` protocol which is stable.

Symbols depended upon — see `backend-py/docs/hermes-internal-surface.md`:

- `run_agent.AIAgent` — the turn-loop class.
- `AIAgent.chat(message: str, stream_callback=None) -> str` — single-turn.
- Provider/model default resolution via `~/.hermes/config.yaml` and env vars
  (`OPENROUTER_API_KEY` / `ANTHROPIC_API_KEY` / etc.).
"""

from __future__ import annotations

import logging
import uuid
from typing import Any, AsyncIterator

import anyio

from ..config import Settings
from .hermes_runner import HermesAgentLike, HermesRunner

log = logging.getLogger(__name__)


class _HermesAgentAdapter:
    """Adapts hermes-agent's sync AIAgent.chat() into our async iterator API.

    MVP shape: one hermes turn → one `final` event. Intermediate reasoning /
    tool calls stream via hermes callbacks; wiring them through into
    `thought` events is a follow-up (can be done without changing any other
    file in `hermes_bridge`).
    """

    def __init__(self, settings: Settings, session_id: str) -> None:
        self._settings = settings
        self._session_id = session_id
        self._agent: Any | None = None
        self._init_error: str | None = None
        try:
            # Import lazily — hermes-agent pulls in a lot of transitive deps
            # and keeping the import out of module scope lets hermes_bridge
            # load even if hermes-agent has a broken import chain.
            from hermes_cli.config import load_config  # type: ignore[import-not-found]
            from hermes_cli.runtime_provider import (  # type: ignore[import-not-found]
                resolve_runtime_provider,
            )
            from run_agent import AIAgent  # type: ignore[import-not-found]

            # Load model/provider from ~/.hermes/config.yaml, then resolve
            # credentials the same way hermes-agent's own CLI does. Without
            # this AIAgent is constructed with model="" and the upstream
            # API rejects the request ("model code cannot be empty").
            cfg = load_config()
            # load_config() expands `model: <name>` into a dict
            # {"default": <name>, "provider": <provider>}. Handle both shapes.
            raw_model = cfg.get("model")
            if isinstance(raw_model, dict):
                model = str(raw_model.get("default") or "")
                requested_provider = raw_model.get("provider") or cfg.get("provider")
            else:
                model = str(raw_model or "")
                requested_provider = cfg.get("provider")
            runtime = resolve_runtime_provider(requested=requested_provider)
            self._agent = AIAgent(
                session_id=session_id,
                quiet_mode=True,
                model=model,
                provider=runtime.get("provider"),
                base_url=runtime.get("base_url"),
                api_key=runtime.get("api_key"),
                api_mode=runtime.get("api_mode"),
            )
        except Exception as exc:  # pragma: no cover — env-dependent
            self._init_error = f"{exc.__class__.__name__}: {exc}"
            log.warning("hermes AIAgent init failed: %s", self._init_error)

    async def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]:
        if self._agent is None:
            yield {
                "kind": "final",
                "id": str(uuid.uuid4()),
                "text": (
                    "hermes_bridge could not start the hermes-agent runtime. "
                    f"Init error: {self._init_error or 'unknown'}. "
                    "Run `uv run --project backend python backend/scripts/init_config.py` "
                    "and ensure a provider API key is set in ~/.hermes/.env."
                ),
            }
            return

        def _blocking_chat() -> str:
            return self._agent.chat(user_content)  # type: ignore[union-attr]

        try:
            text = await anyio.to_thread.run_sync(_blocking_chat)
        except Exception as exc:
            raise RuntimeError(f"hermes chat failed: {exc}") from exc

        yield {
            "kind": "final",
            "id": str(uuid.uuid4()),
            "text": text,
        }


def make_real_runner(settings: Settings, session_id: str) -> HermesRunner:
    agent: HermesAgentLike = _HermesAgentAdapter(settings, session_id)
    return HermesRunner(agent=agent, session_id=session_id)
