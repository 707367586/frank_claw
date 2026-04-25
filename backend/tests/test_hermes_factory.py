import os

from hermes_bridge.bridge import hermes_factory as hf
from hermes_bridge.bridge.agent_store import Agent
from hermes_bridge.bridge.hermes_factory import _HermesAgentAdapter, make_real_runner
from hermes_bridge.config import Settings


class _FakeAIAgent:
    last_kwargs: dict | None = None
    last_cwd: str | None = None

    def __init__(self, **kwargs):
        type(self).last_kwargs = kwargs

    def chat(self, content):
        type(self).last_cwd = os.environ.get("TERMINAL_CWD")
        return f"echo:{content}"


def _patch_aiagent(monkeypatch):
    monkeypatch.setattr(hf, "_resolve_aiagent_class", lambda: _FakeAIAgent)
    monkeypatch.setattr(hf, "_resolve_runtime_kwargs", lambda settings: {"model": "global-model"})


def test_make_real_runner_returns_runner_with_session(tmp_path, monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    s = Settings()
    r = make_real_runner(s, session_id="sess-123")
    assert r.session_id == "sess-123"


def test_adapter_passes_persona_kwargs(monkeypatch, tmp_path):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    _patch_aiagent(monkeypatch)
    s = Settings()
    a = Agent(
        id="x", name="n", description="d", color="#5749F4", icon="Bot",
        system_prompt="be terse", model="custom-model",
        enabled_toolsets=["web"], workspace_dir=str(tmp_path / "ws-x"),
        current_session_id="sid", created_at=1,
    )
    _HermesAgentAdapter(s, "sid", agent=a)
    kw = _FakeAIAgent.last_kwargs or {}
    assert kw["model"] == "custom-model"
    assert kw["ephemeral_system_prompt"] == "be terse"
    assert kw["enabled_toolsets"] == ["web"]


def test_adapter_uses_global_model_when_agent_model_none(monkeypatch, tmp_path):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    _patch_aiagent(monkeypatch)
    s = Settings()
    a = Agent(
        id="x", name="n", description="d", color="#5749F4", icon="Bot",
        system_prompt="p", model=None,
        enabled_toolsets=[], workspace_dir=str(tmp_path / "ws-x"),
        current_session_id="sid", created_at=1,
    )
    _HermesAgentAdapter(s, "sid", agent=a)
    kw = _FakeAIAgent.last_kwargs or {}
    assert kw["model"] == "global-model"
    # empty list maps to None so hermes uses defaults
    assert kw["enabled_toolsets"] is None


def test_make_real_runner_with_unknown_agent_id_falls_back(monkeypatch, tmp_path):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    _patch_aiagent(monkeypatch)
    s = Settings()
    r = make_real_runner(s, session_id="sid", agent_id="missing")
    assert r.session_id == "sid"
    # confirm fallback: AIAgent built without persona kwargs
    kw = _FakeAIAgent.last_kwargs or {}
    assert "ephemeral_system_prompt" not in kw
    assert "enabled_toolsets" not in kw


def test_terminal_cwd_swapped_during_chat_and_restored(monkeypatch, tmp_path):
    import anyio
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    monkeypatch.setenv("TERMINAL_CWD", "/original")
    _patch_aiagent(monkeypatch)
    s = Settings()
    workspace = tmp_path / "ws-y"
    a = Agent(
        id="y", name="n", description="d", color="#5749F4", icon="Bot",
        system_prompt="p", model=None, enabled_toolsets=[],
        workspace_dir=str(workspace), current_session_id="sid", created_at=1,
    )
    adapter = _HermesAgentAdapter(s, "sid", agent=a)

    async def drain():
        async for _ in adapter.run_turn("hello"):
            pass

    anyio.run(drain)
    assert _FakeAIAgent.last_cwd == str(workspace)
    assert os.environ.get("TERMINAL_CWD") == "/original"
    assert workspace.is_dir()


def test_terminal_cwd_unset_before_and_after_chat(monkeypatch, tmp_path):
    import anyio
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    # Ensure TERMINAL_CWD is NOT in the environment before the test
    monkeypatch.delenv("TERMINAL_CWD", raising=False)
    _patch_aiagent(monkeypatch)
    s = Settings()
    workspace = tmp_path / "ws-z"
    a = Agent(
        id="z", name="n", description="d", color="#5749F4", icon="Bot",
        system_prompt="p", model=None, enabled_toolsets=[],
        workspace_dir=str(workspace), current_session_id="sid", created_at=1,
    )
    adapter = _HermesAgentAdapter(s, "sid", agent=a)

    async def drain():
        async for _ in adapter.run_turn("hello"):
            pass

    anyio.run(drain)
    # During chat: env was set
    assert _FakeAIAgent.last_cwd == str(workspace)
    # After chat: env is unset again (since it wasn't there before)
    assert "TERMINAL_CWD" not in os.environ
