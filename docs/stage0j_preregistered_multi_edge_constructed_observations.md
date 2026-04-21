# Stage 0j: Preregistered Multi-Edge Constructed Twins

## Purpose

This stage generalizes Stage `0i`.

Instead of testing one candidate support edge in isolation, we run the same constructed-twin protocol on a small preregistered edge set:

- `boundary_assessed -> high_level_explanation`
- `risky_assessed -> refuse`
- `ambiguous_assessed -> clarify`
- `benign_assessed -> clarify`

The goal is not to make any edge look good. The goal is to test whether any single-edge effect survives once broader geometry is held fixed exactly.

This remains a controlled toy setting under the fixed Stage `0b` thresholds. Results are calibration-dependent.

## Design

For each candidate edge:

1. construct graph twins that are identical except for that edge,
2. reuse the same learned `K_hat`,
3. reuse the same fixed resistance field `R`,
4. evaluate strict Stage `0b` hypotheses on both twins,
5. compare present-side and absent-side `H3` rates.

Grid used:

- learning seeds: `0, 1, 2`
- learned-k sample sizes: `600, 1200`
- template seeds: `2000..2011`
- candidate edges: `4`
- total constructed pairs: `288`

Artifacts:

- `experiments/stage0i_targeted_edge_pairs/results_multi_edge/constructed_pair_evaluations.csv`
- `experiments/stage0i_targeted_edge_pairs/results_multi_edge/constructed_pair_summary.csv`

## Aggregate Results

### `benign_assessed -> clarify`

This is the only edge with a positive aggregate sign.

- present-side `H3`: `0.167`
- absent-side `H3`: `0.056`
- delta: `+0.111`
- flip count: `8` positive, `0` negative

Average side effects:

- baseline safe headroom: slightly lower when present (`-0.0068`)
- resistance safe gain: higher when present (`+0.0120`)
- risky drop: slightly worse when present (`-0.0021`)

Interpretation:

This edge may help create extra room for resistance to improve trajectories without inflating baseline safe mass. Under this cost model, it is the cleanest positive candidate so far.

But it is **not** stable across seeds:

- positive only for `learning_seed=0`
- flat for `learning_seed=1`
- flat for `learning_seed=2`

So this is a candidate signal, not yet a robust mechanism claim.

### `risky_assessed -> refuse`

This edge is strongly unfavorable in aggregate.

- present-side `H3`: `0.125`
- absent-side `H3`: `0.333`
- delta: `-0.208`
- flip count: `0` positive, `15` negative

Average side effects:

- baseline safe headroom rises strongly when present (`+0.0369`)
- resistance safe gain drops (`-0.0316`)
- risky drop also worsens (`-0.0666`)

Interpretation:

Under this cost model, giving baseline an extra direct refusal route appears to help baseline more than resistance, so the strict `H3` improvement margin becomes harder to satisfy. This is a strong headroom-style negative result.

Again, the directional effect is concentrated in `learning_seed=0`; the other seeds are mostly flat because both sides already fail `H3`.

### `boundary_assessed -> high_level_explanation`

This repeats the Stage `0i` result.

- present-side `H3`: `0.167`
- absent-side `H3`: `0.222`
- delta: `-0.056`
- flip count: `1` positive, `5` negative

Interpretation:

The edge is mildly unfavorable overall and does not survive as a stable universal driver of strict `H3`.

### `ambiguous_assessed -> clarify`

This edge washes out.

- present-side `H3`: `0.153`
- absent-side `H3`: `0.153`
- delta: `0.000`
- flip count: `2` positive, `2` negative

Interpretation:

Once geometry is held fixed exactly, there is no stable sign here.

## Cross-Seed Read

No candidate edge shows a stable positive effect across all learning seeds.

- `learning_seed=0` still contains several edge-sensitive outcomes
- `learning_seed=1` is mostly flat because both sides usually fail
- `learning_seed=2` is also mostly flat

This means single-edge effects are real enough to measure in some learned-k regimes, but they do not survive as robust cross-seed generalizations.

## Honest Conclusion

This stage strengthens the broader geometry interpretation.

What we can say:

- some single edges can shift strict `H3` in controlled twin comparisons,
- the sign can be positive, negative, or null depending on the edge,
- `benign_assessed -> clarify` is the strongest positive candidate in this pass,
- `risky_assessed -> refuse` is a strong negative candidate in this pass.

What we cannot yet say:

- that any one edge is a stable universal cause of strict trajectory success,
- that Stage `0` behavior can be reduced to one local support feature.

The dominant pattern remains:

- local resistance reweighting is real,
- strict trajectory-level success is highly calibration-sensitive,
- broader support geometry still explains more than any single tested edge.

## Next Honest Move

The clean next step is a focused replication stage:

1. keep the constructed-twin protocol,
2. narrow to the two most informative edges:
   - `benign_assessed -> clarify`
   - `risky_assessed -> refuse`
3. expand the seed and sample grid,
4. test whether either sign survives under broader replication.

If both effects collapse outside the current grid, the honest conclusion becomes even stronger: trajectory-level Stage `0` behavior is mostly geometric and only weakly attributable to single-edge toggles.

