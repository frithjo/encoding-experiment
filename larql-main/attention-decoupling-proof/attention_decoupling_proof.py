#!/usr/bin/env python3
"""Thin activator for Rust real-GGUF separated attention proof."""

from __future__ import annotations

import argparse
import subprocess
from pathlib import Path


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    root = repo_root()
    default_gguf = root / "attention-decoupling-proof" / "qwen3.5-0.8b.gguf"
    default_out = root / "attention-decoupling-proof"
    parser = argparse.ArgumentParser()
    parser.add_argument("--gguf", type=Path, default=default_gguf)
    parser.add_argument("--out", type=Path, default=default_out)
    parser.add_argument("--layer", type=int)
    parser.add_argument("--seq-len", type=int, default=8)
    parser.add_argument("--tolerance", type=float, default=1.0e-6)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    command = [
        "cargo",
        "run",
        "-q",
        "-p",
        "larql-cli",
        "--",
        "attention-runtime",
        "proof",
        "--gguf",
        str(args.gguf),
        "--seq-len",
        str(args.seq_len),
        "--tolerance",
        str(args.tolerance),
        "--output",
        str(args.out / "separated-attention-runtime-gguf-layer-proof.json"),
    ]
    if args.layer is not None:
        command.extend(["--layer", str(args.layer)])
    return subprocess.run(command, cwd=repo_root(), check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
