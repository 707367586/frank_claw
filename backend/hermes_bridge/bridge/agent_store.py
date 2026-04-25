from __future__ import annotations

import json
import os
import time
import uuid
from dataclasses import asdict, dataclass
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
