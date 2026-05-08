# Product Structure (Workspace Cleanup Plan)

This document defines a productized structure for the current Rust workspace.
Wave 3 introduced the first structural move (`kv-cache-benchmark` into the
`experiments/` segment); follow-on changes should continue in safe waves.

## Goals

- Keep the current Cargo workspace intact.
- Make product boundaries explicit (core vs interfaces vs experimental).
- Reduce mixed concerns across crates and docs.
- Provide a migration map that can be executed in safe waves.

## Current Workspace Inventory

From `larql-main/Cargo.toml` workspace members:

### Product core crates

- `crates/larql-models`
- `crates/larql-compute`
- `crates/larql-core`
- `crates/larql-tokenizer`
- `crates/larql-vindex`
- `crates/larql-inference`
- `crates/larql-lql`
- `crates/larql-tensor`

### Product interface crates

- `crates/larql-cli`
- `crates/larql-server`
- `crates/larql-python`
- `crates/larql-terminal-browser`

### Experimental / performance crates

- `experiments/kv-cache-benchmark`

## Target Logical Zones

Treat crates as living in these zones:

- **Core Platform**: model loading, compute, storage, language engine, inference
- **Interfaces**: CLI, HTTP service, Python SDK/UI, terminal UI
- **Experimental Lab**: benchmark and prototype crates

```text
larql-main/
  crates/
    (core-platform)      larql-models, larql-compute, larql-core,
                         larql-tokenizer, larql-vindex, larql-inference,
                         larql-lql, larql-tensor
    (interfaces)         larql-cli, larql-server, larql-python,
                         larql-terminal-browser
  experiments/
    (experimental)       kv-cache-benchmark
  docs/
    architecture/
      product-structure.md
      release-surface.md
```

## Dependency Direction Rules

1. Interface crates can depend on core crates.
2. Core crates must not depend on interface crates.
3. Experimental crates can depend on core crates, but interface crates should
   not depend on experimental crates.
4. Docs/examples that are benchmark/prototype-oriented should be kept out of
   release-facing crate docs unless clearly labeled.

## Mess Hotspots (Observed)

- Very large active change surface across many crates indicates mixed WIP.
- `kv-cache-benchmark` appears to carry broad experimental throughput and
  strategy work that can distract from release-facing crates.
- CI/CD, signing, and release docs were mixed into root crate README without a
  dedicated architecture reference (addressed by this doc set).
- Generated/local artifacts required ignore hardening (`CLAUDE.md`,
  `async_runs.js`), indicating unclear local-vs-product file boundaries.

## Cleanup Backlog (Prioritized)

### P0 (low risk, immediate)

- Add role labels to each crate README (`core`, `interface`, `experimental`).
- Add CONTRIBUTING section that states where new code should live.
- Keep generated/local artifacts out of VCS with explicit ignore patterns.

### P1 (moderate risk)

- Move benchmark-only docs into `docs/architecture` or `docs/perf`.
- Standardize command surfaces: product commands in CLI docs, experiment
  commands in benchmark docs.
- Add per-crate "public API vs internal" notes.

### P2 (higher risk, future wave)

- Consider physically separating experimental crates under an `experiments/`
  workspace segment.
- Evaluate splitting `larql-python` UI concerns from Python bindings if release
  cadence differs.
- Revisit crate boundaries where product and benchmark logic are interleaved.

## Migration Waves (Execution Later)

1. **Wave 1 - Documentation and policy only**
   - This document set, crate role labeling, contribution rules.
2. **Wave 2 - Safe organization**
   - Non-code moves (docs/scripts/config), no crate path changes.
3. **Wave 3 - Structural refactor**
   - Optional crate moves/renames with workspace and import updates.
