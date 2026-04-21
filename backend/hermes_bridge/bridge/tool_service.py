from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

from ..config import Settings

ToolStatus = Literal["enabled", "disabled", "blocked"]


@dataclass
class ToolInfo:
    name: str
    status: ToolStatus
    description: str | None = None
    category: str | None = None
    config_key: str | None = None
    reason_code: str | None = None


class ToolService:
    """Stores toolset enabled/disabled state in a single JSON file under
    ~/.hermes/toolsets.json. Blocked tools are identified by a non-null
    `blocked_reason` in the available list.

    hermes-agent's `toolsets.py` does not expose a programmatic toggle
    (enablement is an AIAgent-construction arg). Until it does, this JSON
    file is our persistent source of truth; `hermes_factory.py` will read
    it and translate to `enabled_toolsets=`/`disabled_toolsets=` when
    constructing AIAgent (follow-up; scaffolded here).
    """

    FILENAME = "toolsets.json"

    def __init__(self, settings: Settings) -> None:
        self._path: Path = settings.hermes_home / self.FILENAME

    def _load(self) -> dict:
        if not self._path.exists():
            return {"available": [], "enabled": []}
        return json.loads(self._path.read_text() or "{}")

    def _save(self, data: dict) -> None:
        self._path.parent.mkdir(parents=True, exist_ok=True)
        self._path.write_text(json.dumps(data, indent=2))

    def list(self) -> list[ToolInfo]:
        d = self._load()
        enabled = set(d.get("enabled") or [])
        out: list[ToolInfo] = []
        for a in d.get("available") or []:
            blocked = a.get("blocked_reason")
            if blocked:
                status: ToolStatus = "blocked"
            elif a["name"] in enabled:
                status = "enabled"
            else:
                status = "disabled"
            out.append(
                ToolInfo(
                    name=a["name"],
                    status=status,
                    description=a.get("description"),
                    category=a.get("category"),
                    config_key=a.get("config_key"),
                    reason_code=blocked,
                )
            )
        return out

    def set_enabled(self, name: str, enabled: bool) -> None:
        d = self._load()
        available = {a["name"]: a for a in d.get("available") or []}
        if name not in available:
            raise KeyError(name)
        if available[name].get("blocked_reason"):
            raise ValueError(f"tool '{name}' is blocked and cannot be enabled")
        cur = set(d.get("enabled") or [])
        if enabled:
            cur.add(name)
        else:
            cur.discard(name)
        d["enabled"] = sorted(cur)
        self._save(d)
