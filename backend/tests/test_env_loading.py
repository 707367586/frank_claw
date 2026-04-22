"""hermes_bridge must load ~/.hermes/.env into os.environ before the FastAPI
app is constructed, so that AIAgent sees provider credentials."""
from __future__ import annotations

import os
from pathlib import Path

import pytest


def test_load_hermes_env_populates_os_environ(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    # Arrange: a fake HERMES_HOME with a .env containing one key.
    home = tmp_path / ".hermes"
    home.mkdir()
    (home / ".env").write_text("GLM_API_KEY=test-glm-key-not-real\n")
    monkeypatch.setenv("HERMES_HOME", str(home))
    monkeypatch.delenv("GLM_API_KEY", raising=False)

    # Act: call the loader that __main__ will use.
    from hermes_bridge.config import load_hermes_env

    load_hermes_env()

    # Assert
    assert os.environ.get("GLM_API_KEY") == "test-glm-key-not-real"


def test_load_hermes_env_does_not_overwrite_existing(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    home = tmp_path / ".hermes"
    home.mkdir()
    (home / ".env").write_text("GLM_API_KEY=from-dotfile\n")
    monkeypatch.setenv("HERMES_HOME", str(home))
    monkeypatch.setenv("GLM_API_KEY", "from-shell")

    from hermes_bridge.config import load_hermes_env

    load_hermes_env()

    # Shell env wins (principle of least surprise for dev overrides).
    assert os.environ["GLM_API_KEY"] == "from-shell"


def test_load_hermes_env_is_silent_when_file_missing(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    home = tmp_path / ".hermes"
    home.mkdir()  # exists but no .env inside
    monkeypatch.setenv("HERMES_HOME", str(home))

    from hermes_bridge.config import load_hermes_env

    # Must not raise.
    load_hermes_env()
