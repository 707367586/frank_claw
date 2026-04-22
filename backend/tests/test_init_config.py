"""init_config.py: interactive bootstrap for ~/.hermes/{config.yaml,.env}."""
from __future__ import annotations

import os
from pathlib import Path

import pytest


def _run_init(tmp_home: Path, answers: list[str], monkeypatch: pytest.MonkeyPatch) -> int:
    """Invoke init_config.main() with HERMES_HOME pointed at tmp and stdin fed `answers`."""
    import io
    import sys

    monkeypatch.setenv("HERMES_HOME", str(tmp_home))
    # Feed stdin. `getpass` reads from the tty; we monkeypatch it to use input().
    import getpass

    monkeypatch.setattr(getpass, "getpass", lambda prompt="": input(prompt))
    monkeypatch.setattr(sys, "stdin", io.StringIO("\n".join(answers) + "\n"))

    from importlib import reload

    import scripts.init_config as mod

    reload(mod)
    return mod.main()


def test_bootstrap_writes_zhipu_config_and_env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    home = tmp_path / ".hermes"

    # Answers: provider=1 (zai), model=<default>, key=paste
    rc = _run_init(home, answers=["1", "", "test-glm-key-not-real"], monkeypatch=monkeypatch)

    assert rc == 0
    cfg = (home / "config.yaml").read_text()
    assert "provider: zai" in cfg
    assert "model: glm-4.5-flash" in cfg  # default

    env = (home / ".env").read_text()
    assert "GLM_API_KEY=test-glm-key-not-real" in env

    # .env must be mode 0600 (owner-only).
    assert (home / ".env").stat().st_mode & 0o777 == 0o600


def test_bootstrap_custom_model(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    home = tmp_path / ".hermes"
    rc = _run_init(home, answers=["1", "glm-5.1", "test-glm-key-not-real"], monkeypatch=monkeypatch)
    assert rc == 0
    assert "model: glm-5.1" in (home / "config.yaml").read_text()


def test_bootstrap_is_idempotent_and_preserves_existing_key(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    home = tmp_path / ".hermes"
    home.mkdir()
    (home / ".env").write_text("GLM_API_KEY=already-here\n")
    (home / ".env").chmod(0o600)

    # Answers: provider=1 (zai), model=<default>, key=<blank = keep existing>
    rc = _run_init(home, answers=["1", "", ""], monkeypatch=monkeypatch)
    assert rc == 0

    env = (home / ".env").read_text()
    assert "GLM_API_KEY=already-here" in env  # unchanged
    assert env.count("GLM_API_KEY=") == 1  # no duplicate line
