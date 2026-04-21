import json

from hermes_bridge.ws.protocol import (
    ErrorPayload,
    HermesMessage,
    MessageCreatePayload,
    MessageSendPayload,
)


def test_parse_message_send_from_client():
    raw = '{"type":"message.send","id":"abc","payload":{"content":"hi"}}'
    m = HermesMessage.model_validate_json(raw)
    assert m.type == "message.send"
    assert m.id == "abc"
    assert m.payload == {"content": "hi"}


def test_build_message_create_frame_round_trip():
    p = MessageCreatePayload(message_id="m1", content="hello", thought=False)
    m = HermesMessage(type="message.create", payload=p.model_dump(exclude_none=True))
    s = m.model_dump_json(exclude_none=True)
    parsed = json.loads(s)
    assert parsed["type"] == "message.create"
    assert parsed["payload"]["message_id"] == "m1"
    assert parsed["payload"]["content"] == "hello"
    assert "thought" in parsed["payload"]


def test_error_frame_carries_request_id():
    e = ErrorPayload(code="bad_input", message="empty", request_id="req1")
    m = HermesMessage(type="error", payload=e.model_dump(exclude_none=True))
    assert m.payload["request_id"] == "req1"


def test_message_send_payload_accepts_media_variants():
    MessageSendPayload.model_validate({"content": "c"})
    MessageSendPayload.model_validate({"content": "c", "media": "data:..."})
    MessageSendPayload.model_validate({"content": "c", "media": {"kind": "img"}})
    MessageSendPayload.model_validate({"content": "c", "media": ["data:..."]})
