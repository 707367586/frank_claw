from __future__ import annotations

from dataclasses import asdict

from fastapi import APIRouter, Depends, HTTPException, Response, status
from pydantic import BaseModel

from ..auth import require_bearer_token
from ..bridge.tool_service import ToolService
from ..config import Settings


class ToggleBody(BaseModel):
    enabled: bool


def _svc_factory(settings: Settings) -> ToolService:
    return ToolService(settings)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/tools", tags=["tools"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_tools():
        svc = _svc_factory(settings)
        return {"tools": [asdict(t) for t in svc.list()]}

    @r.put("/{name}/state", dependencies=[dep], status_code=status.HTTP_204_NO_CONTENT)
    def toggle(name: str, body: ToggleBody) -> Response:
        svc = _svc_factory(settings)
        try:
            svc.set_enabled(name, body.enabled)
        except KeyError:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "tool not found")
        except ValueError as e:
            raise HTTPException(status.HTTP_409_CONFLICT, str(e))
        return Response(status_code=status.HTTP_204_NO_CONTENT)

    return r
