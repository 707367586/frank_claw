from __future__ import annotations

import hmac
from typing import Callable

from fastapi import Header, HTTPException, status

from .config import Settings


def require_bearer_token(settings: Settings) -> Callable[[str | None], None]:
    def _dep(authorization: str | None = Header(default=None)) -> None:
        expected = settings.launcher_token
        if not expected:
            raise HTTPException(status.HTTP_500_INTERNAL_SERVER_ERROR, "launcher token not configured")
        if not authorization or not authorization.startswith("Bearer "):
            raise HTTPException(status.HTTP_401_UNAUTHORIZED, "missing bearer token")
        token = authorization.removeprefix("Bearer ").strip()
        if not hmac.compare_digest(token, expected):
            raise HTTPException(status.HTTP_401_UNAUTHORIZED, "invalid token")

    return _dep


def verify_ws_subprotocol(subprotocols: list[str], settings: Settings) -> str | None:
    """Return the matching `token.<…>` subprotocol string if valid, else None.

    Browsers send the token as `token.<value>` in `Sec-WebSocket-Protocol`.
    The server must echo back the **same** string in the accept header, which
    FastAPI/Starlette handles if we call `websocket.accept(subprotocol=…)`.
    """
    expected = settings.launcher_token
    if not expected:
        return None
    prefix = "token."
    for sp in subprotocols:
        if sp.startswith(prefix) and hmac.compare_digest(sp.removeprefix(prefix), expected):
            return sp
    return None
