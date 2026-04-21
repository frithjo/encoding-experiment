from __future__ import annotations

import argparse


def main() -> None:
    parser = argparse.ArgumentParser(description="Serve LARQL Workbench UI.")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", default=8000, type=int)
    args = parser.parse_args()

    try:
        import uvicorn
    except ImportError as exc:  # pragma: no cover - depends on local env
        raise SystemExit(
            "UI deps missing. Install with `uv sync --extra ui --group dev` or `pip install 'larql[ui]'`."
        ) from exc

    uvicorn.run("larql.ui.app:create_app", factory=True, host=args.host, port=args.port)


if __name__ == "__main__":
    main()
