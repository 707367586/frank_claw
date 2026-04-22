"""info.check_configured must verify YAML + matching env var, not just file presence."""
from __future__ import annotations

import os
from pathlib import Path

import pytest

from hermes_bridge.api.info import check_configured
from hermes_bridge.config import Settings


def _settings(home: Path) -> Settings:
    return Settings(HERMES_HOME=home)


def test_no_config_file_returns_false(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("GLM_API_KEY", raising=False)
    assert check_configured(_settings(tmp_path)) is False


def test_config_without_env_var_returns_false(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "config.yaml").write_text("provider: zai\nmodel: glm-4.5-flash\n")
    monkeypatch.delenv("GLM_API_KEY", raising=False)
    monkeypatch.delenv("ZAI_API_KEY", raising=False)
    monkeypatch.delenv("Z_AI_API_KEY", raising=False)
    assert check_configured(_settings(tmp_path)) is False


def test_config_plus_env_var_returns_true(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "config.yaml").write_text("provider: zai\nmodel: glm-4.5-flash\n")
    monkeypatch.setenv("GLM_API_KEY", "anything-non-empty")
    assert check_configured(_settings(tmp_path)) is True


def test_unknown_provider_returns_false(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "config.yaml").write_text("provider: totally-made-up\nmodel: foo\n")
    assert check_configured(_settings(tmp_path)) is False


def test_anthropic_provider_reads_anthropic_key(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "config.yaml").write_text("provider: anthropic\nmodel: x\n")
    monkeypatch.setenv("ANTHROPIC_API_KEY", "anthropic-key")
    monkeypatch.delenv("GLM_API_KEY", raising=False)
    assert check_configured(_settings(tmp_path)) is True
