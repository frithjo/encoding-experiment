# Contributing

This repository follows a productized workspace structure contract.

Before opening a PR, classify your change using the architecture docs:

- `docs/architecture/README.md`
- `docs/architecture/product-structure.md`
- `docs/architecture/release-surface.md`
- `docs/architecture/crate-role-matrix.md`
- `docs/architecture/python-bindings-ui-split.md` (for Python SDK/UI changes)

## 1) Pick the right zone

- **Core**: model/compute/storage/inference/language internals
- **Interface**: CLI/server/python/terminal user-facing surfaces
- **Experimental**: benchmark/prototype work

If a change touches multiple zones, explain why in the PR description.

## 2) Respect dependency direction

- Interface crates may depend on core crates.
- Core crates must not depend on interface crates.
- Release-facing interface crates should not depend on experimental crates.

## 3) Document contract impact

If your change affects user-facing behavior, state which contract changed:

- CLI contract (`larql-cli`)
- Server API contract (`larql-server`)
- Python SDK/UI contract (`larql-python`)

Include migration notes for breaking behavior.

## 4) Keep generated/local artifacts out of git

Do not commit local-generated artifacts or temporary outputs unless they are
explicitly intended as product assets.

## 5) Validation baseline

Run at least:

```bash
cd larql-main
cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
cargo test --workspace
```

For large PRs, include targeted crate checks in addition to workspace checks.
