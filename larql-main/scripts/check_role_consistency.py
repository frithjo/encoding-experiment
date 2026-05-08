#!/usr/bin/env python3
"""
Ensure crate README role headers match Cargo product metadata.

Compares:
  crates/<name>/Cargo.toml -> [package.metadata.product]
against:
  crates/<name>/README.md -> "## Crate Role" block bullets
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    print("ERROR: Python 3.11+ is required (tomllib missing).", file=sys.stderr)
    sys.exit(2)


ROOT = Path(__file__).resolve().parents[1]
CARGO_TOML = ROOT / "Cargo.toml"

ROLE_BLOCK_RE = re.compile(
    r"^## Crate Role\s+"
    r"-\s*Role:\s*(?P<role>[^\n]+)\s+"
    r"-\s*Zone:\s*(?P<zone>[^\n]+)\s+"
    r"-\s*Release impact:\s*(?P<release_impact>[^\n]+)\s+"
    r"-\s*Stability target:\s*(?P<stability_target>[^\n]+)",
    re.MULTILINE,
)


def get_workspace_members() -> list[Path]:
    """Read workspace members from Cargo.toml to support non-crates/ locations."""
    doc = read_toml(CARGO_TOML)
    workspace = doc.get("workspace", {})
    members = workspace.get("members", [])
    return [(ROOT / member) for member in members if (ROOT / member).exists()]



def read_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def normalize(val: str) -> str:
    return val.strip().lower().replace(" ", "-")


def parse_readme_role(path: Path) -> dict[str, str] | None:
    text = path.read_text(encoding="utf-8")
    m = ROLE_BLOCK_RE.search(text)
    if not m:
        return None
    return {
        "role": m.group("role").strip(),
        "zone": m.group("zone").strip(),
        "release_impact": m.group("release_impact").strip(),
        "stability_target": m.group("stability_target").strip(),
    }


def parse_manifest_role(path: Path) -> dict[str, str] | None:
    data = read_toml(path)
    product = (
        data.get("package", {})
        .get("metadata", {})
        .get("product")
    )
    if not isinstance(product, dict):
        return None
    return {
        "role": str(product.get("role", "")).strip(),
        "zone": str(product.get("zone", "")).strip(),
        "release_impact": str(product.get("release_impact", "")).strip(),
        "stability_target": str(product.get("stability_target", "")).strip(),
    }


def main() -> int:
    errors: list[str] = []
    checked = 0

    # Use workspace members instead of hardcoded crates/ path
    member_dirs = get_workspace_members()
    for cargo_toml in sorted([d / "Cargo.toml" for d in member_dirs if (d / "Cargo.toml").exists()]):
        crate_dir = cargo_toml.parent
        readme = crate_dir / "README.md"
        rel_cargo = cargo_toml.relative_to(ROOT)
        rel_readme = readme.relative_to(ROOT)

        manifest = parse_manifest_role(cargo_toml)
        if manifest is None:
            errors.append(f"{rel_cargo}: missing [package.metadata.product]")
            continue

        if not readme.exists():
            errors.append(f"{rel_readme}: missing README.md")
            continue

        readme_role = parse_readme_role(readme)
        if readme_role is None:
            errors.append(f"{rel_readme}: missing or malformed '## Crate Role' block")
            continue

        checked += 1
        for key in ("zone", "release_impact", "stability_target"):
            if normalize(readme_role[key]) != normalize(manifest[key]):
                errors.append(
                    f"{rel_readme}: {key} mismatch "
                    f"(README={readme_role[key]!r}, Cargo={manifest[key]!r})"
                )

    if errors:
        print("Crate role consistency check failed:")
        for err in errors:
            print(f" - {err}")
        return 1

    print(f"Crate role consistency check passed ({checked} crates).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
