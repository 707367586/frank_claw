from __future__ import annotations

import sys
import types

from fastapi.testclient import TestClient

from hermes_bridge.api import toolsets as toolsets_api
from hermes_bridge.app import create_app
from hermes_bridge.config import Settings


def _client(monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    app = create_app(Settings())
    return TestClient(app)


def test_list_requires_auth(monkeypatch):
    c = _client(monkeypatch)
    assert c.get("/api/toolsets").status_code == 401


def test_list_returns_projection(monkeypatch):
    fake_module = types.ModuleType("toolsets")
    fake_module.TOOLSETS = {  # type: ignore[attr-defined]
        "web": {"description": "web tools", "tools": ["web_search"], "includes": []},
        "file": {"description": "file ops", "tools": ["read", "write"], "includes": []},
    }
    monkeypatch.setitem(sys.modules, "toolsets", fake_module)
    c = _client(monkeypatch)
    r = c.get("/api/toolsets", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert sorted(t["name"] for t in body) == ["file", "web"]
    web = next(t for t in body if t["name"] == "web")
    assert web["description"] == "web tools"
    assert web["tools"] == ["web_search"]


def test_list_falls_back_to_empty_when_import_fails(monkeypatch):
    # Force the import to raise
    def boom(*_a, **_kw):
        raise ImportError("hermes-agent missing")

    monkeypatch.setattr(toolsets_api, "_load_registry", boom)
    c = _client(monkeypatch)
    r = c.get("/api/toolsets", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    assert r.json() == []
