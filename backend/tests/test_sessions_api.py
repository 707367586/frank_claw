from fastapi.testclient import TestClient

from hermes_bridge.api import sessions as sessions_api
from hermes_bridge.app import create_app
from hermes_bridge.bridge.session_store import (
    SessionDetail,
    SessionMessage,
    SessionSummary,
)
from hermes_bridge.config import Settings


class FakeStore:
    def list(self, offset, limit):
        return [
            SessionSummary(
                id="s1",
                title="t",
                preview="p",
                message_count=2,
                created=1,
                updated=2,
            )
        ]

    def get(self, sid):
        if sid != "s1":
            return None
        return SessionDetail(
            id="s1",
            title="t",
            preview="p",
            message_count=1,
            created=1,
            updated=2,
            messages=[SessionMessage(role="user", content="hi")],
            summary="",
        )

    def delete(self, sid):
        pass


def _client(monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setattr(sessions_api, "_store_factory", lambda _s: FakeStore())
    app = create_app(Settings())
    return TestClient(app)


def test_list_sessions_requires_auth(monkeypatch):
    c = _client(monkeypatch)
    assert c.get("/api/sessions").status_code == 401


def test_list_sessions_ok(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/sessions", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert isinstance(body, list)
    assert body[0]["id"] == "s1"


def test_get_session_404(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/sessions/nope", headers={"Authorization": "Bearer t"})
    assert r.status_code == 404


def test_get_session_ok(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/sessions/s1", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    assert r.json()["messages"][0]["role"] == "user"


def test_delete_session_204(monkeypatch):
    c = _client(monkeypatch)
    r = c.delete("/api/sessions/s1", headers={"Authorization": "Bearer t"})
    assert r.status_code == 204
