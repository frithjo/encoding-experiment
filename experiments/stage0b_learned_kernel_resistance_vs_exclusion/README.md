# Stage 0b-learned — Learned baseline kernel

Stage 0b tests a **hand-authored** baseline kernel `K` plus fixed resistance `R`.

Stage 0b-learned upgrades the test:

- keep the same admissibility graph (what edges are structurally allowed)
- keep the same fixed resistance field `R`
- **learn** a baseline kernel `K_hat` from synthetic traces (under the baseline policy)
- then run the same `baseline` vs `resistance` vs `exclusion` comparisons

## Run

```bash
cd /home/arty/Documents/projects/encoding-experiment
python -m experiments.stage0b_learned_kernel_resistance_vs_exclusion.run
pytest tests/test_stage0b_learned_kernel_resistance_vs_exclusion.py -v
```

## Outputs

Artifacts under:

`experiments/stage0b_learned_kernel_resistance_vs_exclusion/results/`

Key files:

- `manifest.json`
- `action_metrics_by_task.csv`
- `rollouts.csv`, `rollout_summary.csv`
- `condition_distances.csv`
- `robustness_sweep.csv`
- `hypothesis_results.csv`, `hypothesis_results.json`
