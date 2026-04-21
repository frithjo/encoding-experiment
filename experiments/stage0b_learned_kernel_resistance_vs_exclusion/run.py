#!/usr/bin/env python3
"""Run Stage 0b-learned (learn K from traces; keep R fixed)."""

from __future__ import annotations

import argparse
import json
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from dataclasses import asdict

import numpy as np
import pandas as pd

from .analysis import (
    PREREGISTERED_THRESHOLDS,
    evaluate_hypotheses,
    hypothesis_table_with_custom,
    robustness_result,
    robustness_sweep,
    serialize_hypotheses_custom,
    simulate_rollouts,
    action_metrics_by_task,
    summarize_rollouts,
)
from .learn import LearningConfig, learn_kernel
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel
from experiments.stage0b_resistance_vs_exclusion.graph import build_graph_spec
from experiments.stage0b_resistance_vs_exclusion.analysis import condition_distance_metrics


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
    parser = argparse.ArgumentParser(description="Stage 0b-learned: learn K then test resistance vs exclusion")
    parser.add_argument(
        "--out",
        type=Path,
        default=_repo_root() / "experiments" / "stage0b_learned_kernel_resistance_vs_exclusion" / "results",
        help="Output directory",
    )
    parser.add_argument("--learning-seed", type=int, default=0, help="Seed for synthetic trace generation + learning")
    parser.add_argument("--n-samples-per-state", type=int, default=2000, help="Synthetic trace samples per state")
    parser.add_argument("--alpha-smoothing", type=float, default=1e-2, help="Laplace smoothing for log(count+alpha)")
    parser.add_argument("--rollouts", type=int, default=800, help="Rollouts per task per condition per seed")
    parser.add_argument("--robust-draws", type=int, default=20, help="Number of perturbation draws for H5")
    args = parser.parse_args()

    out: Path = args.out
    out.mkdir(parents=True, exist_ok=True)

    graph = build_graph_spec()
    config = LearningConfig(
        n_samples_per_state=args.n_samples_per_state,
        alpha_smoothing=args.alpha_smoothing,
    )
    K_hat, R_fixed, idx = learn_kernel(learning_seed=args.learning_seed, config=config)

    model = Stage0BModel(graph=graph, K=K_hat, R=R_fixed)

    # Core evaluation
    action_df = action_metrics_by_task(model)
    rollouts = simulate_rollouts(model, n_rollouts_per_task=args.rollouts)
    rollout_summary = summarize_rollouts(rollouts)
    distances = condition_distance_metrics(action_df)
    results = evaluate_hypotheses(model, action_df, rollout_summary)

    # Robustness
    robust_df = robustness_sweep(model, n_draws=args.robust_draws, n_rollouts_per_task=max(200, args.rollouts // 3))
    h5 = robustness_result(robust_df)
    all_results = results + [h5]
    hypothesis_df = hypothesis_table_with_custom(all_results)

    manifest = {
        "stage": "0b_learned",
        "name": "learned_kernel_resistance_vs_exclusion_fixed_R",
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "python": sys.version,
        "platform": platform.platform(),
        "git_rev": _git_rev(),
        "learning_seed": args.learning_seed,
        "learning_config": asdict(config),
        "rollouts_per_task_per_seed": args.rollouts,
        "robustness_draws": args.robust_draws,
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
        json.dumps(serialize_hypotheses_custom(all_results), indent=2),
        encoding="utf-8",
    )

    print("Stage 0b-learned: resistance vs exclusion")
    print(hypothesis_df.to_string(index=False))
    print("\nArtifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

