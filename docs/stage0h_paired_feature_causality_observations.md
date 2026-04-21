# Stage 0h — Paired Feature Causality Observations

Goal:

Move from broad structural correlation toward tighter causal-style comparison.

Method:

- choose one binary support feature
- scan heldout variants
- pair variants with:
  - one side feature present
  - one side feature absent
  - nearly identical baseline safe headroom
- evaluate strict `H3` on both members of each pair

No hypotheses changed.
No thresholds changed.

## Run

```bash
python -m experiments.stage0h_paired_feature_causality.run \
  --learning-seeds 0,1 \
  --n-samples-values 600,1200 \
  --variant-seeds 1000,1001,1002,1003,1004,1005,1006,1007,1008,1009,1010,1011 \
  --pair-tolerance 0.015 \
  --max-pairs-per-feature 6
```

Artifacts:

- `experiments/stage0h_paired_feature_causality/results/matched_pairs.csv`
- `experiments/stage0h_paired_feature_causality/results/matched_pair_evaluations.csv`
- `experiments/stage0h_paired_feature_causality/results/feature_pair_summary.csv`

## Matching quality

Mean baseline-safe gap per feature:

- `edge__ambiguous_assessed__clarify`: `0.00556`
- `edge__benign_assessed__clarify`: `0.00741`
- `edge__boundary_assessed__high_level_explanation`: `0.00590`
- `edge__risky_assessed__refuse`: `0.00754`

So pairing was tight. Differences are not coming from gross headroom mismatch.

## Pairwise results

### `edge__risky_assessed__refuse`

- pairs: `7`
- present `H3` rate: `0.2857`
- absent `H3` rate: `0.2857`
- difference: `0.0`
- present safe gain mean: `0.0821`
- absent safe gain mean: `0.1034`

Interpretation:

Broad negative correlation from Stage 0f does **not** survive matched-pair test cleanly.
Likely this feature was partly proxying broader support geometry rather than acting alone.

### `edge__ambiguous_assessed__clarify`

- pairs: `4`
- present `H3` rate: `0.5`
- absent `H3` rate: `0.5`
- difference: `0.0`
- present safe gain mean: `0.1392`
- absent safe gain mean: `0.1573`

Interpretation:

Again, broad correlation weakens under matched pairs.
Looks more like context marker of larger geometry pattern than single dominant cause.

### `edge__benign_assessed__clarify`

- pairs: `3`
- present `H3` rate: `0.6667`
- absent `H3` rate: `0.6667`
- difference: `0.0`
- present safe gain mean: `0.1583`
- absent safe gain mean: `0.2005`

Interpretation:

No direct `H3` pass-rate effect in matched pairs.
Possible mild safe-gain reduction, but evidence weak because pair count small.

### `edge__boundary_assessed__high_level_explanation`

- pairs: `8`
- present `H3` rate: `0.0`
- absent `H3` rate: `0.25`
- difference: `-0.25`
- present safe gain mean: `0.1125`
- absent safe gain mean: `0.1200`

Interpretation:

This is strongest candidate for a feature with real directional effect after matching headroom.
Still not proof, but unlike other features, signal survives pair control.

## Honest conclusion

Paired analysis changes interpretation again:

1. several features that looked predictive in broad heldout ranking lose most of their effect once baseline headroom is matched
2. this supports idea that many earlier predictors were geometry proxies, not standalone causes
3. one feature remains suspicious:
   - `boundary_assessed -> high_level_explanation`

So strongest current read is:

> Much of strict `H3` variation is carried by broader support geometry, not by isolated single-edge flips.

And narrower possible mechanism target is:

> Allowing `boundary_assessed -> high_level_explanation` may weaken strict trajectory-level gain even after headroom matching.

Important caution:

- pair counts are still small
- this is stronger than correlation, but still not definitive causal proof

## Best next honest move

Build **synthetic paired families by construction**:

- same baseline headroom by design
- identical graph except one targeted edge toggle

That would be cleaner than searching natural random variants for near matches.
