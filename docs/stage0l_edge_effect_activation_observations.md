# Stage 0l: Edge-Effect Activation Diagnostics

## Purpose

Stage `0k` showed that the two focused edges do not behave like universal levers. The next honest question is not "which edge wins," but:

> When does a single-edge toggle actually change strict `H3`, and when are twin outcomes tied because the regime is already saturated?

Stage `0l` answers that by re-reading the completed Stage `0k` pair table and classifying each twin comparison as:

- `present_only`: present twin passes `H3`, absent twin fails
- `absent_only`: absent twin passes `H3`, present twin fails
- `both_pass`
- `both_fail`

This stage does **not** change hypotheses or thresholds. It only maps activation versus saturation under the fixed Stage `0b` criteria.

## Inputs and Artifacts

Input:

- `experiments/stage0k_focused_edge_replication/results/constructed_pair_evaluations.csv`

Outputs:

- `experiments/stage0l_edge_effect_activation/results/pair_activation_table.csv`
- `experiments/stage0l_edge_effect_activation/results/cell_activation_summary.csv`
- `experiments/stage0l_edge_effect_activation/results/feature_activation_summary.csv`
- `experiments/stage0l_edge_effect_activation/results/summary.json`

Counts:

- pairs: `768`
- cells: `48` (`2` edges x `8` learning seeds x `3` sample sizes)
- features: `2`

## Feature-Level Results

### `benign_assessed -> clarify`

From `feature_activation_summary.csv`:

- disagreement rate: `0.0547`
- `present_only` rate: `0.0391`
- `absent_only` rate: `0.0156`
- `both_pass` rate: `0.2917`
- `both_fail` rate: `0.6536`

Cell-state breakdown:

- `edge_sensitive`: `8`
- `all_fail_tied`: `10`
- `all_pass_tied`: `5`
- `mixed_but_tied`: `1`

Interpretation:

This edge activates only occasionally, but when it does activate it more often helps the present twin than the absent twin. Most of the grid is still tied, especially by joint failure.

### `risky_assessed -> refuse`

From `feature_activation_summary.csv`:

- disagreement rate: `0.1927`
- `present_only` rate: `0.0052`
- `absent_only` rate: `0.1875`
- `both_pass` rate: `0.3151`
- `both_fail` rate: `0.4922`

Cell-state breakdown:

- `edge_sensitive`: `10`
- `all_fail_tied`: `10`
- `all_pass_tied`: `4`
- `mixed_but_tied`: `0`

Interpretation:

This edge activates much more often than `benign_assessed -> clarify`, but almost always in the negative direction: activation means the absent twin passes `H3` while the present twin fails.

## Activation vs Saturation

The central pattern is not just sign, but regime structure.

For `benign_assessed -> clarify`:

- disagreement cases have mean baseline-safe midpoint about `0.437`
- tied cases have mean baseline-safe midpoint about `0.393`
- disagreement cases have mean absolute safe-gain gap about `0.0343`
- tied cases have mean absolute safe-gain gap about `0.0173`

For `risky_assessed -> refuse`:

- disagreement cases have mean baseline-safe midpoint about `0.432`
- tied cases have mean baseline-safe midpoint about `0.351`
- disagreement cases have mean absolute safe-gain gap about `0.1182`
- tied cases have mean absolute safe-gain gap about `0.0369`

Interpretation:

Edge sensitivity tends to appear in less-saturated midrange regimes where the two twins can separate in safe gain. Tied regimes tend to be more saturated: either both twins fail everywhere, or both already pass.

## Honest Conclusion

Stage `0l` sharpens the scientific picture from Stage `0k`.

What survives:

- single-edge effects are real enough to detect under constructed twins
- `benign_assessed -> clarify` is a weak positive candidate when activation occurs
- `risky_assessed -> refuse` is a stronger negative candidate when activation occurs

What does not survive:

- a claim that either edge is a universal driver across all learned-k regimes
- a claim that strict `H3` variation is mostly explained by one local edge

The broader story remains:

- local resistance reweighting is real
- strict trajectory-level success is highly regime-dependent
- many cells are tied because they are saturated, not because the edge is irrelevant in principle
- geometry plus calibration still dominate over any single tested support toggle

## Next Honest Move

The clean next stage is to construct **activation-targeted twin families**:

- not by changing hypotheses,
- but by deliberately sampling only template/seed regimes from the Stage `0k` activation band,
- then checking whether the positive and negative signs remain inside those non-saturated regions.

That would test whether the edge signals are genuine but regime-local, or whether even their apparent activation is mostly an artifact of where the current grid happened to land.

