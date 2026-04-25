from __future__ import annotations

import dataclasses

from fastapi.testclient import TestClient

from hermes_bridge.api import agents as agents_api
from hermes_bridge.app import create_app
from hermes_bridge.bridge.agent_store import Agent
from hermes_bridge.config import Settings


def _agent(**overrides) -> Agent:
    base = dict(
        id="aid",
        name="n",
        description="d",
        color="#5749F4",
        icon="Bot",
        system_prompt="p",
        model=None,
        enabled_toolsets=[],
        workspace_dir="/tmp/aid",
        current_session_id="sid",
        created_at=1,
    )
    base.update(overrides)
    return Agent(**base)


class FakeStore:
    def __init__(self):
        self.agents: list[Agent] = [_agent()]
        self.create_calls: list[dict] = []
        self.delete_calls: list[str] = []
        self.rotate_calls: list[str] = []

    def list(self):
        return list(self.agents)

    def get(self, aid):
        for a in self.agents:
            if a.id == aid:
                return a
        return None

    def create(self, **kw):
        self.create_calls.append(kw)
        a = _agent(id="new", name=kw["name"])
        self.agents.append(a)
        return a

    def delete(self, aid):
        self.delete_calls.append(aid)
        self.agents = [a for a in self.agents if a.id != aid]

    def rotate_session(self, aid):
        self.rotate_calls.append(aid)
        if not any(a.id == aid for a in self.agents):
            raise KeyError(aid)
        return "rotated-sid"


def _client(monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    fake = FakeStore()
    monkeypatch.setattr(agents_api, "_store_factory", lambda _s: fake)
    app = create_app(Settings())
    return TestClient(app), fake


def test_list_requires_auth(monkeypatch):
    c, _ = _client(monkeypatch)
    assert c.get("/api/agents").status_code == 401


def test_list_returns_agents(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.get("/api/agents", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert isinstance(body, list)
    assert body[0]["id"] == "aid"
    assert body[0]["color"] == "#5749F4"


def test_create_201(monkeypatch):
    c, fake = _client(monkeypatch)
    r = c.post(
        "/api/agents",
        headers={"Authorization": "Bearer t"},
        json={
            "name": "X",
            "description": "",
            "color": "#5749F4",
            "icon": "Bot",
            "system_prompt": "p",
            "model": None,
            "enabled_toolsets": ["web"],
        },
    )
    assert r.status_code == 201
    assert r.json()["id"] == "new"
    assert fake.create_calls[0]["name"] == "X"
    assert fake.create_calls[0]["enabled_toolsets"] == ["web"]
    assert fake.create_calls[0]["workspace_dir"] is None


def test_create_validates_name(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.post(
        "/api/agents",
        headers={"Authorization": "Bearer t"},
        json={"name": "", "description": "", "color": "#5749F4", "icon": "Bot",
              "system_prompt": "p", "model": None, "enabled_toolsets": []},
    )
    assert r.status_code == 422


def test_create_validates_color(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.post(
        "/api/agents",
        headers={"Authorization": "Bearer t"},
        json={"name": "x", "description": "", "color": "blue", "icon": "Bot",
              "system_prompt": "p", "model": None, "enabled_toolsets": []},
    )
    assert r.status_code == 422


def test_delete_204(monkeypatch):
    c, fake = _client(monkeypatch)
    r = c.delete("/api/agents/aid", headers={"Authorization": "Bearer t"})
    assert r.status_code == 204
    assert fake.delete_calls == ["aid"]


def test_delete_404_when_missing(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.delete("/api/agents/nope", headers={"Authorization": "Bearer t"})
    assert r.status_code == 404


def test_rotate_session_returns_new_sid(monkeypatch):
    c, fake = _client(monkeypatch)
    r = c.post("/api/agents/aid/sessions", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    assert r.json() == {"session_id": "rotated-sid"}
    assert fake.rotate_calls == ["aid"]


def test_rotate_session_404(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.post("/api/agents/nope/sessions", headers={"Authorization": "Bearer t"})
    assert r.status_code == 404
