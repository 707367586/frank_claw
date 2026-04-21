from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel

ClientType = Literal["message.send", "media.send", "ping"]
ServerType = Literal[
    "message.create",
    "message.update",
    "media.create",
    "typing.start",
    "typing.stop",
    "error",
    "pong",
]
MessageType = ClientType | ServerType


class HermesMessage(BaseModel):
    type: MessageType
    id: str | None = None
    session_id: str | None = None
    timestamp: int | None = None
    payload: dict[str, Any] | None = None


class MessageSendPayload(BaseModel):
    content: str
    media: str | dict[str, Any] | list[Any] | None = None


class MessageCreatePayload(BaseModel):
    message_id: str
    content: str
    thought: bool | None = None


class ErrorPayload(BaseModel):
    code: str
    message: str
    request_id: str | None = None
