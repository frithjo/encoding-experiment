#!/usr/bin/env python3
"""
Validate per-crate product metadata in Cargo.toml files.

Required table:
  [package.metadata.product]
  zone = "core|interface|experimental"
  role = "<non-empty>"
  release_impact = "critical|high|medium|low"
  stability_target = "stable|managed|iterative"
"""

from __future__ import annotations

import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    print("ERROR: Python 3.11+ is required (tomllib missing).", file=sys.stderr)
    sys.exit(2)


ROOT = Path(__file__).resolve().parents[1]
CARGO_TOML = ROOT / "Cargo.toml"

ALLOWED_ZONES = {"core", "interface", "experimental"}
ALLOWED_IMPACTS = {"critical", "high", "medium", "low"}
ALLOWED_STABILITY = {"stable", "managed", "iterative"}


def get_workspace_members() -> list[Path]:
    """Read workspace members from Cargo.toml to support non-crates/ locations."""
    doc = read_toml(CARGO_TOML)
    workspace = doc.get("workspace", {})
    members = workspace.get("members", [])
    # Resolve relative paths from ROOT
    return [(ROOT / member) for member in members if (ROOT / member).exists()]



def read_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def validate_manifest(path: Path) -> list[str]:
    rel = path.relative_to(ROOT)
    errors: list[str] = []
    doc = read_toml(path)

    pkg = doc.get("package")
    if not isinstance(pkg, dict):
        return [f"{rel}: missing [package] table"]

    metadata = pkg.get("metadata")
    if not isinstance(metadata, dict):
        return [f"{rel}: missing [package.metadata] table"]

    product = metadata.get("product")
    if not isinstance(product, dict):
        return [f"{rel}: missing [package.metadata.product] table"]

    zone = product.get("zone")
    if zone not in ALLOWED_ZONES:
        errors.append(f"{rel}: invalid product.zone={zone!r}, expected one of {sorted(ALLOWED_ZONES)}")

    role = product.get("role")
    if not isinstance(role, str) or not role.strip():
        errors.append(f"{rel}: invalid product.role={role!r}, expected non-empty string")

    impact = product.get("release_impact")
    if impact not in ALLOWED_IMPACTS:
        errors.append(
            f"{rel}: invalid product.release_impact={impact!r}, expected one of {sorted(ALLOWED_IMPACTS)}"
        )

    stability = product.get("stability_target")
    if stability not in ALLOWED_STABILITY:
        errors.append(
            f"{rel}: invalid product.stability_target={stability!r}, expected one of {sorted(ALLOWED_STABILITY)}"
        )

    return errors


def main() -> int:
    # Use workspace members instead of hardcoded crates/ path
    member_dirs = get_workspace_members()
    manifests = sorted([d / "Cargo.toml" for d in member_dirs if (d / "Cargo.toml").exists()])
    if not manifests:
        print("ERROR: no Cargo.toml files found in workspace members", file=sys.stderr)
        return 2

    all_errors: list[str] = []
    for manifest in manifests:
        all_errors.extend(validate_manifest(manifest))

    if all_errors:
        print("Product metadata check failed:")
        for err in all_errors:
            print(f" - {err}")
        return 1

    print(f"Product metadata check passed ({len(manifests)} crates).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
