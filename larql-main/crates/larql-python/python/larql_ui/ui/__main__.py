from __future__ import annotations

import argparse
import sys
from .app import create_app
import uvicorn


def main() -> None:
    """Launch the LARQL workbench UI server."""
    parser = argparse.ArgumentParser(description="LARQL Workbench UI")
    parser.add_argument("--host", default="127.0.0.1", help="Host to bind to")
    parser.add_argument("--port", type=int, default=8000, help="Port to bind to")
    args = parser.parse_args()

    try:
        app = create_app()
    except ImportError as exc:  # pragma: no cover - depends on local env
        raise SystemExit(
            "UI deps missing. Install with `uv sync --extra ui --group dev` or `pip install 'larql[ui]'`."
        ) from exc

    uvicorn.run(app, host=args.host, port=args.port)


if __name__ == "__main__":
    main()
