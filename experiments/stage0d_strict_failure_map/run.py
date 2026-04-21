#!/usr/bin/env python3
"""Stage 0d: map strict failure regions for learned and heldout variants."""

from __future__ import annotations

import argparse
import json
import platform
import subprocess
import sys
from dataclasses import asdict
from datetime import datetime, timezone
from pathlib import Path

import pandas as pd

from experiments.stage0b_learned_kernel_resistance_vs_exclusion.analysis import (
    evaluate_hypotheses as evaluate_learned_hypotheses,
    robustness_result as learned_robustness_result,
    robustness_sweep as learned_robustness_sweep,
)
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import (
    LearningConfig as LearnedKConfig,
    learn_kernel,
)
from experiments.stage0b_heldout_generalization.graph_variants import (
    VariantConfig,
    build_graph_variant,
)
from experiments.stage0b_resistance_vs_exclusion.analysis import (
    action_metrics_by_task,
    evaluate_hypotheses as evaluate_strict_hypotheses,
    simulate_rollouts,
    summarize_rollouts,
)
from experiments.stage0b_resistance_vs_exclusion.graph import build_graph_spec
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel
from experiments.stage0c_learn_q_scale_resistance_vs_exclusion.q_learn import (
    QLearnConfig,
    learn_optimal_w,
    scale_graph_charge,
)


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


def _flatten_results(prefix: str, results) -> dict[str, object]:
    row: dict[str, object] = {}
    for result in results:
        row[f"{prefix}{result.hypothesis}"] = result.passed
        for key, value in result.details.items():
            row[f"{prefix}{result.hypothesis}__{key}"] = value
    return row


def _append_row_csv(path: Path, row: dict[str, object]) -> None:
    """Append one row immediately so long sweeps preserve intermediate data."""
    df = pd.DataFrame([row])
    df.to_csv(path, mode="a", header=not path.exists(), index=False)


