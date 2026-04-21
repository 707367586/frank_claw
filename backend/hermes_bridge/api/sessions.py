from __future__ import annotations

from dataclasses import asdict

from fastapi import APIRouter, Depends, HTTPException, Query, Response, status

from ..auth import require_bearer_token
from ..bridge.session_store import SessionStore
from ..config import Settings


def _store_factory(settings: Settings) -> SessionStore:
    return SessionStore(settings)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/sessions", tags=["sessions"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_sessions(
        offset: int = Query(default=0, ge=0),
        limit: int = Query(default=50, gt=0, le=500),
    ):
        store = _store_factory(settings)
        return [asdict(s) for s in store.list(offset, limit)]

    @r.get("/{sid}", dependencies=[dep])
    def get_session(sid: str):
        store = _store_factory(settings)
        d = store.get(sid)
        if d is None:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "session not found")
        return {
            **{k: v for k, v in asdict(d).items() if k != "messages"},
            "messages": [asdict(m) for m in d.messages],
        }

    @r.delete("/{sid}", dependencies=[dep], status_code=status.HTTP_204_NO_CONTENT)
    def delete_session(sid: str) -> Response:
        _store_factory(settings).delete(sid)
        return Response(status_code=status.HTTP_204_NO_CONTENT)

    return r
