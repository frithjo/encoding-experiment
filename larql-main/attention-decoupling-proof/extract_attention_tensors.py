#!/usr/bin/env python3
"""Thin activator for Rust raw-GGUF attention tensor extraction."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    root = repo_root()
    default_gguf = root / "attention-decoupling-proof" / "qwen3.5-0.8b.gguf"
    default_out = root / "attention-decoupling-proof" / "qwen3.5-0.8b-attention-rust"
    parser = argparse.ArgumentParser()
    parser.add_argument("gguf", nargs="?", type=Path, default=default_gguf)
    parser.add_argument("--out", type=Path, default=default_out)
    parser.add_argument(
        "--scope",
        choices=("decoder", "decoder-mtp", "all"),
        default="all",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "larql-cli",
        "--",
        "attention-runtime",
        "extract-gguf",
        "--gguf",
        str(args.gguf),
        "--scope",
        args.scope,
        "--out",
        str(args.out),
    ]
    return subprocess.run(command, cwd=repo_root(), check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
