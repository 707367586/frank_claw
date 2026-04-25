# Persona Agents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the homepage's mock agent sidebar with real persona agents persisted on disk, each with its own system prompt / model / toolset whitelist / workspace, wired end-to-end through `hermes_bridge` to hermes-agent.

**Architecture:** Backend gains `~/.hermes/agents.json`-backed CRUD via a new `AgentStore` + `/api/agents` + `/api/toolsets` routers, and the WS upgrade accepts `agent_id` so the runner factory can inject persona (system prompt, model override, enabled toolsets, `TERMINAL_CWD` workspace) into hermes-agent's `AIAgent`. Frontend extends `ClawProvider` with agents/toolsets state, replaces the mock `AgentSidebar` with real data, adds a `CreateAgentModal`, and adds a "新对话" button on `ChatPage` that rotates the active agent's session id.

**Tech Stack:** Python 3.11 + FastAPI + Pydantic + pytest (backend); React 19 + Vite + TypeScript + Vitest + Testing Library (frontend); hermes-agent vendored at `backend/vendor/hermes_agent/`.

**Spec:** `docs/superpowers/specs/2026-04-26-persona-agents-design.md`

---

## File Structure

**Backend — new:**
- `backend/hermes_bridge/bridge/agent_store.py` — `Agent` dataclass + `AgentStore` (CRUD, atomic write, seeds, session rotation).
- `backend/hermes_bridge/api/agents.py` — `/api/agents` router.
- `backend/hermes_bridge/api/toolsets.py` — `/api/toolsets` router (lazy hermes-agent import).
- `backend/tests/test_agent_store.py`
- `backend/tests/test_agents_api.py`
- `backend/tests/test_toolsets_api.py`

**Backend — modify:**
- `backend/hermes_bridge/app.py` — register the two new routers.
- `backend/hermes_bridge/ws/chat.py` — accept `agent_id` query param; factory takes `(sid, aid)`.
- `backend/hermes_bridge/bridge/hermes_factory.py` — persona injection + `TERMINAL_CWD` swap.
- `backend/hermes_bridge/__main__.py` — pass `agent_id` to `make_real_runner`.
- `backend/tests/test_hermes_factory.py` — extend for persona injection (live test left untouched).
- `backend/tests/test_ws_chat.py` — update fake-runner factory signature.

**Frontend — new:**
- `apps/clawx-gui/src/lib/agents-rest.ts`
- `apps/clawx-gui/src/lib/__tests__/agents-rest.test.ts`
- `apps/clawx-gui/src/components/CreateAgentModal.tsx`
- `apps/clawx-gui/src/components/__tests__/CreateAgentModal.test.tsx`
- `apps/clawx-gui/src/components/__tests__/AgentSidebar.test.tsx`
- `apps/clawx-gui/src/styles/pages/create-agent-modal.css`

**Frontend — modify:**
- `apps/clawx-gui/src/lib/chat-store.ts` — add `replaceMessages`.
- `apps/clawx-gui/src/lib/__tests__/chat-store.test.ts` — add a test for `replaceMessages`.
- `apps/clawx-gui/src/lib/store.tsx` — agents/toolsets state, `selectAgent`, `createAgent`, `deleteAgent`, `newConversation`, agent-aware WS connect.
- `apps/clawx-gui/src/lib/__tests__/store.test.tsx` — extend.
- `apps/clawx-gui/src/components/AgentSidebar.tsx` — real data + delete + open modal.
- `apps/clawx-gui/src/components/ChatWelcome.tsx` — accept agent props.
- `apps/clawx-gui/src/pages/ChatPage.tsx` — "新对话" button + agent-aware welcome.
- `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx` — seed an agent.
- `apps/clawx-gui/src/styles.css` — `@import` the new modal CSS.

---

## Working Directories

All Python commands run from `backend/` unless noted; all pnpm commands run from `apps/clawx-gui/`.

```bash
# Backend tests (run from repo root)
cd backend && uv run pytest -q

# Frontend tests (run from repo root)
pnpm --filter clawx-gui test

# Frontend type check
cd apps/clawx-gui && pnpm exec tsc -b --noEmit
```

The Vite dev server is already running on `http://localhost:1420` and the bridge on `127.0.0.1:18800` (Vite proxies `/api` and `/hermes/ws`). Restart the bridge after every backend code change:

```bash
# kill running bridge (PID may differ)
kill "$(lsof -ti :18800)"
# start fresh in background
( cd backend && uv run python -m hermes_bridge ) &
```

---

## Task 1: `Agent` dataclass + `AgentStore` skeleton (no seeds yet)

**Files:**
- Create: `backend/hermes_bridge/bridge/agent_store.py`
- Create: `backend/tests/test_agent_store.py`

- [ ] **Step 1: Write failing tests for `Agent` dataclass + `AgentStore.list/get/create/delete/rotate_session`**

```python
# backend/tests/test_agent_store.py
from __future__ import annotations

import json
from pathlib import Path

import pytest

from hermes_bridge.bridge.agent_store import Agent, AgentStore
from hermes_bridge.config import Settings


def _settings(tmp_path: Path) -> Settings:
    return Settings(hermes_home=tmp_path)  # type: ignore[arg-type]


def test_list_returns_seeded_agents_when_file_missing(tmp_path):
    store = AgentStore(_settings(tmp_path))
    out = store.list()
    assert {a.id for a in out} == {"code", "research", "writing", "data"}
    # seed write occurred
    assert (tmp_path / "agents.json").exists()


def test_create_assigns_id_and_session_and_workspace(tmp_path):
    store = AgentStore(_settings(tmp_path))
    a = store.create(
        name="my agent",
        description="d",
        color="#5749F4",
        icon="Bot",
        system_prompt="be helpful",
        model=None,
        enabled_toolsets=["web"],
        workspace_dir=None,
    )
    assert isinstance(a, Agent)
    assert a.id and a.current_session_id
    assert a.workspace_dir == str(tmp_path / "workspaces" / a.id)
    assert (tmp_path / "workspaces" / a.id).is_dir()
    # round-trip on disk
    store2 = AgentStore(_settings(tmp_path))
    assert any(x.id == a.id for x in store2.list())


def test_create_respects_explicit_workspace(tmp_path):
    explicit = tmp_path / "elsewhere"
    store = AgentStore(_settings(tmp_path))
    a = store.create(
        name="x", description="", color="#5749F4", icon="Bot",
        system_prompt="p", model=None, enabled_toolsets=[],
        workspace_dir=str(explicit),
    )
    assert a.workspace_dir == str(explicit)
    assert explicit.is_dir()


def test_get_returns_none_for_unknown(tmp_path):
    store = AgentStore(_settings(tmp_path))
    assert store.get("nope") is None


def test_get_returns_agent_for_seed(tmp_path):
    store = AgentStore(_settings(tmp_path))
    a = store.get("code")
    assert a is not None
    assert a.name == "编程助手"


def test_delete_removes_from_json_but_leaves_workspace(tmp_path):
    store = AgentStore(_settings(tmp_path))
    a = store.create(
        name="x", description="", color="#5749F4", icon="Bot",
        system_prompt="p", model=None, enabled_toolsets=[], workspace_dir=None,
    )
    store.delete(a.id)
    assert store.get(a.id) is None
    assert (tmp_path / "workspaces" / a.id).is_dir()


def test_delete_unknown_is_noop(tmp_path):
    store = AgentStore(_settings(tmp_path))
    store.delete("nonexistent")  # should not raise


def test_rotate_session_changes_id_and_persists(tmp_path):
    store = AgentStore(_settings(tmp_path))
    before = store.get("code")
    assert before is not None
    new_sid = store.rotate_session("code")
    assert new_sid != before.current_session_id
    after = AgentStore(_settings(tmp_path)).get("code")
    assert after is not None
    assert after.current_session_id == new_sid


def test_rotate_unknown_raises_keyerror(tmp_path):
    store = AgentStore(_settings(tmp_path))
    with pytest.raises(KeyError):
        store.rotate_session("nope")


def test_atomic_write_does_not_corrupt_file_on_partial_failure(tmp_path, monkeypatch):
    store = AgentStore(_settings(tmp_path))
    store.list()  # write seeds
    original = (tmp_path / "agents.json").read_text()
    # Force os.replace to fail; the file on disk must remain valid.
    import os
    real_replace = os.replace

    def boom(*a, **kw):
        raise RuntimeError("disk full")

    monkeypatch.setattr(os, "replace", boom)
    with pytest.raises(RuntimeError):
        store.create(
            name="x", description="", color="#5749F4", icon="Bot",
            system_prompt="p", model=None, enabled_toolsets=[], workspace_dir=None,
        )
    monkeypatch.setattr(os, "replace", real_replace)
    assert (tmp_path / "agents.json").read_text() == original
    assert json.loads(original)
```

- [ ] **Step 2: Run tests — verify they fail with import errors**

Run: `cd backend && uv run pytest tests/test_agent_store.py -q`
Expected: collection error or `ModuleNotFoundError: No module named 'hermes_bridge.bridge.agent_store'`.

- [ ] **Step 3: Implement `agent_store.py`**

```python
# backend/hermes_bridge/bridge/agent_store.py
from __future__ import annotations

import json
import os
import time
import uuid
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

from ..config import Settings


@dataclass
class Agent:
    id: str
    name: str
    description: str
    color: str
    icon: str
    system_prompt: str
    model: str | None
    enabled_toolsets: list[str]
    workspace_dir: str
    current_session_id: str
    created_at: int


_DEFAULT_SEEDS: list[dict[str, Any]] = [
    {
        "id": "code",
        "name": "编程助手",
        "description": "代码编写、调试、重构",
        "color": "#5749F4",
        "icon": "Code2",
        "system_prompt": (
            "你是一名资深软件工程师，擅长代码审查、调试与重构。"
            "回答时优先给出可运行的代码示例与简短解释，避免冗长的背景介绍。"
            "对模糊需求主动追问关键约束（语言版本、运行环境、性能目标）。"
            "默认中文回答，但代码、命令与变量名保持英文。"
        ),
        "model": None,
        "enabled_toolsets": ["terminal", "file", "skills", "debugging", "code_execution"],
    },
    {
        "id": "research",
        "name": "研究助手",
        "description": "网络检索、资料整理",
        "color": "#3B82F6",
        "icon": "Search",
        "system_prompt": (
            "你是一名研究分析师，擅长检索、阅读与综合多源信息。"
            "默认调用网络检索工具获取最新事实，回答末尾给出参考链接。"
            "对不确定的事实使用「据 X 报道/X 处显示」等限定语，避免虚构。"
            "默认中文回答，引用原文标题保留原语言。"
        ),
        "model": None,
        "enabled_toolsets": ["web", "search", "vision", "session_search"],
    },
    {
        "id": "writing",
        "name": "写作助手",
        "description": "文档、周报、文案",
        "color": "#EC4899",
        "icon": "PenTool",
        "system_prompt": (
            "你是一名职业写作教练，擅长把零散素材整理成结构化文档。"
            "回答优先给出可直接复用的文本，结构清晰、用词简洁、避免空话。"
            "需要时主动询问目标读者、字数、语气等关键信息。"
            "默认中文输出。"
        ),
        "model": None,
        "enabled_toolsets": ["file", "memory", "todo"],
    },
    {
        "id": "data",
        "name": "数据分析",
        "description": "数据探索、统计、可视化",
        "color": "#F59E0B",
        "icon": "BarChart3",
        "system_prompt": (
            "你是一名数据分析师，擅长用 Python/pandas 做数据清洗与统计。"
            "回答优先给出可运行的代码片段与简洁的结论解释。"
            "对数据形态不明的请求先询问字段、规模、目标指标。"
            "默认中文，代码与字段名保持英文。"
        ),
        "model": None,
        "enabled_toolsets": ["code_execution", "file"],
    },
]


class AgentStore:
    """JSON-backed persistence for persona agents at ~/.hermes/agents.json."""

    FILENAME = "agents.json"
    VERSION = 1

    def __init__(self, settings: Settings) -> None:
        self._home = settings.hermes_home
        self._path = self._home / self.FILENAME
        self._workspaces_root = self._home / "workspaces"

    # --- public API ----------------------------------------------------

    def list(self) -> list[Agent]:
        return self._read()["agents"]

    def get(self, agent_id: str) -> Agent | None:
        for a in self.list():
            if a.id == agent_id:
                return a
        return None

    def create(
        self,
        *,
        name: str,
        description: str,
        color: str,
        icon: str,
        system_prompt: str,
        model: str | None,
        enabled_toolsets: list[str],
        workspace_dir: str | None,
    ) -> Agent:
        new_id = uuid.uuid4().hex
        ws_path = Path(workspace_dir) if workspace_dir else (self._workspaces_root / new_id)
        ws_path.mkdir(parents=True, exist_ok=True)
        agent = Agent(
            id=new_id,
            name=name,
            description=description,
            color=color,
            icon=icon,
            system_prompt=system_prompt,
            model=model,
            enabled_toolsets=list(enabled_toolsets),
            workspace_dir=str(ws_path),
            current_session_id=uuid.uuid4().hex,
            created_at=int(time.time() * 1000),
        )
        data = self._read()
        data["agents"].append(agent)
        self._write(data)
        return agent

    def delete(self, agent_id: str) -> None:
        data = self._read()
        data["agents"] = [a for a in data["agents"] if a.id != agent_id]
        self._write(data)

    def rotate_session(self, agent_id: str) -> str:
        data = self._read()
        for i, a in enumerate(data["agents"]):
            if a.id == agent_id:
                new_sid = uuid.uuid4().hex
                data["agents"][i] = Agent(**{**asdict(a), "current_session_id": new_sid})
                self._write(data)
                return new_sid
        raise KeyError(agent_id)

    # --- internals -----------------------------------------------------

    def _read(self) -> dict[str, Any]:
        if not self._path.exists():
            self._seed()
        raw = json.loads(self._path.read_text() or "{}")
        agents = [Agent(**a) for a in raw.get("agents", [])]
        return {"version": raw.get("version", self.VERSION), "agents": agents}

    def _write(self, data: dict[str, Any]) -> None:
        serialised = {
            "version": self.VERSION,
            "agents": [asdict(a) for a in data["agents"]],
        }
        self._home.mkdir(parents=True, exist_ok=True)
        tmp = self._path.with_suffix(self._path.suffix + ".tmp")
        tmp.write_text(json.dumps(serialised, indent=2, ensure_ascii=False))
        os.replace(tmp, self._path)

    def _seed(self) -> None:
        seeds: list[Agent] = []
        for s in _DEFAULT_SEEDS:
            seeds.append(
                Agent(
                    id=s["id"],
                    name=s["name"],
                    description=s["description"],
                    color=s["color"],
                    icon=s["icon"],
                    system_prompt=s["system_prompt"],
                    model=s["model"],
                    enabled_toolsets=list(s["enabled_toolsets"]),
                    workspace_dir=str(self._workspaces_root / s["id"]),
                    current_session_id=uuid.uuid4().hex,
                    created_at=int(time.time() * 1000),
                )
            )
        self._write({"agents": seeds})
```

