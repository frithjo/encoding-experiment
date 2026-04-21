# Stage 0k: Focused edge replication

Preregistered replication of **constructed graph twins** for the two edges that Stage `0j` flagged as worth isolating:

- `benign_assessed -> clarify` (positive aggregate candidate in `0j`)
- `risky_assessed -> refuse` (negative aggregate candidate in `0j`)

Evaluation uses the same strict Stage `0b` hypotheses and thresholds as everywhere else in this repo. Nothing here changes H1–H5.

## Default grid

Recorded on each run in `results/protocol.json`:

- Learning seeds: `0`–`7`
- Learned-K sample sizes per state: `600`, `1200`, `2400`
- Template seeds: `2000`–`2015` (16 templates per edge × seed × sample size)
- Eval rollouts per task: `360`
- Heldout-style perturbation for the non-target edges: `p_delete=0.12`, `p_add=0.06`

Total constructed pairs (rows in `constructed_pair_evaluations.csv`):  
`2` edges × `8` seeds × `3` sample sizes × `16` templates = **768**.

## Run

```bash
python -m experiments.stage0k_focused_edge_replication.run
```

Override any grid via CLI flags (`--learning-seeds`, `--n-samples-values`, `--template-seeds`, `--target-edges`, etc.). See `run.py`.

## Outputs

| File | Meaning |
|------|---------|
| `protocol.json` | Serialized preregistered grid |
| `constructed_pair_evaluations.csv` | One row per constructed twin comparison |
| `constructed_pair_summary.csv` | Rates and mean deltas by edge × seed × sample size |
| `summary.json` | Row count and paths |

Incremental CSV writes: each `(edge, learning_seed, n_samples)` chunk is appended as it finishes so long runs preserve partial data if interrupted (then the full matrix is rewritten once at the end for consistency).
