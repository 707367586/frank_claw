from fastapi.testclient import TestClient

from hermes_bridge.app import create_app
from hermes_bridge.config import Settings


def test_info_requires_bearer(monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    app = create_app(Settings())
    c = TestClient(app)
    assert c.get("/api/hermes/info").status_code == 401


def test_info_returns_shape(monkeypatch):
    monkeypatch.setattr(
        "hermes_bridge.api.info.check_configured", lambda _s: True
    )
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    app = create_app(Settings())
    c = TestClient(app)
    r = c.get("/api/hermes/info", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert set(body.keys()) == {
        "configured",
        "enabled",
        "ws_url",
        "provider",
        "missing_env_var",
    }
    assert body["configured"] is True
    assert body["enabled"] is True
    assert body["ws_url"] == "ws://127.0.0.1:18800/hermes/ws"
