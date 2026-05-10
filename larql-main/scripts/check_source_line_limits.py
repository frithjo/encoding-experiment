#!/usr/bin/env python3
"""Hard CI gate for source-file line limits.

Policy:
- Any tracked source file over LINE_LIMIT fails CI unless explicitly exempted.
- Exemptions are temporary debt entries in `config/source_line_limit_exemptions.txt`.
- If an exempted file is now <= LINE_LIMIT, CI fails until the exemption is removed.
"""

from __future__ import annotations

import os
import pathlib
import subprocess
import sys
from dataclasses import dataclass

LINE_LIMIT = int(os.environ.get("SOURCE_LINE_LIMIT", "999"))
EXEMPTIONS_FILE = pathlib.Path("config/source_line_limit_exemptions.txt")
SOURCE_EXTENSIONS = {
    ".rs",
    ".py",
    ".js",
    ".ts",
    ".tsx",
    ".jsx",
    ".html",
    ".css",
    ".scss",
    ".toml",
    ".yaml",
    ".yml",
}


@dataclass(frozen=True)
class LineCount:
    path: str
    lines: int


@dataclass(frozen=True)
class Exemption:
    path: str
    max_lines: int


def git_tracked_files() -> list[str]:
    out = subprocess.check_output(["git", "ls-files"], text=True)
    return [line.strip() for line in out.splitlines() if line.strip()]


def is_source_file(path: str) -> bool:
    return pathlib.Path(path).suffix.lower() in SOURCE_EXTENSIONS


def count_lines(path: str) -> int:
    with open(path, "r", encoding="utf-8", errors="ignore") as handle:
        return sum(1 for _ in handle)


def load_exemptions() -> dict[str, Exemption]:
    if not EXEMPTIONS_FILE.exists():
        return {}
    rows: dict[str, Exemption] = {}
    for raw in EXEMPTIONS_FILE.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if "|" not in line:
            raise ValueError(
                f"Invalid exemption entry '{line}'. Expected format: path|max_lines"
            )
        path, max_lines_raw = line.rsplit("|", 1)
        path = path.strip()
        if not path:
            raise ValueError(f"Invalid exemption entry '{line}': empty path")
        try:
            max_lines = int(max_lines_raw.strip())
        except ValueError as exc:
            raise ValueError(
                f"Invalid exemption entry '{line}': max_lines must be an integer"
            ) from exc
        if max_lines <= LINE_LIMIT:
            raise ValueError(
                f"Invalid exemption entry '{line}': max_lines must be > {LINE_LIMIT}"
            )
        if path in rows:
            raise ValueError(f"Duplicate exemption path: {path}")
        rows[path] = Exemption(path=path, max_lines=max_lines)
    return rows


def main() -> int:
    tracked = git_tracked_files()
    source_files = [f for f in tracked if is_source_file(f)]
    exemptions = load_exemptions()

    over_limit: list[LineCount] = []
    missing_exemptions: list[LineCount] = []
    stale_exemptions: list[LineCount] = []
    unknown_exemptions: list[str] = []
    exceeded_caps: list[tuple[LineCount, Exemption]] = []

    line_map: dict[str, int] = {}
    for path in source_files:
        lines = count_lines(path)
        line_map[path] = lines
        if lines > LINE_LIMIT:
            over_limit.append(LineCount(path=path, lines=lines))
            ex = exemptions.get(path)
            if ex is None:
                missing_exemptions.append(LineCount(path=path, lines=lines))
            elif lines > ex.max_lines:
                exceeded_caps.append((LineCount(path=path, lines=lines), ex))

    for path, ex in sorted(exemptions.items()):
        if path not in line_map:
            if pathlib.Path(path).exists():
                lines = count_lines(path)
                line_map[path] = lines
            else:
                unknown_exemptions.append(path)
                continue
        lines = line_map[path]
        if lines <= LINE_LIMIT:
            stale_exemptions.append(LineCount(path=path, lines=lines))

    over_limit.sort(key=lambda item: (item.lines, item.path), reverse=True)
    missing_exemptions.sort(key=lambda item: (item.lines, item.path), reverse=True)
    stale_exemptions.sort(key=lambda item: item.path)

    print(f"[line-limit] source files scanned: {len(source_files)}")
    print(f"[line-limit] limit: {LINE_LIMIT}")
    print(f"[line-limit] over-limit files: {len(over_limit)}")
    if over_limit:
        for item in over_limit:
            ex = exemptions.get(item.path)
            flag = f" (EXEMPT max={ex.max_lines})" if ex else ""
            print(f"  - {item.path}: {item.lines}{flag}")

    failed = False
    if missing_exemptions:
        failed = True
        print("\n[line-limit] ERROR: over-limit files missing exemptions:")
        for item in missing_exemptions:
            print(f"  - {item.path}: {item.lines}")

    if stale_exemptions:
        failed = True
        print("\n[line-limit] ERROR: stale exemptions (file no longer over limit):")
        for item in stale_exemptions:
            print(f"  - {item.path}: {item.lines}")

    if exceeded_caps:
        failed = True
        print("\n[line-limit] ERROR: exempted files exceeded their max_lines cap:")
        for item, ex in exceeded_caps:
            print(f"  - {item.path}: {item.lines} (cap: {ex.max_lines})")

    if unknown_exemptions:
        failed = True
        print("\n[line-limit] ERROR: unknown exemption paths:")
        for path in unknown_exemptions:
            print(f"  - {path}")

    if failed:
        print(
            "\n[line-limit] FAIL: keep source files <= limit or update "
            "config/source_line_limit_exemptions.txt with intent."
        )
        return 1

    print("[line-limit] PASS")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ValueError as exc:
        print(f"[line-limit] ERROR: {exc}")
        raise SystemExit(1)
