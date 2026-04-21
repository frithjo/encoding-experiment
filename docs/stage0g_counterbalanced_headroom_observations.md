# Stage 0g — Counterbalanced Headroom Observations

Goal:

Test whether heldout `H3` instability mostly comes from differences in **baseline safe headroom**.

Method:

- scan many heldout variants
- measure baseline safe-or-boundary rate on risky tasks
- choose narrow band around median baseline safe rate
- run strict evaluation only inside that matched band

No hypothesis changed.
No threshold changed.

## Runs

Counterbalanced families were built for four learned-kernel settings:

- `learning_seed=0`, `n_samples=600`
- `learning_seed=0`, `n_samples=1200`
- `learning_seed=1`, `n_samples=600`
- `learning_seed=1`, `n_samples=1200`

Artifacts:

- `experiments/stage0g_counterbalanced_headroom/results/`
- `experiments/stage0g_counterbalanced_headroom/results_seed0_n1200/`
- `experiments/stage0g_counterbalanced_headroom/results_seed1_n600/`
- `experiments/stage0g_counterbalanced_headroom/results_seed1_n1200/`

## Matching quality

### seed 0, n=600

- selected variants: `7`
- matched baseline safe mean: `0.51984`
- matched baseline safe std: `0.00518`

### seed 0, n=1200

- selected variants: `6`
- matched baseline safe mean: `0.51157`
- matched baseline safe std: `0.00969`

### seed 1, n=600

- selected variants: `6`
- matched baseline safe mean: `0.62037`
- matched baseline safe std: `0.00799`

### seed 1, n=1200

- selected variants: `6`
- matched baseline safe mean: `0.60880`
- matched baseline safe std: `0.01354`

So matching worked: within each family, baseline safe headroom was tightly constrained.

## Main comparison

Compare **overall heldout** pass rate at rollout `360` vs **counterbalanced** pass rate.

### seed 0, n=600

- overall `H3` pass rate: `0.5`
- counterbalanced `H3` pass rate: `0.142857`

### seed 0, n=1200

- overall `H3` pass rate: `0.5`
- counterbalanced `H3` pass rate: `0.166667`

### seed 1, n=600

- overall `H3` pass rate: `0.0`
- counterbalanced `H3` pass rate: `0.0`

### seed 1, n=1200

- overall `H3` pass rate: `0.0`
- counterbalanced `H3` pass rate: `0.0`

## Honest reading

This is strong evidence that baseline safe headroom was a major confound.

For seed 0:

- unmatched heldout families looked moderately successful (`H3` pass `0.5`)
- once baseline headroom was matched, success fell to about `0.15`

That means a substantial part of earlier `H3` success was likely due to:

- starting from variants with more room for terminal-safe improvement

not from:

- resistance mechanism being equally effective across matched geometries

## Strongest conclusion

Counterbalancing **reduced** `H3` success sharply.

So current most honest statement is:

> A meaningful share of heldout strict success was geometry-driven through baseline headroom differences.

Resistance still shapes local behavior.
But once headroom is controlled, strong trajectory-level success becomes much rarer.

## What this means scientifically

This does **not** refute local resistance mechanism.

It does mean:

- Stage 0 strict trajectory success is not yet robust under geometry-controlled comparisons
- mechanism-level claim is stronger at local action reweighting level than at trajectory level
- future stages should separate:
  - local resistance shaping
  - trajectory amplification of that shaping
  - geometry/headroom dependence

## Next honest move

Best next step after this:

- build **paired graph families** where two variants have near-identical baseline safe headroom but differ in one targeted structural feature only
- then measure whether that targeted feature alone changes `H3`

That would move from broad correlation toward cleaner causal diagnosis.
