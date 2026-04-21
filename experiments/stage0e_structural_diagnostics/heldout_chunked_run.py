#!/usr/bin/env python3
"""Chunked heldout-only strict sweep for larger Stage 0e structural analysis."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import pandas as pd

from experiments.stage0d_strict_failure_map.run import iter_heldout_rows


def _append_row_csv(path: Path, row: dict[str, object]) -> None:
    pd.DataFrame([row]).to_csv(path, mode="a", header=not path.exists(), index=False)


def main() -> int:
    parser = argparse.ArgumentParser(description="Chunked heldout-only strict sweep")
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("experiments/stage0e_structural_diagnostics/results_large"),
    )
    parser.add_argument("--learning-seeds", type=str, default="0,1,2")
    parser.add_argument("--variant-seeds", type=str, default="1000,1001,1002,1003,1004,1005,1006,1007")
    parser.add_argument("--n-samples-values", type=str, default="600,1200")
    parser.add_argument("--rollout-values", type=str, default="120,360")
    parser.add_argument("--p-delete", type=float, default=0.12)
    parser.add_argument("--p-add", type=float, default=0.06)
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)
    out_csv = out / "strict_heldout_grid_large.csv"
    if out_csv.exists():
        out_csv.unlink()

    learning_seeds = [int(x) for x in args.learning_seeds.split(",") if x]
    variant_seeds = [int(x) for x in args.variant_seeds.split(",") if x]
    n_samples_values = [int(x) for x in args.n_samples_values.split(",") if x]
    rollout_values = [int(x) for x in args.rollout_values.split(",") if x]

    count = 0
    for row in iter_heldout_rows(
        learning_seeds=learning_seeds,
        variant_seeds=variant_seeds,
        n_samples_values=n_samples_values,
        rollout_values=rollout_values,
        p_delete=args.p_delete,
        p_add=args.p_add,
    ):
        _append_row_csv(out_csv, row)
        count += 1
        print(
            f"[heldout-large] row {count}: "
            f"seed={row['learning_seed']} variant={row['variant_seed']} "
            f"samples={row['n_samples_per_state']} rollouts={row['rollouts_per_task']} "
            f"pass_all={row['pass_all']}"
        )

    summary = {
        "rows": count,
        "learning_seeds": learning_seeds,
        "variant_seeds": variant_seeds,
        "n_samples_values": n_samples_values,
        "rollout_values": rollout_values,
        "p_delete": args.p_delete,
        "p_add": args.p_add,
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

