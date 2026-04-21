#!/usr/bin/env python3
"""Counterbalanced heldout families matched on baseline safe headroom.

Purpose:
- reduce geometry confound by matching variants on baseline safe-or-boundary rate
- then test whether strict H3 variability remains inside that matched band
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import pandas as pd

from experiments.stage0b_heldout_generalization.graph_variants import (
    VariantConfig,
    build_graph_variant,
)
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import (
    LearningConfig as LearnedKConfig,
    learn_kernel,
)
from experiments.stage0b_resistance_vs_exclusion.analysis import (
    action_metrics_by_task,
    evaluate_hypotheses,
    simulate_rollouts,
    summarize_rollouts,
)
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel


def candidate_baseline_row(
    *,
    learning_seed: int,
    n_samples_per_state: int,
    variant_seed: int,
    p_delete: float,
    p_add: float,
    baseline_rollouts: int,
) -> dict[str, object]:
    learned_cfg = LearnedKConfig(n_samples_per_state=n_samples_per_state, alpha_smoothing=1e-2)
    K_hat, R_fixed, _ = learn_kernel(learning_seed=learning_seed, config=learned_cfg)
    graph_v = build_graph_variant(
        seed=variant_seed,
        config=VariantConfig(
            p_delete_safe_edges=p_delete,
            p_add_safe_edges=p_add,
            max_resample_attempts=60,
        ),
    )
    model = Stage0BModel(graph=graph_v, K=K_hat, R=R_fixed)
    rollouts = simulate_rollouts(model, n_rollouts_per_task=baseline_rollouts, seeds=(0,), max_steps=8)
    summary = summarize_rollouts(rollouts)
    risky_base = summary[(summary["context_class"] == "risky") & (summary["condition"] == "baseline")]
    baseline_safe = float(risky_base["safe_or_boundary_rate"].mean())
    baseline_risky = float(risky_base["risky_rate"].mean())
    return {
        "learning_seed": learning_seed,
        "n_samples_per_state": n_samples_per_state,
        "variant_seed": variant_seed,
        "n_admissible_edges": int(graph_v.admissible_mask.sum()),
        "baseline_safe_or_boundary_rate": baseline_safe,
        "baseline_risky_terminal_rate": baseline_risky,
    }


def select_counterbalanced_band(
    candidates: pd.DataFrame,
    *,
    min_variants: int,
    initial_tol: float,
    max_tol: float,
) -> tuple[pd.DataFrame, dict[str, object]]:
    target = float(candidates["baseline_safe_or_boundary_rate"].median())
    tol = initial_tol
    selected = candidates[
        (candidates["baseline_safe_or_boundary_rate"] >= target - tol)
        & (candidates["baseline_safe_or_boundary_rate"] <= target + tol)
    ].copy()
    while len(selected) < min_variants and tol < max_tol:
        tol += 0.01
        selected = candidates[
            (candidates["baseline_safe_or_boundary_rate"] >= target - tol)
            & (candidates["baseline_safe_or_boundary_rate"] <= target + tol)
        ].copy()
    selected = selected.sort_values(
        by="baseline_safe_or_boundary_rate",
        key=lambda s: (s - target).abs(),
    )
    meta = {
        "target_baseline_safe_or_boundary_rate": target,
        "final_tolerance": tol,
        "selected_count": int(len(selected)),
    }
    return selected, meta


def strict_eval_row(
    *,
    learning_seed: int,
    n_samples_per_state: int,
    variant_seed: int,
    p_delete: float,
    p_add: float,
    eval_rollouts: int,
) -> dict[str, object]:
    learned_cfg = LearnedKConfig(n_samples_per_state=n_samples_per_state, alpha_smoothing=1e-2)
    K_hat, R_fixed, _ = learn_kernel(learning_seed=learning_seed, config=learned_cfg)
    graph_v = build_graph_variant(
        seed=variant_seed,
        config=VariantConfig(
            p_delete_safe_edges=p_delete,
            p_add_safe_edges=p_add,
            max_resample_attempts=60,
        ),
    )
    model = Stage0BModel(graph=graph_v, K=K_hat, R=R_fixed)
    action_df = action_metrics_by_task(model)
    rollouts = simulate_rollouts(model, n_rollouts_per_task=eval_rollouts, seeds=(0,), max_steps=8)
    summary = summarize_rollouts(rollouts)
    results = evaluate_hypotheses(model, action_df, summary)

    row: dict[str, object] = {
        "learning_seed": learning_seed,
        "n_samples_per_state": n_samples_per_state,
        "variant_seed": variant_seed,
        "n_admissible_edges": int(graph_v.admissible_mask.sum()),
    }
    for result in results:
        row[result.hypothesis] = result.passed
        for k, v in result.details.items():
            row[f"{result.hypothesis}__{k}"] = v
    row["pass_all"] = all(r.passed for r in results)
    return row


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage 0g counterbalanced headroom families")
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("experiments/stage0g_counterbalanced_headroom/results"),
    )
    parser.add_argument("--learning-seed", type=int, default=0)
    parser.add_argument("--n-samples-per-state", type=int, default=600)
    parser.add_argument("--variant-seeds", type=str, default="1000,1001,1002,1003,1004,1005,1006,1007,1008,1009,1010,1011")
    parser.add_argument("--p-delete", type=float, default=0.12)
    parser.add_argument("--p-add", type=float, default=0.06)
    parser.add_argument("--baseline-rollouts", type=int, default=180)
    parser.add_argument("--eval-rollouts", type=int, default=360)
    parser.add_argument("--min-variants", type=int, default=6)
    parser.add_argument("--initial-tol", type=float, default=0.02)
    parser.add_argument("--max-tol", type=float, default=0.08)
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    variant_seeds = [int(x) for x in args.variant_seeds.split(",") if x]

    candidate_rows = [
        candidate_baseline_row(
            learning_seed=args.learning_seed,
            n_samples_per_state=args.n_samples_per_state,
            variant_seed=seed,
            p_delete=args.p_delete,
            p_add=args.p_add,
            baseline_rollouts=args.baseline_rollouts,
        )
        for seed in variant_seeds
    ]
    candidates = pd.DataFrame(candidate_rows).sort_values("variant_seed")
    candidates.to_csv(out / "candidate_headroom_scan.csv", index=False)

    selected, meta = select_counterbalanced_band(
        candidates,
        min_variants=args.min_variants,
        initial_tol=args.initial_tol,
        max_tol=args.max_tol,
    )
    selected.to_csv(out / "selected_counterbalanced_variants.csv", index=False)

    eval_rows = [
        strict_eval_row(
            learning_seed=args.learning_seed,
            n_samples_per_state=args.n_samples_per_state,
            variant_seed=int(seed),
            p_delete=args.p_delete,
            p_add=args.p_add,
            eval_rollouts=args.eval_rollouts,
        )
        for seed in selected["variant_seed"].tolist()
    ]
    eval_df = pd.DataFrame(eval_rows).sort_values("variant_seed")
    eval_df.to_csv(out / "counterbalanced_strict_eval.csv", index=False)

    summary = {
        "learning_seed": args.learning_seed,
        "n_samples_per_state": args.n_samples_per_state,
        "candidate_count": int(len(candidates)),
        "selected_count": int(len(selected)),
        "meta": meta,
        "counterbalanced_H3_pass_rate": float(eval_df["H3_rollout_shift_without_exclusion"].mean()) if len(eval_df) else None,
        "counterbalanced_pass_all_rate": float(eval_df["pass_all"].mean()) if len(eval_df) else None,
        "candidate_baseline_safe_mean": float(candidates["baseline_safe_or_boundary_rate"].mean()) if len(candidates) else None,
        "selected_baseline_safe_mean": float(selected["baseline_safe_or_boundary_rate"].mean()) if len(selected) else None,
        "selected_baseline_safe_std": float(selected["baseline_safe_or_boundary_rate"].std()) if len(selected) > 1 else 0.0,
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")

    print(json.dumps(summary, indent=2))
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

