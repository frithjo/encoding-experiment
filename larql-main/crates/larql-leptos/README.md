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

Provides a Rust/WASM projection UI where backend facts and execution stay in Rust command handlers.

- Primary shell target: `apps/larql-workbench-tauri/src-tauri`
- Command core: `crates/larql-workbench-core`
- This crate renders state and submits user intent only.

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

### Tauri flow (Rust-only command boundary)

- The `LQL Console` route (`/lql`) invokes Tauri command `run_lql_query_command`.
- The `Explorer` route (`/explorer`) invokes Tauri command `run_describe_command`.
- The `Batch DLA` route (`/batch-dla`) invokes Tauri command `run_analyze_infer_command`.
- The command calls `larql-workbench-core::run_lql_query`, which owns:
  - workspace path validation
  - LQL parsing
  - session execution and output generation
- Explorer commands call `larql-workbench-core::run_describe`, which owns:
  - workspace path/entity/band validation
  - statement construction
  - session execution and output generation
- Batch DLA calls `larql-workbench-core::run_analyze_infer`, which owns:
  - server URL/prompt validation
  - strict scientific clause validation (mode/span requirements)
  - LQL `ANALYZE INFER ... FORMAT JSON` execution
  - full structured scientific result parsing and return

## Related Documentation

- [docs/leptos-migration-guide.md](../../docs/leptos-migration-guide.md) - Migration patterns and status
- [docs/ui/README.md](../../docs/ui/README.md) - Overall UI strategy
- [VISION.md](../../VISION.md) - Core analytic framework vision