- [ ] **Step 4: Run tests — verify all pass**

Run: `cd backend && uv run pytest tests/test_agent_store.py -q`
Expected: 9 passed.

- [ ] **Step 5: Commit**

```bash
git add backend/hermes_bridge/bridge/agent_store.py backend/tests/test_agent_store.py
git commit -m "feat(backend): add AgentStore for persona agent persistence"
```

---

## Task 2: `/api/agents` router

**Files:**
- Create: `backend/hermes_bridge/api/agents.py`
- Create: `backend/tests/test_agents_api.py`
- Modify: `backend/hermes_bridge/app.py`

- [ ] **Step 1: Write failing tests**

```python
# backend/tests/test_agents_api.py
from __future__ import annotations

import dataclasses

from fastapi.testclient import TestClient

from hermes_bridge.api import agents as agents_api
from hermes_bridge.app import create_app
from hermes_bridge.bridge.agent_store import Agent
from hermes_bridge.config import Settings


def _agent(**overrides) -> Agent:
    base = dict(
        id="aid",
        name="n",
        description="d",
        color="#5749F4",
        icon="Bot",
        system_prompt="p",
        model=None,
        enabled_toolsets=[],
        workspace_dir="/tmp/aid",
        current_session_id="sid",
        created_at=1,
    )
    base.update(overrides)
    return Agent(**base)


class FakeStore:
    def __init__(self):
        self.agents: list[Agent] = [_agent()]
        self.create_calls: list[dict] = []
        self.delete_calls: list[str] = []
        self.rotate_calls: list[str] = []

    def list(self):
        return list(self.agents)

    def get(self, aid):
        for a in self.agents:
            if a.id == aid:
                return a
        return None

    def create(self, **kw):
        self.create_calls.append(kw)
        a = _agent(id="new", name=kw["name"])
        self.agents.append(a)
        return a

    def delete(self, aid):
        self.delete_calls.append(aid)
        self.agents = [a for a in self.agents if a.id != aid]

    def rotate_session(self, aid):
        self.rotate_calls.append(aid)
        if not any(a.id == aid for a in self.agents):
            raise KeyError(aid)
        return "rotated-sid"


def _client(monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    fake = FakeStore()
    monkeypatch.setattr(agents_api, "_store_factory", lambda _s: fake)
    app = create_app(Settings())
    return TestClient(app), fake


def test_list_requires_auth(monkeypatch):
    c, _ = _client(monkeypatch)
    assert c.get("/api/agents").status_code == 401


def test_list_returns_agents(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.get("/api/agents", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert isinstance(body, list)
    assert body[0]["id"] == "aid"
    assert body[0]["color"] == "#5749F4"


def test_create_201(monkeypatch):
    c, fake = _client(monkeypatch)
    r = c.post(
        "/api/agents",
        headers={"Authorization": "Bearer t"},
        json={
            "name": "X",
            "description": "",
            "color": "#5749F4",
            "icon": "Bot",
            "system_prompt": "p",
            "model": None,
            "enabled_toolsets": ["web"],
        },
    )
    assert r.status_code == 201
    assert r.json()["id"] == "new"
    assert fake.create_calls[0]["name"] == "X"
    assert fake.create_calls[0]["enabled_toolsets"] == ["web"]
    assert fake.create_calls[0]["workspace_dir"] is None


def test_create_validates_name(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.post(
        "/api/agents",
        headers={"Authorization": "Bearer t"},
        json={"name": "", "description": "", "color": "#5749F4", "icon": "Bot",
              "system_prompt": "p", "model": None, "enabled_toolsets": []},
    )
    assert r.status_code == 422


def test_create_validates_color(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.post(
        "/api/agents",
        headers={"Authorization": "Bearer t"},
        json={"name": "x", "description": "", "color": "blue", "icon": "Bot",
              "system_prompt": "p", "model": None, "enabled_toolsets": []},
    )
    assert r.status_code == 422


def test_delete_204(monkeypatch):
    c, fake = _client(monkeypatch)
    r = c.delete("/api/agents/aid", headers={"Authorization": "Bearer t"})
    assert r.status_code == 204
    assert fake.delete_calls == ["aid"]


def test_delete_404_when_missing(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.delete("/api/agents/nope", headers={"Authorization": "Bearer t"})
    assert r.status_code == 404


def test_rotate_session_returns_new_sid(monkeypatch):
    c, fake = _client(monkeypatch)
    r = c.post("/api/agents/aid/sessions", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    assert r.json() == {"session_id": "rotated-sid"}
    assert fake.rotate_calls == ["aid"]


def test_rotate_session_404(monkeypatch):
    c, _ = _client(monkeypatch)
    r = c.post("/api/agents/nope/sessions", headers={"Authorization": "Bearer t"})
    assert r.status_code == 404
```

- [ ] **Step 2: Run tests — fail with import error**

Run: `cd backend && uv run pytest tests/test_agents_api.py -q`
Expected: `ModuleNotFoundError: No module named 'hermes_bridge.api.agents'`.

- [ ] **Step 3: Implement `api/agents.py` and wire into `app.py`**

```python
# backend/hermes_bridge/api/agents.py
from __future__ import annotations

from dataclasses import asdict

from fastapi import APIRouter, Depends, HTTPException, Response, status
from pydantic import BaseModel, Field, constr

from ..auth import require_bearer_token
from ..bridge.agent_store import AgentStore
from ..config import Settings


HEX_COLOR = r"^#[0-9A-Fa-f]{6}$"


class AgentCreate(BaseModel):
    name: constr(min_length=1, max_length=64)  # type: ignore[valid-type]
    description: str = ""
    color: str = Field(pattern=HEX_COLOR)
    icon: constr(min_length=1, max_length=64)  # type: ignore[valid-type]
    system_prompt: constr(min_length=1)  # type: ignore[valid-type]
    model: str | None = None
    enabled_toolsets: list[str] = Field(default_factory=list)
    workspace_dir: str | None = None


def _store_factory(settings: Settings) -> AgentStore:
    return AgentStore(settings)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/agents", tags=["agents"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_agents():
        store = _store_factory(settings)
        return [asdict(a) for a in store.list()]

    @r.post("", dependencies=[dep], status_code=status.HTTP_201_CREATED)
    def create_agent(body: AgentCreate):
        store = _store_factory(settings)
        a = store.create(
            name=body.name,
            description=body.description,
            color=body.color,
            icon=body.icon,
            system_prompt=body.system_prompt,
            model=body.model,
            enabled_toolsets=body.enabled_toolsets,
            workspace_dir=body.workspace_dir,
        )
        return asdict(a)

    @r.delete("/{aid}", dependencies=[dep], status_code=status.HTTP_204_NO_CONTENT)
    def delete_agent(aid: str) -> Response:
        store = _store_factory(settings)
        if store.get(aid) is None:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "agent not found")
        store.delete(aid)
        return Response(status_code=status.HTTP_204_NO_CONTENT)

    @r.post("/{aid}/sessions", dependencies=[dep])
    def rotate_session(aid: str):
        store = _store_factory(settings)
        try:
            sid = store.rotate_session(aid)
        except KeyError:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "agent not found")
        return {"session_id": sid}

    return r
```

Then add to `backend/hermes_bridge/app.py`:

```python
# at top with the other imports:
from .api import agents as agents_api

# inside create_app, after the existing include_router calls and before ws_chat:
    app.include_router(agents_api.make_router(s))
```

- [ ] **Step 4: Run tests**

Run: `cd backend && uv run pytest tests/test_agents_api.py tests/test_agent_store.py -q`
Expected: 18 passed (9 from Task 1 + 9 here).

- [ ] **Step 5: Commit**

```bash
git add backend/hermes_bridge/api/agents.py backend/tests/test_agents_api.py backend/hermes_bridge/app.py
git commit -m "feat(backend): add /api/agents CRUD router"
```

---

## Task 3: `/api/toolsets` router

**Files:**
- Create: `backend/hermes_bridge/api/toolsets.py`
- Create: `backend/tests/test_toolsets_api.py`
- Modify: `backend/hermes_bridge/app.py`

- [ ] **Step 1: Write failing tests**

```python
# backend/tests/test_toolsets_api.py
from __future__ import annotations

import sys
import types

from fastapi.testclient import TestClient

from hermes_bridge.api import toolsets as toolsets_api
from hermes_bridge.app import create_app
from hermes_bridge.config import Settings


def _client(monkeypatch):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    app = create_app(Settings())
    return TestClient(app)


def test_list_requires_auth(monkeypatch):
    c = _client(monkeypatch)
    assert c.get("/api/toolsets").status_code == 401


def test_list_returns_projection(monkeypatch):
    fake_module = types.ModuleType("toolsets")
    fake_module.TOOLSETS = {  # type: ignore[attr-defined]
        "web": {"description": "web tools", "tools": ["web_search"], "includes": []},
        "file": {"description": "file ops", "tools": ["read", "write"], "includes": []},
    }
    monkeypatch.setitem(sys.modules, "toolsets", fake_module)
    c = _client(monkeypatch)
    r = c.get("/api/toolsets", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert sorted(t["name"] for t in body) == ["file", "web"]
    web = next(t for t in body if t["name"] == "web")
    assert web["description"] == "web tools"
    assert web["tools"] == ["web_search"]


def test_list_falls_back_to_empty_when_import_fails(monkeypatch):
    # Force the import to raise
    def boom(*a, **kw):
        raise ImportError("hermes-agent missing")

    monkeypatch.setattr(toolsets_api, "_load_registry", boom)
    c = _client(monkeypatch)
    r = c.get("/api/toolsets", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    assert r.json() == []
```

