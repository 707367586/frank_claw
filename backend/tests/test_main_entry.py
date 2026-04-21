from pathlib import Path

from hermes_bridge.__main__ import _ensure_token
from hermes_bridge.config import Settings


def test_ensure_token_uses_env_when_set(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "env-token")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    s = Settings()
    assert _ensure_token(s) == "env-token"


def test_ensure_token_generates_and_persists(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.delenv("HERMES_LAUNCHER_TOKEN", raising=False)
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    s = Settings()
    t1 = _ensure_token(s)
    assert t1
    assert (tmp_path / "launcher-token").read_text().strip() == t1
    # Idempotent: second call returns the same
    s2 = Settings()
    t2 = _ensure_token(s2)
    assert t1 == t2
