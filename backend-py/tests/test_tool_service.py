import json

import pytest

from hermes_bridge.bridge.tool_service import ToolService
from hermes_bridge.config import Settings


@pytest.fixture
def svc(tmp_path, monkeypatch):
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    cfg = tmp_path / "toolsets.json"
    cfg.write_text(
        json.dumps(
            {
                "available": [
                    {"name": "fs_read", "description": "read files", "category": "fs"},
                    {"name": "shell", "description": "run shell", "category": "sys", "blocked_reason": None},
                    {
                        "name": "internet",
                        "description": "fetch web",
                        "category": "net",
                        "blocked_reason": "missing api key",
                    },
                ],
                "enabled": ["fs_read"],
            }
        )
    )
    return ToolService(Settings())


def test_list_reflects_status(svc):
    tools = {t.name: t for t in svc.list()}
    assert tools["fs_read"].status == "enabled"
    assert tools["shell"].status == "disabled"
    assert tools["internet"].status == "blocked"
    assert tools["internet"].reason_code == "missing api key"


def test_set_enabled_true(svc):
    svc.set_enabled("shell", True)
    t = {x.name: x for x in svc.list()}["shell"]
    assert t.status == "enabled"


def test_set_enabled_false(svc):
    svc.set_enabled("fs_read", False)
    t = {x.name: x for x in svc.list()}["fs_read"]
    assert t.status == "disabled"


def test_set_enabled_blocked_is_rejected(svc):
    with pytest.raises(ValueError):
        svc.set_enabled("internet", True)


def test_set_enabled_unknown_raises(svc):
    with pytest.raises(KeyError):
        svc.set_enabled("does-not-exist", True)
