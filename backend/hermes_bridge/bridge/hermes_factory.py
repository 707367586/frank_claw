# backend/hermes_bridge/bridge/hermes_factory.py
from __future__ import annotations

import logging
import os
import uuid
from typing import Any, AsyncIterator

import anyio

from ..config import Settings
from .agent_store import Agent, AgentStore
from .hermes_runner import HermesAgentLike, HermesRunner

log = logging.getLogger(__name__)

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
        if self._workspace_dir:
            from pathlib import Path
            Path(self._workspace_dir).mkdir(parents=True, exist_ok=True)
            os.environ["TERMINAL_CWD"] = self._workspace_dir

        def _blocking_chat() -> str:
            return self._llm.chat(user_content)  # type: ignore[union-attr]

        try:
            text = await anyio.to_thread.run_sync(_blocking_chat)
        except Exception as exc:
            raise RuntimeError(_redact_secrets(f"hermes chat failed: {exc}")) from exc
        finally:
            if self._workspace_dir:
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
    if agent_id:
        agent = AgentStore(settings).get(agent_id)
        if agent is None:
            log.warning("agent_id %s not found; falling back to global config", agent_id)
    adapter: HermesAgentLike = _HermesAgentAdapter(settings, session_id, agent=agent)
    return HermesRunner(agent=adapter, session_id=session_id)
