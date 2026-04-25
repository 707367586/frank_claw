from __future__ import annotations

import argparse
import secrets
import sys
from pathlib import Path

import uvicorn

from .app import create_app
from .bridge.hermes_factory import make_real_runner
from .config import Settings, get_settings, load_hermes_env
from .ws import chat as ws_chat


def _ensure_token(settings: Settings) -> str:
    if settings.launcher_token:
        return settings.launcher_token
    token_file = settings.hermes_home / "launcher-token"
    token_file.parent.mkdir(parents=True, exist_ok=True)
    if token_file.exists():
        t = token_file.read_text().strip()
        if t:
            return t
    t = secrets.token_urlsafe(32)
    token_file.write_text(t)
    return t


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="hermes-bridge")
    p.add_argument("--port", type=int, default=None)
    p.add_argument("--host", default=None)
    p.add_argument("--webroot", type=Path, default=None)
    p.add_argument("--no-browser", action="store_true")
    args = p.parse_args(argv)

    load_hermes_env()

    settings = get_settings()
    if args.port is not None:
        settings.port = args.port
    if args.host is not None:
        settings.host = args.host
    if args.webroot is not None:
        settings.webroot = args.webroot
    if args.no_browser:
        settings.no_browser = True

    token = _ensure_token(settings)
    settings.launcher_token = token
    print(f"dashboardToken: {token}", flush=True)

    app = create_app(settings)
    ws_chat.bind_runner_factory(
        lambda session_id, _agent_id: make_real_runner(settings, session_id)
    )

    uvicorn.run(app, host=settings.host, port=settings.port, log_level=settings.log_level.lower())
    return 0


if __name__ == "__main__":
    sys.exit(main())
