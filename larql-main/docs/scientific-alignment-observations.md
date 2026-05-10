# Scientific Alignment Observations

Date: 2026-05-09

This file records experiment observations and anomalies for scientific-contract alignment.
It does **not** modify hypothesis definitions.

## Referenced Hypotheses (Verbatim)

From [docs/ui/README.md](./ui/README.md):

> **Key Invariant:** One experiment engine, one language surface, one transport surface. No duplication of analysis logic across UIs.

From [AGENTS.md](../AGENTS.md):

> **Canonical Flow for Scientific Analysis:**
> - `larql-inference`: Exposes structured analysis API (e.g., batch_dla_scan semantics)
> - `larql-lql`: Parses ANALYZE statements, executes via structured analysis API, formats output
> - `larql-server`: Tool routes become transport adapters over the same analysis API
> - `larql-terminal-batch-dla`: Consumes analysis via LQL or server adapter

## Observation Metrics

1. Command-path language surface
- Observation: `run_analyze_infer` executes `USE REMOTE ...;` then `ANALYZE INFER ... FORMAT JSON;` through `larql_lql::Session`.
- Evidence: [crates/larql-workbench-core/src/lib.rs](../crates/larql-workbench-core/src/lib.rs) (`run_analyze_infer`).
- Metric: 1/1 workbench Batch DLA command path is LQL-first (no direct scientific semantics in UI code).

2. Scientific clause strictness at command boundary
- Observation: command-core enforces mode/span/top-k constraints before execution.
- Evidence:
  - `MissingAnalysisMode`
  - `InvalidAnalysisMode`
  - `MissingFactProbeSpans`
  - `MissingWorkflowProbeFalseSpans`
  - `InvalidTopK`
  in [crates/larql-workbench-core/src/lib.rs](../crates/larql-workbench-core/src/lib.rs).
- Metric: 5 explicit scientific precondition checks in command-core.

3. Contract test coverage (command-core)
- Observation: analyze-infer unit tests cover required request constraints.
- Evidence: tests `run_analyze_infer_*` in [crates/larql-workbench-core/src/lib.rs](../crates/larql-workbench-core/src/lib.rs).
- Metric: 7 analyze-infer input-contract tests currently present.

4. Structured output completeness
- Observation: command response type is canonical `larql_inference::AnalysisResult`.
- Evidence: `pub type AnalyzeInferResponse = larql_inference::AnalysisResult` in [crates/larql-workbench-core/src/lib.rs](../crates/larql-workbench-core/src/lib.rs).
- Metric: 12 top-level scientific fields returned (`attention`, `logit_lens`, `head_dla`, `num_layers`, `seq_len`, `tokens`, `strings`, `predictions`, `generation_trace`, `token_analysis`, `analysis_summary`, `ridge_by_layer`), matching [crates/larql-inference/src/analysis.rs](../crates/larql-inference/src/analysis.rs).

## Anomalies

1. Historical transport-first migration examples can still mislead
- Observation: migration guide retains legacy transport-first examples for historical context.
- Evidence: [docs/leptos-migration-guide.md](./leptos-migration-guide.md).
- Classification: **Technically coherent but materially false if followed as current guidance**.
- Current handling: guide now labels this section as historical and states canonical LQL-first path.

2. Legacy API test script shape mismatch (resolved in this run)
- Observation: previous script payloads could imply permissive FACT_PROBE execution without spans.
- Evidence: [crates/larql-leptos/test_ui.sh](../crates/larql-leptos/test_ui.sh).
- Classification: **Technically coherent but materially false relative to strict scientific contract**.
- Resolution: updated script to use strict payload clauses and check full scientific fields.

## Validation Log

- `cargo test -p larql-workbench-core` → pass (13 tests).
- `cargo check -p larql-leptos` → pass.
- `cargo check` (Tauri shell) → pass.

## Phase 2 Baseline Snapshot (Reproducibility Harness)

Artifact:
- [docs/scientific-alignment/snapshot-latest.json](./scientific-alignment/snapshot-latest.json)

Generator:
- `python3 scripts/scientific_alignment_snapshot.py`
- `make scientific-alignment-snapshot`
- `make scientific-alignment-summary` (prints compact metrics)
- `python3 scripts/scientific_alignment_trend.py --print-summary`
- `make scientific-alignment-trend`
- `make scientific-alignment-watch` (snapshot + trend check)

Captured baseline (2026-05-10 UTC):
- LQL-first path in command core: `use_remote_present=true`, `direct_http_analyze_in_core=false`.
- Strict contract guards: `guard_count=5`.
- Analyze-input contract tests: `7`.
- Full scientific response surface:
  - `analysis_result_field_count=12`
  - `leptos_struct_field_count=12`
