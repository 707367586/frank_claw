from fastapi.testclient import TestClient

from hermes_bridge.api import tools as tools_api
from hermes_bridge.app import create_app
from hermes_bridge.bridge.tool_service import ToolInfo
from hermes_bridge.config import Settings


class FakeSvc:
    data = [
        ToolInfo(name="fs_read", status="enabled"),
        ToolInfo(name="internet", status="blocked", reason_code="no api key"),
    ]
    toggled: list[tuple[str, bool]] = []

    def list(self):
        return self.data

    def set_enabled(self, name, enabled):
        if name == "missing":
            raise KeyError(name)
        if name == "internet":
            raise ValueError("blocked")
        self.toggled.append((name, enabled))


def _client(monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setattr(tools_api, "_svc_factory", lambda _s: FakeSvc())
    app = create_app(Settings())
    return TestClient(app)


def test_tools_list(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/tools", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert body == {
        "tools": [
            {
                "name": "fs_read",
                "status": "enabled",
                "description": None,
                "category": None,
                "config_key": None,
                "reason_code": None,
            },
            {
                "name": "internet",
                "status": "blocked",
                "description": None,
                "category": None,
                "config_key": None,
                "reason_code": "no api key",
            },
        ]
    }


def test_tools_set_enabled_ok(monkeypatch):
    c = _client(monkeypatch)
    r = c.put("/api/tools/fs_read/state", json={"enabled": False}, headers={"Authorization": "Bearer t"})
    assert r.status_code == 204


def test_tools_set_enabled_unknown_404(monkeypatch):
    c = _client(monkeypatch)
    r = c.put("/api/tools/missing/state", json={"enabled": True}, headers={"Authorization": "Bearer t"})
    assert r.status_code == 404


def test_tools_set_enabled_blocked_409(monkeypatch):
    c = _client(monkeypatch)
    r = c.put("/api/tools/internet/state", json={"enabled": True}, headers={"Authorization": "Bearer t"})
    assert r.status_code == 409
