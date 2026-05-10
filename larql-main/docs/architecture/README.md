# Architecture Docs Index

This folder defines the productized structure contract for the current
workspace cleanup effort.

## Documents

- [`product-structure.md`](./product-structure.md)
  - Workspace-level structure goals, logical zones, dependency direction rules,
    and phased migration plan.
- [`release-surface.md`](./release-surface.md)
  - Defines release-facing vs internal vs experimental surfaces and policy.
- [`crate-role-matrix.md`](./crate-role-matrix.md)
  - Crate-by-crate role, release impact, stability target, and migration
    recommendations.
- [`validation-policy.md`](./validation-policy.md)
  - Validation reporting policy for non-fatal warning disclosure.
- [`validation-warnings.md`](./validation-warnings.md)
  - Exact warning registry for `cargo check --workspace`.

## Suggested Reading Order

1. Read `product-structure.md` for the overall model.
2. Read `release-surface.md` for user-facing contract boundaries.
3. Use `crate-role-matrix.md` for implementation planning and PR triage.

## Operating Usage

- For refactors: start with `crate-role-matrix.md` and verify zone constraints.
- For API/CLI/SDK changes: check `release-surface.md` before implementation.
- For roadmap/migration planning: use `product-structure.md` migration waves.
- Metadata enforcement runs in CI via `scripts/check_product_metadata.py`.
- README/Cargo consistency is enforced via `scripts/check_role_consistency.py`.
- Cross-zone dependency direction is enforced via
  `scripts/check_dependency_direction.py`.
- Validation warning disclosure is enforced via
  `scripts/check_validation_disclosure.py`.