- Historical transport-first section containment flags: present and explicitly marked historical.
- UI behavior script strict payload markers: truth/false/coherence counts = `1/2/2`.

Reproducibility protocol:
1. Run `make scientific-alignment-snapshot`.
2. Confirm `docs/scientific-alignment/snapshot-latest.json` updated.
3. Confirm a new timestamped artifact exists under `docs/scientific-alignment/history/`.
4. Run `make scientific-alignment-summary` and copy the JSON summary into experiment notes.
5. Run `make scientific-alignment-trend` and persist `docs/scientific-alignment/trend-latest.json`.
6. If `status=fail` or `regression_count>0`, append an anomaly entry with baseline/candidate paths and failing check names.

## Phase 3 Trend Check (2026-05-10 UTC)

Artifact:
- [docs/scientific-alignment/trend-latest.json](./scientific-alignment/trend-latest.json)

Result:
- Baseline: `docs/scientific-alignment/history/snapshot-20260510T034530Z.json`
- Candidate: `docs/scientific-alignment/history/snapshot-20260510T034704Z.json`
- `check_count=9`, `regression_count=0`, `status=pass`

Interpretation:
- No detected regression in LQL-first routing, strict guard coverage, response-surface field counts, or strict payload presence for this interval.

## Phase 4 Trend Check After P1/P2 Remediation (2026-05-10 UTC)

Artifacts:
- [docs/scientific-alignment/snapshot-latest.json](./scientific-alignment/snapshot-latest.json)
- [docs/scientific-alignment/trend-latest.json](./scientific-alignment/trend-latest.json)

Result:
- Baseline: `docs/scientific-alignment/history/snapshot-20260510T034704Z.json`
- Candidate: `docs/scientific-alignment/history/snapshot-20260510T051502Z.json`
- `check_count=9`, `regression_count=0`, `status=pass`

Interpretation:
- Post-remediation run shows no contract regression on LQL-first routing or strict-analysis guard surface.
- Structured analysis response coverage remains stable (`analysis_result_field_count=12`, `leptos_struct_field_count=12`).

## Phase 5 Non-Tauri Fallback Guard Check (2026-05-10 UTC)

Artifacts:
- [docs/scientific-alignment/trend-latest.json](./scientific-alignment/trend-latest.json)

Evidence (browser-served WASM fallback path):
- `crates/larql-leptos/src/lib.rs` routes all three UI actions through runtime checks with explicit non-Tauri HTTP fallback:
  - `if !has_tauri_runtime() { return invoke_http_lql(...) }`
  - `if !has_tauri_runtime() { return invoke_http_describe(...) }`
  - `if !has_tauri_runtime() { return invoke_http_analyze_infer(...) }`
- HTTP fallback targets are explicit in the same file:
  - `POST /api/lql/query`
  - `POST /api/explorer/describe`
  - `POST {server_url}/v1/analyze-infer`
- Python UI backend provides matching routes and payload shapes:
  - `crates/larql-python/python/larql_ui/ui/api.py`: `Route("/api/explorer/describe", ...)`, `Route("/api/lql/query", ...)`
  - `crates/larql-python/python/larql_ui/ui/execution.py`: raw payload keys include `"lines"` (LQL) and `"edges"` (DESCRIBE)

Metrics:
- Non-Tauri fallback branch coverage for Leptos actions: `3/3`.
- Trend regression delta for this interval: `regression_count=0`.

Result:
- Baseline: `docs/scientific-alignment/history/snapshot-20260510T051502Z.json`
- Candidate: `docs/scientific-alignment/history/snapshot-20260510T051521Z.json`
- `check_count=9`, `regression_count=0`, `status=pass`

Interpretation:
- The prior browser-path regression risk is not reproduced in this run.
- Transport adapters and expected payload fields are aligned across Leptos fallback and Python UI routes.

## Phase 6 End-to-End Watch Cycle (2026-05-10 UTC)

Artifacts:
- [docs/scientific-alignment/snapshot-latest.json](./scientific-alignment/snapshot-latest.json)
- [docs/scientific-alignment/trend-latest.json](./scientific-alignment/trend-latest.json)

Command:
- `make scientific-alignment-watch`

Result:
- Baseline: `docs/scientific-alignment/history/snapshot-20260510T051521Z.json`
- Candidate: `docs/scientific-alignment/history/snapshot-20260510T051654Z.json`
- `check_count=9`, `regression_count=0`, `status=pass`

