# Crate Role Matrix

This matrix turns the product structure plan into an actionable operating view.
It records each crate's role, release criticality, stability expectation, and
recommended migration path.

## Legend

- **Zone**: `core`, `interface`, `experimental`
- **Release impact**:
  - `critical`: directly affects user-facing release artifacts
  - `high`: core dependency of release-facing crates
  - `medium`: important but not always on the primary path
  - `low`: optional/experimental
- **Stability target**:
  - `stable`: avoid breaking changes without migration notes
  - `managed`: can evolve, but changes should be coordinated
  - `iterative`: rapid change acceptable

## Matrix

| Crate | Zone | Primary purpose | Release impact | Stability target | Migration recommendation |
|---|---|---|---|---|---|
| `larql-models` | core | Model architecture/loading and quant/dequant | high | stable | Keep in core; tighten public module boundaries |
| `larql-compute` | core | CPU/Metal compute kernels and pipeline | high | managed | Keep in core; isolate benchmark-only helpers |
| `larql-core` | core | Shared graph/data engine primitives | high | stable | Keep in core; enforce minimal outward API |
| `larql-tokenizer` | core | Tokenizer integration and vocab support | high | stable | Keep in core; document tokenizer contract |
| `larql-vindex` | core | Vindex lifecycle, load/query/mutate/patch | high | stable | Keep in core; separate format internals from public APIs |
| `larql-inference` | core | Forward/inference and tracing layers | high | managed | Keep in core; split experimental paths over time |
| `larql-lql` | core | LQL parser/executor and REPL integration | high | stable | Keep in core; clarify parser/executor extension points |
| `larql-tensor` | core | Tensor/sgemm utilities | medium | managed | Keep in core; harden lint/test baseline |
| `larql-cli` | interface | User CLI commands and orchestration | critical | stable | Keep in interface; strict command-surface compatibility |
| `larql-server` | interface | Network API surface (HTTP/gRPC) | critical | stable | Keep in interface; codify API compatibility policy |
| `larql-python` | interface | Python bindings and UI flows | critical | managed | Keep in interface; consider split between SDK vs UI later |
| `larql-terminal-browser` | interface | Terminal-first browsing UX | medium | iterative | Keep in interface; evaluate if long-term supported surface |
| `kv-cache-benchmark` | experimental | Benchmark/prototyping for KV strategies | low | iterative | Keep isolated; avoid runtime coupling into interface crates |

## Priority Actions by Crate

### Immediate (P0)

- `larql-cli`, `larql-server`, `larql-python`:
  - Add clear "public contract" sections in crate docs/README.
- `kv-cache-benchmark`:
  - Confirm no release-facing crates depend on it.

### Near-term (P1)

- `larql-compute`, `larql-inference`, `larql-vindex`:
  - Mark experimental modules explicitly.
  - Move benchmark-centric docs/examples under perf-oriented documentation.

### Later (P2)

- `larql-python`:
  - Evaluate split of Python SDK core vs UI app surface if release cadence or
    dependency policy diverges.
- `larql-terminal-browser`:
  - Decide whether it remains release-facing or transitions to optional tooling.

## Ownership / PR Checklist

For PRs touching multiple zones:

1. State which zone(s) changed.
2. State whether any release-facing contract changed.
3. If yes, include migration/compatibility notes.
4. If experimental code touches interface/core crates, justify why.
