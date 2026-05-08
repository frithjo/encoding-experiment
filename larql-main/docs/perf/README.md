# Performance Docs Index

This section centralizes performance-oriented documentation, benchmark guidance,
and profiling references so product-facing docs stay focused on user contracts.

## Purpose

- Separate performance research from release-surface/API contract docs.
- Keep benchmark workflows discoverable for contributors.
- Provide a stable place for profiling and optimization writeups.

## Recommended Content

- Cross-crate benchmark methodology and reproducibility notes.
- Hardware/backend comparison guidance (CPU BLAS vs Metal).
- Profiling playbooks and interpretation notes.
- Performance-track doc mirrors:
  - `docs/perf/walk-boundary-sweep.md`
  - `docs/perf/findings.md`
  - `docs/perf/validation.md`
- Links to crate-specific performance docs:
  - `crates/larql-compute/PERFORMANCE.md`
  - `crates/larql-inference/PERFORMANCE.md`
  - `crates/larql-vindex/PERFORMANCE.md`
  - `experiments/kv-cache-benchmark/docs/*`

## Usage

- For feature/API work, start in architecture docs and crate READMEs.
- For speed/memory regressions or optimization work, start here and then follow
  the crate-specific performance docs.
