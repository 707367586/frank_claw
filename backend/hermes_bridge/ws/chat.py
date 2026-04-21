from __future__ import annotations

import logging
import time
from typing import Callable

from fastapi import APIRouter, Query, WebSocket, WebSocketDisconnect, status

from ..auth import verify_ws_subprotocol
from ..bridge.hermes_runner import HermesRunner, RunnerEvent
from ..config import Settings
from .protocol import HermesMessage

log = logging.getLogger(__name__)


# Replaced with a real factory in Task 3.5; tests monkey-patch this.
def make_runner(session_id: str) -> HermesRunner:
    raise RuntimeError(
        "make_runner not configured; override via monkeypatch in tests or call "
        "hermes_bridge.ws.chat.bind_runner_factory(...) at startup"
    )


def bind_runner_factory(factory: Callable[[str], HermesRunner]) -> None:
    global make_runner
    make_runner = factory  # type: ignore[assignment]


def _to_wire(ev: RunnerEvent) -> dict[str, object]:
    if ev.kind == "typing_start":
        return {"type": "typing.start", "timestamp": int(time.time() * 1000)}
    if ev.kind == "typing_stop":
        return {"type": "typing.stop", "timestamp": int(time.time() * 1000)}
    if ev.kind == "message_create":
        payload: dict[str, object] = {
            "message_id": ev.message_id or "",
            "content": ev.content or "",
        }
        if ev.thought is not None:
            payload["thought"] = ev.thought
        return {
            "type": "message.create",
            "timestamp": int(time.time() * 1000),
            "payload": payload,
        }
    if ev.kind == "error":
        return {
            "type": "error",
            "timestamp": int(time.time() * 1000),
            "payload": {
                "code": ev.code or "error",
                "message": ev.message or "",
            },
        }
    raise ValueError(f"unknown runner event kind: {ev.kind}")


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter()

    @r.websocket("/hermes/ws")
    async def ws_chat(websocket: WebSocket, session_id: str = Query(...)) -> None:
        requested = list(websocket.scope.get("subprotocols") or [])
        matched = verify_ws_subprotocol(requested, settings)
        if not matched:
            await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
            return
        await websocket.accept(subprotocol=matched)

        runner = make_runner(session_id)
        try:
            while True:
                raw = await websocket.receive_json()
                try:
                    msg = HermesMessage.model_validate(raw)
                except Exception as exc:
                    await websocket.send_json(
                        {
                            "type": "error",
                            "payload": {
                                "code": "bad_frame",
                                "message": str(exc),
                            },
                        }
                    )
                    continue

                if msg.type == "ping":
                    await websocket.send_json({"type": "pong", "id": msg.id})
                    continue

                if msg.type == "message.send":
                    content = (msg.payload or {}).get("content", "")
                    if not isinstance(content, str) or not content:
                        await websocket.send_json(
                            {
                                "type": "error",
                                "payload": {
                                    "code": "bad_input",
                                    "message": "content must be non-empty string",
                                    "request_id": msg.id,
                                },
                            }
                        )
                        continue
                    async for ev in runner.run_turn(content):
                        await websocket.send_json(_to_wire(ev))
                    continue

                # media.send — not implemented this phase
                await websocket.send_json(
                    {
                        "type": "error",
                        "payload": {
                            "code": "not_implemented",
                            "message": f"type {msg.type} not implemented",
                            "request_id": msg.id,
                        },
                    }
                )
        except WebSocketDisconnect:
            log.info("ws disconnect session=%s", session_id)

    return r
