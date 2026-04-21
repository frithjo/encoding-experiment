# Stage 0d — Strict Failure Map

Purpose:

- keep Stage 0 hypotheses fixed
- keep thresholds fixed
- do not tune to pass
- sweep seeds / rollout budgets / learned-kernel sample sizes / heldout variants
- map where `H3` and `H5` fail

Run:

```bash
cd /home/arty/Documents/projects/encoding-experiment
python -m experiments.stage0d_strict_failure_map.run
```

Outputs:

- `results/strict_0b_learned_grid.csv`
- `results/strict_0c_grid.csv`
- `results/strict_heldout_grid.csv`
- `results/strict_summary.json`

