# larql-leptos

## Crate Role

- Role: Rust WASM workbench frontend
- Zone: interface
- Release impact: medium
- Stability target: iterative

Leptos frontend (Rust/WASM) for model-specific tools with larql-server backend.

## Status

**Active Development:** This is the migration target from the React-based chuk-kv-anatomist UI. See [docs/leptos-migration-guide.md](../../docs/leptos-migration-guide.md) for migration details and status. See [docs/ui/README.md](../../docs/ui/README.md) for overall UI strategy.

## Purpose

Provides a Rust/WASM frontend for model-specific analytic tools, leveraging the larql-server backend for model computations. This replaces the React-based chuk-kv-anatomist UI.

## Technology Stack

- **Framework:** Leptos (Rust/WASM)
- **Backend:** larql-server (HTTP API)
- **Build:** wasm-bindgen, wasm-pack

## Development

```bash
# Build the WASM package
cd crates/larql-leptos
wasm-pack build --target web

# Or use cargo with leptos features
cargo build --release
```

## Related Documentation

- [docs/leptos-migration-guide.md](../../docs/leptos-migration-guide.md) - Migration patterns and status
- [docs/ui/README.md](../../docs/ui/README.md) - Overall UI strategy
- [VISION.md](../../VISION.md) - Core analytic framework vision
