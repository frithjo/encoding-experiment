#!/usr/bin/env python3
"""Paired feature diagnostics with matched baseline headroom.

Idea:
- choose one binary support feature
- scan many heldout variants
- split into feature present vs absent
- pair variants by nearest baseline safe headroom within tolerance
- evaluate strict H3 on both sides of each pair

This does not prove causality, but is stronger than broad correlation.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import pandas as pd

from experiments.stage0e_structural_diagnostics.run import variant_feature_row
from experiments.stage0g_counterbalanced_headroom.run import candidate_baseline_row, strict_eval_row


DEFAULT_TARGET_FEATURES: tuple[str, ...] = (
    "edge__risky_assessed__refuse",
    "edge__ambiguous_assessed__clarify",
    "edge__benign_assessed__clarify",
    "edge__boundary_assessed__high_level_explanation",
)


def build_candidate_table(
    *,
    learning_seed: int,
    n_samples_per_state: int,
    variant_seeds: list[int],
    p_delete: float,
    p_add: float,
    baseline_rollouts: int,
) -> pd.DataFrame:
    baseline_rows = [
        candidate_baseline_row(
            learning_seed=learning_seed,
            n_samples_per_state=n_samples_per_state,
            variant_seed=seed,
            p_delete=p_delete,
            p_add=p_add,
            baseline_rollouts=baseline_rollouts,
        )
        for seed in variant_seeds
    ]
    feat_rows = [
        variant_feature_row(
            variant_seed=seed,
            p_delete=p_delete,
            p_add=p_add,
        )
        for seed in variant_seeds
    ]
    base_df = pd.DataFrame(baseline_rows)
    feat_df = pd.DataFrame(feat_rows)
    return base_df.merge(feat_df, on=["variant_seed", "n_admissible_edges"], how="left")


def greedy_match_pairs(
    *,
    candidates: pd.DataFrame,
    feature: str,
    tolerance: float,
    max_pairs: int | None = None,
) -> pd.DataFrame:
    present = candidates[candidates[feature] == 1].copy()
    absent = candidates[candidates[feature] == 0].copy()
    if len(present) == 0 or len(absent) == 0:
        return pd.DataFrame()

    pair_candidates: list[dict[str, object]] = []
    for _, p in present.iterrows():
        for _, a in absent.iterrows():
            delta = abs(float(p["baseline_safe_or_boundary_rate"]) - float(a["baseline_safe_or_boundary_rate"]))
            if delta <= tolerance:
                pair_candidates.append(
                    {
                        "variant_present": int(p["variant_seed"]),
                        "variant_absent": int(a["variant_seed"]),
                        "baseline_safe_present": float(p["baseline_safe_or_boundary_rate"]),
                        "baseline_safe_absent": float(a["baseline_safe_or_boundary_rate"]),
                        "baseline_safe_gap": delta,
                    }
                )

    if not pair_candidates:
        return pd.DataFrame()

    pairs_df = pd.DataFrame(pair_candidates).sort_values("baseline_safe_gap")
    used_present: set[int] = set()
    used_absent: set[int] = set()
    chosen_rows: list[dict[str, object]] = []
    for _, row in pairs_df.iterrows():
        vp = int(row["variant_present"])
        va = int(row["variant_absent"])
        if vp in used_present or va in used_absent:
            continue
        chosen_rows.append(row.to_dict())
        used_present.add(vp)
        used_absent.add(va)
        if max_pairs is not None and len(chosen_rows) >= max_pairs:
            break
    return pd.DataFrame(chosen_rows)


def evaluate_pairs(
    *,
    learning_seed: int,
    n_samples_per_state: int,
    pairs: pd.DataFrame,
    feature: str,
    p_delete: float,
    p_add: float,
    eval_rollouts: int,
) -> pd.DataFrame:
    rows: list[dict[str, object]] = []
    for pair_id, (_, pair) in enumerate(pairs.iterrows(), start=1):
        eval_present = strict_eval_row(
            learning_seed=learning_seed,
            n_samples_per_state=n_samples_per_state,
            variant_seed=int(pair["variant_present"]),
            p_delete=p_delete,
            p_add=p_add,
            eval_rollouts=eval_rollouts,
        )
        eval_absent = strict_eval_row(
            learning_seed=learning_seed,
            n_samples_per_state=n_samples_per_state,
            variant_seed=int(pair["variant_absent"]),
            p_delete=p_delete,
            p_add=p_add,
            eval_rollouts=eval_rollouts,
        )

        rows.append(
            {
                "feature": feature,
                "pair_id": pair_id,
                "learning_seed": learning_seed,
                "n_samples_per_state": n_samples_per_state,
                "variant_present": int(pair["variant_present"]),
                "variant_absent": int(pair["variant_absent"]),
                "baseline_safe_present": float(pair["baseline_safe_present"]),
                "baseline_safe_absent": float(pair["baseline_safe_absent"]),
                "baseline_safe_gap": float(pair["baseline_safe_gap"]),
                "present_H3": bool(eval_present["H3_rollout_shift_without_exclusion"]),
                "absent_H3": bool(eval_absent["H3_rollout_shift_without_exclusion"]),
                "present_pass_all": bool(eval_present["pass_all"]),
                "absent_pass_all": bool(eval_absent["pass_all"]),
                "present_safe_gain": float(
                    eval_present["H3_rollout_shift_without_exclusion__resistance_safe_or_boundary_rate"]
                    - eval_present["H3_rollout_shift_without_exclusion__baseline_safe_or_boundary_rate"]
                ),
                "absent_safe_gain": float(
                    eval_absent["H3_rollout_shift_without_exclusion__resistance_safe_or_boundary_rate"]
                    - eval_absent["H3_rollout_shift_without_exclusion__baseline_safe_or_boundary_rate"]
                ),
                "present_risky_terminal": float(
                    eval_present["H3_rollout_shift_without_exclusion__resistance_risky_terminal_rate"]
                ),
                "absent_risky_terminal": float(
                    eval_absent["H3_rollout_shift_without_exclusion__resistance_risky_terminal_rate"]
                ),
            }
        )
    return pd.DataFrame(rows)


def summarize_pair_effects(pairs_eval: pd.DataFrame) -> pd.DataFrame:
    if len(pairs_eval) == 0:
        return pd.DataFrame()
    rows: list[dict[str, object]] = []
    for feature, sub in pairs_eval.groupby("feature"):
        rows.append(
            {
                "feature": feature,
                "n_pairs": int(len(sub)),
                "mean_baseline_safe_gap": float(sub["baseline_safe_gap"].mean()),
                "present_H3_rate": float(sub["present_H3"].mean()),
                "absent_H3_rate": float(sub["absent_H3"].mean()),
                "present_minus_absent_H3_rate": float(sub["present_H3"].mean() - sub["absent_H3"].mean()),
                "mean_present_safe_gain": float(sub["present_safe_gain"].mean()),
                "mean_absent_safe_gain": float(sub["absent_safe_gain"].mean()),
                "mean_present_minus_absent_safe_gain": float(
                    (sub["present_safe_gain"] - sub["absent_safe_gain"]).mean()
                ),
                "mean_present_risky_terminal": float(sub["present_risky_terminal"].mean()),
                "mean_absent_risky_terminal": float(sub["absent_risky_terminal"].mean()),
            }
        )
    return pd.DataFrame(rows).sort_values("feature")


def main() -> int:
    parser = argparse.ArgumentParser(description="Paired feature diagnostics under matched baseline headroom")
    parser.add_argument("--out", type=Path, default=Path("experiments/stage0h_paired_feature_causality/results"))
    parser.add_argument("--learning-seeds", type=str, default="0,1")
    parser.add_argument("--n-samples-values", type=str, default="600,1200")
    parser.add_argument("--variant-seeds", type=str, default="1000,1001,1002,1003,1004,1005,1006,1007,1008,1009,1010,1011")
    parser.add_argument("--features", type=str, default=",".join(DEFAULT_TARGET_FEATURES))
    parser.add_argument("--baseline-rollouts", type=int, default=180)
    parser.add_argument("--eval-rollouts", type=int, default=360)
    parser.add_argument("--pair-tolerance", type=float, default=0.015)
    parser.add_argument("--max-pairs-per-feature", type=int, default=6)
    parser.add_argument("--p-delete", type=float, default=0.12)
    parser.add_argument("--p-add", type=float, default=0.06)
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    learning_seeds = [int(x) for x in args.learning_seeds.split(",") if x]
    n_samples_values = [int(x) for x in args.n_samples_values.split(",") if x]
    variant_seeds = [int(x) for x in args.variant_seeds.split(",") if x]
    features = [x for x in args.features.split(",") if x]

    candidate_tables: list[pd.DataFrame] = []
    pair_tables: list[pd.DataFrame] = []
    pair_eval_tables: list[pd.DataFrame] = []

    for learning_seed in learning_seeds:
        for n_samples in n_samples_values:
            candidates = build_candidate_table(
                learning_seed=learning_seed,
                n_samples_per_state=n_samples,
                variant_seeds=variant_seeds,
                p_delete=args.p_delete,
                p_add=args.p_add,
                baseline_rollouts=args.baseline_rollouts,
            )
            candidates["learning_seed"] = learning_seed
            candidates["n_samples_per_state"] = n_samples
            candidate_tables.append(candidates)

            for feature in features:
                pairs = greedy_match_pairs(
                    candidates=candidates,
                    feature=feature,
                    tolerance=args.pair_tolerance,
                    max_pairs=args.max_pairs_per_feature,
                )
                if len(pairs) == 0:
                    continue
                pairs["feature"] = feature
                pairs["learning_seed"] = learning_seed
                pairs["n_samples_per_state"] = n_samples
                pair_tables.append(pairs)

                pair_eval = evaluate_pairs(
                    learning_seed=learning_seed,
                    n_samples_per_state=n_samples,
                    pairs=pairs,
                    feature=feature,
                    p_delete=args.p_delete,
                    p_add=args.p_add,
                    eval_rollouts=args.eval_rollouts,
                )
                pair_eval_tables.append(pair_eval)

    candidates_df = pd.concat(candidate_tables, ignore_index=True) if candidate_tables else pd.DataFrame()
    pairs_df = pd.concat(pair_tables, ignore_index=True) if pair_tables else pd.DataFrame()
    pair_eval_df = pd.concat(pair_eval_tables, ignore_index=True) if pair_eval_tables else pd.DataFrame()
    summary_df = summarize_pair_effects(pair_eval_df)

    candidates_df.to_csv(out / "candidate_variants.csv", index=False)
    pairs_df.to_csv(out / "matched_pairs.csv", index=False)
    pair_eval_df.to_csv(out / "matched_pair_evaluations.csv", index=False)
    summary_df.to_csv(out / "feature_pair_summary.csv", index=False)

    summary = {
        "learning_seeds": learning_seeds,
        "n_samples_values": n_samples_values,
        "variant_count": len(variant_seeds),
        "features": features,
        "pair_tolerance": args.pair_tolerance,
        "max_pairs_per_feature": args.max_pairs_per_feature,
        "total_pairs": int(len(pair_eval_df)),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