- [ ] **Step 2: Run — fail with import error**

Run: `cd backend && uv run pytest tests/test_toolsets_api.py -q`
Expected: `ModuleNotFoundError: No module named 'hermes_bridge.api.toolsets'`.

- [ ] **Step 3: Implement `api/toolsets.py` and register in `app.py`**

```python
# backend/hermes_bridge/api/toolsets.py
from __future__ import annotations

import logging

from fastapi import APIRouter, Depends

from ..auth import require_bearer_token
from ..config import Settings

log = logging.getLogger(__name__)


def _load_registry() -> dict:
    """Imported lazily so hermes_bridge can boot even when hermes-agent is unavailable."""
    from toolsets import TOOLSETS  # type: ignore[import-not-found]
    return TOOLSETS


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/toolsets", tags=["toolsets"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_toolsets():
        try:
            registry = _load_registry()
        except Exception as exc:  # noqa: BLE001 — we want any import failure to degrade gracefully
            log.warning("toolset registry import failed: %s", exc)
            return []
        out = []
        for name, info in registry.items():
            out.append({
                "name": name,
                "description": info.get("description", "") or "",
                "tools": list(info.get("tools", []) or []),
            })
        out.sort(key=lambda t: t["name"])
        return out

    return r
```

Add to `backend/hermes_bridge/app.py`:

```python
from .api import toolsets as toolsets_api

# inside create_app, alongside agents:
    app.include_router(toolsets_api.make_router(s))
```

- [ ] **Step 4: Run tests**

Run: `cd backend && uv run pytest tests/test_toolsets_api.py -q`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add backend/hermes_bridge/api/toolsets.py backend/tests/test_toolsets_api.py backend/hermes_bridge/app.py
git commit -m "feat(backend): add /api/toolsets registry endpoint"
```

---

## Task 4: WS accepts `agent_id`; runner factory takes `(sid, aid)`

**Files:**
- Modify: `backend/hermes_bridge/ws/chat.py`
- Modify: `backend/tests/test_ws_chat.py`

- [ ] **Step 1: Update existing WS tests to the new factory signature; add a test asserting `agent_id` reaches the factory**

Replace `backend/tests/test_ws_chat.py`:

```python
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


def _install_fake_runner(monkeypatch, captured: list | None = None):
    def factory(session_id: str, agent_id: str | None) -> HermesRunner:
        if captured is not None:
            captured.append((session_id, agent_id))
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


def test_ws_passes_agent_id_to_factory(monkeypatch):
    captured: list = []
    _install_fake_runner(monkeypatch, captured)
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    app = create_app(Settings())
    c = TestClient(app)
    with c.websocket_connect(
        "/hermes/ws?session_id=s1&agent_id=code",
        subprotocols=["token.t"],
    ) as ws:
        ws.send_json({"type": "ping", "id": "x"})
        ws.receive_json()
    assert captured == [("s1", "code")]


def test_ws_agent_id_optional(monkeypatch):
    captured: list = []
    _install_fake_runner(monkeypatch, captured)
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    app = create_app(Settings())
    c = TestClient(app)
    with c.websocket_connect(
        "/hermes/ws?session_id=s1",
        subprotocols=["token.t"],
    ) as ws:
        ws.send_json({"type": "ping", "id": "x"})
        ws.receive_json()
    assert captured == [("s1", None)]
```

- [ ] **Step 2: Run — verify failures from old single-arg factory**

Run: `cd backend && uv run pytest tests/test_ws_chat.py -q`
Expected: 5 failures — old `make_runner(session_id)` raises a `RuntimeError`, and the new tests fail.

- [ ] **Step 3: Modify `ws/chat.py`** — replace the existing `make_runner` stub and the route signature:

```python
# backend/hermes_bridge/ws/chat.py — update these regions only
# (1) make_runner default + bind_runner_factory typing
def make_runner(session_id: str, agent_id: str | None) -> HermesRunner:
    raise RuntimeError(
        "make_runner not configured; override via monkeypatch in tests or call "
        "hermes_bridge.ws.chat.bind_runner_factory(...) at startup"
    )


def bind_runner_factory(factory: Callable[[str, str | None], HermesRunner]) -> None:
    global make_runner
    make_runner = factory  # type: ignore[assignment]


# (2) route signature + factory call
    @r.websocket("/hermes/ws")
    async def ws_chat(
        websocket: WebSocket,
        session_id: str = Query(...),
        agent_id: str | None = Query(default=None),
    ) -> None:
        requested = list(websocket.scope.get("subprotocols") or [])
        matched = verify_ws_subprotocol(requested, settings)
        if not matched:
            await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
            return
        await websocket.accept(subprotocol=matched)

        runner = make_runner(session_id, agent_id)
        # ... rest unchanged
```

- [ ] **Step 4: Run tests**

Run: `cd backend && uv run pytest tests/test_ws_chat.py -q`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add backend/hermes_bridge/ws/chat.py backend/tests/test_ws_chat.py
git commit -m "feat(backend): WS accepts agent_id query and passes it to factory"
```

---

## Task 5: Persona injection in `hermes_factory.py` + `__main__.py` glue

**Files:**
- Modify: `backend/hermes_bridge/bridge/hermes_factory.py`
- Modify: `backend/hermes_bridge/__main__.py`
- Modify: `backend/tests/test_hermes_factory.py`

- [ ] **Step 1: Extend `test_hermes_factory.py` with persona + cwd assertions**

Append to `backend/tests/test_hermes_factory.py`:

```python
import os

from hermes_bridge.bridge import hermes_factory as hf
from hermes_bridge.bridge.agent_store import Agent
from hermes_bridge.bridge.hermes_factory import _HermesAgentAdapter, make_real_runner


class _FakeAIAgent:
    last_kwargs: dict | None = None

    def __init__(self, **kwargs):
        type(self).last_kwargs = kwargs

    def chat(self, content):
        type(self).last_cwd = os.environ.get("TERMINAL_CWD")
        return f"echo:{content}"


def _patch_aiagent(monkeypatch):
    monkeypatch.setattr(hf, "_resolve_aiagent_class", lambda: _FakeAIAgent)
    monkeypatch.setattr(hf, "_resolve_runtime_kwargs", lambda settings: {"model": "global-model"})


def test_adapter_passes_persona_kwargs(monkeypatch, tmp_path):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    _patch_aiagent(monkeypatch)
    from hermes_bridge.config import Settings
    s = Settings()
    a = Agent(
        id="x", name="n", description="d", color="#5749F4", icon="Bot",
        system_prompt="be terse", model="custom-model",
        enabled_toolsets=["web"], workspace_dir=str(tmp_path / "ws-x"),
        current_session_id="sid", created_at=1,
    )
    _HermesAgentAdapter(s, "sid", agent=a)
    kw = _FakeAIAgent.last_kwargs or {}
    assert kw["model"] == "custom-model"
    assert kw["ephemeral_system_prompt"] == "be terse"
    assert kw["enabled_toolsets"] == ["web"]


def test_adapter_uses_global_model_when_agent_model_none(monkeypatch, tmp_path):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    _patch_aiagent(monkeypatch)
    from hermes_bridge.config import Settings
    s = Settings()
    a = Agent(
        id="x", name="n", description="d", color="#5749F4", icon="Bot",
        system_prompt="p", model=None,
        enabled_toolsets=[], workspace_dir=str(tmp_path / "ws-x"),
        current_session_id="sid", created_at=1,
    )
    _HermesAgentAdapter(s, "sid", agent=a)
    kw = _FakeAIAgent.last_kwargs or {}
    assert kw["model"] == "global-model"
    # empty list maps to None so hermes uses defaults
    assert kw["enabled_toolsets"] is None


def test_make_real_runner_with_unknown_agent_id_falls_back(monkeypatch, tmp_path):
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    _patch_aiagent(monkeypatch)
    from hermes_bridge.config import Settings
    s = Settings()
    r = make_real_runner(s, session_id="sid", agent_id="missing")
    assert r.session_id == "sid"


def test_terminal_cwd_swapped_during_chat_and_restored(monkeypatch, tmp_path):
    import anyio
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    monkeypatch.setenv("TERMINAL_CWD", "/original")
    _patch_aiagent(monkeypatch)
    from hermes_bridge.config import Settings
    s = Settings()
    workspace = tmp_path / "ws-y"
    a = Agent(
        id="y", name="n", description="d", color="#5749F4", icon="Bot",
        system_prompt="p", model=None, enabled_toolsets=[],
        workspace_dir=str(workspace), current_session_id="sid", created_at=1,
    )
    adapter = _HermesAgentAdapter(s, "sid", agent=a)

    async def drain():
        async for _ in adapter.run_turn("hello"):
            pass

    anyio.run(drain)
    assert _FakeAIAgent.last_cwd == str(workspace)
    assert os.environ.get("TERMINAL_CWD") == "/original"
    assert workspace.is_dir()
```

- [ ] **Step 2: Run — fail because `agent` kwarg / `_resolve_*` helpers don't exist**

Run: `cd backend && uv run pytest tests/test_hermes_factory.py -q`
Expected: failures referencing missing attributes.

- [ ] **Step 3: Refactor `hermes_factory.py`**

Replace the file's `_HermesAgentAdapter` and `make_real_runner` with a version that admits a per-agent persona and the helper seams the tests patch. Keep the existing `_redact_secrets` and `_SECRET_ENV_VARS` constants at module top.

