from hermes_bridge.bridge.hermes_factory import make_real_runner
from hermes_bridge.config import Settings


def test_make_real_runner_returns_runner_with_session(tmp_path, monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    s = Settings()
    r = make_real_runner(s, session_id="sess-123")
    assert r.session_id == "sess-123"
