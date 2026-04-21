from fastapi import FastAPI

from .api import info as info_api
from .config import Settings, get_settings
from .logging_setup import configure_logging


def create_app(settings: Settings | None = None) -> FastAPI:
    s = settings or get_settings()
    configure_logging(s.log_level)
    app = FastAPI(title="hermes_bridge", version="0.1.0")

    @app.get("/healthz")
    def healthz() -> dict[str, bool]:
        return {"ok": True}

    app.include_router(info_api.make_router(s))
    return app
