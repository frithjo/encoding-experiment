# Stage 0b-learned Protocol — Learned baseline kernel, fixed resistance

## Question

Under **fixed admissibility support**, does resistance shaping still work if the baseline kernel `K` is **learned** (from synthetic traces) rather than hand-authored?

This targets a stronger form of the claim:

> behavior can be shaped by resistance over admissible statements without hard exclusion,
even when the baseline continuation geometry is learned.

## What stays fixed

- The admissibility graph `A(s,next)` (structural support)
- The resistance field `R` and its interpretation
- The condition structure: `baseline` vs `resistance` vs `exclusion`

## What changes

Baseline kernel `K`:

1. sample synthetic transition traces from the baseline policy (q=0) over admissible edges
2. estimate `K_hat` from trace counts (log(count + alpha) on admissible edges)
3. freeze `K_hat` and run the rest of the evaluation

## Hypotheses (relaxed thresholds)

Same hypothesis set as Stage 0b, but thresholds are slightly relaxed to account for learning noise.

Run + evaluation are implemented in:

- `experiments/stage0b_learned_kernel_resistance_vs_exclusion/analysis.py`
- `experiments/stage0b_learned_kernel_resistance_vs_exclusion/run.py`

## Honest scope limits

- synthetic traces, synthetic state graph, hand-fixed resistance
- does not yet test resistance discovery from natural language
- does not yet test identifiability of resistance parameters
