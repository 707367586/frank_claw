from fastapi.testclient import TestClient
from hermes_bridge.app import create_app


def test_create_app_returns_fastapi_with_health():
    app = create_app()
    client = TestClient(app)
    r = client.get("/healthz")
    assert r.status_code == 200
    assert r.json() == {"ok": True}
