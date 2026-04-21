from __future__ import annotations

from dataclasses import asdict

from fastapi import APIRouter, Depends, HTTPException, Response, status
from pydantic import BaseModel

from ..auth import require_bearer_token
from ..bridge.skill_service import SkillService
from ..config import Settings


class InstallRequest(BaseModel):
    name: str


def _svc_factory(settings: Settings) -> SkillService:
    return SkillService(settings)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/skills", tags=["skills"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_skills():
        svc = _svc_factory(settings)
        return {"skills": [asdict(s) for s in svc.list()]}

    @r.post("/install", dependencies=[dep])
    def install(req: InstallRequest):
        svc = _svc_factory(settings)
        try:
            info = svc.install(req.name)
        except NotImplementedError:
            raise HTTPException(status.HTTP_501_NOT_IMPLEMENTED, "install not implemented")
        return asdict(info)

    @r.delete("/{name}", dependencies=[dep], status_code=status.HTTP_204_NO_CONTENT)
    def uninstall(name: str) -> Response:
        svc = _svc_factory(settings)
        try:
            svc.uninstall(name)
        except FileNotFoundError:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "skill not found")
        return Response(status_code=status.HTTP_204_NO_CONTENT)

    return r