```python
# backend/hermes_bridge/bridge/hermes_factory.py
from __future__ import annotations

import logging
import os
import uuid
from typing import Any, AsyncIterator

import anyio

from ..config import Settings
from .agent_store import Agent, AgentStore
from .hermes_runner import HermesAgentLike, HermesRunner

log = logging.getLogger(__name__)

_SECRET_ENV_VARS = (
    "GLM_API_KEY", "ZAI_API_KEY", "Z_AI_API_KEY",
    "ANTHROPIC_API_KEY", "OPENROUTER_API_KEY",
    "OPENAI_API_KEY", "DEEPSEEK_API_KEY",
)


def _redact_secrets(s: str) -> str:
    for name in _SECRET_ENV_VARS:
        v = os.environ.get(name)
        if v and v in s:
            s = s.replace(v, "***")
    return s


def _resolve_aiagent_class():
    """Seam for tests; production path imports the real AIAgent."""
    from run_agent import AIAgent  # type: ignore[import-not-found]
    return AIAgent


def _resolve_runtime_kwargs(settings: Settings) -> dict[str, Any]:
    """Reads ~/.hermes/config.yaml + provider runtime; returns a dict of kwargs
    suitable for AIAgent(**kwargs). Tests stub this out.
    """
    from hermes_cli.config import load_config  # type: ignore[import-not-found]
    from hermes_cli.runtime_provider import resolve_runtime_provider  # type: ignore[import-not-found]

    cfg = load_config()
    raw_model = cfg.get("model")
    if isinstance(raw_model, dict):
        model = str(raw_model.get("default") or "")
        requested_provider = raw_model.get("provider") or cfg.get("provider")
    elif isinstance(raw_model, str) or raw_model is None:
        model = raw_model or ""
        requested_provider = cfg.get("provider")
    else:
        raise TypeError(
            f"config.yaml `model` must be str or dict, got {type(raw_model).__name__}"
        )
    runtime = resolve_runtime_provider(requested=requested_provider)
    return {
        "model": model,
        "provider": runtime.get("provider"),
        "base_url": runtime.get("base_url"),
        "api_key": runtime.get("api_key"),
        "api_mode": runtime.get("api_mode"),
    }


class _HermesAgentAdapter:
    def __init__(
        self,
        settings: Settings,
        session_id: str,
        agent: Agent | None = None,
    ) -> None:
        self._settings = settings
        self._session_id = session_id
        self._agent_record = agent
        self._workspace_dir: str | None = agent.workspace_dir if agent else None
        self._llm = None
        self._init_error: str | None = None
        try:
            AIAgent = _resolve_aiagent_class()
            runtime_kwargs = _resolve_runtime_kwargs(settings)
            kwargs: dict[str, Any] = {
                "session_id": session_id,
                "quiet_mode": True,
                **runtime_kwargs,
            }
            if agent is not None:
                if agent.model:
                    kwargs["model"] = agent.model
                if agent.system_prompt:
                    kwargs["ephemeral_system_prompt"] = agent.system_prompt
                kwargs["enabled_toolsets"] = list(agent.enabled_toolsets) or None
            self._llm = AIAgent(**kwargs)
        except Exception as exc:  # pragma: no cover — env-dependent
            self._init_error = _redact_secrets(f"{exc.__class__.__name__}: {exc}")
            log.warning("hermes AIAgent init failed: %s", self._init_error)

    async def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]:
        if self._llm is None:
            yield {
                "kind": "final",
                "id": str(uuid.uuid4()),
                "text": (
                    "hermes_bridge could not start the hermes-agent runtime. "
                    f"Init error: {self._init_error or 'unknown'}. "
                    "Run `uv run --project backend python backend/scripts/init_config.py` "
                    "and ensure a provider API key is set in ~/.hermes/.env."
                ),
            }
            return

        prev_cwd = os.environ.get("TERMINAL_CWD")
        if self._workspace_dir:
            from pathlib import Path
            Path(self._workspace_dir).mkdir(parents=True, exist_ok=True)
            os.environ["TERMINAL_CWD"] = self._workspace_dir

        def _blocking_chat() -> str:
            return self._llm.chat(user_content)  # type: ignore[union-attr]

        try:
            text = await anyio.to_thread.run_sync(_blocking_chat)
        except Exception as exc:
            raise RuntimeError(_redact_secrets(f"hermes chat failed: {exc}")) from exc
        finally:
            if self._workspace_dir:
                if prev_cwd is None:
                    os.environ.pop("TERMINAL_CWD", None)
                else:
                    os.environ["TERMINAL_CWD"] = prev_cwd

        yield {
            "kind": "final",
            "id": str(uuid.uuid4()),
            "text": text,
        }


def make_real_runner(
    settings: Settings,
    session_id: str,
    agent_id: str | None = None,
) -> HermesRunner:
    agent: Agent | None = None
    if agent_id:
        agent = AgentStore(settings).get(agent_id)
        if agent is None:
            log.warning("agent_id %s not found; falling back to global config", agent_id)
    adapter: HermesAgentLike = _HermesAgentAdapter(settings, session_id, agent=agent)
    return HermesRunner(agent=adapter, session_id=session_id)
```

- [ ] **Step 4: Update `__main__.py` runner-binding line**

In `backend/hermes_bridge/__main__.py`, change:

```python
ws_chat.bind_runner_factory(lambda session_id: make_real_runner(settings, session_id))
```

to:

```python
ws_chat.bind_runner_factory(
    lambda session_id, agent_id: make_real_runner(settings, session_id, agent_id)
)
```

- [ ] **Step 5: Run tests**

Run: `cd backend && uv run pytest tests/test_hermes_factory.py tests/test_ws_chat.py -q`
Expected: 1 (existing) + 4 new factory tests + 5 ws tests = 10 passed.

- [ ] **Step 6: Run the full backend suite to catch downstream breakage**

Run: `cd backend && uv run pytest -q`
Expected: all previously-green tests still pass; new tests pass.

- [ ] **Step 7: Commit**

```bash
git add backend/hermes_bridge/bridge/hermes_factory.py backend/hermes_bridge/__main__.py backend/tests/test_hermes_factory.py
git commit -m "feat(backend): inject persona + TERMINAL_CWD into hermes AIAgent"
```

---

## Task 6: Frontend `agents-rest.ts` client

**Files:**
- Create: `apps/clawx-gui/src/lib/agents-rest.ts`
- Create: `apps/clawx-gui/src/lib/__tests__/agents-rest.test.ts`

- [ ] **Step 1: Write failing tests**

```ts
// apps/clawx-gui/src/lib/__tests__/agents-rest.test.ts
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  listAgents,
  createAgent,
  deleteAgent,
  rotateAgentSession,
  listToolsets,
} from "../agents-rest";

const mockFetch = vi.fn();

beforeEach(() => {
  vi.stubGlobal("fetch", mockFetch);
});
afterEach(() => {
  mockFetch.mockReset();
  vi.unstubAllGlobals();
});

function ok(body: unknown, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: "ok",
    json: async () => body,
  } as Response;
}

describe("agents-rest", () => {
  it("listAgents calls GET /api/agents with bearer", async () => {
    mockFetch.mockResolvedValueOnce(ok([{ id: "a1" }]));
    const out = await listAgents("T");
    expect(out).toEqual([{ id: "a1" }]);
    const [url, init] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/agents");
    expect((init as RequestInit).headers).toMatchObject({ Authorization: "Bearer T" });
  });

  it("createAgent POSTs JSON body", async () => {
    mockFetch.mockResolvedValueOnce(ok({ id: "new" }, 201));
    const payload = {
      name: "X", description: "", color: "#5749F4", icon: "Bot",
      system_prompt: "p", model: null, enabled_toolsets: ["web"],
    };
    const out = await createAgent(payload, "T");
    expect(out).toEqual({ id: "new" });
    const [url, init] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/agents");
    expect((init as RequestInit).method).toBe("POST");
    expect(JSON.parse((init as RequestInit).body as string)).toEqual(payload);
  });

  it("deleteAgent DELETE /api/agents/:id", async () => {
    mockFetch.mockResolvedValueOnce({ ok: true, status: 204 } as Response);
    await deleteAgent("aid", "T");
    const [url, init] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/agents/aid");
    expect((init as RequestInit).method).toBe("DELETE");
  });

  it("rotateAgentSession POSTs to /sessions and returns body", async () => {
    mockFetch.mockResolvedValueOnce(ok({ session_id: "rot" }));
    const out = await rotateAgentSession("aid", "T");
    expect(out).toEqual({ session_id: "rot" });
    const [url, init] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/agents/aid/sessions");
    expect((init as RequestInit).method).toBe("POST");
  });

  it("listToolsets calls GET /api/toolsets", async () => {
    mockFetch.mockResolvedValueOnce(ok([{ name: "web", description: "", tools: [] }]));
    const out = await listToolsets("T");
    expect(out[0].name).toBe("web");
  });
});
```

- [ ] **Step 2: Run — fail with import error**

Run: `pnpm --filter clawx-gui test -- --run src/lib/__tests__/agents-rest.test.ts`
Expected: file does not export `listAgents` etc.

- [ ] **Step 3: Implement `agents-rest.ts`**

```ts
// apps/clawx-gui/src/lib/agents-rest.ts
import { HermesApiError } from "./hermes-rest";

export interface Agent {
  id: string;
  name: string;
  description: string;
  color: string;
  icon: string;
  system_prompt: string;
  model: string | null;
  enabled_toolsets: string[];
  workspace_dir: string;
  current_session_id: string;
  created_at: number;
}

export interface AgentCreate {
  name: string;
  description: string;
  color: string;
  icon: string;
  system_prompt: string;
  model: string | null;
  enabled_toolsets: string[];
  workspace_dir?: string;
}

export interface Toolset {
  name: string;
  description: string;
  tools: string[];
}

async function call<T>(
  path: string,
  init: RequestInit & { token?: string } = {},
): Promise<T> {
  const { token, ...rest } = init;
  const headers: Record<string, string> = {
    ...(rest.headers as Record<string, string> | undefined),
  };
  if (token) headers.Authorization = `Bearer ${token}`;
  if (rest.body && !headers["Content-Type"]) headers["Content-Type"] = "application/json";
  const res = await fetch(path, { ...rest, headers });
  if (!res.ok) {
    let msg = `${res.status} ${res.statusText}`;
    try {
      const body = await res.json();
      if (body?.message) msg = body.message;
    } catch { /* ignore */ }
    throw new HermesApiError(res.status, msg);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export function listAgents(token: string): Promise<Agent[]> {
  return call<Agent[]>("/api/agents", { token });
}

export function createAgent(payload: AgentCreate, token: string): Promise<Agent> {
  return call<Agent>("/api/agents", { method: "POST", token, body: JSON.stringify(payload) });
}

export function deleteAgent(id: string, token: string): Promise<void> {
  return call<void>(`/api/agents/${encodeURIComponent(id)}`, { method: "DELETE", token });
}

export function rotateAgentSession(
  id: string,
  token: string,
): Promise<{ session_id: string }> {
  return call<{ session_id: string }>(
    `/api/agents/${encodeURIComponent(id)}/sessions`,
    { method: "POST", token },
  );
}

export function listToolsets(token: string): Promise<Toolset[]> {
  return call<Toolset[]>("/api/toolsets", { token });
}
```

- [ ] **Step 4: Run tests**

Run: `pnpm --filter clawx-gui test -- --run src/lib/__tests__/agents-rest.test.ts`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add apps/clawx-gui/src/lib/agents-rest.ts apps/clawx-gui/src/lib/__tests__/agents-rest.test.ts
git commit -m "feat(frontend): add agents-rest client (agents + toolsets)"
```

---

## Task 7: `ChatStore.replaceMessages`

**Files:**
- Modify: `apps/clawx-gui/src/lib/chat-store.ts`
- Modify: `apps/clawx-gui/src/lib/__tests__/chat-store.test.ts`

- [ ] **Step 1: Add a failing test**

Append to `apps/clawx-gui/src/lib/__tests__/chat-store.test.ts`:

```ts
import { ChatStore } from "../chat-store";

describe("replaceMessages", () => {
  it("maps server messages to ChatMessage[] and resets typing/error", () => {
    const s = new ChatStore();
    s.typing = true;
    s.lastError = { code: "x", message: "y" };
    s.addUser("old");
    s.replaceMessages([
      { role: "user", content: "u1" },
      { role: "assistant", content: "a1" },
    ]);
    expect(s.messages).toHaveLength(2);
    expect(s.messages[0].role).toBe("user");
    expect(s.messages[0].content).toBe("u1");
    expect(s.messages[1].role).toBe("assistant");
    expect(s.typing).toBe(false);
    expect(s.lastError).toBeNull();
  });

  it("ignores 'system' role from history", () => {
    const s = new ChatStore();
    s.replaceMessages([
      { role: "system", content: "ignored" },
      { role: "user", content: "ok" },
    ]);
    expect(s.messages.map((m) => m.role)).toEqual(["user"]);
  });
});
```

- [ ] **Step 2: Run — fail because `replaceMessages` doesn't exist**

Run: `pnpm --filter clawx-gui test -- --run src/lib/__tests__/chat-store.test.ts`
Expected: TypeError on missing method.

- [ ] **Step 3: Implement**

In `apps/clawx-gui/src/lib/chat-store.ts`, add:

```ts
import type { SessionMessage } from "./hermes-rest";
```

(at the existing import block)

…and add a method to the class:

```ts
  replaceMessages(serverMsgs: SessionMessage[]): void {
    this.messages = serverMsgs
      .filter((m) => m.role === "user" || m.role === "assistant")
      .map((m) => ({
        id: crypto.randomUUID(),
        role: m.role as "user" | "assistant",
        content: m.content,
        thought: false,
        ts: Date.now(),
      }));
    this.typing = false;
    this.lastError = null;
    this.emit();
  }
```

- [ ] **Step 4: Run tests**

Run: `pnpm --filter clawx-gui test -- --run src/lib/__tests__/chat-store.test.ts`
Expected: all chat-store tests pass.

- [ ] **Step 5: Commit**

```bash
git add apps/clawx-gui/src/lib/chat-store.ts apps/clawx-gui/src/lib/__tests__/chat-store.test.ts
git commit -m "feat(frontend): ChatStore.replaceMessages for history hydration"
```

---

## Task 8: Extend `ClawProvider` with agents/toolsets state

**Files:**
- Modify: `apps/clawx-gui/src/lib/store.tsx`
- Modify: `apps/clawx-gui/src/lib/__tests__/store.test.tsx`

- [ ] **Step 1: Write failing tests for new context surface**

Append to `apps/clawx-gui/src/lib/__tests__/store.test.tsx`:

```tsx
import { ClawProvider, useClaw } from "../store";
import { renderHook, act } from "@testing-library/react";

