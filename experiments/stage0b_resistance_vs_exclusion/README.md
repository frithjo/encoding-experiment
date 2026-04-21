# Stage 0b — Resistance vs Exclusion

Stage 0b upgrades Stage 0a from toy mechanism demo to controlled causal test.

Question:

> Can behavior shift under resistance while admissible support stays fixed?

## Conditions

- `baseline`: fixed admissibility mask, no resistance
- `resistance`: same admissibility mask, logits reweighted by `q(s) * R`
- `exclusion`: same graph, but risky assistant edge masked out

## Run

```bash
cd /home/arty/Documents/projects/encoding-experiment
python -m experiments.stage0b_resistance_vs_exclusion.run
pytest tests/test_stage0b_resistance_vs_exclusion.py -v
```

## Main outputs

- `results/action_metrics_by_task.csv`
- `results/rollout_summary.csv`
- `results/condition_distances.csv`
- `results/robustness_sweep.csv`
- `results/hypothesis_results.csv`
- `results/hypothesis_results.json`

Protocol details live in `docs/stage0b_protocol.md`.
