import pytest
from typing import Any, AsyncIterator

from hermes_bridge.bridge.hermes_runner import HermesRunner, RunnerEvent


class FakeHermesAgent:
    """Simulates the hermes agent we wrap. One turn = two events
    (thought + final)."""

    def __init__(self) -> None:
        self.calls: list[str] = []

    async def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]:
        self.calls.append(user_content)
        yield {"kind": "thought", "id": "t1", "text": "thinking…"}
        yield {"kind": "final", "id": "m1", "text": f"echo: {user_content}"}


@pytest.mark.asyncio
async def test_runner_turn_emits_typing_and_messages():
    fake = FakeHermesAgent()
    r = HermesRunner(agent=fake, session_id="sess-1")
    events: list[RunnerEvent] = []
    async for ev in r.run_turn("hello"):
        events.append(ev)

    kinds = [e.kind for e in events]
    assert kinds == ["typing_start", "message_create", "message_create", "typing_stop"]
    assert events[1].message_id == "t1"
    assert events[1].thought is True
    assert events[2].message_id == "m1"
    assert events[2].thought is False
    assert events[2].content == "echo: hello"
    assert fake.calls == ["hello"]


@pytest.mark.asyncio
async def test_runner_surfaces_agent_error_as_error_event():
    class Boom:
        async def run_turn(self, user_content: str):
            raise RuntimeError("boom")
            yield  # noqa: B901 — make this an async generator in type-checker's eyes

    r = HermesRunner(agent=Boom(), session_id="sess-1")
    events = [e async for e in r.run_turn("x")]
    kinds = [e.kind for e in events]
    assert kinds[0] == "typing_start"
    assert "error" in kinds
    assert kinds[-1] == "typing_stop"
