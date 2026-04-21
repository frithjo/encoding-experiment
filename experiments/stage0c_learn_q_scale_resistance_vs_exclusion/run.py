#!/usr/bin/env python3
"""Stage 0c: learn resistance intensity (q-scale) then test resistance vs exclusion."""

from __future__ import annotations

import argparse
import json
import platform
import subprocess
import sys
from dataclasses import asdict
from datetime import datetime, timezone
from pathlib import Path

import numpy as np

from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import LearningConfig as LearnedKConfig
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import learn_kernel
from experiments.stage0b_resistance_vs_exclusion.graph import build_graph_spec
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel
from experiments.stage0b_resistance_vs_exclusion.analysis import (
    action_metrics_by_task,
    condition_distance_metrics,
    hypothesis_table,
    simulate_rollouts,
    summarize_rollouts,
)
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.analysis import (
    PREREGISTERED_THRESHOLDS,
    evaluate_hypotheses,
    robustness_result,
    robustness_sweep,
)

from .q_learn import QLearnConfig, learn_optimal_w, scale_graph_charge


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
    parser = argparse.ArgumentParser(description="Stage 0c: learn q-scale for resistance")
    parser.add_argument("--out", type=Path, default=_repo_root() / "experiments" / "stage0c_learn_q_scale_resistance_vs_exclusion" / "results")
    parser.add_argument("--learning-seed", type=int, default=0)
    parser.add_argument("--n-samples-per-state", type=int, default=1200)
    parser.add_argument("--alpha-smoothing", type=float, default=1e-2)
    parser.add_argument("--w-max", type=float, default=3.0)
    parser.add_argument("--w-step", type=float, default=0.05)
    parser.add_argument("--rollouts", type=int, default=600)
    parser.add_argument("--robust-draws", type=int, default=12)
    args = parser.parse_args()

    out: Path = args.out
    out.mkdir(parents=True, exist_ok=True)

    graph_base = build_graph_spec()
    learned_k_cfg = LearnedKConfig(n_samples_per_state=args.n_samples_per_state, alpha_smoothing=args.alpha_smoothing)
    q_grid = tuple(float(x) for x in np.arange(0.0, args.w_max + 1e-9, args.w_step))
    qlearn_cfg = QLearnConfig(w_grid=q_grid)

    w_chosen, w_search_rows, _meta = learn_optimal_w(
        learning_seed=args.learning_seed,
        learned_k_config=learned_k_cfg,
        qlearn_config=qlearn_cfg,
    )

    # Re-learn K_hat so run is self-contained and artifacts match selection model.
    K_hat, R_fixed, _idx = learn_kernel(learning_seed=args.learning_seed, config=learned_k_cfg)
    graph_w = scale_graph_charge(graph_base, w=w_chosen)
    model = Stage0BModel(graph=graph_w, K=K_hat, R=R_fixed)

    action_df = action_metrics_by_task(model)
    rollouts = simulate_rollouts(model, n_rollouts_per_task=args.rollouts, seeds=(0, 1, 2))
    rollout_summary = summarize_rollouts(rollouts)
    distances = condition_distance_metrics(action_df)
    results = evaluate_hypotheses(model, action_df, rollout_summary)
    robust_df = robustness_sweep(model, n_draws=args.robust_draws, n_rollouts_per_task=max(200, args.rollouts // 3))
    h5 = robustness_result(robust_df)
    all_results = results + [h5]
    hypothesis_df = hypothesis_table(all_results)

    manifest = {
        "stage": "0c",
        "name": "learn_q_scale_resistance_vs_exclusion_fixed_support",
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "python": sys.version,
        "platform": platform.platform(),
        "git_rev": _git_rev(),
        "learning_seed": args.learning_seed,
        "learning_config": asdict(learned_k_cfg),
        "qlearn_config": {"w_grid_step": args.w_step, "w_max": args.w_max, "w_grid_len": len(q_grid)},
        "chosen_w": w_chosen,
        "rollouts_per_task_per_seed": args.rollouts,
        "robustness_draws": args.robust_draws,
        "thresholds": PREREGISTERED_THRESHOLDS,
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    (out / "w_search.json").write_text(json.dumps(w_search_rows, indent=2), encoding="utf-8")
    np.save(out / "chosen_w.npy", np.array([w_chosen], dtype=np.float64))

    action_df.to_csv(out / "action_metrics_by_task.csv", index=False)
    rollouts.to_csv(out / "rollouts.csv", index=False)
    rollout_summary.to_csv(out / "rollout_summary.csv", index=False)
    distances.to_csv(out / "condition_distances.csv", index=False)
    robust_df.to_csv(out / "robustness_sweep.csv", index=False)
    hypothesis_df.to_csv(out / "hypothesis_results.csv", index=False)

    print("Stage 0c: q-scale learned")
    print("chosen_w:", w_chosen)
    print(hypothesis_df.to_string(index=False))
    print("\nArtifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