vi.mock("../agents-rest", () => ({
  listAgents: vi.fn().mockResolvedValue([
    {
      id: "a1", name: "n", description: "", color: "#5749F4", icon: "Bot",
      system_prompt: "p", model: null, enabled_toolsets: [],
      workspace_dir: "/tmp/a1", current_session_id: "sid-1", created_at: 1,
    },
  ]),
  listToolsets: vi.fn().mockResolvedValue([
    { name: "web", description: "", tools: [] },
  ]),
  createAgent: vi.fn(),
  deleteAgent: vi.fn().mockResolvedValue(undefined),
  rotateAgentSession: vi.fn().mockResolvedValue({ session_id: "sid-2" }),
}));

vi.mock("../hermes-rest", async () => {
  const actual = await vi.importActual<typeof import("../hermes-rest")>("../hermes-rest");
  return {
    ...actual,
    fetchHermesInfo: vi.fn().mockResolvedValue({
      configured: true, enabled: true, ws_url: "ws://x", provider: null, missing_env_var: null,
    }),
    getSession: vi.fn().mockResolvedValue({
      id: "sid-1", title: "", preview: "", message_count: 1,
      created: 0, updated: 0, summary: "",
      messages: [{ role: "user", content: "hi" }],
    }),
  };
});

describe("ClawProvider — agents", () => {
  it("loads agents + toolsets after token bootstraps", async () => {
    localStorage.setItem("clawx.dashboard_token", "T");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(result.current.agents.map((a) => a.id)).toEqual(["a1"]);
    expect(result.current.toolsets[0].name).toBe("web");
  });

  it("auto-selects first agent and hydrates history", async () => {
    localStorage.setItem("clawx.dashboard_token", "T");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(result.current.activeAgentId).toBe("a1");
    expect(result.current.chat.messages.map((m) => m.content)).toEqual(["hi"]);
  });

  it("newConversation rotates session_id and clears messages", async () => {
    localStorage.setItem("clawx.dashboard_token", "T");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    await act(async () => {
      await result.current.newConversation();
    });
    const a = result.current.agents.find((x) => x.id === "a1")!;
    expect(a.current_session_id).toBe("sid-2");
    expect(result.current.chat.messages).toEqual([]);
  });

  it("deleteAgent splices and falls back to first remaining", async () => {
    localStorage.setItem("clawx.dashboard_token", "T");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    await act(async () => { await result.current.deleteAgent("a1"); });
    expect(result.current.agents).toEqual([]);
    expect(result.current.activeAgentId).toBeNull();
  });
});
```

- [ ] **Step 2: Run — fail (`agents`, `activeAgentId`, `newConversation`, `deleteAgent` not on context)**

Run: `pnpm --filter clawx-gui test -- --run src/lib/__tests__/store.test.tsx`
Expected: type errors / runtime undefined.

- [ ] **Step 3: Update `store.tsx`**

Replace `apps/clawx-gui/src/lib/store.tsx` entirely:

```tsx
import {
  createContext, useCallback, useContext, useEffect, useMemo, useRef, useState,
  type ReactNode,
} from "react";
import { fetchHermesInfo, getSession, type HermesInfo } from "./hermes-rest";
import { HermesSocket } from "./hermes-socket";
import { ChatStore } from "./chat-store";
import {
  listAgents, listToolsets, createAgent as restCreateAgent,
  deleteAgent as restDeleteAgent, rotateAgentSession,
  type Agent, type AgentCreate, type Toolset,
} from "./agents-rest";

const TOKEN_KEY = "clawx.dashboard_token";
const ACTIVE_AGENT_KEY = "clawx.active_agent";

export interface ClawContextValue {
  token: string | null;
  wsUrl: string | null;
  enabled: boolean;
  configured: boolean;
  provider: string | null;
  missingEnvVar: string | null;
  agents: Agent[];
  toolsets: Toolset[];
  activeAgentId: string | null;
  activeAgent: Agent | null;
  chat: ChatStore;
  setToken: (token: string) => void;
  clearToken: () => void;
  refreshInfo: () => Promise<HermesInfo | null>;
  selectAgent: (id: string) => void;
  createAgent: (payload: AgentCreate) => Promise<Agent>;
  deleteAgent: (id: string) => Promise<void>;
  newConversation: () => Promise<void>;
  sendUserMessage: (content: string) => void;
}

const Ctx = createContext<ClawContextValue | null>(null);

export function ClawProvider({ children }: { children: ReactNode }) {
  const [token, setTokenState] = useState<string | null>(() => {
    try { return localStorage.getItem(TOKEN_KEY); } catch { return null; }
  });
  const [wsUrl, setWsUrl] = useState<string | null>(null);
  const [enabled, setEnabled] = useState(false);
  const [configured, setConfigured] = useState(false);
  const [provider, setProvider] = useState<string | null>(null);
  const [missingEnvVar, setMissingEnvVar] = useState<string | null>(null);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [toolsets, setToolsets] = useState<Toolset[]>([]);
  const [activeAgentId, setActiveAgentId] = useState<string | null>(() => {
    try { return localStorage.getItem(ACTIVE_AGENT_KEY); } catch { return null; }
  });

  const chatRef = useRef(new ChatStore());
  const sockRef = useRef<HermesSocket | null>(null);
  const [, forceRender] = useState(0);

  useEffect(() => chatRef.current.subscribe(() => forceRender((n) => n + 1)), []);

  const refreshInfo = useCallback(async (): Promise<HermesInfo | null> => {
    if (!token) {
      setWsUrl(null); setEnabled(false); setConfigured(false);
      setProvider(null); setMissingEnvVar(null);
      return null;
    }
    try {
      const info = await fetchHermesInfo(token);
      setWsUrl(info.ws_url); setEnabled(info.enabled); setConfigured(info.configured);
      setProvider(info.provider ?? null); setMissingEnvVar(info.missing_env_var ?? null);
      return info;
    } catch {
      setWsUrl(null); setEnabled(false); setConfigured(false);
      setProvider(null); setMissingEnvVar(null);
      return null;
    }
  }, [token]);

  useEffect(() => { void refreshInfo(); }, [refreshInfo]);

  // Bootstrap agents + toolsets after token resolves
  useEffect(() => {
    if (!token) { setAgents([]); setToolsets([]); return; }
    (async () => {
      try { setAgents(await listAgents(token)); } catch { /* ignore */ }
      try { setToolsets(await listToolsets(token)); } catch { /* ignore */ }
    })();
  }, [token]);

  // Auto-select first agent if none selected
  useEffect(() => {
    if (agents.length === 0) return;
    if (activeAgentId && agents.some((a) => a.id === activeAgentId)) return;
    setActiveAgentId(agents[0].id);
  }, [agents, activeAgentId]);

  // Persist active agent id
  useEffect(() => {
    try {
      if (activeAgentId) localStorage.setItem(ACTIVE_AGENT_KEY, activeAgentId);
      else localStorage.removeItem(ACTIVE_AGENT_KEY);
    } catch { /* ignore */ }
  }, [activeAgentId]);

  const activeAgent = useMemo<Agent | null>(
    () => agents.find((a) => a.id === activeAgentId) ?? null,
    [agents, activeAgentId],
  );

  // Hydrate history when active agent changes
  useEffect(() => {
    if (!token || !activeAgent) {
      chatRef.current.replaceMessages([]);
      return;
    }
    (async () => {
      try {
        const detail = await getSession(activeAgent.current_session_id, token);
        chatRef.current.replaceMessages(detail.messages);
      } catch {
        chatRef.current.replaceMessages([]);
      }
    })();
  }, [token, activeAgent?.id, activeAgent?.current_session_id]);

  // Connect WS for the active agent's current session
  useEffect(() => {
    if (!token || !wsUrl || !activeAgent) return;
    sockRef.current?.close();
    const s = new HermesSocket({
      wsBase: wsUrl,
      sessionId: activeAgent.current_session_id,
      token,
      onMessage: (m) => chatRef.current.applyServer(m),
      // append agent_id via opts.extraQuery — see HermesSocket update in this task
    });
    s.connect(activeAgent.id);
    sockRef.current = s;
    return () => s.close();
  }, [token, wsUrl, activeAgent?.id, activeAgent?.current_session_id]);

  const setToken = (t: string) => {
    try { localStorage.setItem(TOKEN_KEY, t); } catch { /* */ }
    setTokenState(t);
  };
  const clearToken = () => {
    try { localStorage.removeItem(TOKEN_KEY); } catch { /* */ }
    setTokenState(null);
  };

  const selectAgent = useCallback((id: string) => setActiveAgentId(id), []);

  const createAgentFn = useCallback(async (payload: AgentCreate) => {
    if (!token) throw new Error("not authenticated");
    const a = await restCreateAgent(payload, token);
    setAgents((prev) => [...prev, a]);
    setActiveAgentId(a.id);
    return a;
  }, [token]);

  const deleteAgentFn = useCallback(async (id: string) => {
    if (!token) return;
    await restDeleteAgent(id, token);
    setAgents((prev) => {
      const next = prev.filter((a) => a.id !== id);
      if (activeAgentId === id) setActiveAgentId(next[0]?.id ?? null);
      return next;
    });
  }, [token, activeAgentId]);

  const newConversation = useCallback(async () => {
    if (!token || !activeAgent) return;
    const { session_id } = await rotateAgentSession(activeAgent.id, token);
    setAgents((prev) =>
      prev.map((a) => a.id === activeAgent.id ? { ...a, current_session_id: session_id } : a),
    );
    chatRef.current.replaceMessages([]);
  }, [token, activeAgent]);

  const sendUserMessage = (content: string) => {
    if (!sockRef.current) return;
    const reqId = chatRef.current.addUser(content);
    sockRef.current.send({ type: "message.send", id: reqId, payload: { content } });
  };

  const value = useMemo<ClawContextValue>(
    () => ({
      token, wsUrl, enabled, configured, provider, missingEnvVar,
      agents, toolsets, activeAgentId, activeAgent,
      chat: chatRef.current,
      setToken, clearToken, refreshInfo,
      selectAgent, createAgent: createAgentFn, deleteAgent: deleteAgentFn,
      newConversation, sendUserMessage,
    }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [token, wsUrl, enabled, configured, provider, missingEnvVar,
     agents, toolsets, activeAgentId, activeAgent, refreshInfo,
     selectAgent, createAgentFn, deleteAgentFn, newConversation],
  );

  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useClaw(): ClawContextValue {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("useClaw must be used inside <ClawProvider>");
  return ctx;
}
```

- [ ] **Step 4: Update `HermesSocket` to accept `agent_id` in URL**

Modify `apps/clawx-gui/src/lib/hermes-socket.ts`'s `connect` signature:

```ts
  connect(agentId?: string | null): void {
    this.closedByUser = false;
    const params = new URLSearchParams({ session_id: this.opts.sessionId });
    if (agentId) params.set("agent_id", agentId);
    const url = `${this.opts.wsBase}?${params.toString()}`;
    const ws = new WebSocket(url, [`token.${this.opts.token}`]);
    // ... rest unchanged
  }
```

- [ ] **Step 5: Update existing first store test that asserts `startNewSession`**

Edit the first test in `apps/clawx-gui/src/lib/__tests__/store.test.tsx` ("with no stored token"): replace the `expect(typeof v.startNewSession).toBe("function")` line with:

```ts
    expect(typeof v.selectAgent).toBe("function");
    expect(typeof v.newConversation).toBe("function");
```

(The old `startNewSession` API is gone.)

- [ ] **Step 6: Run all frontend tests**

Run: `pnpm --filter clawx-gui test`
Expected: store tests and the new agent-bootstrap tests pass; ChatPage tests will likely fail — fixed in Task 11.

- [ ] **Step 7: Commit**

```bash
git add apps/clawx-gui/src/lib/store.tsx apps/clawx-gui/src/lib/hermes-socket.ts apps/clawx-gui/src/lib/__tests__/store.test.tsx
git commit -m "feat(frontend): ClawProvider manages agents/toolsets/active agent"
```

---

## Task 9: Real `AgentSidebar` driven by store

**Files:**
- Modify: `apps/clawx-gui/src/components/AgentSidebar.tsx`
- Create: `apps/clawx-gui/src/components/__tests__/AgentSidebar.test.tsx`

- [ ] **Step 1: Write failing test**

```tsx
// apps/clawx-gui/src/components/__tests__/AgentSidebar.test.tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import AgentSidebar from "../AgentSidebar";

const mockSelect = vi.fn();
const mockDelete = vi.fn();

vi.mock("../../lib/store", () => ({
  useClaw: () => ({
    agents: [
      { id: "a1", name: "编程助手", description: "", color: "#5749F4", icon: "Code2",
        system_prompt: "", model: null, enabled_toolsets: [],
        workspace_dir: "/", current_session_id: "s", created_at: 0 },
      { id: "a2", name: "研究助手", description: "", color: "#3B82F6", icon: "Search",
        system_prompt: "", model: null, enabled_toolsets: [],
        workspace_dir: "/", current_session_id: "s", created_at: 0 },
    ],
    activeAgentId: "a1",
    chat: { typing: false, messages: [] },
    selectAgent: mockSelect,
    deleteAgent: mockDelete,
  }),
}));

describe("AgentSidebar", () => {
  it("renders all agents and marks active", () => {
    render(<AgentSidebar />);
    const a1 = screen.getByRole("button", { name: /编程助手/ });
    const a2 = screen.getByRole("button", { name: /研究助手/ });
    expect(a1.className).toContain("is-active");
    expect(a2.className).not.toContain("is-active");
  });

  it("clicking an agent calls selectAgent", () => {
    render(<AgentSidebar />);
    fireEvent.click(screen.getByRole("button", { name: /研究助手/ }));
    expect(mockSelect).toHaveBeenCalledWith("a2");
  });

  it("delete button confirms then calls deleteAgent", () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<AgentSidebar />);
    fireEvent.click(screen.getByLabelText(/删除 编程助手/));
    expect(mockDelete).toHaveBeenCalledWith("a1");
  });

  it("delete cancelled — does not call deleteAgent", () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<AgentSidebar />);
    fireEvent.click(screen.getByLabelText(/删除 研究助手/));
    expect(mockDelete).not.toHaveBeenCalledWith("a2");
  });
});
```

- [ ] **Step 2: Run — fail (component still uses internal mock)**

Run: `pnpm --filter clawx-gui test -- --run src/components/__tests__/AgentSidebar.test.tsx`
Expected: failures.

- [ ] **Step 3: Replace `AgentSidebar.tsx`**

```tsx
// apps/clawx-gui/src/components/AgentSidebar.tsx
import { useState, type ComponentType } from "react";
import {
  Menu, ChevronDown, Plus, Search, Trash2,
  Code2, PenTool, BarChart3, Bot, MessageSquare, FileText, Lightbulb,
  Sparkles, Database, Globe, Wrench,
} from "lucide-react";
import IconButton from "./ui/IconButton";
import Input from "./ui/Input";
import CreateAgentModal from "./CreateAgentModal";
import { useClaw } from "../lib/store";

