from __future__ import annotations

from dataclasses import dataclass
from typing import Any, AsyncIterator, Protocol


class HermesAgentLike(Protocol):
    def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]: ...


@dataclass
class RunnerEvent:
    kind: str  # "typing_start" | "typing_stop" | "message_create" | "error"
    message_id: str | None = None
    content: str | None = None
    thought: bool | None = None
    code: str | None = None
    message: str | None = None


class HermesRunner:
    """Wraps one hermes agent conversation; converts native hermes events to
    Pico-shaped frames.

    The concrete `agent` is injected so tests can use a fake. In production,
    `hermes_bridge.bridge.hermes_factory.make_real_runner()` returns a real
    hermes-agent instance.
    """

    def __init__(self, agent: HermesAgentLike, session_id: str) -> None:
        self._agent = agent
        self.session_id = session_id

    async def run_turn(self, user_content: str) -> AsyncIterator[RunnerEvent]:
        yield RunnerEvent(kind="typing_start")
        try:
            async for raw in self._agent.run_turn(user_content):
                kind = raw.get("kind")
                if kind == "thought":
                    yield RunnerEvent(
                        kind="message_create",
                        message_id=str(raw.get("id", "")),
                        content=str(raw.get("text", "")),
                        thought=True,
                    )
                elif kind == "final":
                    yield RunnerEvent(
                        kind="message_create",
                        message_id=str(raw.get("id", "")),
                        content=str(raw.get("text", "")),
                        thought=False,
                    )
                else:
                    yield RunnerEvent(
                        kind="error",
                        code="unknown_event",
                        message=f"unrecognized hermes event kind: {kind!r}",
                    )
        except Exception as exc:  # surface to the wire; never crash the socket
            yield RunnerEvent(
                kind="error",
                code="runner_failure",
                message=str(exc) or exc.__class__.__name__,
            )
        finally:
            yield RunnerEvent(kind="typing_stop")
