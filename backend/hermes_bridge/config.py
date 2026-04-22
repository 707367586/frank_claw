from pathlib import Path

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    host: str = "127.0.0.1"
    port: int = 18800
    launcher_token: str | None = Field(default=None, alias="HERMES_LAUNCHER_TOKEN")
    hermes_home: Path = Field(
        default_factory=lambda: Path.home() / ".hermes",
        alias="HERMES_HOME",
    )
    log_level: str = Field(default="INFO", alias="HERMES_BRIDGE_LOG_LEVEL")
    webroot: Path | None = Field(default=None, alias="HERMES_BRIDGE_WEBROOT")
    no_browser: bool = Field(default=True, alias="HERMES_BRIDGE_NO_BROWSER")

    model_config = SettingsConfigDict(env_prefix="", extra="ignore")


def get_settings() -> Settings:
    return Settings()  # reads env on each call; fine for a local dev tool


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
