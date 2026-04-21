#!/usr/bin/env python3
"""Run Stage 0b experiment and write reproducible artifacts."""

from __future__ import annotations

import argparse
import json
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import pandas as pd

from .analysis import (
    PREREGISTERED_THRESHOLDS,
    action_metrics_by_task,
    condition_distance_metrics,
    evaluate_hypotheses,
    hypothesis_table,
    robustness_result,
    robustness_sweep,
    serialize_hypotheses,
    simulate_rollouts,
    summarize_rollouts,
)
from .kernel import Stage0BModel


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _git_rev() -> str | None:
    try:
        r = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=_repo_root(),
            capture_output=True,
            text=True,
            check=False,
        )
        return r.stdout.strip() if r.returncode == 0 else None
    except OSError:
        return None


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage 0b resistance vs exclusion test")
    parser.add_argument(
        "--out",
        type=Path,
        default=_repo_root() / "experiments" / "stage0b_resistance_vs_exclusion" / "results",
        help="Directory for CSV/JSON outputs",
    )
    parser.add_argument(
        "--rollouts",
        type=int,
        default=1500,
        help="Rollouts per task per seed",
    )
    parser.add_argument(
        "--robust-draws",
        type=int,
        default=40,
        help="Number of perturbation draws for H5",
    )
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    model = Stage0BModel.default()
    action_df = action_metrics_by_task(model)
    rollouts = simulate_rollouts(model, n_rollouts_per_task=args.rollouts)
    rollout_summary = summarize_rollouts(rollouts)
    distances = condition_distance_metrics(action_df)
    results = evaluate_hypotheses(model, action_df, rollout_summary)
    robust_df = robustness_sweep(model, n_draws=args.robust_draws)
    h5 = robustness_result(robust_df)
    all_results = results + [h5]
    hypothesis_df = hypothesis_table(all_results)

    manifest = {
        "stage": "0b",
        "name": "resistance_vs_exclusion_fixed_support",
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "python": sys.version,
        "platform": platform.platform(),
        "git_rev": _git_rev(),
        "rollouts_per_task_per_seed": args.rollouts,
        "robustness_draws": args.robust_draws,
        "states": len(model.states),
        "tasks": [task.name for task in model.graph.tasks],
        "thresholds": PREREGISTERED_THRESHOLDS,
    }

    (out / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    action_df.to_csv(out / "action_metrics_by_task.csv", index=False)
    rollouts.to_csv(out / "rollouts.csv", index=False)
    rollout_summary.to_csv(out / "rollout_summary.csv", index=False)
    distances.to_csv(out / "condition_distances.csv", index=False)
    robust_df.to_csv(out / "robustness_sweep.csv", index=False)
    hypothesis_df.to_csv(out / "hypothesis_results.csv", index=False)
    (out / "hypothesis_results.json").write_text(
        json.dumps(serialize_hypotheses(all_results), indent=2),
        encoding="utf-8",
    )

    print("Stage 0b: resistance vs exclusion")
    print(hypothesis_df.to_string(index=False))
    print("\nArtifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
