# larql-workbench-tauri

Rust-only desktop shell for the workbench:
- Backend commands: Rust (`larql-workbench-core`)
- Frontend: Rust/WASM (`crates/larql-leptos`)
- Host: Tauri

## Scope

This app is the migration path for a projection-only UI boundary:
- facts/validation/execution live in Rust commands
- WASM view only captures intent and renders returned state

## Current command surface

- `run_lql_query` — execute an LQL statement against a workspace path.
- `run_describe` — execute a `DESCRIBE` query with controlled band/verbose options.
- `run_analyze_infer` — execute strict LQL `ANALYZE INFER ... FORMAT JSON` via Rust command core.

## Local development

1. Build the WASM frontend:
   - `cd crates/larql-leptos`
   - `trunk build`
2. Run Tauri shell:
   - `cd apps/larql-workbench-tauri/src-tauri`
   - `cargo tauri dev`

By default, Tauri loads static assets from `../../crates/larql-leptos/dist`.
