#!/usr/bin/env python3
"""Guardrail for oversized Cargo target directories."""

from __future__ import annotations

import argparse
from pathlib import Path


def dir_size_bytes(path: Path) -> int:
    total = 0
    for p in path.rglob("*"):
        if p.is_file():
            total += p.stat().st_size
    return total


def format_gib(size_bytes: int) -> str:
    gib = size_bytes / (1024**3)
    return f"{gib:.2f} GiB"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--path",
        default="target",
        help="Directory to measure (default: target)",
    )
    parser.add_argument(
        "--max-gib",
        type=float,
        default=30.0,
        help="Fail if size exceeds this threshold in GiB (default: 30)",
    )
    parser.add_argument(
        "--warn-only",
        action="store_true",
        help="Never fail; emit warning only",
    )
    args = parser.parse_args()

    target = Path(args.path)
    if not target.exists():
        print(f"[target-size] OK: {target} does not exist")
        return 0

    size_bytes = dir_size_bytes(target)
    limit_bytes = int(args.max_gib * (1024**3))
    size_msg = f"[target-size] {target}: {format_gib(size_bytes)} (limit {args.max_gib:.2f} GiB)"

    if size_bytes <= limit_bytes:
        print(f"{size_msg} -> OK")
        return 0

    if args.warn_only:
        print(f"{size_msg} -> WARN")
        return 0

    print(f"{size_msg} -> FAIL")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
