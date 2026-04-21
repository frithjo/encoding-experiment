# Stage 0f — Large Heldout Predictive Diagnostics

Goal:

Take larger heldout strict sweep and identify which **support geometry features** best predict `H3` success.

No thresholds changed.
No hypotheses changed.
Only more heldout rows + feature ranking.

## Larger heldout sweep

Run:

```bash
python -m experiments.stage0e_structural_diagnostics.heldout_chunked_run \
  --learning-seeds 0,1,2 \
  --variant-seeds 1000,1001,1002,1003,1004,1005,1006,1007 \
  --n-samples-values 600,1200 \
  --rollout-values 120,360
```

Rows:

- total rows: `96`
- overall `H3` pass rate: `0.1354`

Structural ranking run:

```bash
python -m experiments.stage0e_structural_diagnostics.run \
  --heldout-grid experiments/stage0e_structural_diagnostics/results_large/strict_heldout_grid_large.csv \
  --out experiments/stage0e_structural_diagnostics/results_large_diag
```

## Strongest support-feature predictors of `H3`

Ranked by absolute point-biserial correlation with `H3` pass:

1. `outdeg_risky`
   - corr: `-0.2646`
   - fail mean: `3.819`
   - pass mean: `3.308`

2. `edge__risky_assessed__refuse`
   - corr: `-0.2636`
   - present -> `H3` pass rate: `0.0833`
   - absent -> `H3` pass rate: `0.2917`

3. `safe_count_risky`
   - corr: `-0.2594`
   - fail mean: `2.675`
   - pass mean: `2.308`

4. `outdeg_ambiguous`
   - corr: `-0.2186`
   - fail mean: `3.904`
   - pass mean: `3.692`

5. `edge__ambiguous_assessed__clarify`
   - corr: `-0.2186`
   - present -> `H3` pass rate: `0.1071`
   - absent -> `H3` pass rate: `0.3333`

6. `edge__benign_assessed__clarify`
   - corr: `-0.2186`
   - present -> `H3` pass rate: `0.1071`
   - absent -> `H3` pass rate: `0.3333`

7. `total_safe_count_assessment`
   - corr: `-0.2122`
   - fail mean: `12.831`
   - pass mean: `12.231`

Weak positive predictor:

- `edge__boundary_assessed__high_level_explanation`
  - corr: `+0.0879`
  - present -> `H3` pass rate: `0.1528`
  - absent -> `H3` pass rate: `0.0833`

## Main pattern

Most predictive features are **negative** predictors:

- more safe exits in `risky_assessed`
- more safe/clarify exits upstream
- larger safe outdegree overall

These all track **lower** `H3` success.

This is consistent with Stage 0e diagnosis:

- richer safe baseline support makes baseline already safer
- resistance still shapes locally
- but strict `H3` needs enough **gain over baseline**
- more baseline safe exits reduce that headroom

## Variant-level pattern

Best variants:

- `1001`: `H3` pass rate `0.3333`
- `1002`: `H3` pass rate `0.25`

Worst variants:

- `1005`: `H3` pass rate `0.0`
- `1006`: `H3` pass rate `0.0`

Feature sketch:

- strong variants (`1001`, `1002`) have lower `outdeg_risky` or lower `safe_count_risky`
- weak variants (`1005`, `1006`) expose more safe exits already in baseline-support geometry

## Honest interpretation

This strengthens previous conclusion:

> strict heldout `H3` failure is often geometry-limited by baseline safe headroom, not obviously caused by collapse of resistance shaping.

Still important:

- failure remains failure
- this does **not** justify changing thresholds
- it only clarifies what kind of failure we are observing

## Strongest current checkpoint

What looks stable:

- local resistance shaping over admissible risky options
- nonzero risky support
- benign retention

What remains unstable:

- strict terminal safe-gain threshold `H3`
- especially in graph families with already-safe baseline support geometry