const ICONS: Record<string, ComponentType<{ size?: number }>> = {
  Code2, Search, PenTool, BarChart3, Bot, MessageSquare,
  FileText, Lightbulb, Sparkles, Database, Globe, Wrench,
};

export default function AgentSidebar() {
  const claw = useClaw();
  const [query, setQuery] = useState("");
  const [modalOpen, setModalOpen] = useState(false);

  const filtered = claw.agents.filter((a) =>
    a.name.toLowerCase().includes(query.trim().toLowerCase()),
  );

  return (
    <aside className="agent-sidebar" aria-label="Agent list">
      <div className="agent-sidebar__head">
        <button type="button" className="agent-sidebar__brand" aria-label="切换工作区">
          <Menu size={16} />
          <span>ZettClaw</span>
          <ChevronDown size={14} />
        </button>
      </div>

      <div className="agent-sidebar__search">
        <Input
          size="sm"
          leftIcon={<Search size={14} />}
          placeholder="搜索 Agent..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <IconButton
          icon={<Plus size={16} />}
          aria-label="新建 Agent"
          variant="ghost"
          size="sm"
          onClick={() => setModalOpen(true)}
        />
      </div>

      <div className="agent-sidebar__list">
        {filtered.length === 0 ? (
          <div className="agent-sidebar__placeholder">未找到匹配的 Agent</div>
        ) : (
          filtered.map((a) => {
            const Icon = ICONS[a.icon] ?? Bot;
            const isActive = a.id === claw.activeAgentId;
            const status = isActive && claw.chat.typing ? "running" : "idle";
            const statusText = status === "running" ? "Running" : "Idle";
            return (
              <button
                key={a.id}
                type="button"
                aria-label={a.name}
                className={`agent-item ${isActive ? "is-active" : ""}`.trim()}
                onClick={() => claw.selectAgent(a.id)}
              >
                <span
                  className="agent-item__avatar"
                  style={{ background: a.color }}
                  aria-hidden
                >
                  <Icon size={16} />
                </span>
                <span className="agent-item__text">
                  <span className="agent-item__name">{a.name}</span>
                  <span className="agent-item__status">
                    <span className={`agent-item__dot agent-item__dot--${status}`} />
                    {statusText}
                  </span>
                </span>
                <span
                  className="agent-item__delete"
                  role="button"
                  tabIndex={0}
                  aria-label={`删除 ${a.name}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    if (window.confirm(`删除 Agent 「${a.name}」？此操作不可撤销。`)) {
                      void claw.deleteAgent(a.id);
                    }
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      e.stopPropagation();
                      if (window.confirm(`删除 Agent 「${a.name}」？此操作不可撤销。`)) {
                        void claw.deleteAgent(a.id);
                      }
                    }
                  }}
                >
                  <Trash2 size={14} />
                </span>
              </button>
            );
          })
        )}
      </div>

      <CreateAgentModal open={modalOpen} onClose={() => setModalOpen(false)} />
    </aside>
  );
}
```

- [ ] **Step 4: Add hover-only delete affordance + a sliver of styling**

Append to `apps/clawx-gui/src/styles/pages/agent-sidebar.css`:

```css
.agent-item { position: relative; }
.agent-item__delete {
  display: none;
  position: absolute; right: 8px; top: 50%; transform: translateY(-50%);
  width: 24px; height: 24px;
  align-items: center; justify-content: center;
  border-radius: 6px;
  color: var(--muted-foreground);
}
.agent-item:hover .agent-item__delete { display: inline-flex; }
.agent-item__delete:hover { background: var(--destructive); color: var(--destructive-foreground); }
```

- [ ] **Step 5: Stub the modal so the test compiles**

Create `apps/clawx-gui/src/components/CreateAgentModal.tsx` (placeholder; full impl in Task 10):

```tsx
interface Props { open: boolean; onClose: () => void }
export default function CreateAgentModal({ open, onClose }: Props) {
  if (!open) return null;
  return (
    <div role="dialog" onClick={onClose}>
      <p>create agent</p>
    </div>
  );
}
```

- [ ] **Step 6: Run sidebar test**

Run: `pnpm --filter clawx-gui test -- --run src/components/__tests__/AgentSidebar.test.tsx`
Expected: 4 passed.

- [ ] **Step 7: Commit**

```bash
git add apps/clawx-gui/src/components/AgentSidebar.tsx apps/clawx-gui/src/components/CreateAgentModal.tsx apps/clawx-gui/src/components/__tests__/AgentSidebar.test.tsx apps/clawx-gui/src/styles/pages/agent-sidebar.css
git commit -m "feat(frontend): AgentSidebar driven by store with hover-delete"
```

---

## Task 10: `CreateAgentModal` full implementation

**Files:**
- Modify: `apps/clawx-gui/src/components/CreateAgentModal.tsx`
- Create: `apps/clawx-gui/src/components/__tests__/CreateAgentModal.test.tsx`
- Create: `apps/clawx-gui/src/styles/pages/create-agent-modal.css`
- Modify: `apps/clawx-gui/src/styles.css`

- [ ] **Step 1: Write failing test**

```tsx
// apps/clawx-gui/src/components/__tests__/CreateAgentModal.test.tsx
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import CreateAgentModal from "../CreateAgentModal";

const mockCreate = vi.fn().mockResolvedValue({ id: "new" });

vi.mock("../../lib/store", () => ({
  useClaw: () => ({
    toolsets: [
      { name: "web", description: "web tools", tools: [] },
      { name: "file", description: "file tools", tools: [] },
    ],
    createAgent: mockCreate,
  }),
}));

beforeEach(() => mockCreate.mockClear());

describe("CreateAgentModal", () => {
  it("required fields gate submission", async () => {
    const onClose = vi.fn();
    render(<CreateAgentModal open onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: /创建/ }));
    expect(mockCreate).not.toHaveBeenCalled();
    expect(screen.getByText(/请输入名称/)).toBeInTheDocument();
    expect(screen.getByText(/请输入 System Prompt/)).toBeInTheDocument();
  });

  it("submits payload with defaults: model=null, all toolsets enabled", async () => {
    const onClose = vi.fn();
    render(<CreateAgentModal open onClose={onClose} />);
    fireEvent.change(screen.getByLabelText(/名称/), { target: { value: "X" } });
    fireEvent.change(screen.getByLabelText(/System Prompt/), { target: { value: "be helpful" } });
    fireEvent.click(screen.getByRole("button", { name: /创建/ }));
    await waitFor(() => expect(mockCreate).toHaveBeenCalledTimes(1));
    const payload = mockCreate.mock.calls[0][0];
    expect(payload.name).toBe("X");
    expect(payload.system_prompt).toBe("be helpful");
    expect(payload.model).toBeNull();
    expect(payload.enabled_toolsets).toEqual(["web", "file"]);
    expect(onClose).toHaveBeenCalled();
  });

  it("custom model — picking a preset sets the value", async () => {
    render(<CreateAgentModal open onClose={() => {}} />);
    fireEvent.change(screen.getByLabelText(/名称/), { target: { value: "X" } });
    fireEvent.change(screen.getByLabelText(/System Prompt/), { target: { value: "p" } });
    fireEvent.click(screen.getByRole("button", { name: /自定义模型/ }));
    fireEvent.change(screen.getByLabelText(/选择模型/), { target: { value: "Sonnet 4.6" } });
    fireEvent.click(screen.getByRole("button", { name: /创建/ }));
    await waitFor(() => expect(mockCreate).toHaveBeenCalledTimes(1));
    expect(mockCreate.mock.calls[0][0].model).toBe("Sonnet 4.6");
  });

  it("toolsets — 全不选 then submit yields []", async () => {
    render(<CreateAgentModal open onClose={() => {}} />);
    fireEvent.change(screen.getByLabelText(/名称/), { target: { value: "X" } });
    fireEvent.change(screen.getByLabelText(/System Prompt/), { target: { value: "p" } });
    fireEvent.click(screen.getByRole("button", { name: /高级/ }));
    fireEvent.click(screen.getByRole("button", { name: /全不选/ }));
    fireEvent.click(screen.getByRole("button", { name: /创建/ }));
    await waitFor(() => expect(mockCreate).toHaveBeenCalledTimes(1));
    expect(mockCreate.mock.calls[0][0].enabled_toolsets).toEqual([]);
  });
});
```

- [ ] **Step 2: Run — fail (placeholder modal has no inputs)**

Run: `pnpm --filter clawx-gui test -- --run src/components/__tests__/CreateAgentModal.test.tsx`
Expected: 4 failures.

- [ ] **Step 3: Implement the modal**

Replace `apps/clawx-gui/src/components/CreateAgentModal.tsx`:

```tsx
import { useMemo, useState } from "react";
import {
  Code2, PenTool, BarChart3, Bot, MessageSquare, FileText, Lightbulb,
  Sparkles, Database, Globe, Wrench, Search,
} from "lucide-react";
import Dialog from "./ui/Dialog";
import Input from "./ui/Input";
import { useClaw } from "../lib/store";

const COLORS = [
  "#5749F4", "#3B82F6", "#EC4899", "#F59E0B",
  "#22C55E", "#EF4444", "#14B8A6", "#8B5CF6",
  "#F97316", "#06B6D4", "#84CC16", "#6366F1",
];

const ICON_OPTIONS: { name: string; Icon: React.ComponentType<{ size?: number }> }[] = [
  { name: "Code2", Icon: Code2 }, { name: "Search", Icon: Search },
  { name: "PenTool", Icon: PenTool }, { name: "BarChart3", Icon: BarChart3 },
  { name: "Bot", Icon: Bot }, { name: "MessageSquare", Icon: MessageSquare },
  { name: "FileText", Icon: FileText }, { name: "Lightbulb", Icon: Lightbulb },
  { name: "Sparkles", Icon: Sparkles }, { name: "Database", Icon: Database },
  { name: "Globe", Icon: Globe }, { name: "Wrench", Icon: Wrench },
];

const MODEL_PRESETS = [
  "Sonnet 4.6", "Opus 4.6", "Haiku 4.5",
  "GLM-4.5-Air", "GLM-4.5-Plus", "GPT-4o", "DeepSeek-V3",
];

interface Props { open: boolean; onClose: () => void }

export default function CreateAgentModal({ open, onClose }: Props) {
  const claw = useClaw();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [color, setColor] = useState(COLORS[0]);
  const [icon, setIcon] = useState(ICON_OPTIONS[0].name);
  const [systemPrompt, setSystemPrompt] = useState("");
  const [modelMode, setModelMode] = useState<"global" | "custom">("global");
  const [modelPreset, setModelPreset] = useState<string>(MODEL_PRESETS[0]);
  const [modelCustom, setModelCustom] = useState("");
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [enabledToolsets, setEnabledToolsets] = useState<Set<string>>(
    () => new Set(claw.toolsets.map((t) => t.name)),
  );
  const [submitErr, setSubmitErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [showErrors, setShowErrors] = useState(false);

  const allToolsetNames = useMemo(() => claw.toolsets.map((t) => t.name), [claw.toolsets]);

  const errors = {
    name: !name.trim() ? "请输入名称" : null,
    systemPrompt: !systemPrompt.trim() ? "请输入 System Prompt" : null,
  };

  function pickedModel(): string | null {
    if (modelMode === "global") return null;
    return modelPreset === "自定义..." ? modelCustom.trim() || null : modelPreset;
  }

  async function submit() {
    setShowErrors(true);
    if (errors.name || errors.systemPrompt) return;
    setSubmitErr(null);
    setBusy(true);
    try {
      await claw.createAgent({
        name: name.trim(),
        description: description.trim(),
        color,
        icon,
        system_prompt: systemPrompt,
        model: pickedModel(),
        enabled_toolsets:
          enabledToolsets.size === allToolsetNames.length
            ? allToolsetNames
            : allToolsetNames.filter((n) => enabledToolsets.has(n)),
      });
      onClose();
    } catch (e) {
      setSubmitErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onClose={onClose} title="新建 Agent" width={560}>
      <div className="create-agent">
        {submitErr && <div className="create-agent__error">{submitErr}</div>}

        <label className="create-agent__field">
          <span>名称</span>
          <Input
            size="md"
            value={name}
            onChange={(e) => setName(e.target.value)}
            aria-label="名称"
            placeholder="给它起个名字"
          />
          {showErrors && errors.name && <em className="create-agent__err">{errors.name}</em>}
        </label>

        <label className="create-agent__field">
          <span>描述</span>
          <Input
            size="md"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            aria-label="描述"
            placeholder="一句话副标题，可空"
          />
        </label>

        <div className="create-agent__field">
          <span>颜色</span>
          <div className="create-agent__swatches">
            {COLORS.map((c) => (
              <button
                key={c}
                type="button"
                aria-label={`颜色 ${c}`}
                aria-pressed={color === c}
                className={`create-agent__swatch ${color === c ? "is-active" : ""}`}
                style={{ background: c }}
                onClick={() => setColor(c)}
              />
            ))}
          </div>
        </div>

        <div className="create-agent__field">
          <span>图标</span>
          <div className="create-agent__icons">
            {ICON_OPTIONS.map(({ name: n, Icon }) => (
              <button
                key={n}
                type="button"
                aria-label={`图标 ${n}`}
                aria-pressed={icon === n}
                className={`create-agent__icon ${icon === n ? "is-active" : ""}`}
                onClick={() => setIcon(n)}
              >
                <Icon size={16} />
              </button>
            ))}
          </div>
        </div>

        <label className="create-agent__field">
          <span>System Prompt</span>
          <textarea
            className="ui-textarea"
            value={systemPrompt}
            onChange={(e) => setSystemPrompt(e.target.value)}
            rows={8}
            aria-label="System Prompt"
            placeholder="描述这个 Agent 的角色、风格、约束……"
          />
          {showErrors && errors.systemPrompt && (
            <em className="create-agent__err">{errors.systemPrompt}</em>
          )}
        </label>

        <div className="create-agent__field">
          <span>模型</span>
          <div className="create-agent__model-row">
            <button
              type="button"
              className={`create-agent__pill ${modelMode === "global" ? "is-active" : ""}`}
              onClick={() => setModelMode("global")}
            >
              跟随全局
            </button>
            <button
              type="button"
              aria-label="自定义模型"
              className={`create-agent__pill ${modelMode === "custom" ? "is-active" : ""}`}
              onClick={() => setModelMode("custom")}
            >
              自定义
            </button>
          </div>
          {modelMode === "custom" && (
            <>
              <select
                className="ui-select__control create-agent__select"
                aria-label="选择模型"
                value={modelPreset}
                onChange={(e) => setModelPreset(e.target.value)}
              >
                {MODEL_PRESETS.map((m) => (
                  <option key={m} value={m}>{m}</option>
                ))}
                <option value="自定义...">自定义...</option>
              </select>
              {modelPreset === "自定义..." && (
                <Input
                  size="sm"
                  aria-label="自定义模型名"
                  placeholder="例如 anthropic/claude-sonnet-4-6"
                  value={modelCustom}
                  onChange={(e) => setModelCustom(e.target.value)}
                />
              )}
            </>
          )}
        </div>

        <div className="create-agent__field">
          <button
            type="button"
            className="create-agent__advanced"
            onClick={() => setAdvancedOpen((v) => !v)}
          >
            {advancedOpen ? "▾" : "▸"} 高级
          </button>
          {advancedOpen && (
            <div className="create-agent__toolsets">
              <div className="create-agent__toolset-actions">
                <button type="button" onClick={() => setEnabledToolsets(new Set(allToolsetNames))}>全选</button>
                <button type="button" onClick={() => setEnabledToolsets(new Set())}>全不选</button>
              </div>
              {claw.toolsets.map((t) => (
                <label key={t.name} className="create-agent__toolset">
                  <input
                    type="checkbox"
                    checked={enabledToolsets.has(t.name)}
                    onChange={(e) => {
                      setEnabledToolsets((prev) => {
                        const next = new Set(prev);
                        if (e.target.checked) next.add(t.name);
                        else next.delete(t.name);
                        return next;
                      });
                    }}
                  />
                  <span>
                    <strong>{t.name}</strong>
                    {t.description && <small> — {t.description}</small>}
                  </span>
                </label>
              ))}
            </div>
          )}
        </div>

        <div className="create-agent__footer">
          <button type="button" onClick={onClose} disabled={busy}>取消</button>
          <button
            type="button"
            className="create-agent__submit"
            onClick={() => void submit()}
            disabled={busy}
          >
            {busy ? "创建中..." : "创建"}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
```

- [ ] **Step 4: Add CSS**

Create `apps/clawx-gui/src/styles/pages/create-agent-modal.css`:

```css
.create-agent { display: flex; flex-direction: column; gap: 14px; padding: 4px 0; }
.create-agent__field { display: flex; flex-direction: column; gap: 6px; }
.create-agent__field > span:first-child { color: var(--muted-foreground); font-size: 12px; }
.create-agent__error { background: var(--color-error); color: var(--color-error-foreground); padding: 8px 12px; border-radius: 8px; font-size: 13px; }
.create-agent__err { color: var(--destructive); font-size: 12px; font-style: normal; }
.create-agent__swatches, .create-agent__icons { display: flex; flex-wrap: wrap; gap: 6px; }
.create-agent__swatch { width: 24px; height: 24px; border-radius: 6px; border: 2px solid transparent; cursor: pointer; }
.create-agent__swatch.is-active { border-color: var(--foreground); }
.create-agent__icon { width: 32px; height: 32px; border-radius: 8px; background: var(--input); color: var(--foreground); display: inline-flex; align-items: center; justify-content: center; cursor: pointer; }
.create-agent__icon.is-active { background: var(--primary); color: var(--primary-foreground); }
.create-agent__model-row { display: flex; gap: 6px; }
.create-agent__pill { padding: 6px 14px; border-radius: 999px; background: var(--sidebar-accent); color: var(--muted-foreground); font-size: 12px; }
.create-agent__pill.is-active { background: var(--primary); color: var(--primary-foreground); }
.create-agent__select { background: var(--input); color: var(--foreground); border: 1px solid var(--border); border-radius: 8px; padding: 8px 12px; font: inherit; }
.create-agent__advanced { color: var(--muted-foreground); font-size: 12px; align-self: flex-start; cursor: pointer; }
.create-agent__toolsets { display: flex; flex-direction: column; gap: 4px; max-height: 240px; overflow: auto; padding: 8px; background: var(--input); border-radius: 8px; }
.create-agent__toolset-actions { display: flex; gap: 8px; margin-bottom: 6px; }
.create-agent__toolset-actions button { background: transparent; color: var(--muted-foreground); font-size: 12px; cursor: pointer; }
.create-agent__toolset { display: flex; align-items: flex-start; gap: 8px; font-size: 13px; padding: 4px 0; cursor: pointer; }
.create-agent__toolset small { color: var(--muted-foreground); }
.create-agent__footer { display: flex; gap: 8px; justify-content: flex-end; padding-top: 12px; border-top: 1px solid var(--border); }
.create-agent__footer button { padding: 8px 16px; border-radius: 8px; background: var(--sidebar-accent); color: var(--foreground); font: inherit; cursor: pointer; }
.create-agent__submit { background: var(--primary) !important; color: var(--primary-foreground) !important; }
.create-agent__footer button:disabled { opacity: 0.5; cursor: not-allowed; }
```

- [ ] **Step 5: Register CSS**

Add to `apps/clawx-gui/src/styles.css`:

```css
@import "./styles/pages/create-agent-modal.css";
```

- [ ] **Step 6: Run modal tests**

Run: `pnpm --filter clawx-gui test -- --run src/components/__tests__/CreateAgentModal.test.tsx`
Expected: 4 passed.

- [ ] **Step 7: Commit**

```bash
git add apps/clawx-gui/src/components/CreateAgentModal.tsx apps/clawx-gui/src/components/__tests__/CreateAgentModal.test.tsx apps/clawx-gui/src/styles/pages/create-agent-modal.css apps/clawx-gui/src/styles.css
git commit -m "feat(frontend): CreateAgentModal with persona fields + toolset picker"
```

---

## Task 11: ChatPage agent-aware welcome + "新对话" button

**Files:**
- Modify: `apps/clawx-gui/src/pages/ChatPage.tsx`
- Modify: `apps/clawx-gui/src/components/ChatWelcome.tsx`
- Modify: `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx`

- [ ] **Step 1: Update `ChatPage.test.tsx` to seed an agent + assert "新对话" button**

Replace the body of the first happy-path test in `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx` so the test mocks `agents-rest` and `getSession`:

```tsx
// at the top of the file, alongside the existing vi.mock("../../lib/hermes-rest", ...)
vi.mock("../../lib/agents-rest", () => ({
  listAgents: vi.fn().mockResolvedValue([
    {
      id: "a1", name: "test-agent", description: "", color: "#5749F4", icon: "Bot",
      system_prompt: "p", model: null, enabled_toolsets: [],
      workspace_dir: "/tmp/a1", current_session_id: "sid-1", created_at: 0,
    },
  ]),
  listToolsets: vi.fn().mockResolvedValue([]),
  createAgent: vi.fn(),
  deleteAgent: vi.fn(),
  rotateAgentSession: vi.fn(),
}));
```

In the existing `vi.mock("../../lib/hermes-rest", ...)` block, add `getSession` to the mock object that returns `{ messages: [] }`. If the existing block uses object literal style, append:

```ts
  getSession: vi.fn().mockResolvedValue({
    id: "sid-1", title: "", preview: "", message_count: 0,
    created: 0, updated: 0, summary: "", messages: [],
  }),
```

Add a new test:

```tsx
it("shows '新对话' button next to tabs", async () => {
  render(
    <MemoryRouter>
      <ClawProvider>
        <ChatPage />
      </ClawProvider>
    </MemoryRouter>,
  );
  await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
  expect(screen.getByRole("button", { name: /新对话/ })).toBeInTheDocument();
});
```

- [ ] **Step 2: Run the file — fail**

Run: `pnpm --filter clawx-gui test -- --run src/pages/__tests__/ChatPage.test.tsx`
Expected: failure on "button name 新对话" not found.

- [ ] **Step 3: Update `ChatPage.tsx`**

Replace `apps/clawx-gui/src/pages/ChatPage.tsx`:

```tsx
import { useEffect, useState } from "react";
import { RotateCcw } from "lucide-react";
import ChatInput from "../components/ChatInput";
import ChatWelcome from "../components/ChatWelcome";
import MessageBubble from "../components/MessageBubble";
import IconButton from "../components/ui/IconButton";
import { useClaw } from "../lib/store";

type ChatTab = "chat" | "artifacts";

export default function ChatPage() {
  const claw = useClaw();
  const [tab, setTab] = useState<ChatTab>("chat");

  useEffect(() => {
    /* WS connect handled inside ClawProvider; nothing to do here. */
  }, []);

  if (!claw.token) {
    return (
      <div className="empty-state">
        No dashboard token. Open <a href="/settings" className="underline">Settings</a> to paste yours.
      </div>
    );
  }
  if (!claw.enabled) {
    if (claw.missingEnvVar) {
      return (
        <div className="empty-state">
          Hermes is not ready: <code>{claw.missingEnvVar}</code> is not set.
          Add it to <code>~/.hermes/.env</code> (one line, e.g.
          <code className="mx-1">{claw.missingEnvVar}=...</code>)
          and restart the backend.
        </div>
      );
    }
    return (
      <div className="empty-state">
        Hermes is not configured. Run the bootstrap
        (<code>uv run --project backend python backend/scripts/init_config.py</code>)
        and restart <code className="mx-1">hermes_bridge</code>.
      </div>
    );
  }

  if (!claw.activeAgent) {
    return <div className="empty-state">请在左侧选择一个 Agent。</div>;
  }

  const { messages, typing } = claw.chat;

  function newConv() {
    if (window.confirm("开启新对话？当前会话历史将不再显示（仍保留在后端）。")) {
      void claw.newConversation();
    }
  }

  return (
    <div className="chat-page">
      <header className="chat-page__head">
        <nav className="page-tabs" role="tablist" aria-label="主区视图">
          <button
            type="button" role="tab" aria-selected={tab === "chat"}
            className={`page-tabs__trigger ${tab === "chat" ? "is-active" : ""}`}
            onClick={() => setTab("chat")}
          >对话</button>
          <button
            type="button" role="tab" aria-selected={tab === "artifacts"}
            className={`page-tabs__trigger ${tab === "artifacts" ? "is-active" : ""}`}
            onClick={() => setTab("artifacts")}
          >产物</button>
        </nav>
        <span className="chat-page__head-spacer" />
        <IconButton
          icon={<RotateCcw size={16} />}
          aria-label="新对话"
          title="新对话"
          variant="ghost"
          size="sm"
          onClick={newConv}
        />
      </header>

      <div className="chat-page__body">
        {tab === "artifacts" ? (
          <div className="chat-page__placeholder">暂无产物</div>
        ) : messages.length === 0 ? (
          <ChatWelcome agent={claw.activeAgent} />
        ) : (
          messages.map((m) => (
            <MessageBubble
              key={m.id}
              role={m.role}
              content={m.content}
              thought={m.thought}
            />
          ))
        )}
        {tab === "chat" && typing && (
          <div className="msg msg--assistant" data-testid="typing">
            <div className="msg__bubble msg__bubble--streaming">
              <span className="typing-indicator" aria-label="正在生成">
                <span /><span /><span />
              </span>
            </div>
          </div>
        )}
      </div>

      <footer className="chat-page__foot">
        <ChatInput onSubmit={(text) => claw.sendUserMessage(text)} />
      </footer>
    </div>
  );
}
```

Add to `apps/clawx-gui/src/styles/pages/chat-page.css`:

```css
.chat-page__head { display: flex; align-items: center; gap: 16px; }
.chat-page__head-spacer { flex: 1; }
```

- [ ] **Step 4: Update `ChatWelcome.tsx` to accept an optional `agent` prop**

```tsx
// apps/clawx-gui/src/components/ChatWelcome.tsx
import {
  Bot, MessageSquare, FileText, Lightbulb, ChevronRight,
  Code2, Search, PenTool, BarChart3, Sparkles, Database, Globe, Wrench,
} from "lucide-react";
import type { ComponentType } from "react";
import type { Agent } from "../lib/agents-rest";

const ICONS: Record<string, ComponentType<{ size?: number }>> = {
  Code2, Search, PenTool, BarChart3, Bot, MessageSquare,
  FileText, Lightbulb, Sparkles, Database, Globe, Wrench,
};

const TAGS = ["对话", "文件处理", "任务规划", "代码生成"];
const SUGGESTIONS = [
  { icon: MessageSquare, text: "帮我分析这段代码的性能问题" },
  { icon: FileText,      text: "将这份报告整理为周报格式" },
  { icon: Lightbulb,     text: "为新功能设计一个技术方案" },
];

interface Props { agent?: Agent | null }

export default function ChatWelcome({ agent }: Props) {
  const Icon = (agent && ICONS[agent.icon]) || Bot;
  const title = agent?.name ?? "MaxClaw";
  const subtitle =
    agent?.description ||
    "您的智能 AI 助手，擅长编程、研究和创意任务。随时提问或试下方的建议。";
  const heroBg = agent?.color ?? "var(--primary)";

  return (
    <div data-testid="chat-welcome" className="chat-welcome">
      <div className="chat-welcome__hero">
        <div className="chat-welcome__icon" style={{ background: heroBg }}>
          <Icon size={30} />
        </div>
        <h1 className="chat-welcome__title">{title}</h1>
        <p className="chat-welcome__subtitle">{subtitle}</p>
      </div>
      <div className="chat-welcome__tags">
        {TAGS.map((t) => (
          <button key={t} type="button" className="chat-welcome__tag">{t}</button>
        ))}
      </div>
      <ul className="chat-welcome__suggestions">
        {SUGGESTIONS.map((s) => (
          <li key={s.text}>
            <button type="button" className="chat-welcome__suggestion">
              <s.icon size={16} className="chat-welcome__suggestion-icon" />
              <span>{s.text}</span>
              <ChevronRight size={14} className="chat-welcome__suggestion-chevron" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

- [ ] **Step 5: Run the full frontend suite**

Run: `pnpm --filter clawx-gui test`
Expected: all tests pass (40+ from before + new ones added in tasks 6–11).

- [ ] **Step 6: Run TypeScript check**

Run: `cd apps/clawx-gui && pnpm exec tsc -b --noEmit`
Expected: no output (success).

- [ ] **Step 7: Commit**

```bash
git add apps/clawx-gui/src/pages/ChatPage.tsx apps/clawx-gui/src/components/ChatWelcome.tsx apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx apps/clawx-gui/src/styles/pages/chat-page.css
git commit -m "feat(frontend): ChatPage agent-aware welcome + 新对话 button"
```

---

## Task 12: End-to-end smoke in the browser

**Files:** none (verification only)

- [ ] **Step 1: Restart the bridge**

```bash
kill "$(lsof -ti :18800)" 2>/dev/null
( cd backend && uv run python -m hermes_bridge ) &
sleep 2
curl -s -H "Authorization: Bearer $(cat ~/.hermes/launcher-token)" http://127.0.0.1:18800/api/agents | python3 -m json.tool | head -20
```
Expected: 4 seeded agents (`code`, `research`, `writing`, `data`) returned, each with a `current_session_id` and `workspace_dir` under `~/.hermes/workspaces/`.

- [ ] **Step 2: Verify `/api/toolsets` returns hermes-agent's TOOLSETS**

```bash
curl -s -H "Authorization: Bearer $(cat ~/.hermes/launcher-token)" http://127.0.0.1:18800/api/toolsets | python3 -c 'import json,sys; d=json.load(sys.stdin); print(len(d), [t["name"] for t in d[:5]])'
```
Expected: roughly 24 toolsets, including `web`, `terminal`, `file`, `skills`.

- [ ] **Step 3: Browser smoke**

1. Open http://localhost:1420 in Chrome.
2. Confirm the four seeded agents render in the sidebar.
3. Click "编程助手" → send a message ("写一个 Python 反转字符串的函数") → assistant replies.
4. Click "新对话" → confirm dialog → message list clears → send a different prompt → reply.
5. Click "+" → modal opens. Fill name="测试 Agent", system_prompt="你是一个简洁的助手，回答不超过 30 字". Submit → modal closes → new agent appears + auto-selected → send "你是谁?" → reply ≤ 30 字.
6. Hover the "测试 Agent" row → trash icon → confirm → row disappears → first remaining agent becomes active.
7. Reload the page → confirm agents persist + active agent and most recent thread re-hydrate.

If any step fails, capture: console errors (chrome-devtools), `~/.hermes/agents.json` contents, and bridge stdout. Open a TODO with the specific symptom and stop here — do not commit a "fix" until root cause is understood.

- [ ] **Step 4: Run the full test suites one more time**

```bash
cd backend && uv run pytest -q
pnpm --filter clawx-gui test
```
Expected: both green.

- [ ] **Step 5: Final commit (if any verification-driven tweaks were made)**

If no changes were needed, this step is a no-op. Otherwise:

```bash
git add -A
git commit -m "fix(persona-agents): post-smoke adjustments"
```

---

## Self-Review

**Spec coverage:**

- §3.1 Agent record → Task 1 (`Agent` dataclass) + Task 6 (TS interface)
- §3.2 Storage (atomic write, version) → Task 1 (`_write` uses tmp+replace; version key in seed)
- §3.3 Default seeds → Task 1 (`_DEFAULT_SEEDS`, `_seed`)
- §4.1 AgentStore CRUD → Task 1 + tests
- §4.2 `/api/agents` router → Task 2
- §4.3 `/api/toolsets` router → Task 3
- §4.4 WS query param + factory signature → Task 4
- §4.5 Persona injection + TERMINAL_CWD → Task 5
- §4.6 Backend tests → Tasks 1–5 each include TDD tests
- §5.1 `agents-rest.ts` → Task 6
- §5.2 ClawProvider extensions → Task 8
- §5.3 `ChatStore.replaceMessages` → Task 7
- §5.4 Real `AgentSidebar` → Task 9
- §5.5 `CreateAgentModal` → Task 10
- §5.6 ChatPage adjustments + welcome agent prop → Task 11
- §5.7 Frontend tests → present in Tasks 6–11
- §6 End-to-end flows → Task 12 smoke
- §7 Risks acknowledged → Task 12 explicitly relies on single-agent-at-a-time assumption (no concurrency test)

**Type consistency:** `Agent` shape matches between `agent_store.py` (Python dataclass), `api/agents.py` (Pydantic shape minus server fields), `agents-rest.ts` (TS interface — same field names), and `store.tsx` consumers. `current_session_id` / `workspace_dir` / `enabled_toolsets` / `system_prompt` are spelled identically in every layer. WS query params match between `ws/chat.py`, `hermes-socket.ts`, and the `connect(agentId)` call in `store.tsx`.

**Placeholder scan:** none — every code step contains complete code, every command has expected output. The only "describes-what-not-how" line is "If any step fails, capture: console errors..." which is intentional triage guidance, not an implementation gap.

**Scope:** twelve tasks, each producing self-contained changes. Backend tasks 1–5 are independently committable; frontend tasks 6–11 build on each other but each lands on green tests; task 12 is verification-only.
