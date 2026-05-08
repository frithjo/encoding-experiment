# Factoid Board Tauri Host

Tauri host shell for the Rust-owned factoid board projection.

## Invariants

- Rust owns facts and projection grouping.
- UI only renders projection output.
- No runtime authority is granted to loose prose records.

## Architecture

- `larql-factboard`: canonical projection model and grouping logic.
- `larql-leptos`: WASM UI renderer.
- `apps/factboard-tauri/src-tauri`: desktop host that exposes projection as a typed command.

## Run

```bash
cd apps/factboard-tauri/src-tauri
cargo run
```
