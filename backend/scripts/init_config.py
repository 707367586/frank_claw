"""Bootstrap `~/.hermes/` with a minimal config that makes the bridge `enabled`.

This replaces the Go `backend/scripts/init-config` used in v5.0. It does not
install hermes-agent itself (that is already a pip dep); it just drops a
starter YAML so `check_configured` returns True.
"""

from __future__ import annotations

import os
from pathlib import Path


CONFIG_TEMPLATE = """\
# ~/.hermes/config.yaml — minimal starter
# Replace `provider` and `model` with something you have credentials for.
# See hermes docs for supported providers.
provider: openrouter
model: anthropic/claude-3.5-sonnet

# Set the API key via env var matching the provider (e.g. OPENROUTER_API_KEY),
# or keep them in ~/.hermes/.env (hermes will read them at boot).
"""


def main() -> int:
    home = Path(os.environ.get("HERMES_HOME", Path.home() / ".hermes"))
    home.mkdir(parents=True, exist_ok=True)
    cfg = home / "config.yaml"
    if cfg.exists():
        print(f"exists, leaving untouched: {cfg}")
    else:
        cfg.write_text(CONFIG_TEMPLATE)
        print(f"wrote starter config: {cfg}")
    secrets = home / ".env"
    if not secrets.exists():
        secrets.write_text("# Add provider API keys here, one per line.\n# e.g. OPENROUTER_API_KEY=sk-...\n")
        os.chmod(secrets, 0o600)
        print(f"wrote (0600): {secrets}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