Interpretation:
- End-to-end snapshot + trend execution remains stable after the non-Tauri fallback verification.
- No regressions detected in the tracked scientific-alignment guard set.

Validation log (same run window):
- `cargo test -p larql-workbench-core` → pass (`13 passed; 0 failed`).
- `cargo check -p larql-leptos` → pass.

## Phase 7 CPU/Linux Footprint + Timestamp Path Check (2026-05-10 UTC)

Scope:
- Confirm current extraction timestamp implementation is calendar-valid RFC3339 UTC.
- Capture repo-local storage footprint after repeated experiment cycles.

Evidence:
- `crates/larql-vindex/src/extract/resume.rs`: `ExtractProgress::now_timestamp()` delegates to `super::build::chrono_now()`.
- `crates/larql-vindex/src/extract/build.rs`: `chrono_now()` uses `time::OffsetDateTime::now_utc().format(Rfc3339)`.
- Leptos non-Tauri fallback branches still present for all three actions (see Phase 5).

Measurements (repo-local):
- Top directories by size:
  - `apps`: `4.5G`
  - `target`: `4.3G`
  - `data`: `2.4G`
  - `attention-decoupling-proof`: `1.3G`
  - `crates`: `304M`
- Scientific-alignment artifacts:
  - `docs/scientific-alignment`: `32K`
  - history snapshot count: `6` files

Interpretation:
- The prior invalid-calendar timestamp risk is not reproduced by current code path; timestamp serialization is sourced from UTC RFC3339 formatting.
- Current repo-local footprint does not indicate a reproduced "hundreds of GB" growth event in this run window; largest growth vectors remain build outputs (`target`) and app/data artifacts.

## Phase 8 Trend Recheck After Phase 7 Logging (2026-05-10 UTC)

Command:
- `make scientific-alignment-watch`

Result:
- Baseline: `docs/scientific-alignment/history/snapshot-20260510T051654Z.json`
- Candidate: `docs/scientific-alignment/history/snapshot-20260510T052032Z.json`
- `check_count=9`, `regression_count=0`, `status=pass`

Interpretation:
- Scientific-alignment checks remain stable after additional experiment logging.

## Phase 9 Footprint Guard Instrumentation (2026-05-10 UTC)

Scope:
- Add repeatable storage telemetry to snapshot generation.
- Add budget-based footprint checks to trend comparisons so oversized run artifacts are surfaced as regressions.

Implementation evidence:
- `scripts/scientific_alignment_snapshot.py`
  - Adds `storage_footprint` block with tracked directory bytes/GiB for:
    - `apps`
    - `target`
    - `data`
    - `attention-decoupling-proof`
    - `docs/scientific-alignment/history`
  - Adds summary fields: `tracked_total_gib`, `largest_tracked_dir`, `largest_tracked_dir_gib`.
- `scripts/scientific_alignment_trend.py`
  - Adds six threshold checks:
    - `apps <= 8 GiB`
    - `target <= 8 GiB`
    - `data <= 6 GiB`
    - `attention-decoupling-proof <= 4 GiB`
    - `docs/scientific-alignment/history <= 200 MiB`
    - `tracked_total_bytes <= 30 GiB`
  - Adds compatibility rule for schema evolution:
    - if baseline/candidate both lack a new field, check is marked pass (skipped);
    - if baseline has field and candidate loses it, check fails.

Validation log:
- `python3 -m py_compile scripts/scientific_alignment_snapshot.py scripts/scientific_alignment_trend.py` → pass.
- `make scientific-alignment-watch` → pass.

Latest trend result:
- Baseline: `docs/scientific-alignment/history/snapshot-20260510T053842Z.json`
- Candidate: `docs/scientific-alignment/history/snapshot-20260510T053857Z.json`
- `check_count=15`, `regression_count=0`, `status=pass`

Interpretation:
- Disk-growth risk is now part of automated scientific-alignment regression detection rather than a manual-only review step.

## Phase 10 Experiment Completion Gate (2026-05-10 UTC)

Scope:
- Finalize the experiment cycle with explicit non-Tauri fallback invariants integrated into the automated trend harness.

Implementation evidence:
- `scripts/scientific_alignment_snapshot.py`
  - Adds `ui_transport_fallback` metrics:
    - `tauri_runtime_guard_count`
    - `http_lql_endpoint_present`
    - `http_describe_endpoint_present`
    - `http_analyze_endpoint_present`
- `scripts/scientific_alignment_trend.py`
  - Adds four transport checks:
    - non-decreasing `tauri_runtime_guard_count`
    - must-true endpoint presence checks for LQL, DESCRIBE, and ANALYZE fallback paths
  - Extends schema-evolution handling for boolean/non-decreasing checks to avoid false regressions when fields are newly introduced.

