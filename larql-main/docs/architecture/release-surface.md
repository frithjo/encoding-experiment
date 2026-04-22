# Release Surface Definition

This file defines what is considered release-facing versus internal/experimental
for the current workspace.

## Release-Facing Surfaces

These crates form the user-facing product surface:

- `crates/larql-cli` (CLI binary)
- `crates/larql-server` (HTTP/gRPC service surface)
- `crates/larql-python` (Python package and UI integration)
- `crates/larql-terminal-browser` (terminal browsing interface)

## Core Runtime Surfaces (Not Directly User-Facing)

These are foundational but generally consumed through interfaces:

- `crates/larql-models`
- `crates/larql-compute`
- `crates/larql-core`
- `crates/larql-tokenizer`
- `crates/larql-vindex`
- `crates/larql-inference`
- `crates/larql-lql`
- `crates/larql-tensor`

## Experimental / Non-Release Surface

- `experiments/kv-cache-benchmark`

Policy:
- Experimental crates are allowed to iterate faster and may not meet the same
  API stability expectations as release-facing crates.
- Interface crates should not take hard runtime dependencies on experimental
  crates.

## What "Productized" Means Here

- Stable command and API contracts documented for release-facing crates.
- Clear ownership of "public contract" versus "implementation detail".
- New features must specify whether they affect:
  - CLI surface,
  - Server API,
  - Python SDK/UI,
  - or internal runtime only.

## CI/CD Relevance

- PR checks should always cover release-facing crates and their transitively
  required core crates.
- Security and release workflows should prioritize release-facing artifacts.
- Experimental crates can be tested in separate jobs/lanes if they materially
  slow default product checks.

## Checklist for Future Changes

Before adding a new module/crate:

1. Is it release-facing or internal?
2. Does it belong in core, interface, or experimental zone?
3. Does it introduce a new external contract (CLI/API/SDK)?
4. Does it need separate CI gating from product-critical checks?

Record these answers in the crate README or PR description.
