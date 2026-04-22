from __future__ import annotations

import os
from pathlib import Path

import yaml
from fastapi import APIRouter, Depends
from pydantic import BaseModel

from ..auth import require_bearer_token
from ..config import Settings


class InfoResponse(BaseModel):
    configured: bool
    enabled: bool
    ws_url: str
    provider: str | None = None
    missing_env_var: str | None = None


# Known providers. Must match keys in hermes-agent's provider registry
# (hermes_cli/providers.py). Keep in sync with scripts/init_config.PROVIDERS.
_PROVIDER_ENV_VARS: dict[str, tuple[str, ...]] = {
    "zai": ("GLM_API_KEY", "ZAI_API_KEY", "Z_AI_API_KEY"),
    "anthropic": ("ANTHROPIC_API_KEY",),
    "openrouter": ("OPENROUTER_API_KEY",),
    "openai": ("OPENAI_API_KEY",),
    "deepseek": ("DEEPSEEK_API_KEY",),
}


def _load_provider(home: Path) -> str | None:
    for name in ("config.yaml", "config.yml"):
        path = home / name
        if not path.exists():
            continue
        try:
            doc = yaml.safe_load(path.read_text()) or {}
        except yaml.YAMLError:
            return None
        provider = doc.get("provider")
        return provider if isinstance(provider, str) else None
    return None


def check_configured(settings: Settings) -> bool:
    """True iff config.yaml picks a known provider AND its env var is set."""
    provider = _load_provider(settings.hermes_home)
    if provider is None:
        return False
    env_vars = _PROVIDER_ENV_VARS.get(provider)
    if not env_vars:
        return False
    return any(os.environ.get(v) for v in env_vars)


def _missing_env_var(settings: Settings) -> str | None:
    """For the selected provider, the canonical env var if none is set."""
    provider = _load_provider(settings.hermes_home)
    if provider is None:
        return None
    env_vars = _PROVIDER_ENV_VARS.get(provider, ())
    if not env_vars or any(os.environ.get(v) for v in env_vars):
        return None
    return env_vars[0]  # canonical


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/hermes", tags=["info"])
    dep = Depends(require_bearer_token(settings))

    @r.get("/info", response_model=InfoResponse, dependencies=[dep])
    def get_info() -> InfoResponse:
        provider = _load_provider(settings.hermes_home)
        configured = check_configured(settings)
        return InfoResponse(
            configured=configured,
            enabled=configured,
            ws_url=f"ws://{settings.host}:{settings.port}/hermes/ws",
            provider=provider,
            missing_env_var=_missing_env_var(settings) if not configured else None,
        )

    return r
