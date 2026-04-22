# Zhipu (GLM) Chat End-to-End Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Opening the ClawX client (`pnpm dev`, then `http://localhost:1420`) lets the user paste their dashboard token once, type a message into ChatPage, and receive a real reply from hermes-agent 0.10.0 running on top of Zhipu GLM (`zai` provider) — no manual YAML editing required.

**Architecture:** Everything hermes-side already exists in `backend/hermes_bridge/`. This plan (a) makes `backend/scripts/init_config.py` an interactive provider bootstrap that writes `~/.hermes/config.yaml` + `~/.hermes/.env` for Zhipu, (b) loads `~/.hermes/.env` into the backend process at startup so `GLM_API_KEY` reaches `AIAgent`, (c) upgrades `check_configured` to verify the selected provider's env var is present (not just that the file exists), and (d) rewords the frontend "not configured" banner to mention the missing env var. No changes to the wire protocol or React chat store. hermes-agent is kept as a git dependency at the pinned commit already in `backend/uv.lock` (`NousResearch/hermes-agent@7fc1e91`).

**Tech Stack:** Python 3.11, FastAPI, `python-dotenv` (already installed transitively), `pyyaml` (already installed transitively), pytest + pytest-asyncio. React 19 + Vite (frontend-side change is a tiny copy edit).

**Provider target:** Zhipu (`zai` in hermes-agent's provider registry) — OpenAI-compatible transport, default CN base URL `https://open.bigmodel.cn/api/paas/v4`. Default chat model: `glm-4.5-flash` (cheapest/fastest; matches hermes-agent's `auxiliary_client.py` default for this provider). Users can edit `~/.hermes/config.yaml` to switch to `glm-5.1`, `glm-4.7`, etc.

---

## Critical Context for the Engineer

Before starting Task 1, read these files (all already in the repo):

1. `backend/hermes_bridge/__main__.py` — uvicorn entry point; this is where env loading is inserted.
2. `backend/hermes_bridge/bridge/hermes_factory.py` — imports `run_agent.AIAgent`; already correct for hermes-agent 0.10.0 (verified — `AIAgent` is defined at line 708 of the installed `run_agent.py`).
3. `backend/scripts/init_config.py` — the current bootstrap; this plan replaces it.
4. `backend/hermes_bridge/api/info.py` — the `check_configured` helper; this plan strengthens it.
5. `apps/clawx-gui/src/pages/ChatPage.tsx` — the "not configured" branch at line 22; this plan rewords the message.

### What already works (do not touch)

- `HermesRunner`, the WS `/hermes/ws` path, the subprotocol auth, the Pydantic wire models, the `/api/hermes/info` endpoint, and the React `hermes-rest.ts`/`hermes-socket.ts` clients. Chat plumbing is complete end-to-end *once* `AIAgent` boots with a valid provider + key.

### What's broken (what this plan fixes)

- `~/.hermes/config.yaml` is written with `provider: openrouter` by the current `init_config.py`, which is wrong for a Zhipu-first user.
- `~/.hermes/.env` is created empty with no prompt for a key.
- Even if the user pastes `GLM_API_KEY=...` into `~/.hermes/.env`, the backend process never reads that file, so `AIAgent` initializes without the key and hermes fails at first request.
- `check_configured` returns `True` as soon as `config.yaml` exists, so the frontend reports "enabled" and ChatPage sends a message that then dies inside `AIAgent.chat()`, producing a confusing `runner_failure` toast.

### Zhipu provider facts (hermes-agent 0.10.0, verified in `.venv`)

- Provider key in hermes registry: `zai` (aliases `zhipu`, `glm`, `z-ai`, `z.ai` are recognized by `model_normalize.py`).
- Env var: `GLM_API_KEY` (also `ZAI_API_KEY`, `Z_AI_API_KEY`).
- Default base URL (CN region): `https://open.bigmodel.cn/api/paas/v4` (hermes auth.py line 432). Override via `GLM_BASE_URL`.
- Transport: `openai_chat` — same wire format as OpenAI chat completions; auth is `Authorization: Bearer <GLM_API_KEY>`.
- Supported chat models include `glm-5`, `glm-5.1`, `glm-5v-turbo`, `glm-4.7`, `glm-4.5-flash`. Plan default = `glm-4.5-flash`.

### Security rule (enforced throughout)

