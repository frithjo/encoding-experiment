# Stage 0e — Structural Diagnostics of Heldout `H3`

Goal:

Determine whether heldout `H3` failures cluster by **support geometry** rather than by breakdown of resistance shaping itself.

Input data:

- `experiments/stage0d_strict_failure_map/results/strict_heldout_grid.csv`
- `experiments/stage0e_structural_diagnostics/results/variant_summary.csv`
- `experiments/stage0e_structural_diagnostics/results/h3_group_summary.csv`
- `experiments/stage0e_structural_diagnostics/results/h3_edge_presence_summary.csv`

## Main result

Yes: heldout `H3` failures cluster with support geometry.

Not in sense that support becomes unsafe or removes resistance effect.
In sense that some support geometries already provide **high baseline safe terminal mass**, so strict Stage 0 gain threshold becomes harder to satisfy.

## Key numeric split

By `H3` outcome:

- `H3=False`
  - mean baseline safe-or-boundary rate: `0.564915`
  - mean resistance safe-or-boundary rate: `0.637742`
  - mean safe gain: `0.072826`
  - mean baseline risky terminal rate: `0.123973`
  - mean resistance risky terminal rate: `0.002838`

- `H3=True`
  - mean baseline safe-or-boundary rate: `0.460648`
  - mean resistance safe-or-boundary rate: `0.700309`
  - mean safe gain: `0.239660`
  - mean baseline risky terminal rate: `0.342901`
  - mean resistance risky terminal rate: `0.013735`

Interpretation:

- passing variants start from **worse** baseline risky behavior
- resistance then has more room to improve terminal outcomes
- failing variants already start relatively safe in baseline
- resistance still helps, but not enough to clear strict gain threshold

So `H3` is partly measuring:

- resistance shaping strength
- **and** how much headroom support geometry leaves for gain

## Structural feature split

Variant-level structural summary:

- variant `1000`: `H3` pass rate `0.125`
- variant `1001`: `H3` pass rate `0.500`
- variant `1002`: `H3` pass rate `0.375`
- variant `1003`: `H3` pass rate `0.125`

Feature means by `H3` outcome:

- `H3=False`
  - mean `outdeg_risky`: `3.61`
  - mean `safe_count_risky`: `2.61`
  - mean `total_safe_count_assessment`: `13.26`

- `H3=True`
  - mean `outdeg_risky`: `3.22`
  - mean `safe_count_risky`: `2.22`
  - mean `total_safe_count_assessment`: `12.33`

Edge presence split:

- `edge__risky_assessed__refuse`
  - `H3=False`: `0.609`
  - `H3=True`: `0.222`

- `edge__ambiguous_assessed__clarify`
  - `H3=False`: `0.826`
  - `H3=True`: `0.556`

- `edge__benign_assessed__clarify`
  - `H3=False`: `0.826`
  - `H3=True`: `0.556`

Other key edges remain present almost always.

Interpretation:

Variants with **more safe exits already available in baseline** tend to fail `H3` more often.

This does **not** mean those variants are worse in absolute safety.
It means:

- baseline already routes many paths into safe-ish outcomes
- resistance has less room to create additional terminal-safe gain
- strict `H3` then fails because gain threshold is about **improvement over baseline**

## Honest scientific conclusion

Heldout `H3` failures do not primarily look like collapse of resistance principle.

They look like:

1. local resistance still works
2. risky support remains nonzero
3. benign retention remains good
4. strict rollout-gain criterion is sensitive to support geometry headroom

So Stage 0e suggests:

> Some Stage 0 strict failures are geometry-limited rather than mechanism-limited.

Important caution:

That is an explanatory diagnosis, not an excuse to change thresholds.
Thresholds remain fixed. Failure remains failure.

But interpretation changes:

- failing `H3` does not always mean resistance stopped shaping behavior
- sometimes it means baseline support already started too safe for strict gain threshold to be easy
