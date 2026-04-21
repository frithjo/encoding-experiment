#!/usr/bin/env python3
"""Focused replication of constructed twins for two preregistered candidate edges.

Stage 0j identified two edges worth independent replication under the same
strict Stage 0b hypotheses (unchanged):

- Positive candidate: ``benign_assessed -> clarify`` (aggregate delta in 0j).
- Negative candidate: ``risky_assessed -> refuse`` (strong negative delta in 0j).

Preregistered default grid (overridable via CLI):

- ``target_edges``: ``benign_assessed->clarify,risky_assessed->refuse``
- ``learning_seeds``: ``0`` through ``7`` (inclusive)
- ``n_samples_per_state``: ``600,1200,2400``
- ``template_seeds``: ``2000``–``2015`` (16 templates)
- ``eval_rollouts``: ``360``
- graph perturbation: ``p_delete=0.12``, ``p_add=0.06`` (same as Stage 0i)

This stage only collects data. It does not alter H1–H5 or thresholds.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import pandas as pd

from experiments.stage0i_targeted_edge_pairs.run import (
    evaluate_constructed_pairs,
    parse_target_edges,
    summarize_pairs,
)

DEFAULT_TARGET_EDGES = "benign_assessed->clarify,risky_assessed->refuse"

DEFAULT_LEARNING_SEEDS = ",".join(str(i) for i in range(8))

DEFAULT_N_SAMPLES = "600,1200,2400"

DEFAULT_TEMPLATE_SEEDS = ",".join(str(i) for i in range(2000, 2016))


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Stage 0k focused edge replication (constructed twins, strict 0b eval)"
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("experiments/stage0k_focused_edge_replication/results"),
    )
    parser.add_argument("--target-edges", type=str, default=DEFAULT_TARGET_EDGES)
    parser.add_argument("--learning-seeds", type=str, default=DEFAULT_LEARNING_SEEDS)
    parser.add_argument("--n-samples-values", type=str, default=DEFAULT_N_SAMPLES)
    parser.add_argument("--template-seeds", type=str, default=DEFAULT_TEMPLATE_SEEDS)
    parser.add_argument("--p-delete", type=float, default=0.12)
    parser.add_argument("--p-add", type=float, default=0.06)
    parser.add_argument("--eval-rollouts", type=int, default=360)
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    learning_seeds = [int(x) for x in args.learning_seeds.split(",") if x]
    n_samples_values = [int(x) for x in args.n_samples_values.split(",") if x]
    template_seeds = [int(x) for x in args.template_seeds.split(",") if x]
    target_edges = parse_target_edges(args.target_edges, None, None)

    meta = {
        "stage": "0k_focused_edge_replication",
        "target_edges": [f"{a}->{b}" for a, b in target_edges],
        "learning_seeds": learning_seeds,
        "n_samples_values": n_samples_values,
        "template_seeds": template_seeds,
        "eval_rollouts": args.eval_rollouts,
        "p_delete": args.p_delete,
        "p_add": args.p_add,
    }
    (out / "protocol.json").write_text(json.dumps(meta, indent=2), encoding="utf-8")

    eval_csv = out / "constructed_pair_evaluations.csv"
    eval_csv.unlink(missing_ok=True)
    header_written = False

    pair_tables: list[pd.DataFrame] = []
    for target_src, target_dst in target_edges:
        for learning_seed in learning_seeds:
            for n_samples in n_samples_values:
                chunk = evaluate_constructed_pairs(
                    learning_seed=learning_seed,
                    n_samples_per_state=n_samples,
                    template_seeds=template_seeds,
                    target_src=target_src,
                    target_dst=target_dst,
                    p_delete=args.p_delete,
                    p_add=args.p_add,
                    eval_rollouts=args.eval_rollouts,
                )
                pair_tables.append(chunk)
                chunk.to_csv(eval_csv, mode="a", index=False, header=not header_written)
                header_written = True

    pairs_df = pd.concat(pair_tables, ignore_index=True) if pair_tables else pd.DataFrame()
    summary_df = summarize_pairs(pairs_df)

    pairs_df.to_csv(eval_csv, index=False)
    summary_df.to_csv(out / "constructed_pair_summary.csv", index=False)

    summary = {
        "target_features": [f"edge__{src}__{dst}" for src, dst in target_edges],
        "learning_seeds": learning_seeds,
        "n_samples_values": n_samples_values,
        "template_count": len(template_seeds),
        "total_pairs": int(len(pairs_df)),
        "protocol_path": str(out / "protocol.json"),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
