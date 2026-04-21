import sqlite3
import time

import pytest

from hermes_bridge.bridge.session_store import SessionStore
from hermes_bridge.config import Settings


HERMES_SCHEMA = """
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    user_id TEXT,
    model TEXT,
    model_config TEXT,
    system_prompt TEXT,
    parent_session_id TEXT,
    started_at REAL NOT NULL,
    ended_at REAL,
    end_reason TEXT,
    message_count INTEGER DEFAULT 0,
    tool_call_count INTEGER DEFAULT 0,
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    cache_read_tokens INTEGER DEFAULT 0,
    cache_write_tokens INTEGER DEFAULT 0,
    reasoning_tokens INTEGER DEFAULT 0,
    billing_provider TEXT,
    billing_base_url TEXT,
    billing_mode TEXT,
    estimated_cost_usd REAL,
    actual_cost_usd REAL,
    cost_status TEXT,
    cost_source TEXT,
    pricing_version TEXT,
    title TEXT
);
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    role TEXT NOT NULL,
    content TEXT,
    tool_call_id TEXT,
    tool_calls TEXT,
    tool_name TEXT,
    timestamp REAL NOT NULL,
    token_count INTEGER,
    finish_reason TEXT,
    reasoning TEXT,
    reasoning_details TEXT,
    codex_reasoning_items TEXT
);
"""


def _seed(db_path):
    con = sqlite3.connect(db_path)
    con.executescript(HERMES_SCHEMA)
    now = time.time()
    con.execute(
        "INSERT INTO sessions(id,source,started_at,ended_at,message_count,title) VALUES(?,?,?,?,?,?)",
        ("s1", "cli", now, now, 2, "hello world"),
    )
    con.execute(
        "INSERT INTO messages(session_id,role,content,timestamp) VALUES(?,?,?,?)",
        ("s1", "user", "hi", now),
    )
    con.execute(
        "INSERT INTO messages(session_id,role,content,timestamp) VALUES(?,?,?,?)",
        ("s1", "assistant", "hello!", now),
    )
    con.commit()
    con.close()


@pytest.fixture
def store(tmp_path, monkeypatch):
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    _seed(tmp_path / "state.db")
    s = Settings()
    return SessionStore(s)


def test_list_sessions_returns_summary(store):
    rows = store.list(offset=0, limit=50)
    assert len(rows) == 1
    assert rows[0].id == "s1"
    assert rows[0].title == "hello world"
    assert rows[0].message_count == 2
    # timestamp should be in millis
    assert rows[0].created > 1_000_000_000_000


def test_get_session_returns_messages(store):
    d = store.get("s1")
    assert d is not None
    assert len(d.messages) == 2
    assert d.messages[0].role == "user"
    assert d.messages[1].content == "hello!"


def test_get_missing_session_returns_none(store):
    assert store.get("does-not-exist") is None


def test_delete_session(store):
    store.delete("s1")
    assert store.list(offset=0, limit=50) == []


def test_list_empty_when_db_missing(tmp_path, monkeypatch):
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    s = Settings()
    assert SessionStore(s).list(offset=0, limit=10) == []
