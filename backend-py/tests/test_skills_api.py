from fastapi.testclient import TestClient

from hermes_bridge.api import skills as skills_api
from hermes_bridge.app import create_app
from hermes_bridge.bridge.skill_service import SkillInfo
from hermes_bridge.config import Settings


class FakeSvc:
    def __init__(self) -> None:
        self.calls_uninstall: list[str] = []

    def list(self):
        return [SkillInfo(name="a", description="hello", installed=True)]

    def uninstall(self, name):
        self.calls_uninstall.append(name)

    def install(self, name):
        if name == "bad":
            raise NotImplementedError()
        return SkillInfo(name=name, installed=True)


def _client(monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setattr(skills_api, "_svc_factory", lambda _s: FakeSvc())
    app = create_app(Settings())
    return TestClient(app)


def test_skills_list(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/skills", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    assert r.json() == {"skills": [{"name": "a", "description": "hello", "installed": True}]}


def test_skills_delete(monkeypatch):
    c = _client(monkeypatch)
    r = c.delete("/api/skills/a", headers={"Authorization": "Bearer t"})
    assert r.status_code == 204


def test_skills_install_not_implemented(monkeypatch):
    c = _client(monkeypatch)
    r = c.post(
        "/api/skills/install",
        json={"name": "bad"},
        headers={"Authorization": "Bearer t"},
    )
    assert r.status_code == 501


def test_skills_install_ok(monkeypatch):
    c = _client(monkeypatch)
    r = c.post(
        "/api/skills/install",
        json={"name": "good"},
        headers={"Authorization": "Bearer t"},
    )
    assert r.status_code == 200
    assert r.json()["name"] == "good"
