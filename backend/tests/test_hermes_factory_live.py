import os

import pytest

from hermes_bridge.bridge.hermes_factory import make_real_runner
from hermes_bridge.config import Settings


@pytest.mark.skipif(
    not os.environ.get("HERMES_BRIDGE_LIVE"),
    reason="set HERMES_BRIDGE_LIVE=1 and ensure ~/.hermes is configured",
)
@pytest.mark.asyncio
async def test_live_roundtrip():
    s = Settings()
    r = make_real_runner(s, session_id="live-test")
    events = [e async for e in r.run_turn("say hi in three words")]
    kinds = [e.kind for e in events]
    assert kinds[0] == "typing_start"
    assert kinds[-1] == "typing_stop"
    assert any(e.kind == "message_create" and e.thought is False for e in events)
