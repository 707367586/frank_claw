from __future__ import annotations

import re
import shutil
from dataclasses import dataclass
from pathlib import Path

from ..config import Settings


@dataclass
class SkillInfo:
    name: str
    description: str | None = None
    installed: bool = True


_FM_RE = re.compile(r"^---\s*\n(.*?)\n---\s*\n", re.DOTALL)


def _parse_frontmatter(text: str) -> dict[str, str]:
    m = _FM_RE.match(text)
    if not m:
        return {}
    out: dict[str, str] = {}
    for line in m.group(1).splitlines():
        if ":" in line:
            k, v = line.split(":", 1)
            out[k.strip()] = v.strip()
    return out


class SkillService:
    def __init__(self, settings: Settings) -> None:
        self._root: Path = settings.hermes_home / "skills"

    def _iter_skill_dirs(self) -> list[Path]:
        if not self._root.exists():
            return []
        dirs: list[Path] = []
        for p in self._root.rglob("SKILL.md"):
            dirs.append(p.parent)
        return sorted(set(dirs))

    def list(self) -> list[SkillInfo]:
        out: list[SkillInfo] = []
        for d in self._iter_skill_dirs():
            fm = _parse_frontmatter((d / "SKILL.md").read_text(encoding="utf-8"))
            out.append(
                SkillInfo(
                    name=fm.get("name") or d.name,
                    description=fm.get("description") or None,
                    installed=True,
                )
            )
        return out

    def uninstall(self, name: str) -> None:
        for d in self._iter_skill_dirs():
            fm = _parse_frontmatter((d / "SKILL.md").read_text(encoding="utf-8"))
            if (fm.get("name") or d.name) == name:
                shutil.rmtree(d)
                return
        raise FileNotFoundError(name)

    def install(self, name: str) -> SkillInfo:
        """Fetch from agentskills.io or ClawHub. Not implemented in Phase 5;
        raises NotImplementedError so the REST call surfaces 501 until the
        fetcher lands."""
        raise NotImplementedError("skill install not implemented yet")
