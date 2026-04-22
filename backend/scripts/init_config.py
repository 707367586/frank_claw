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
