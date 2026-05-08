#!/usr/bin/env python3
"""
Validate crate dependency direction using package.metadata.product.zone.

Rules:
1) interface -> core is allowed.
2) core -> interface is forbidden.
3) interface -> experimental is forbidden.
4) core -> experimental is forbidden.
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


def get_workspace_members() -> list[Path]:
    """Read workspace members from Cargo.toml to support non-crates/ locations."""
    doc = load_toml(CARGO_TOML)
    workspace = doc.get("workspace", {})
    members = workspace.get("members", [])
    return [(ROOT / member) for member in members if (ROOT / member).exists()]


def load_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def collect_crates() -> dict[str, dict]:
    crates: dict[str, dict] = {}
    # Use workspace members instead of hardcoded crates/ path
    member_dirs = get_workspace_members()
    for manifest in sorted([d / "Cargo.toml" for d in member_dirs if (d / "Cargo.toml").exists()]):
        data = load_toml(manifest)
        package = data.get("package", {})
        name = package.get("name")
        if not isinstance(name, str) or not name:
            continue

        zone = (
            package.get("metadata", {})
            .get("product", {})
            .get("zone")
        )
        crates[name] = {"manifest": manifest, "zone": zone, "data": data}
    return crates


def dep_names_for_table(table: dict) -> list[str]:
    names: list[str] = []
    for dep_name, dep_spec in table.items():
        # Rename form: ndarray = { package = "larql-tensor", ... }
        if isinstance(dep_spec, dict):
            pkg = dep_spec.get("package")
            if isinstance(pkg, str) and pkg:
                names.append(pkg)
                continue
        # Normal form: larql-core = { ... } or larql-core = "..."
        names.append(dep_name)
    return names


def gather_internal_dependencies(crate_data: dict, known_crates: set[str]) -> set[str]:
    data = crate_data["data"]
    out: set[str] = set()
    for section in (
        "dependencies",
        "dev-dependencies",
        "build-dependencies",
        "target",
    ):
        block = data.get(section)
        if not isinstance(block, dict):
            continue

        if section != "target":
            out.update(d for d in dep_names_for_table(block) if d in known_crates)
            continue

        # target.<triple>.<deps-table>
        for target_cfg in block.values():
            if not isinstance(target_cfg, dict):
                continue
            for dep_section in ("dependencies", "dev-dependencies", "build-dependencies"):
                dep_table = target_cfg.get(dep_section)
                if isinstance(dep_table, dict):
                    out.update(d for d in dep_names_for_table(dep_table) if d in known_crates)
    return out


def validate() -> list[str]:
    errors: list[str] = []
    crates = collect_crates()
    known = set(crates.keys())

    # Zone presence and validity.
    for name, info in crates.items():
        zone = info["zone"]
        rel = info["manifest"].relative_to(ROOT)
        if zone not in ALLOWED_ZONES:
            errors.append(f"{rel}: crate {name!r} has invalid zone {zone!r}")

    # Dependency direction.
    for src_name, src in crates.items():
        src_zone = src["zone"]
        if src_zone not in ALLOWED_ZONES:
            continue

        src_rel = src["manifest"].relative_to(ROOT)
        internal_deps = gather_internal_dependencies(src, known)
        for dep_name in sorted(internal_deps):
            if dep_name == src_name:
                continue
            dep = crates[dep_name]
            dep_zone = dep["zone"]
            if dep_zone not in ALLOWED_ZONES:
                continue

            # Forbidden edges
            if src_zone == "core" and dep_zone == "interface":
                errors.append(
                    f"{src_rel}: forbidden dependency {src_name}({src_zone}) -> "
                    f"{dep_name}({dep_zone})"
                )
            if src_zone == "core" and dep_zone == "experimental":
                errors.append(
                    f"{src_rel}: forbidden dependency {src_name}({src_zone}) -> "
                    f"{dep_name}({dep_zone})"
                )
            if src_zone == "interface" and dep_zone == "experimental":
                errors.append(
                    f"{src_rel}: forbidden dependency {src_name}({src_zone}) -> "
                    f"{dep_name}({dep_zone})"
                )
    return errors


def main() -> int:
    errors = validate()
    if errors:
        print("Dependency direction check failed:")
        for err in errors:
            print(f" - {err}")
        return 1

    print("Dependency direction check passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