The API key never goes into any file inside the repo. It lives only in `~/.hermes/.env` (mode `0600`, outside the repo, already in the user's local filesystem). `init_config.py` reads the key via `input()` / `getpass.getpass()` — never from `sys.argv` (would leak into shell history) — and writes it directly to `~/.hermes/.env`. No test commits a real key; test fixtures use the string `"test-glm-key-not-real"`.

---

## File Structure

### Changed

```
backend/
├── hermes_bridge/
│   ├── __main__.py                # MODIFIED — load ~/.hermes/.env before create_app
│   ├── api/
│   │   └── info.py                # MODIFIED — check_configured parses YAML + verifies env var
│   └── config.py                  # MODIFIED — add `HERMES_ENV_FILE` setting
├── scripts/
│   └── init_config.py             # REWRITTEN — interactive provider + key bootstrap
└── tests/
    ├── test_env_loading.py        # NEW
    ├── test_init_config.py        # NEW
    └── test_info_check.py         # NEW (or ADDED TO if a test_info file already exists)

apps/clawx-gui/src/
├── pages/
│   └── ChatPage.tsx               # MODIFIED — "not configured" message mentions the env var
└── pages/__tests__/
    └── ChatPage.test.tsx          # MODIFIED — update assertion on the banner text

README.md                          # MODIFIED — Zhipu quick-start section replaces the stub
```

### Unchanged

Everything in `backend/hermes_bridge/bridge/`, `backend/hermes_bridge/ws/`, `backend/hermes_bridge/api/sessions.py`, `backend/hermes_bridge/api/skills.py`, `backend/hermes_bridge/api/tools.py`, and all of `apps/clawx-gui/src/lib/`. The Python dep graph and `pyproject.toml` are unchanged (`python-dotenv` and `pyyaml` are already resolved in `uv.lock` via hermes-agent's transitive closure — confirmed by `ls backend/.venv/lib/python3.11/site-packages | grep -iE "dotenv|yaml"`).

---

## Task 1: Load `~/.hermes/.env` at backend startup

**Files:**
- Modify: `backend/hermes_bridge/config.py`
- Modify: `backend/hermes_bridge/__main__.py`
- Test: `backend/tests/test_env_loading.py`

**Why first:** Without this, every later task's smoke test would fail even after the user writes a correct `.env` — the Python process simply doesn't see the key. TDD confirms the loading happens *before* `create_app`, which is what `AIAgent` eventually depends on.

- [ ] **Step 1.1: Write the failing test**

Create `backend/tests/test_env_loading.py`:

```python
"""hermes_bridge must load ~/.hermes/.env into os.environ before the FastAPI
app is constructed, so that AIAgent sees provider credentials."""
from __future__ import annotations

import os
from pathlib import Path

import pytest


def test_load_hermes_env_populates_os_environ(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    # Arrange: a fake HERMES_HOME with a .env containing one key.
    home = tmp_path / ".hermes"
    home.mkdir()
    (home / ".env").write_text("GLM_API_KEY=test-glm-key-not-real\n")
    monkeypatch.setenv("HERMES_HOME", str(home))
    monkeypatch.delenv("GLM_API_KEY", raising=False)

    # Act: call the loader that __main__ will use.
    from hermes_bridge.config import load_hermes_env

    load_hermes_env()

    # Assert
    assert os.environ.get("GLM_API_KEY") == "test-glm-key-not-real"


def test_load_hermes_env_does_not_overwrite_existing(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    home = tmp_path / ".hermes"
    home.mkdir()
    (home / ".env").write_text("GLM_API_KEY=from-dotfile\n")
    monkeypatch.setenv("HERMES_HOME", str(home))
    monkeypatch.setenv("GLM_API_KEY", "from-shell")

    from hermes_bridge.config import load_hermes_env

    load_hermes_env()

    # Shell env wins (principle of least surprise for dev overrides).
    assert os.environ["GLM_API_KEY"] == "from-shell"


def test_load_hermes_env_is_silent_when_file_missing(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    home = tmp_path / ".hermes"
    home.mkdir()  # exists but no .env inside
    monkeypatch.setenv("HERMES_HOME", str(home))

    from hermes_bridge.config import load_hermes_env

    # Must not raise.
    load_hermes_env()
```

- [ ] **Step 1.2: Run the test to verify it fails**

Run: `cd backend && uv run pytest tests/test_env_loading.py -v`
Expected: FAIL with `ImportError: cannot import name 'load_hermes_env' from 'hermes_bridge.config'`

- [ ] **Step 1.3: Implement `load_hermes_env` in config.py**

Append to `backend/hermes_bridge/config.py` (keep the existing `Settings` class unchanged):

```python
def load_hermes_env() -> None:
    """Populate os.environ from ~/.hermes/.env if present. Shell env wins.

    Called from __main__ before create_app() so AIAgent sees provider keys.
    No-op when the file does not exist (lets tests and first-run work).
    """
    import os

    from dotenv import load_dotenv

    home = Path(os.environ.get("HERMES_HOME", Path.home() / ".hermes"))
    env_file = home / ".env"
    if env_file.exists():
        load_dotenv(env_file, override=False)
```

- [ ] **Step 1.4: Run the test to verify it passes**

Run: `cd backend && uv run pytest tests/test_env_loading.py -v`
Expected: 3 passed

- [ ] **Step 1.5: Wire the loader into `__main__.py`**

Edit `backend/hermes_bridge/__main__.py`. Replace:

```python
from .config import Settings, get_settings
```

with:

```python
from .config import Settings, get_settings, load_hermes_env
```

Then inside `main()`, add a call **before** `settings = get_settings()` (so `Settings.hermes_home` resolves against the same environment the dotenv populated):

```python
def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="hermes-bridge")
    p.add_argument("--port", type=int, default=None)
    p.add_argument("--host", default=None)
    p.add_argument("--webroot", type=Path, default=None)
    p.add_argument("--no-browser", action="store_true")
    args = p.parse_args(argv)

    load_hermes_env()  # ← new line

    settings = get_settings()
    # ... rest unchanged
```

- [ ] **Step 1.6: Sanity-run the backend boots**

Run: `cd backend && uv run python -c "from hermes_bridge.__main__ import main; import sys; sys.argv=['hermes-bridge','--help']; main()"`
Expected: argparse help text printed, no import error.

- [ ] **Step 1.7: Commit**

```bash
git add backend/hermes_bridge/config.py backend/hermes_bridge/__main__.py backend/tests/test_env_loading.py
git commit -m "feat(backend): load ~/.hermes/.env into os.environ at uvicorn startup"
```

---

## Task 2: Rewrite `init_config.py` with interactive Zhipu bootstrap

**Files:**
- Rewrite: `backend/scripts/init_config.py`
- Test: `backend/tests/test_init_config.py`

**Why:** After Task 1, the dotenv mechanism exists, but nothing helps the user write the file correctly. This task makes `pnpm dev:backend:setup` a one-shot interactive bootstrap: pick provider (Zhipu default), paste key, done.

- [ ] **Step 2.1: Write the failing test (happy path + idempotency + key preservation)**

Create `backend/tests/test_init_config.py`:

```python
"""init_config.py: interactive bootstrap for ~/.hermes/{config.yaml,.env}."""
from __future__ import annotations

import os
from pathlib import Path

import pytest


def _run_init(tmp_home: Path, answers: list[str], monkeypatch: pytest.MonkeyPatch) -> int:
    """Invoke init_config.main() with HERMES_HOME pointed at tmp and stdin fed `answers`."""
    import io
    import sys

    monkeypatch.setenv("HERMES_HOME", str(tmp_home))
    # Feed stdin. `getpass` reads from the tty; we monkeypatch it to use input().
    import getpass

    monkeypatch.setattr(getpass, "getpass", lambda prompt="": input(prompt))
    monkeypatch.setattr(sys, "stdin", io.StringIO("\n".join(answers) + "\n"))

    from importlib import reload

    import scripts.init_config as mod

    reload(mod)
    return mod.main()


def test_bootstrap_writes_zhipu_config_and_env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    home = tmp_path / ".hermes"

    # Answers: provider=1 (zai), model=<default>, key=paste
    rc = _run_init(home, answers=["1", "", "test-glm-key-not-real"], monkeypatch=monkeypatch)

    assert rc == 0
    cfg = (home / "config.yaml").read_text()
    assert "provider: zai" in cfg
    assert "model: glm-4.5-flash" in cfg  # default

    env = (home / ".env").read_text()
    assert "GLM_API_KEY=test-glm-key-not-real" in env

    # .env must be mode 0600 (owner-only).
    assert (home / ".env").stat().st_mode & 0o777 == 0o600


def test_bootstrap_custom_model(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    home = tmp_path / ".hermes"
    rc = _run_init(home, answers=["1", "glm-5.1", "test-glm-key-not-real"], monkeypatch=monkeypatch)
    assert rc == 0
    assert "model: glm-5.1" in (home / "config.yaml").read_text()


def test_bootstrap_is_idempotent_and_preserves_existing_key(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    home = tmp_path / ".hermes"
    home.mkdir()
    (home / ".env").write_text("GLM_API_KEY=already-here\n")
    (home / ".env").chmod(0o600)

    # Answers: provider=1 (zai), model=<default>, key=<blank = keep existing>
    rc = _run_init(home, answers=["1", "", ""], monkeypatch=monkeypatch)
    assert rc == 0

    env = (home / ".env").read_text()
    assert "GLM_API_KEY=already-here" in env  # unchanged
    assert env.count("GLM_API_KEY=") == 1  # no duplicate line
```

- [ ] **Step 2.2: Run the test to verify it fails**

Run: `cd backend && uv run pytest tests/test_init_config.py -v`
Expected: FAIL (either `ImportError` on `scripts.init_config` — because there's no `__init__.py` in `scripts/` — or assertion mismatch).

If the import fails, add an empty `backend/scripts/__init__.py` so pytest can discover the module:

```bash
touch backend/scripts/__init__.py
```

Re-run the test and confirm it now fails on the assertion, not the import.

- [ ] **Step 2.3: Rewrite `backend/scripts/init_config.py`**

Replace the whole file contents with:

```python
"""Interactive bootstrap for ~/.hermes/config.yaml and ~/.hermes/.env.

Picks a provider (Zhipu GLM by default), writes a minimal config, and stores
the API key in ~/.hermes/.env with mode 0600. Safe to re-run: prompts with
"[keep existing]" when a key is already present.

Replaces the v5.0 Go backend/scripts/init-config and the earlier stub that
only wrote an OpenRouter default.
"""
from __future__ import annotations

import getpass
import os
import sys
from dataclasses import dataclass
from pathlib import Path

# Provider registry — a small slice of hermes-agent's `HERMES_OVERLAYS` suitable
# for interactive selection. Keep ordering stable: the first entry is the
# default pick (Zhipu).
@dataclass(frozen=True)
class ProviderChoice:
    key: str           # hermes provider id (goes into config.yaml `provider:`)
    label: str         # user-facing display name
    env_var: str       # canonical env var for the API key
    default_model: str # sensible default model id for config.yaml `model:`


PROVIDERS: list[ProviderChoice] = [
    ProviderChoice("zai", "Zhipu GLM (智谱 / Z.AI)", "GLM_API_KEY", "glm-4.5-flash"),
    ProviderChoice("anthropic", "Anthropic Claude", "ANTHROPIC_API_KEY", "claude-3-5-sonnet-latest"),
    ProviderChoice("openrouter", "OpenRouter (aggregator)", "OPENROUTER_API_KEY", "anthropic/claude-3.5-sonnet"),
    ProviderChoice("openai", "OpenAI", "OPENAI_API_KEY", "gpt-4o-mini"),
    ProviderChoice("deepseek", "DeepSeek", "DEEPSEEK_API_KEY", "deepseek-chat"),
]


def _ask(prompt: str, default: str = "") -> str:
    """Prompt wrapper. Blank response returns the default."""
    suffix = f" [{default}]" if default else ""
    ans = input(f"{prompt}{suffix}: ").strip()
    return ans or default


def _pick_provider() -> ProviderChoice:
    print("Pick a provider:")
    for i, p in enumerate(PROVIDERS, start=1):
        print(f"  {i}. {p.label}")
    while True:
        choice = _ask("Provider number", default="1")
        if choice.isdigit() and 1 <= int(choice) <= len(PROVIDERS):
            return PROVIDERS[int(choice) - 1]
        print("Invalid choice, try again.")


def _write_config_yaml(path: Path, provider: ProviderChoice, model: str) -> None:
    body = (
        "# ~/.hermes/config.yaml — written by backend/scripts/init_config.py\n"
        "# Edit freely; re-running init_config.py will prompt before overwriting.\n"
        f"provider: {provider.key}\n"
        f"model: {model}\n"
    )
    path.write_text(body)
    print(f"wrote: {path}")


def _read_env_file(path: Path) -> dict[str, str]:
    if not path.exists():
        return {}
    out: dict[str, str] = {}
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, _, v = line.partition("=")
        out[k.strip()] = v.strip()
    return out


def _write_env_file(path: Path, entries: dict[str, str]) -> None:
    header = "# ~/.hermes/.env — provider API keys. Mode 0600. NEVER commit.\n"
    body = header + "\n".join(f"{k}={v}" for k, v in entries.items()) + "\n"
    path.write_text(body)
    os.chmod(path, 0o600)
    print(f"wrote (0600): {path}")


def main() -> int:
    home = Path(os.environ.get("HERMES_HOME", Path.home() / ".hermes"))
    home.mkdir(parents=True, exist_ok=True)

    provider = _pick_provider()
    model = _ask(f"Model id for {provider.label}", default=provider.default_model)

    existing_env = _read_env_file(home / ".env")
    has_existing_key = bool(existing_env.get(provider.env_var))
    key_prompt = (
        f"{provider.env_var} (paste; hidden)"
        + (" [keep existing, press Enter]" if has_existing_key else "")
    )
    try:
        pasted = getpass.getpass(key_prompt + ": ").strip()
    except EOFError:  # non-tty (CI); treat as blank
        pasted = ""

    if pasted:
        existing_env[provider.env_var] = pasted
    elif not has_existing_key:
        print(
            f"WARNING: no {provider.env_var} provided and none in ~/.hermes/.env; "
            "the bridge will report `enabled=false` until you paste a key.",
            file=sys.stderr,
        )

    _write_config_yaml(home / "config.yaml", provider, model)
    _write_env_file(home / ".env", existing_env)
    print("done. Restart `pnpm dev` (or the backend) to pick up the new config.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2.4: Run the test to verify it passes**

Run: `cd backend && uv run pytest tests/test_init_config.py -v`
Expected: 3 passed

- [ ] **Step 2.5: Manual dry-run**

Run: `cd backend && HERMES_HOME=/tmp/hermes-dryrun uv run python scripts/init_config.py`
Answer: `1` → Enter → paste a dummy string like `dummy-key`.
Expected: `/tmp/hermes-dryrun/config.yaml` contains `provider: zai`; `/tmp/hermes-dryrun/.env` is mode `0600` and contains `GLM_API_KEY=dummy-key`.

Clean up: `rm -rf /tmp/hermes-dryrun`.

- [ ] **Step 2.6: Commit**

```bash
git add backend/scripts/__init__.py backend/scripts/init_config.py backend/tests/test_init_config.py
git commit -m "feat(backend): interactive init_config with Zhipu/GLM as default provider"
```

---

## Task 3: Strengthen `check_configured` — parse YAML + verify env var

**Files:**
- Modify: `backend/hermes_bridge/api/info.py`
- Test: `backend/tests/test_info_check.py` (new file; if one already exists, add the test cases to it instead)

**Why:** After Task 1 + 2, the happy path works. But if the user runs `init_config.py` and never pastes a key (or exports one later that disappears on reboot), the current `check_configured` still returns `True` because the YAML file exists — and the frontend misleadingly says "Hermes is enabled" before the WS errors out on the first chat. This task makes `enabled` mean "the backend has everything it needs to talk to a model."

- [ ] **Step 3.1: Write the failing test**

Create `backend/tests/test_info_check.py`:

```python
"""info.check_configured must verify YAML + matching env var, not just file presence."""
from __future__ import annotations

import os
from pathlib import Path

import pytest

from hermes_bridge.api.info import check_configured
from hermes_bridge.config import Settings


def _settings(home: Path) -> Settings:
    return Settings(HERMES_HOME=home)


def test_no_config_file_returns_false(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.delenv("GLM_API_KEY", raising=False)
    assert check_configured(_settings(tmp_path)) is False


def test_config_without_env_var_returns_false(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "config.yaml").write_text("provider: zai\nmodel: glm-4.5-flash\n")
    monkeypatch.delenv("GLM_API_KEY", raising=False)
    monkeypatch.delenv("ZAI_API_KEY", raising=False)
    monkeypatch.delenv("Z_AI_API_KEY", raising=False)
    assert check_configured(_settings(tmp_path)) is False


def test_config_plus_env_var_returns_true(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "config.yaml").write_text("provider: zai\nmodel: glm-4.5-flash\n")
    monkeypatch.setenv("GLM_API_KEY", "anything-non-empty")
    assert check_configured(_settings(tmp_path)) is True


def test_unknown_provider_returns_false(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "config.yaml").write_text("provider: totally-made-up\nmodel: foo\n")
    assert check_configured(_settings(tmp_path)) is False


def test_anthropic_provider_reads_anthropic_key(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    (tmp_path / "config.yaml").write_text("provider: anthropic\nmodel: x\n")
    monkeypatch.setenv("ANTHROPIC_API_KEY", "anthropic-key")
    monkeypatch.delenv("GLM_API_KEY", raising=False)
    assert check_configured(_settings(tmp_path)) is True
```

- [ ] **Step 3.2: Run the test to verify it fails**

Run: `cd backend && uv run pytest tests/test_info_check.py -v`
Expected: FAIL — existing implementation returns `True` whenever the file exists, so `test_config_without_env_var_returns_false` and `test_unknown_provider_returns_false` fail.

- [ ] **Step 3.3: Rewrite `check_configured` in `info.py`**

Replace the body of `check_configured` in `backend/hermes_bridge/api/info.py`. Also add a small module-level `_PROVIDER_ENV_VARS` mapping. The file becomes:

```python
from __future__ import annotations

import os
from pathlib import Path

import yaml
from fastapi import APIRouter, Depends
from pydantic import BaseModel

from ..auth import require_bearer_token
from ..config import Settings


class InfoResponse(BaseModel):
    configured: bool
    enabled: bool
    ws_url: str
    provider: str | None = None
    missing_env_var: str | None = None


# Known providers. Must match keys in hermes-agent's provider registry
# (hermes_cli/providers.py). Keep in sync with scripts/init_config.PROVIDERS.
_PROVIDER_ENV_VARS: dict[str, tuple[str, ...]] = {
    "zai": ("GLM_API_KEY", "ZAI_API_KEY", "Z_AI_API_KEY"),
    "anthropic": ("ANTHROPIC_API_KEY",),
    "openrouter": ("OPENROUTER_API_KEY",),
    "openai": ("OPENAI_API_KEY",),
    "deepseek": ("DEEPSEEK_API_KEY",),
}


def _load_provider(home: Path) -> str | None:
    for name in ("config.yaml", "config.yml"):
        path = home / name
        if not path.exists():
            continue
        try:
            doc = yaml.safe_load(path.read_text()) or {}
        except yaml.YAMLError:
            return None
        provider = doc.get("provider")
        return provider if isinstance(provider, str) else None
    return None


def check_configured(settings: Settings) -> bool:
    """True iff config.yaml picks a known provider AND its env var is set."""
    provider = _load_provider(settings.hermes_home)
    if provider is None:
        return False
    env_vars = _PROVIDER_ENV_VARS.get(provider)
    if not env_vars:
        return False
    return any(os.environ.get(v) for v in env_vars)


def _missing_env_var(settings: Settings) -> str | None:
    """For the selected provider, the canonical env var if none is set."""
    provider = _load_provider(settings.hermes_home)
    if provider is None:
        return None
    env_vars = _PROVIDER_ENV_VARS.get(provider, ())
    if not env_vars or any(os.environ.get(v) for v in env_vars):
        return None
    return env_vars[0]  # canonical


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/hermes", tags=["info"])
    dep = Depends(require_bearer_token(settings))

    @r.get("/info", response_model=InfoResponse, dependencies=[dep])
    def get_info() -> InfoResponse:
        provider = _load_provider(settings.hermes_home)
        configured = check_configured(settings)
        return InfoResponse(
            configured=configured,
            enabled=configured,
            ws_url=f"ws://{settings.host}:{settings.port}/hermes/ws",
            provider=provider,
            missing_env_var=_missing_env_var(settings) if not configured else None,
        )

    return r
```

- [ ] **Step 3.4: Run the test to verify it passes**

Run: `cd backend && uv run pytest tests/test_info_check.py -v`
Expected: 5 passed.

- [ ] **Step 3.5: Run the full backend test suite to check nothing else regressed**

Run: `cd backend && uv run pytest -q`
Expected: all tests pass. If a prior test asserted `enabled=True` from file-only presence, update that test to also set `GLM_API_KEY` (or whatever provider it used) in the arrange step — the new contract is strictly stronger.

- [ ] **Step 3.6: Commit**

```bash
git add backend/hermes_bridge/api/info.py backend/tests/test_info_check.py
git commit -m "feat(backend): check_configured verifies provider env var, not just YAML presence"
```

---

## Task 4: Update ChatPage "not configured" banner

**Files:**
- Modify: `apps/clawx-gui/src/pages/ChatPage.tsx`
- Modify (if present): `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx`
- Modify: `apps/clawx-gui/src/lib/hermes-types.ts` (the `/api/hermes/info` response type)

**Why:** Task 3 taught the backend to report *why* it's not enabled (which env var is missing). The frontend should surface that instead of the generic "run the config bootstrap" message — otherwise Task 3's extra signal is wasted.

- [ ] **Step 4.1: Locate the info type definition**

Run: `grep -n "ws_url\|InfoResponse\|HermesInfo" apps/clawx-gui/src/lib/hermes-types.ts | head`

If the file declares e.g. `export interface HermesInfo { configured: boolean; enabled: boolean; ws_url: string }`, extend it:

```ts
export interface HermesInfo {
  configured: boolean;
  enabled: boolean;
  ws_url: string;
  provider: string | null;
  missing_env_var: string | null;
}
```

If the file uses a different name, extend that instead. Run `grep -rn "hermes/info\|HermesInfo\|enabled" apps/clawx-gui/src/lib/` to find all readers and make sure they tolerate the two new optional-ish fields (all consumers should continue to work since the new fields are additive).

- [ ] **Step 4.2: Write/update the ChatPage test**

If `apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx` exists, add this test case; otherwise create the file:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import ChatPage from "../ChatPage";
import * as storeModule from "../../lib/store";

describe("ChatPage — not-configured banner", () => {
  it("shows missing env var when backend reports one", () => {
    vi.spyOn(storeModule, "useClaw").mockReturnValue({
      token: "t",
      enabled: false,
      missingEnvVar: "GLM_API_KEY",
      provider: "zai",
      sessionId: null,
      chat: { messages: [], typing: false },
      startNewSession: () => {},
      sendUserMessage: () => {},
    } as unknown as ReturnType<typeof storeModule.useClaw>);

    render(<ChatPage />);
    expect(screen.getByText(/GLM_API_KEY/)).toBeInTheDocument();
    expect(screen.getByText(/~\/\.hermes\/\.env/)).toBeInTheDocument();
  });

  it("falls back to generic message when no hint available", () => {
    vi.spyOn(storeModule, "useClaw").mockReturnValue({
      token: "t",
      enabled: false,
      missingEnvVar: null,
      provider: null,
      sessionId: null,
      chat: { messages: [], typing: false },
      startNewSession: () => {},
      sendUserMessage: () => {},
    } as unknown as ReturnType<typeof storeModule.useClaw>);

    render(<ChatPage />);
    expect(screen.getByText(/init_config\.py/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 4.3: Run the test to verify it fails**

Run: `pnpm --filter clawx-gui test ChatPage`
Expected: FAIL — both assertions miss.

- [ ] **Step 4.4: Thread the new fields through the store**

Open `apps/clawx-gui/src/lib/store.tsx`. Find where `/api/hermes/info` is fetched and the result stored. Extend the store's exported shape to include `missingEnvVar: string | null` and `provider: string | null`, populated from `info.missing_env_var` and `info.provider`. Be careful: keep existing fields unchanged so current callers (ChatInput, MessageBubble, SettingsPage) keep compiling. Concretely, when you find code like:

```ts
setState({ enabled: info.enabled, ws_url: info.ws_url, ... });
```

extend to:

```ts
setState({
  enabled: info.enabled,
  ws_url: info.ws_url,
  provider: info.provider ?? null,
  missingEnvVar: info.missing_env_var ?? null,
  // ...
});
```

Initialize both new fields to `null` in the store's initial state.

- [ ] **Step 4.5: Replace the `ChatPage` banner**

Edit `apps/clawx-gui/src/pages/ChatPage.tsx`. Replace lines 22-31 (the `if (!claw.enabled)` branch) with:

```tsx
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
```

- [ ] **Step 4.6: Run the test to verify it passes**

Run: `pnpm --filter clawx-gui test ChatPage`
Expected: both new test cases pass; previously existing ChatPage tests still pass.

- [ ] **Step 4.7: Run all frontend tests**

Run: `pnpm --filter clawx-gui test`
Expected: full green. If any test references the old banner copy, update it to use the new "not ready" / "not configured" split.

- [ ] **Step 4.8: Commit**

```bash
git add apps/clawx-gui/src/pages/ChatPage.tsx apps/clawx-gui/src/lib/hermes-types.ts apps/clawx-gui/src/lib/store.tsx apps/clawx-gui/src/pages/__tests__/ChatPage.test.tsx
git commit -m "feat(gui): surface missing provider env var in ChatPage not-configured banner"
```

---

## Task 5: README Zhipu quick-start

**Files:**
- Modify: `README.md` (top-level)

**Why:** The current README says "Put the API key in `~/.hermes/.env`" without saying *which* key or *how*. After Tasks 1-4 the happy path is one command — the README should show it.

- [ ] **Step 5.1: Replace the "Quick start" section**

Open `README.md`. Replace the block beginning at the "## Quick start" heading and ending at the "This brings up:" paragraph with:

````markdown
## Quick start

```bash
# 1. Install JS deps + Python deps, and run the interactive bootstrap.
pnpm install
pnpm dev:backend:setup
# The bootstrap will:
#   - prompt for a provider (default: Zhipu GLM — press Enter)
#   - prompt for a model (default: glm-4.5-flash — press Enter)
#   - prompt for your API key (paste; input is hidden)
#     -> for Zhipu, use your key from https://open.bigmodel.cn/
# It writes ~/.hermes/config.yaml and ~/.hermes/.env (mode 0600).

# 2. Start everything.
pnpm dev
```

This brings up:
- **Backend** (`hermes_bridge`, Python/FastAPI/uvicorn) on `http://127.0.0.1:18800`
- **Frontend** (Vite dev server) on `http://localhost:1420`

Open `http://localhost:1420`. On first load the backend prints `dashboardToken: <…>` to its stdout — paste it into the Settings page once; after that it's in `localStorage`.
````

Leave the rest of the README as-is.

- [ ] **Step 5.2: Commit**

```bash
git add README.md
git commit -m "docs(readme): Zhipu-first quick start with interactive bootstrap"
```

---

## Task 6: Manual end-to-end smoke

**Files:** None (this is a checklist, not code). If anything fails, file the fix as a follow-up task rather than inlining here — this plan's contract is the four code tasks above.

- [ ] **Step 6.1: Clean slate (optional, do once to reproduce a first-run)**

```bash
rm -rf ~/.hermes/config.yaml ~/.hermes/.env
```

(Leaves `~/.hermes/sessions/`, `~/.hermes/memories/` etc. intact.)

- [ ] **Step 6.2: Run the bootstrap and paste the Zhipu key**

```bash
cd /Users/zhoulingfeng/Desktop/code/makemoney/frank_claw
pnpm dev:backend:setup
```

Answer:
- Provider: Enter (default: `1` = Zhipu GLM)
- Model: Enter (default: `glm-4.5-flash`)
- Key: paste your Zhipu API key (input is hidden)

Expected output:
```
wrote: /Users/zhoulingfeng/.hermes/config.yaml
wrote (0600): /Users/zhoulingfeng/.hermes/.env
done. Restart `pnpm dev` (or the backend) to pick up the new config.
```

- [ ] **Step 6.3: Verify files**

```bash
cat ~/.hermes/config.yaml
ls -l ~/.hermes/.env   # must show -rw-------
```

Expected: `provider: zai`, `model: glm-4.5-flash`; `.env` is mode `0600`.

- [ ] **Step 6.4: Boot `pnpm dev` and capture the dashboard token**

```bash
pnpm dev
```

Watch for the line `dashboardToken: <token>` on the backend stream. Copy it.

- [ ] **Step 6.5: Load the UI, paste the token, send a message**

1. Open `http://localhost:1420`.
2. Navigate to Settings. Paste the dashboard token. Save.
3. Navigate back to `/` (Chat).
4. The "not configured" banner must NOT appear. If it does with `missingEnvVar: GLM_API_KEY`, stop — Task 1's dotenv loading did not fire. Debug: `echo $GLM_API_KEY` inside the uvicorn process (add a one-line `print(os.environ.get("GLM_API_KEY"))` in `__main__.py` before `uvicorn.run` to confirm).
5. Type `你好，介绍一下你自己` and send.
6. Expected: typing indicator flashes, then a Chinese GLM reply appears in a new message bubble.

- [ ] **Step 6.6: Verify the WS lifecycle in browser devtools**

Network → WS → `/hermes/ws`. Frames should include:
- Outbound: `{ type: "message.send", content: "你好..." }`
- Inbound: `{ type: "typing.start" }` → `{ type: "message.create", message_id, content, thought: false }` → `{ type: "typing.stop" }`

If the inbound side emits `{ type: "error", code: "runner_failure" }`, copy the error message and investigate — most common cause will be a wrong model id (Zhipu rejects unknown models with an HTTP 400); change `model:` in `~/.hermes/config.yaml` to `glm-4-flash` or `glm-5` and restart the backend.

- [ ] **Step 6.7: Cleanup**

Stop `pnpm dev` (Ctrl-C). Plan complete.

---

## Self-Review

Ran through the plan against the goal ("open client → chat with agent via Zhipu"):

1. **Env loading gap** closed by Task 1. Without it, pasting a key does nothing.
2. **Config bootstrap rewritten** by Task 2 — default is Zhipu, key entry is hidden via `getpass`, idempotent re-runs don't clobber existing keys.
3. **`check_configured` hardened** by Task 3 so the frontend stops lying about readiness.
4. **Frontend banner** updated by Task 4 to turn the new signal into actionable UI text.
5. **README** by Task 5 so a new teammate can reproduce the setup.
6. **Manual smoke** by Task 6 covers the actual user-visible flow.

**Placeholder scan:** Every code step contains the full code change. No `TBD` / `implement later` / `similar to above` / "add appropriate error handling" strings. The README paragraph block is the exact text to paste.

**Type consistency:** `missingEnvVar` (camelCase) is the frontend property; `missing_env_var` (snake_case) is the JSON field from FastAPI. The conversion happens at the store boundary (Task 4.4). `HermesInfo` includes both new fields as nullable, matching the `InfoResponse` Pydantic model from Task 3. `check_configured` and `_missing_env_var` share the same `_PROVIDER_ENV_VARS` table — adding a provider is a one-line change in two files (`init_config.PROVIDERS` + `info._PROVIDER_ENV_VARS`); the README's "supported providers" line implicitly derives from those two.

**Spec coverage:** The user's request has one concrete requirement — "open client → chat with agent" — with an implicit "on Zhipu". All six tasks serve that single path. No dead-weight tasks.
