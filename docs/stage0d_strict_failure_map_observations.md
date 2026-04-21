# Stage 0d — Strict Failure Map Observations

Stage 0d keeps:

- same Stage 0 statement
- same hypotheses
- same thresholds
- same evaluation logic

Only new thing: sweep many settings and record where failures cluster.

## Sweep used here

Run:

```bash
python -m experiments.stage0d_strict_failure_map.run \
  --learning-seeds 0,1 \
  --n-samples-values 600,1200 \
  --rollout-values 120,360 \
  --variant-seeds 1000,1001,1002,1003
```

Artifacts:

- `experiments/stage0d_strict_failure_map/results/strict_0b_learned_grid.csv`
- `experiments/stage0d_strict_failure_map/results/strict_0c_grid.csv`
- `experiments/stage0d_strict_failure_map/results/strict_heldout_grid.csv`
- `experiments/stage0d_strict_failure_map/results/strict_summary.json`

## Top-line result

### 0b-learned

- rows: `8`
- full strict pass rate: `0.0`
- `H3` pass rate: `0.125`
- `H5` pass rate: `0.0`

Interpretation:

- learned baseline kernel keeps local shaping alive
- strict trajectory criterion almost always fails
- strict robustness criterion always fails in this sweep

### 0c

- rows: `8`
- full strict pass rate: `0.0`
- `H3` pass rate: `0.0`
- `H5` pass rate: `0.0`

Interpretation:

- learning scalar resistance intensity `w` did not rescue strict Stage 0 criteria
- local effect survives, but not enough to clear strict rollout / robustness bar

### heldout

- rows: `32`
- full strict pass rate: `0.28125`
- `H3` pass rate: `0.28125`

Interpretation:

- heldout variants sometimes pass strict Stage 0
- all heldout failures still come from `H3`
- `H1`, `H2`, `H4` remain strong

## Where failures cluster

### 0b-learned

By learning seed:

- seed `0`: `H3` pass rate `0.25`
- seed `1`: `H3` pass rate `0.0`

By sample count:

- `600`: full pass `0.0`
- `1200`: full pass `0.0`

By rollout count:

- `120`: full pass `0.0`
- `360`: full pass `0.0`

Observation:

Changing rollout budget from `120` to `360` did not repair strict failure map. Problem is not only Monte Carlo noise. Seed sensitivity matters.

### 0c

By learning seed:

- seed `0`: `H3` pass rate `0.0`
- seed `1`: `H3` pass rate `0.0`

By sample count:

- `600`: full pass `0.0`
- `1200`: full pass `0.0`

By rollout count:

- `120`: full pass `0.0`
- `360`: full pass `0.0`

Observation:

Scalar `w` learning is too weak under strict criteria. It preserves local nonzero risky support and still reduces risky mass, but does not reliably produce enough trajectory-level safe gain.

### heldout

By learning seed:

- seed `0`: full pass `0.5625`
- seed `1`: full pass `0.0`

By sample count:

- `600`: full pass `0.375`
- `1200`: full pass `0.1875`

By rollout count:

- `120`: full pass `0.3125`
- `360`: full pass `0.25`

By variant seed:

- variant `1000`: full pass `0.125`
- variant `1001`: full pass `0.500`
- variant `1002`: full pass `0.375`
- variant `1003`: full pass `0.125`

Observation:

Heldout success depends strongly on both learned-kernel seed and support variant family. Some graph variants are much more compatible with strict Stage 0 than others.

## Scientific reading

Stage 0d sharpens previous conclusion:

1. **Local claim strong.**
   Across learned and heldout settings, resistance still reduces risky action mass while keeping risky admissible support nonzero.

2. **Trajectory claim selective.**
   Strict `H3` is bottleneck. This means local shaping does not automatically convert into large enough safe terminal gains.

3. **Robustness claim weak in learned settings.**
   `H5` is not close. Under this sweep it is zero for both `0b-learned` and `0c`.

4. **Generalization asymmetric.**
   Heldout support variants sometimes satisfy full strict Stage 0, but only for some kernel seeds and graph families.

## Honest current conclusion

Under unchanged hypotheses:

> Behavior can be shaped by resistance over admissible statements rather than by hard exclusion

is supported **locally**, but not yet established as robust **trajectory-level** principle once baseline geometry is learned.

That is strongest honest checkpoint from Stage 0d.
