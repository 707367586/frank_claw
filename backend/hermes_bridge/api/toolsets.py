from __future__ import annotations

import logging

from fastapi import APIRouter, Depends

from ..auth import require_bearer_token
from ..config import Settings

log = logging.getLogger(__name__)


def _load_registry() -> dict:
    """Imported lazily so hermes_bridge can boot even when hermes-agent is unavailable."""
    from toolsets import TOOLSETS  # type: ignore[import-not-found]
    return TOOLSETS


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/toolsets", tags=["toolsets"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_toolsets():
        try:
            registry = _load_registry()
        except Exception as exc:  # noqa: BLE001 — we want any import failure to degrade gracefully
            log.warning("toolset registry import failed: %s", exc)
            return []
        out = []
        for name, info in registry.items():
            out.append({
                "name": name,
                "description": info.get("description", "") or "",
                "tools": list(info.get("tools", []) or []),
            })
        out.sort(key=lambda t: t["name"])
        return out

    return r
