import textwrap

import pytest

from hermes_bridge.bridge.skill_service import SkillService
from hermes_bridge.config import Settings


@pytest.fixture
def svc(tmp_path, monkeypatch):
    monkeypatch.setenv("HERMES_HOME", str(tmp_path))
    monkeypatch.setenv("HERMES_LAUNCHER_TOKEN", "t")
    (tmp_path / "skills" / "openclaw-imports" / "my-skill").mkdir(parents=True)
    (tmp_path / "skills" / "openclaw-imports" / "my-skill" / "SKILL.md").write_text(
        textwrap.dedent(
            """\
            ---
            name: my-skill
            description: Does a thing
            ---

            body
            """
        )
    )
    (tmp_path / "skills" / "bare").mkdir(parents=True)
    (tmp_path / "skills" / "bare" / "SKILL.md").write_text("no frontmatter here")
    return SkillService(Settings())


def test_list_reads_frontmatter(svc):
    skills = svc.list()
    by_name = {s.name: s for s in skills}
    assert "my-skill" in by_name
    assert by_name["my-skill"].description == "Does a thing"
    assert by_name["my-skill"].installed is True
    assert "bare" in by_name
    assert by_name["bare"].description is None


def test_uninstall_removes_skill_dir(svc, tmp_path):
    svc.uninstall("my-skill")
    assert not (tmp_path / "skills" / "openclaw-imports" / "my-skill").exists()


def test_uninstall_missing_raises(svc):
    with pytest.raises(FileNotFoundError):
        svc.uninstall("does-not-exist")


def test_install_raises_not_implemented(svc):
    with pytest.raises(NotImplementedError):
        svc.install("some-skill")
