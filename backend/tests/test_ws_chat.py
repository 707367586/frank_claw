from __future__ import annotations

from typing import Any, AsyncIterator

import pytest
from fastapi.testclient import TestClient

from hermes_bridge.app import create_app
from hermes_bridge.bridge.hermes_runner import HermesRunner
from hermes_bridge.config import Settings
from hermes_bridge.ws import chat as chat_mod


class FakeAgent:
    async def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]:
        yield {"kind": "final", "id": "m1", "text": f"hi-{user_content}"}


def _install_fake_runner(monkeypatch):
    def factory(session_id: str) -> HermesRunner:
        return HermesRunner(agent=FakeAgent(), session_id=session_id)

    monkeypatch.setattr(chat_mod, "make_runner", factory)


def test_ws_rejects_without_subprotocol(monkeypatch):
    _install_fake_runner(monkeypatch)
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    app = create_app(Settings())
    c = TestClient(app)
    with pytest.raises(Exception):
        with c.websocket_connect("/hermes/ws?session_id=s1"):
            pass


def test_ws_roundtrip(monkeypatch):
    _install_fake_runner(monkeypatch)
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    app = create_app(Settings())
    c = TestClient(app)
    with c.websocket_connect(
        "/hermes/ws?session_id=s1",
        subprotocols=["token.t"],
    ) as ws:
        ws.send_json({"type": "message.send", "id": "r1", "payload": {"content": "bob"}})
        f1 = ws.receive_json()
        f2 = ws.receive_json()
        f3 = ws.receive_json()
        assert f1["type"] == "typing.start"
        assert f2["type"] == "message.create"
        assert f2["payload"]["content"] == "hi-bob"
        assert f3["type"] == "typing.stop"


def test_ws_ping_pong(monkeypatch):
    _install_fake_runner(monkeypatch)
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    app = create_app(Settings())
    c = TestClient(app)
    with c.websocket_connect("/hermes/ws?session_id=s1", subprotocols=["token.t"]) as ws:
        ws.send_json({"type": "ping", "id": "nonce"})
        frame = ws.receive_json()
        assert frame["type"] == "pong"
        assert frame["id"] == "nonce"
