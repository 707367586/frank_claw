from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from hermes_bridge.bridge.agent_store import Agent, AgentStore
from hermes_bridge.config import Settings


def _settings(tmp_path: Path) -> Settings:
    # Settings's HERMES_HOME alias makes Pyright unable to see the field-name kwarg
    # even with populate_by_name=True; runtime accepts it.
    return Settings(hermes_home=tmp_path)  # type: ignore[call-arg]


def test_list_returns_seeded_agents_when_file_missing(tmp_path):
    store = AgentStore(_settings(tmp_path))
    out = store.list()
    assert {a.id for a in out} == {"code", "research", "writing", "data"}
    # seed write occurred
    assert (tmp_path / "agents.json").exists()
    # workspace dirs are created lazily on first turn, NOT at seed time
    for sid in ("code", "research", "writing", "data"):
        assert not (tmp_path / "workspaces" / sid).exists()


def test_create_assigns_id_and_session_and_workspace(tmp_path):
    store = AgentStore(_settings(tmp_path))
    a = store.create(
        name="my agent",
        description="d",
        color="#5749F4",
        icon="Bot",
        system_prompt="be helpful",
        model=None,
        enabled_toolsets=["web"],
        workspace_dir=None,
    )
    assert isinstance(a, Agent)
    assert a.id and a.current_session_id
    assert a.workspace_dir == str(tmp_path / "workspaces" / a.id)
    assert (tmp_path / "workspaces" / a.id).is_dir()
    # round-trip on disk
    store2 = AgentStore(_settings(tmp_path))
    assert any(x.id == a.id for x in store2.list())


def test_create_respects_explicit_workspace(tmp_path):
    explicit = tmp_path / "elsewhere"
    store = AgentStore(_settings(tmp_path))
    a = store.create(
        name="x", description="", color="#5749F4", icon="Bot",
        system_prompt="p", model=None, enabled_toolsets=[],
        workspace_dir=str(explicit),
    )
    assert a.workspace_dir == str(explicit)
    assert explicit.is_dir()


def test_get_returns_none_for_unknown(tmp_path):
    store = AgentStore(_settings(tmp_path))
    assert store.get("nope") is None


def test_get_returns_agent_for_seed(tmp_path):
    store = AgentStore(_settings(tmp_path))
    a = store.get("code")
    assert a is not None
    assert a.name == "编程助手"


def test_delete_removes_from_json_but_leaves_workspace(tmp_path):
    store = AgentStore(_settings(tmp_path))
    a = store.create(
        name="x", description="", color="#5749F4", icon="Bot",
        system_prompt="p", model=None, enabled_toolsets=[], workspace_dir=None,
    )
    store.delete(a.id)
    assert store.get(a.id) is None
    assert (tmp_path / "workspaces" / a.id).is_dir()


def test_delete_unknown_is_noop(tmp_path):
    store = AgentStore(_settings(tmp_path))
    store.delete("nonexistent")  # should not raise


def test_rotate_session_changes_id_and_persists(tmp_path):
    store = AgentStore(_settings(tmp_path))
    before = store.get("code")
    assert before is not None
    new_sid = store.rotate_session("code")
    assert new_sid != before.current_session_id
    after = AgentStore(_settings(tmp_path)).get("code")
    assert after is not None
    assert after.current_session_id == new_sid


def test_rotate_unknown_raises_keyerror(tmp_path):
    store = AgentStore(_settings(tmp_path))
    with pytest.raises(KeyError):
        store.rotate_session("nope")


def test_atomic_write_does_not_corrupt_file_on_partial_failure(tmp_path, monkeypatch):
    store = AgentStore(_settings(tmp_path))
    store.list()  # write seeds
    original = (tmp_path / "agents.json").read_text()
    # Force os.replace to fail; the file on disk must remain valid.
    real_replace = os.replace

    def boom(*_a, **_kw):
        raise RuntimeError("disk full")

    monkeypatch.setattr(os, "replace", boom)
    with pytest.raises(RuntimeError):
        store.create(
            name="x", description="", color="#5749F4", icon="Bot",
            system_prompt="p", model=None, enabled_toolsets=[], workspace_dir=None,
        )
    monkeypatch.setattr(os, "replace", real_replace)
    assert (tmp_path / "agents.json").read_text() == original
    assert json.loads(original)
