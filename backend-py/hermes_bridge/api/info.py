from __future__ import annotations

from fastapi import APIRouter, Depends
from pydantic import BaseModel

from ..auth import require_bearer_token
from ..config import Settings


class InfoResponse(BaseModel):
    configured: bool
    enabled: bool
    ws_url: str


def check_configured(settings: Settings) -> bool:
    """Return True iff hermes has at least one usable LLM adapter configured.

    Implementation approach: check for presence of ~/.hermes/config.yaml (or
    whatever hermes's canonical config file is; verify with a grep against the
    cloned hermes-agent repo during Task 3.1) AND that at least one provider
    API key is readable. For the scaffold phase, fall back to filesystem check.
    """
    cfg_candidates = [
        settings.hermes_home / "config.yaml",
        settings.hermes_home / "config.yml",
        settings.hermes_home / "config.json",
    ]
    return any(p.exists() for p in cfg_candidates)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/hermes", tags=["info"])
    dep = Depends(require_bearer_token(settings))

    @r.get("/info", response_model=InfoResponse, dependencies=[dep])
    def get_info() -> InfoResponse:
        configured = check_configured(settings)
        ws_url = f"ws://{settings.host}:{settings.port}/hermes/ws"
        return InfoResponse(configured=configured, enabled=configured, ws_url=ws_url)

    return r