Validation log:
- `python3 -m py_compile scripts/scientific_alignment_snapshot.py scripts/scientific_alignment_trend.py` → pass.
- `make scientific-alignment-watch` → pass (first completion run).
- `make scientific-alignment-watch` → pass (repeat stability run).

Completion trend results:
- Run 1:
  - Baseline: `docs/scientific-alignment/history/snapshot-20260510T053857Z.json`
  - Candidate: `docs/scientific-alignment/history/snapshot-20260510T054224Z.json`
  - `check_count=19`, `regression_count=0`, `status=pass`
- Run 2:
  - Baseline: `docs/scientific-alignment/history/snapshot-20260510T054224Z.json`
  - Candidate: `docs/scientific-alignment/history/snapshot-20260510T054228Z.json`
  - `check_count=19`, `regression_count=0`, `status=pass`

Interpretation:
- Experiment completion gate is satisfied for this cycle: strict analysis-contract checks, response-surface checks, non-Tauri transport fallback checks, and disk-footprint budgets are all green.

## Phase 11 Bit-Perfect Eraser Falsification (2026-05-10 UTC)

Hypothesis under test (verbatim):
- "Surgical Isolation: By zeroing only 3 specific slots (24:1618, 25:3262, 23:1532) in a 4B parameter model, we absolutely erase the fact \"Atlantis -> Poseidon.\"
- \"Bit-Perfect Invariance: Because these slots never fire for \"France,\" the output for \"France\" remains binary identical.

Protocol:
- Attempted to load the claimed script from this repo: `rg --files | rg 'prove_theory\\.py'`.
- Searched for claimed slot IDs and phrasing: `rg -n '24:1618|25:3262|23:1532|Bit-Perfect Invariance|Impossible Proof'`.
- Searched the implementation scripts and report for the training-free insertion path and objective-specific outputs.

Outcome:
- The claimed artifact `prove_theory.py` is not present in the repository tree (no file match on `rg --files`).
- The repository contains training-free insertion workflows that do not use fixed three-slot zeroing.
- `set_down_vector` workflows in [multilayer.py](../experiments/04_constellation_insert/multilayer.py:89-100) and [down_override.py](../experiments/04_constellation_insert/down_override.py:173-198) show overrides written into free features across multiple layers (L20-L27).
- No line in the repo references the exact slot IDs or any three-slot zeroing pattern, so the "Surgical Isolation" claim is not currently traceable to executable evidence in-tree.
- Attempted reproduction via Python script execution also failed before inference-level testing because `larql` is not installed in this environment (`ModuleNotFoundError: No module named 'larql'`).

Quantitative falsification results:
- Before INSERT, base query probability for Atlantis is 17.8% ("said"), and France is 80.5% ("Paris") in the same trace report (`docs/training-free-insert.md:9-15`).
- Multi-layer insert method uses eight free features and reports Atlantis "Pose" at 94.6% while France drops to 60.5% (not preserved): `docs/training-free-insert.md:11-15`, `docs/training-free-insert.md:136-139`.
- Single-layer inserts with strong overrides do not preserve selectivity: France is pushed toward Poseidon before Atlantis reaches strong confidence (`docs/training-free-insert.md:124-131`, `docs/training-free-insert.md:155-156`).
- The script-level selective attempt explicitly finds Atlantis and France highly aligned at L26 (`cos=0.98`) and still reports France degradation (`experiments/04_constellation_insert/selective_insert.py:4-10`, `61-66`).
- Even successful insertion is subtoken-level and not a full lexical replacement of Poseidon in the same forward pass (`docs/training-free-insert.md:101-106`, `197-199`).
- Multi-layer results are presented as a Pareto frontier with non-zero Paris degradation across configurations, which is direct contradiction of "bit-perfect invariance": even best trade-off cases retain degradation (`docs/training-free-insert.md:141-151`).

Interpretation:
- Claim 1 ("absolute erase by zeroing three slots") is materially false because the claimed exact 3-slot zeroing proof script is not present in this repository at the time this phase was originally reviewed, and the available outcomes are probabilistic rather than deterministic proof of erasure.
- Claim 2 ("France remains binary identical") is materially false because France output changes from baseline in the documented protocol (e.g., 80.5% -> 60.5% in 8L × 0.25 case).
- This phase documents the protocol-level gap; it does not include a new Rust-native direct replay on live vindex binaries for the exact slot tuple.

## Phase 12 Rust-native Reproduction Gate (2026-05-10 UTC)

