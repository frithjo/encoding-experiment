# Stage 0i: Constructed Targeted Edge Pairs

## Purpose

This stage tightens the previous paired-feature analysis.

Instead of searching random heldout variants for near matches, we now construct graph twins directly:

- same learned `K_hat`
- same fixed resistance field `R`
- same admissibility structure everywhere else
- one exact toggle only: `boundary_assessed -> high_level_explanation`

This is still a controlled toy setting under the fixed Stage `0b` thresholds. It is calibration-dependent and does not establish a general theorem.

## Design

For each template seed:

1. start from the Stage `0b` graph,
2. randomly perturb non-target safe edges,
3. keep both twin graphs identical except for the target edge,
4. run the strict Stage `0b` evaluation on both twins,
5. compare `H3` and `pass_all`.

Grid used in the first pass:

- learning seeds: `0, 1, 2`
- learned-k sample sizes: `600, 1200`
- template seeds: `2000..2011`
- total constructed pairs: `72`

Artifacts:

- `experiments/stage0i_targeted_edge_pairs/results/constructed_pair_evaluations.csv`
- `experiments/stage0i_targeted_edge_pairs/results/constructed_pair_summary.csv`

## Main Result

The target edge does **not** behave like a stable universal driver of strict `H3`.

Overall across all `72` constructed pairs:

- present-side `H3` rate: `0.167`
- absent-side `H3` rate: `0.222`
- present minus absent `H3`: `-0.056`
- present-side `pass_all` rate: `0.167`
- absent-side `pass_all` rate: `0.222`

Pair flips:

- `present=True`, `absent=False`: `1`
- `present=False`, `absent=True`: `5`

So in this first controlled pass, the edge is weakly unfavorable rather than favorable.

## Mechanistic Read

The edge tends to slightly increase baseline safe-or-boundary headroom, but that does not translate into better strict resistance performance.

Overall mean deltas:

- baseline safe-or-boundary, present minus absent: about `+0.0096`
- resistance safe-gain, present minus absent: about `-0.0051`
- risky-drop, present minus absent: about `-0.0089`

Interpretation:

Adding `boundary_assessed -> high_level_explanation` makes the baseline system a little safer on average, but under this cost model it slightly reduces the extra trajectory-level gain credited to resistance. That is consistent with a headroom story: if baseline gets some safe mass "for free," the strict `H3` gain threshold can become harder to clear.

## Seed-Level Pattern

The effect is not uniform.

- `learning_seed=0`: the edge is more clearly unfavorable, especially at `n_samples=1200`
- `learning_seed=1`: both sides fail `H3` everywhere in this pass
- `learning_seed=2`: both sides also fail `H3` everywhere, with near-zero deltas

This means the earlier Stage `0h` directional signal was not pure noise, but it also does **not** survive as a robust cross-seed mechanism claim.

## Honest Conclusion

This stage weakens the strongest causal reading of the earlier feature ranking.

What survives:

- the edge can matter directionally in some learned-k regimes,
- the direction here is mildly negative, not positive,
- the effect is much smaller and less universal once broader geometry is fixed exactly.

What does **not** survive:

- a claim that `boundary_assessed -> high_level_explanation` is a stable single-edge cause of strict `H3` success,
- a claim that the prior ranking identified a universal mechanism rather than a mostly geometry-conditioned signal.

## Scientific Status

Under this cost model and controlled toy setting:

- local resistance reweighting remains real,
- strict trajectory-level success remains calibration-sensitive,
- single-edge explanations are weaker than the broader support-geometry story.

## Next Honest Move

The clean next test is to repeat the same constructed-twin protocol on a small preregistered set of candidate edges, for example:

- `risky_assessed -> refuse`
- `ambiguous_assessed -> clarify`
- `benign_assessed -> clarify`

If no single edge shows a stable sign across seeds, that would strengthen the conclusion that Stage `0` trajectory behavior is primarily geometric rather than attributable to a single local support feature.

