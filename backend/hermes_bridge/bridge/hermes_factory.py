"""The ONLY file in hermes_bridge that imports hermes-agent's internals.

On upstream version bumps, this file absorbs the change. All other code
depends on `HermesRunner` + `HermesAgentLike` protocol which is stable.

T5 (persona-agents) added two test seams here:
- `_resolve_aiagent_class()` — lazy `from run_agent import AIAgent`.
- `_resolve_runtime_kwargs(settings)` — reads `~/.hermes/config.yaml` and
  resolves provider/api_key via hermes_cli.runtime_provider.

Per-agent persona is injected by passing an `Agent` record into the adapter's
constructor: `model` overrides the global, `system_prompt` becomes
`ephemeral_system_prompt`, and `enabled_toolsets` becomes the AIAgent
whitelist (empty list → None so hermes uses defaults). The agent's
`workspace_dir` is set into `os.environ["TERMINAL_CWD"]` for the duration
of each turn — this is process-global, so v1 only supports one in-flight
turn at a time (see spec §7).
"""

from __future__ import annotations

import logging
import os
import uuid
from pathlib import Path
from typing import Any, AsyncIterator

import anyio

from ..config import Settings
from .agent_store import Agent, AgentStore
from .hermes_runner import HermesAgentLike, HermesRunner

log = logging.getLogger(__name__)


# Env vars whose values must never appear in any string surfaced to the
# client (error messages, chat transcripts). Keep in sync with
# hermes_bridge.api.info._PROVIDER_ENV_VARS. httpx and some SDKs occasionally
# echo the Authorization header into exception strings — this is the last
# line of defense.
_SECRET_ENV_VARS = (
    "GLM_API_KEY", "ZAI_API_KEY", "Z_AI_API_KEY",
    "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY",
    "OPENAI_API_KEY", "DEEPSEEK_API_KEY",
)


def _redact_secrets(s: str) -> str:
    for name in _SECRET_ENV_VARS:
        v = os.environ.get(name)
        if v and v in s:
            s = s.replace(v, "***")
    return s


def _resolve_aiagent_class():
    """Seam for tests; production path imports the real AIAgent."""
    from run_agent import AIAgent  # type: ignore[import-not-found]
    return AIAgent


def _resolve_runtime_kwargs(settings: Settings) -> dict[str, Any]:
    """Reads ~/.hermes/config.yaml + provider runtime; returns a dict of kwargs
    suitable for AIAgent(**kwargs). Tests stub this out.
    """
    from hermes_cli.config import load_config  # type: ignore[import-not-found]
    from hermes_cli.runtime_provider import resolve_runtime_provider  # type: ignore[import-not-found]

    cfg = load_config()
    raw_model = cfg.get("model")
    if isinstance(raw_model, dict):
        model = str(raw_model.get("default") or "")
        requested_provider = raw_model.get("provider") or cfg.get("provider")
    elif isinstance(raw_model, str) or raw_model is None:
        model = raw_model or ""
        requested_provider = cfg.get("provider")
    else:
        raise TypeError(
            f"config.yaml `model` must be str or dict, got {type(raw_model).__name__}"
        )
    runtime = resolve_runtime_provider(requested=requested_provider)
    return {
        "model": model,
        "provider": runtime.get("provider"),
        "base_url": runtime.get("base_url"),
        "api_key": runtime.get("api_key"),
        "api_mode": runtime.get("api_mode"),
    }


class _HermesAgentAdapter:
    """Adapts hermes-agent's sync AIAgent.chat() into our async iterator API.

    Behaviour:
      - When `agent` is None: build AIAgent from the global config; no
        TERMINAL_CWD swap.
      - When `agent` is given: override `model` (if non-empty), inject
        `ephemeral_system_prompt`, and apply `enabled_toolsets` whitelist
        (empty list → None). Each turn temporarily sets
        `TERMINAL_CWD=agent.workspace_dir` and restores it in `finally`.
    """

    def __init__(
        self,
        settings: Settings,
        session_id: str,
        agent: Agent | None = None,
    ) -> None:
        self._settings = settings
        self._session_id = session_id
        self._agent_record = agent
        self._workspace_dir: str | None = agent.workspace_dir if agent else None
        self._llm = None
        self._init_error: str | None = None
        try:
            AIAgent = _resolve_aiagent_class()
            runtime_kwargs = _resolve_runtime_kwargs(settings)
            kwargs: dict[str, Any] = {
                "session_id": session_id,
                "quiet_mode": True,
                **runtime_kwargs,
            }
            if agent is not None:
                if agent.model:
                    kwargs["model"] = agent.model
                if agent.system_prompt:
                    kwargs["ephemeral_system_prompt"] = agent.system_prompt
                kwargs["enabled_toolsets"] = list(agent.enabled_toolsets) or None
            self._llm = AIAgent(**kwargs)
        except Exception as exc:  # pragma: no cover — env-dependent
            self._init_error = _redact_secrets(f"{exc.__class__.__name__}: {exc}")
            log.warning("hermes AIAgent init failed: %s", self._init_error)

    async def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]:
        if self._llm is None:
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

        prev_cwd = os.environ.get("TERMINAL_CWD")
        cwd_changed = False

        def _blocking_chat() -> str:
            return self._llm.chat(user_content)  # type: ignore[union-attr]

        try:
            if self._workspace_dir:
                Path(self._workspace_dir).mkdir(parents=True, exist_ok=True)
                os.environ["TERMINAL_CWD"] = self._workspace_dir
                cwd_changed = True
            try:
                text = await anyio.to_thread.run_sync(_blocking_chat)
            except Exception as exc:
                raise RuntimeError(_redact_secrets(f"hermes chat failed: {exc}")) from exc
        finally:
            if cwd_changed:
                if prev_cwd is None:
                    os.environ.pop("TERMINAL_CWD", None)
                else:
                    os.environ["TERMINAL_CWD"] = prev_cwd

        yield {
            "kind": "final",
            "id": str(uuid.uuid4()),
            "text": text,
        }


def make_real_runner(
    settings: Settings,
    session_id: str,
    agent_id: str | None = None,
) -> HermesRunner:
    agent: Agent | None = None
    if agent_id is not None:
        if not agent_id:
            log.warning("agent_id is empty string; falling back to global config")
        else:
            agent = AgentStore(settings).get(agent_id)
            if agent is None:
                log.warning("agent_id %r not found; falling back to global config", agent_id)
    adapter: HermesAgentLike = _HermesAgentAdapter(settings, session_id, agent=agent)
    return HermesRunner(agent=adapter, session_id=session_id)