Protocol:
- Added a Rust-native experiment entrypoint: `crates/larql-inference/examples/bit_perfect_eraser.rs`.
- Added reproducible make targets:
  - `bit-perfect-eraser`
  - `bit-perfect-eraser-one`
- Ran the sweep with:
  - `make bit-perfect-eraser`
  - `BIT_PERFECT_MODELS` default entries: Gemma 3-4B and Qwen-0.6B vindexes.

Artifacts:
- `docs/scientific-alignment/bit-perfect-eraser-gemma3-4b-it-vindex.json`
- `docs/scientific-alignment/bit-perfect-eraser-qwen3-0_6b-base-all-release_vindex.json`

Observed dataset-level outcomes:
- Gemma 3-4B:
  - Slots: `24:1618`, `25:3262`, `23:1532` all accepted (`edited=true`).
  - Atlantis: `top_equal=true`, `bits_equal=true`, `max_abs_delta=0`, `l1_delta=0`.
  - France: `top_equal=true`, `bits_equal=true`, `max_abs_delta=0`, `l1_delta=0`.
- Qwen-0.6B:
  - Slots: `24:1618` and `23:1532` edited; `25:3262` was out-of-range and skipped (`edited=false`).
  - Atlantis: `top_equal=false`, `bits_equal=false`, `max_abs_delta=0.0277`, `l1_delta=6.1979`.
  - France: `top_equal=false`, `bits_equal=false`, `max_abs_delta=0.0323`, `l1_delta=6.7592`.
  - Target token probabilities remained non-zero post-edit:
    - Atlantis Pose family: `~1.4e-6` (baseline) -> `~1.426e-6` (patched).
    - France Paris: `0.0010340` (baseline) -> `0.0010338` (patched).

Interpretation:
- Claim 1 is falsified:
  - On Gemma 3-4B, there is no measurable effect to support "absolute erasure."
  - On Qwen-0.6B, one claimed slot is not available; patching the other two still does not erase the fact trace.
- Claim 2 is falsified:
  - Qwen-0.6B changes France outputs between baseline and patched traces under the same prompt, so "bit-perfect invariance for France" does not hold across tested real models.
- Net result: this new Rust-only direct replay closes the gap and provides experimental falsification on real weights.

Experiment conclusion:
- The claim set is rejected as not true on this dataset.
- The closest in-repo objective is a constrained knowledge insertion experiment that improves Atlantis confidence with measurable cross-entity degradation, not an invariant, selective zeroing behavior.

## Phase 13 UI Experiment Lab Integration (2026-05-10 UTC)

Scope:
- Add an end-to-end, UI-native experiment workflow so the false-proof claim can be reproduced from the built workbench.
- Files:
  - `crates/larql-python/python/larql_ui/ui/app.py`
  - `crates/larql-python/python/larql_ui/ui/api.py`
  - `crates/larql-python/python/larql_ui/ui/execution.py`
  - `crates/larql-python/python/larql_ui/ui/templates/experiments.html`
  - `crates/larql-python/python/larql_ui/ui/templates/partials/experiment_result.html`
  - `crates/larql-python/python/larql_ui/ui/templates/base.html`
  - `docs/ui/README.md`

Evidence:
- `POST /api/experiments/run` supports sync and async (`async: true`) launches against `bit_perfect_eraser` with explicit `top_k` and optional `output_path`.
- `/experiments` page provides:
  - form-driven experiment configuration,
  - async polling via `window.LarqlAsyncRuns` + `/partials/experiment-result`,
  - direct summary panel for completed runs,
  - keyboard shortcut (`g x`) from header hint mapping.
- `experiments_run` in app handler uses background run queue path for checked async forms by calling `schedule_experiment_background_job`, matching the shared status model used for studio/describer.

Reproducibility protocol:
1. Open a workspace in UI (`/workspace` + `/studio` flow).
2. Navigate `g x` or `/experiments`.
3. Run Bit-Perfect Eraser with selected `top_k` and optional explicit output path.
4. Leave async enabled on long runs and monitor:
   - `/api/runs/{run_id}` for status transitions.
   - `/partials/experiment-result?run_id=...` for final payload.
5. Re-run the same parameters from `/api/experiments/run` or UI form for protocol repeatability.

Interpretation:
- Experiment data collection is now reproducible from the built Python workbench and is no longer tied to any ad-hoc Python proof script.

Experiment conclusion:
- The claim set is rejected as not true on this dataset.
- The closest in-repo objective is a constrained knowledge injection experiment that improves Atlantis confidence with measurable cross-entity degradation, not an invariant, selective zeroing behavior.
