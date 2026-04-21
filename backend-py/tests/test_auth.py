from fastapi import APIRouter, Depends, FastAPI
from fastapi.testclient import TestClient

from hermes_bridge.auth import require_bearer_token
from hermes_bridge.config import Settings


def _mk(token: str, monkeypatch) -> TestClient:
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", token)
    s = Settings()
    app = FastAPI()
    r = APIRouter()

    @r.get("/whoami", dependencies=[Depends(require_bearer_token(s))])
    def whoami() -> dict[str, str]:
        return {"who": "ok"}

    app.include_router(r)
    return TestClient(app)


def test_missing_header_401(monkeypatch):
    c = _mk("xyz", monkeypatch)
    assert c.get("/whoami").status_code == 401


def test_wrong_token_401(monkeypatch):
    c = _mk("xyz", monkeypatch)
    assert c.get("/whoami", headers={"Authorization": "Bearer nope"}).status_code == 401


def test_right_token_200(monkeypatch):
    c = _mk("xyz", monkeypatch)
    r = c.get("/whoami", headers={"Authorization": "Bearer xyz"})
    assert r.status_code == 200
    assert r.json() == {"who": "ok"}
