#!/usr/bin/env python3
"""
Enforce validation warning disclosure policy.

Policy:
1) Run `cargo check --workspace`.
2) Collect non-fatal warnings printed by Cargo/Rust.
3) Ensure warning set matches documented set in docs/architecture/validation-warnings.md.

Fails when:
- A runtime warning exists but is not documented.
- A documented warning no longer appears (stale disclosure entry).
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WARNINGS_DOC = ROOT / "docs" / "architecture" / "validation-warnings.md"

WARN_LINE_RE = re.compile(r"^warning:\s+(.+)$", re.MULTILINE)
DOC_WARN_RE = re.compile(r"^\s*-\s*`warning:\s+(.+?)`\s*$", re.MULTILINE)


def run_cargo_check() -> str:
    proc = subprocess.run(
        ["cargo", "check", "--workspace"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    output = (proc.stdout or "") + ("\n" + proc.stderr if proc.stderr else "")
    if proc.returncode != 0:
        print(output, end="")
        raise SystemExit(proc.returncode)
    return output


def runtime_warnings(output: str) -> set[str]:
    return {m.group(1).strip() for m in WARN_LINE_RE.finditer(output)}


def documented_warnings(path: Path) -> set[str]:
    if not path.exists():
        print(f"ERROR: missing warnings registry: {path.relative_to(ROOT)}", file=sys.stderr)
        raise SystemExit(2)
    text = path.read_text(encoding="utf-8")
    return {m.group(1).strip() for m in DOC_WARN_RE.finditer(text)}


def main() -> int:
    runtime = runtime_warnings(run_cargo_check())
    documented = documented_warnings(WARNINGS_DOC)

    undocumented = sorted(runtime - documented)
    stale = sorted(documented - runtime)

    if undocumented or stale:
        print("Validation disclosure check failed.")
        if undocumented:
            print("Undocumented runtime warnings:")
            for line in undocumented:
                print(f" - warning: {line}")
        if stale:
            print("Stale documented warnings (not seen at runtime):")
            for line in stale:
                print(f" - warning: {line}")
        print(
            "\nUpdate docs/architecture/validation-warnings.md to match current `cargo check --workspace` warnings."
        )
        return 1

    print(f"Validation disclosure check passed ({len(runtime)} warning(s) documented).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