def iter_0b_learned_rows(
    *,
    learning_seeds: list[int],
    n_samples_values: list[int],
    rollout_values: list[int],
) -> object:
    graph = build_graph_spec()
    for learning_seed in learning_seeds:
        for n_samples in n_samples_values:
            cfg = LearnedKConfig(n_samples_per_state=n_samples, alpha_smoothing=1e-2)
            K_hat, R_fixed, _ = learn_kernel(learning_seed=learning_seed, config=cfg)
            model = Stage0BModel(graph=graph, K=K_hat, R=R_fixed)
            for n_rollouts in rollout_values:
                action_df = action_metrics_by_task(model)
                rollouts = simulate_rollouts(model, n_rollouts_per_task=n_rollouts, seeds=(0, 1, 2))
                summary = summarize_rollouts(rollouts)
                results = evaluate_learned_hypotheses(model, action_df, summary)
                robust_df = learned_robustness_sweep(
                    model,
                    n_draws=4,
                    n_rollouts_per_task=max(120, n_rollouts // 2),
                )
                h5 = learned_robustness_result(robust_df)
                row = {
                    "stage": "0b_learned",
                    "learning_seed": learning_seed,
                    "n_samples_per_state": n_samples,
                    "rollouts_per_task": n_rollouts,
                }
                row.update(_flatten_results("", results + [h5]))
                row["pass_all"] = all(result.passed for result in results + [h5])
                yield row


def iter_0c_rows(
    *,
    learning_seeds: list[int],
    n_samples_values: list[int],
    rollout_values: list[int],
) -> object:
    graph_base = build_graph_spec()
    q_grid = tuple(float(x) for x in [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0])

    for learning_seed in learning_seeds:
        for n_samples in n_samples_values:
            learned_cfg = LearnedKConfig(n_samples_per_state=n_samples, alpha_smoothing=1e-2)
            q_cfg = QLearnConfig(
                w_grid=q_grid,
                min_risky_probability_epsilon=1e-4,
                benign_help_retention_delta_max=0.12,
            )
            chosen_w, w_search_rows, _ = learn_optimal_w(
                learning_seed=learning_seed,
                learned_k_config=learned_cfg,
                qlearn_config=q_cfg,
            )
            K_hat, R_fixed, _ = learn_kernel(learning_seed=learning_seed, config=learned_cfg)
            model = Stage0BModel(
                graph=scale_graph_charge(graph_base, chosen_w),
                K=K_hat,
                R=R_fixed,
            )
            for n_rollouts in rollout_values:
                action_df = action_metrics_by_task(model)
                rollouts = simulate_rollouts(model, n_rollouts_per_task=n_rollouts, seeds=(0, 1, 2))
                summary = summarize_rollouts(rollouts)
                results = evaluate_learned_hypotheses(model, action_df, summary)
                robust_df = learned_robustness_sweep(
                    model,
                    n_draws=4,
                    n_rollouts_per_task=max(120, n_rollouts // 2),
                )
                h5 = learned_robustness_result(robust_df)
                row = {
                    "stage": "0c",
                    "learning_seed": learning_seed,
                    "n_samples_per_state": n_samples,
                    "rollouts_per_task": n_rollouts,
                    "chosen_w": chosen_w,
                }
                row.update(_flatten_results("", results + [h5]))
                row["pass_all"] = all(result.passed for result in results + [h5])
                yield row


def iter_heldout_rows(
    *,
    learning_seeds: list[int],
    variant_seeds: list[int],
    n_samples_values: list[int],
    rollout_values: list[int],
    p_delete: float,
    p_add: float,
) -> object:
    variant_cfg = VariantConfig(
        p_delete_safe_edges=p_delete,
        p_add_safe_edges=p_add,
        max_resample_attempts=60,
    )
    for learning_seed in learning_seeds:
        for n_samples in n_samples_values:
            learned_cfg = LearnedKConfig(n_samples_per_state=n_samples, alpha_smoothing=1e-2)
            K_hat, R_fixed, _ = learn_kernel(learning_seed=learning_seed, config=learned_cfg)
            for variant_seed in variant_seeds:
                graph_v = build_graph_variant(seed=variant_seed, config=variant_cfg)
                model = Stage0BModel(graph=graph_v, K=K_hat, R=R_fixed)
                for n_rollouts in rollout_values:
                    action_df = action_metrics_by_task(model)
                    rollouts = simulate_rollouts(model, n_rollouts_per_task=n_rollouts, seeds=(0,), max_steps=8)
                    summary = summarize_rollouts(rollouts)
                    results = evaluate_strict_hypotheses(model, action_df, summary)
                    row = {
                        "stage": "heldout",
                        "learning_seed": learning_seed,
                        "variant_seed": variant_seed,
                        "n_samples_per_state": n_samples,
                        "rollouts_per_task": n_rollouts,
                        "n_admissible_edges": int(graph_v.admissible_mask.sum()),
                    }
                    row.update(_flatten_results("", results))
                    row["pass_all"] = all(result.passed for result in results)
                    yield row


def summarize_failure_regions(
    df_0b: pd.DataFrame,
    df_0c: pd.DataFrame,
    df_heldout: pd.DataFrame,
) -> dict[str, object]:
    return {
        "0b_learned": {
            "rows": int(len(df_0b)),
            "pass_all_rate": float(df_0b["pass_all"].mean()) if len(df_0b) else None,
            "H3_pass_rate": float(df_0b["H3_rollout_shift_without_exclusion"].mean()) if len(df_0b) else None,
            "H5_pass_rate": float(df_0b["H5_perturbation_robustness"].mean()) if len(df_0b) else None,
        },
        "0c": {
            "rows": int(len(df_0c)),
            "pass_all_rate": float(df_0c["pass_all"].mean()) if len(df_0c) else None,
            "H3_pass_rate": float(df_0c["H3_rollout_shift_without_exclusion"].mean()) if len(df_0c) else None,
            "H5_pass_rate": float(df_0c["H5_perturbation_robustness"].mean()) if len(df_0c) else None,
        },
        "heldout": {
            "rows": int(len(df_heldout)),
            "pass_all_rate": float(df_heldout["pass_all"].mean()) if len(df_heldout) else None,
            "H3_pass_rate": float(df_heldout["H3_rollout_shift_without_exclusion"].mean()) if len(df_heldout) else None,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage 0d strict failure map")
    parser.add_argument(
        "--out",
        type=Path,
        default=_repo_root() / "experiments" / "stage0d_strict_failure_map" / "results",
    )
    parser.add_argument("--learning-seeds", type=str, default="0,1,2")
    parser.add_argument("--n-samples-values", type=str, default="600,1200,2400")
    parser.add_argument("--rollout-values", type=str, default="120,240,480")
    parser.add_argument("--variant-seeds", type=str, default="1000,1001,1002,1003,1004,1005")
    parser.add_argument("--p-delete", type=float, default=0.12)
    parser.add_argument("--p-add", type=float, default=0.06)
    args = parser.parse_args()

    out: Path = args.out
    out.mkdir(parents=True, exist_ok=True)

    learning_seeds = [int(x) for x in args.learning_seeds.split(",") if x]
    n_samples_values = [int(x) for x in args.n_samples_values.split(",") if x]
    rollout_values = [int(x) for x in args.rollout_values.split(",") if x]
    variant_seeds = [int(x) for x in args.variant_seeds.split(",") if x]
    path_0b = out / "strict_0b_learned_grid.csv"
    path_0c = out / "strict_0c_grid.csv"
    path_heldout = out / "strict_heldout_grid.csv"
    for path in (path_0b, path_0c, path_heldout):
        if path.exists():
            path.unlink()

    count_0b = 0
    for row in iter_0b_learned_rows(
        learning_seeds=learning_seeds,
        n_samples_values=n_samples_values,
        rollout_values=rollout_values,
    ):
        _append_row_csv(path_0b, row)
        count_0b += 1
        print(f"[0b_learned] wrote row {count_0b}: seed={row['learning_seed']} samples={row['n_samples_per_state']} rollouts={row['rollouts_per_task']} pass_all={row['pass_all']}")

    count_0c = 0
    for row in iter_0c_rows(
        learning_seeds=learning_seeds,
        n_samples_values=n_samples_values,
        rollout_values=rollout_values,
    ):
        _append_row_csv(path_0c, row)
        count_0c += 1
        print(f"[0c] wrote row {count_0c}: seed={row['learning_seed']} samples={row['n_samples_per_state']} rollouts={row['rollouts_per_task']} w={row['chosen_w']} pass_all={row['pass_all']}")

    count_heldout = 0
    for row in iter_heldout_rows(
        learning_seeds=learning_seeds,
        variant_seeds=variant_seeds,
        n_samples_values=n_samples_values,
        rollout_values=rollout_values,
        p_delete=args.p_delete,
        p_add=args.p_add,
    ):
        _append_row_csv(path_heldout, row)
        count_heldout += 1
        print(f"[heldout] wrote row {count_heldout}: seed={row['learning_seed']} variant={row['variant_seed']} samples={row['n_samples_per_state']} rollouts={row['rollouts_per_task']} pass_all={row['pass_all']}")

    df_0b = pd.read_csv(path_0b) if path_0b.exists() else pd.DataFrame()
    df_0c = pd.read_csv(path_0c) if path_0c.exists() else pd.DataFrame()
    df_heldout = pd.read_csv(path_heldout) if path_heldout.exists() else pd.DataFrame()

    summary = {
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "python": sys.version,
        "platform": platform.platform(),
        "git_rev": _git_rev(),
        "config": {
            "learning_seeds": learning_seeds,
            "n_samples_values": n_samples_values,
            "rollout_values": rollout_values,
            "variant_seeds": variant_seeds,
            "p_delete": args.p_delete,
            "p_add": args.p_add,
        },
        "summary": summarize_failure_regions(df_0b, df_0c, df_heldout),
    }
    (out / "strict_summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")

    print(json.dumps(summary["summary"], indent=2))
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

