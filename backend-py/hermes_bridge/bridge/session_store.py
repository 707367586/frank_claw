from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from pathlib import Path

from ..config import Settings


@dataclass
class SessionSummary:
    id: str
    title: str
    preview: str
    message_count: int
    created: int      # unix millis
    updated: int      # unix millis


@dataclass
class SessionMessage:
    role: str
    content: str
    media: object | None = None


@dataclass
class SessionDetail:
    id: str
    title: str
    preview: str
    message_count: int
    created: int
    updated: int
    messages: list[SessionMessage]
    summary: str


def _sec_to_ms(v: float | None) -> int:
    if v is None:
        return 0
    return int(v * 1000)


class SessionStore:
    """Reads hermes-agent's SQLite session store directly.

    DB: `~/.hermes/state.db` (schema documented in
    `backend-py/docs/hermes-internal-surface.md`). Columns `started_at` /
    `ended_at` are unix **seconds** (REAL); our API serialises millis.
    """

    DB_FILENAME = "state.db"

    def __init__(self, settings: Settings) -> None:
        self._db_path: Path = settings.hermes_home / self.DB_FILENAME

    def _connect(self) -> sqlite3.Connection:
        con = sqlite3.connect(self._db_path)
        con.row_factory = sqlite3.Row
        return con

    def list(self, offset: int, limit: int) -> list[SessionSummary]:
        if not self._db_path.exists():
            return []
        with self._connect() as con:
            rows = con.execute(
                """
                SELECT s.id, s.title, s.started_at, s.ended_at, s.message_count,
                       COALESCE(
                           (SELECT content FROM messages
                            WHERE session_id = s.id
                            ORDER BY timestamp DESC LIMIT 1),
                           ''
                       ) AS last_content
                FROM sessions s
                ORDER BY s.started_at DESC
                LIMIT ? OFFSET ?
                """,
                (limit, offset),
            ).fetchall()
        return [
            SessionSummary(
                id=r["id"],
                title=r["title"] or "",
                preview=(r["last_content"] or "")[:120],
                message_count=r["message_count"] or 0,
                created=_sec_to_ms(r["started_at"]),
                updated=_sec_to_ms(r["ended_at"] or r["started_at"]),
            )
            for r in rows
        ]

    def get(self, session_id: str) -> SessionDetail | None:
        if not self._db_path.exists():
            return None
        with self._connect() as con:
            head = con.execute(
                """
                SELECT id, title, started_at, ended_at, message_count
                FROM sessions WHERE id = ?
                """,
                (session_id,),
            ).fetchone()
            if head is None:
                return None
            msg_rows = con.execute(
                """
                SELECT role, content FROM messages
                WHERE session_id = ?
                ORDER BY timestamp ASC
                """,
                (session_id,),
            ).fetchall()
        messages = [SessionMessage(role=m["role"], content=m["content"] or "") for m in msg_rows]
        preview = messages[-1].content[:120] if messages else ""
        return SessionDetail(
            id=head["id"],
            title=head["title"] or "",
            preview=preview,
            message_count=head["message_count"] or len(messages),
            created=_sec_to_ms(head["started_at"]),
            updated=_sec_to_ms(head["ended_at"] or head["started_at"]),
            messages=messages,
            summary="",
        )

    def delete(self, session_id: str) -> None:
        if not self._db_path.exists():
            return
        with self._connect() as con:
            con.execute("DELETE FROM messages WHERE session_id = ?", (session_id,))
            con.execute("DELETE FROM sessions WHERE id = ?", (session_id,))
            con.commit()
