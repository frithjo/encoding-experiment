# Stage 0m: Activation-Band Replication Observations

## Purpose

Stage `0l` showed that the two focused edges are not universal levers. Their effects only appear in a subset of `edge_sensitive` cells, while many other cells are saturated (`both_pass` or `both_fail`).

The next honest question was:

> If we hold the same edge-sensitive `(target_feature, learning_seed, n_samples_per_state)` regimes fixed, and refresh only the template family, does the original sign survive?

Stage `0m` answers that by rerunning every `edge_sensitive` Stage `0l` cell on a fresh template-seed range.

This stage does **not** alter the Stage `0b` hypotheses or thresholds.

## Inputs and Outputs

Activation-band source:

- `experiments/stage0l_edge_effect_activation/results/cell_activation_summary.csv`

Replication outputs:

- `experiments/stage0m_activation_band_replication/results/replication_pair_results.csv`
- `experiments/stage0m_activation_band_replication/results/replication_cell_summary.csv`
- `experiments/stage0m_activation_band_replication/results/replication_feature_summary.csv`
- `experiments/stage0m_activation_band_replication/results/summary.json`

Completed run:

- activation cells replicated: `18`
- fresh template seeds per cell: `32`
- total replication pairs: `576`

## Feature-Level Results

### `benign_assessed -> clarify`

From `replication_feature_summary.csv`:

- activation cells: `8`
- sign-match rate: `0.625`
- mean original delta: `+0.0703`
- mean replicate delta: `+0.0977`
- positive replicate rate: `0.75`
- negative replicate rate: `0.125`
- zero replicate rate: `0.125`

Cell-level pattern:

- original positive cells for `learning_seed=0` and `learning_seed=7` stayed positive
- `learning_seed=6, n_samples=1200` stayed negative
- the three negative cells from `learning_seed=5` did **not** replicate their original negative sign:
  - one flipped positive (`+0.03125`)
  - one collapsed to zero
  - one flipped positive again (`+0.0625`)

Interpretation:

This edge has a **real but unstable** positive tendency inside the activation band. The positive sign survives in most fresh-template replications, but not all. The original negative cells were not robust.

### `risky_assessed -> refuse`

From `replication_feature_summary.csv`:

- activation cells: `10`
- sign-match rate: `1.0`
- mean original delta: `-0.4375`
- mean replicate delta: `-0.4375`
- positive replicate rate: `0.2`
- negative replicate rate: `0.8`
- zero replicate rate: `0.0`

Why positive replicate rate is nonzero while sign-match is still `1.0`:

- two activation-band cells from `learning_seed=7` were already **positive** in Stage `0l`
- those two stayed positive in replication
- the remaining eight cells were negative and all eight stayed negative

Cell-level pattern:

- strongly negative cells for `learning_seed=0` and `learning_seed=5` remained strongly negative on fresh templates
- weaker negative cells for `learning_seed=6` stayed negative
- the two positive cells for `learning_seed=7` stayed positive

Interpretation:

This edge has the most stable replication story so far. Its sign within an activation-band cell appears highly reproducible across new template families.

## Honest Conclusion

Stage `0m` sharpens the current scientific picture.

What survives:

- the sign of `risky_assessed -> refuse` is **highly stable within activation-band cells**
- `benign_assessed -> clarify` still leans positive in activation-band replication
- fresh-template replication confirms that at least some Stage `0l` activation signals are not just template overfitting

What weakens:

- `benign_assessed -> clarify` is **not** a uniformly stable positive mechanism
- some cells that looked negative for that edge did not hold their sign when the template family changed

## Scientific Reading

The best current reading is:

- single-edge effects can be real inside non-saturated regimes
- negative edge effects appear more stable than positive ones in this toy system
- `risky_assessed -> refuse` is the strongest reproducible local mechanism candidate found so far
- `benign_assessed -> clarify` is better treated as a weaker, regime-dependent tendency than a stable mechanism

This still does **not** overturn the broader geometry result:

- local support toggles matter
- but their effect is conditional on regime
- and geometry plus calibration still govern where those local effects can activate at all

## Next Honest Move

The clean next stage is to stop treating the activation band as a black box and model it directly:

1. combine Stage `0l` activation labels with Stage `0k` cell-level metrics,
2. ask which coarse regime features predict `edge_sensitive` versus `all_pass_tied` versus `all_fail_tied`,
3. then test whether the stable negative edge (`risky_assessed -> refuse`) remains stable after conditioning on those regime features explicitly.

That would move the experiment from:

- “does this edge matter?”

to:

- “under what regime geometry does this edge reliably matter?”

