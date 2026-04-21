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
