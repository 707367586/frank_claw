from __future__ import annotations

from dataclasses import asdict

from fastapi import APIRouter, Depends, HTTPException, Response, status
from pydantic import BaseModel, Field, constr

from ..auth import require_bearer_token
from ..bridge.agent_store import AgentStore
from ..config import Settings


HEX_COLOR = r"^#[0-9A-Fa-f]{6}$"


class AgentCreate(BaseModel):
    name: constr(min_length=1, max_length=64)  # type: ignore[valid-type]
    description: str = ""
    color: str = Field(pattern=HEX_COLOR)
    icon: constr(min_length=1, max_length=64)  # type: ignore[valid-type]
    system_prompt: constr(min_length=1)  # type: ignore[valid-type]
    model: str | None = None
    enabled_toolsets: list[str] = Field(default_factory=list)
    workspace_dir: str | None = None


def _store_factory(settings: Settings) -> AgentStore:
    return AgentStore(settings)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/agents", tags=["agents"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_agents():
        store = _store_factory(settings)
        return [asdict(a) for a in store.list()]

    @r.post("", dependencies=[dep], status_code=status.HTTP_201_CREATED)
    def create_agent(body: AgentCreate):
        store = _store_factory(settings)
        a = store.create(
            name=body.name,
            description=body.description,
            color=body.color,
            icon=body.icon,
            system_prompt=body.system_prompt,
            model=body.model,
            enabled_toolsets=body.enabled_toolsets,
            workspace_dir=body.workspace_dir,
        )
        return asdict(a)

    @r.delete("/{aid}", dependencies=[dep], status_code=status.HTTP_204_NO_CONTENT)
    def delete_agent(aid: str) -> Response:
        store = _store_factory(settings)
        if store.get(aid) is None:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "agent not found")
        store.delete(aid)
        return Response(status_code=status.HTTP_204_NO_CONTENT)

    @r.post("/{aid}/sessions", dependencies=[dep])
    def rotate_session(aid: str):
        store = _store_factory(settings)
        try:
            sid = store.rotate_session(aid)
        except KeyError:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "agent not found")
        return {"session_id": sid}

    return r
