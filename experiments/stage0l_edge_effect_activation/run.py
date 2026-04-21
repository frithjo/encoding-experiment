#!/usr/bin/env python3
"""Map when single-edge effects activate versus collapse into tied regimes.

This stage reads completed Stage 0k constructed-twin results and asks:

- when do present/absent twins disagree on strict H3?
- when are cells saturated because both twins always pass or always fail?
- where do mixed but tied cells appear, meaning outcomes vary by template
  but the edge toggle itself does not change H3?

It does not modify the Stage 0b hypotheses or thresholds.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import pandas as pd


def load_pair_table(path: Path) -> pd.DataFrame:
    df = pd.read_csv(path)

    df["present_H3"] = df["present_H3"].astype(bool)
    df["absent_H3"] = df["absent_H3"].astype(bool)

    df["present_only_h3"] = df["present_H3"] & ~df["absent_H3"]
    df["absent_only_h3"] = ~df["present_H3"] & df["absent_H3"]
    df["both_pass_h3"] = df["present_H3"] & df["absent_H3"]
    df["both_fail_h3"] = ~df["present_H3"] & ~df["absent_H3"]
    df["disagreement_h3"] = df["present_H3"] != df["absent_H3"]

    df["baseline_safe_midpoint"] = 0.5 * (df["baseline_safe_present"] + df["baseline_safe_absent"])
    df["safe_gain_midpoint"] = 0.5 * (df["safe_gain_present"] + df["safe_gain_absent"])
    df["risky_drop_midpoint"] = 0.5 * (df["risky_drop_present"] + df["risky_drop_absent"])

    df["abs_baseline_safe_gap"] = (df["baseline_safe_present"] - df["baseline_safe_absent"]).abs()
    df["abs_safe_gain_gap"] = (df["safe_gain_present"] - df["safe_gain_absent"]).abs()
    df["abs_risky_drop_gap"] = (df["risky_drop_present"] - df["risky_drop_absent"]).abs()

    outcome = pd.Series("both_fail", index=df.index, dtype="object")
    outcome[df["both_pass_h3"]] = "both_pass"
    outcome[df["present_only_h3"]] = "present_only"
    outcome[df["absent_only_h3"]] = "absent_only"
    df["pair_h3_outcome"] = outcome
    return df


def summarize_cells(pairs_df: pd.DataFrame) -> pd.DataFrame:
    rows: list[dict[str, object]] = []
    group_cols = ["target_feature", "learning_seed", "n_samples_per_state"]

    for keys, sub in pairs_df.groupby(group_cols):
        target_feature, learning_seed, n_samples_per_state = keys
        n_pairs = int(len(sub))

        both_pass_rate = float(sub["both_pass_h3"].mean())
        both_fail_rate = float(sub["both_fail_h3"].mean())
        present_only_rate = float(sub["present_only_h3"].mean())
        absent_only_rate = float(sub["absent_only_h3"].mean())
        disagreement_rate = float(sub["disagreement_h3"].mean())

        if both_pass_rate == 1.0:
            cell_state = "all_pass_tied"
        elif both_fail_rate == 1.0:
            cell_state = "all_fail_tied"
        elif disagreement_rate > 0.0:
            cell_state = "edge_sensitive"
        else:
            cell_state = "mixed_but_tied"

        disagree_sub = sub[sub["disagreement_h3"]]
        tied_sub = sub[~sub["disagreement_h3"]]

        rows.append(
            {
                "target_feature": target_feature,
                "learning_seed": int(learning_seed),
                "n_samples_per_state": int(n_samples_per_state),
                "n_pairs": n_pairs,
                "cell_state": cell_state,
                "present_H3_rate": float(sub["present_H3"].mean()),
                "absent_H3_rate": float(sub["absent_H3"].mean()),
                "present_minus_absent_H3_rate": float(sub["present_H3"].mean() - sub["absent_H3"].mean()),
                "disagreement_h3_rate": disagreement_rate,
                "present_only_h3_rate": present_only_rate,
                "absent_only_h3_rate": absent_only_rate,
                "both_pass_h3_rate": both_pass_rate,
                "both_fail_h3_rate": both_fail_rate,
                "mean_baseline_safe_midpoint": float(sub["baseline_safe_midpoint"].mean()),
                "mean_safe_gain_midpoint": float(sub["safe_gain_midpoint"].mean()),
                "mean_risky_drop_midpoint": float(sub["risky_drop_midpoint"].mean()),
                "mean_abs_baseline_safe_gap": float(sub["abs_baseline_safe_gap"].mean()),
                "mean_abs_safe_gain_gap": float(sub["abs_safe_gain_gap"].mean()),
                "mean_abs_risky_drop_gap": float(sub["abs_risky_drop_gap"].mean()),
                "disagreement_mean_baseline_safe_midpoint": (
                    float(disagree_sub["baseline_safe_midpoint"].mean()) if len(disagree_sub) else None
                ),
                "tied_mean_baseline_safe_midpoint": (
                    float(tied_sub["baseline_safe_midpoint"].mean()) if len(tied_sub) else None
                ),
                "disagreement_mean_abs_safe_gain_gap": (
                    float(disagree_sub["abs_safe_gain_gap"].mean()) if len(disagree_sub) else None
                ),
                "tied_mean_abs_safe_gain_gap": (
                    float(tied_sub["abs_safe_gain_gap"].mean()) if len(tied_sub) else None
                ),
            }
        )
    return pd.DataFrame(rows).sort_values(group_cols).reset_index(drop=True)


def summarize_features(pairs_df: pd.DataFrame, cells_df: pd.DataFrame) -> pd.DataFrame:
    rows: list[dict[str, object]] = []
    for target_feature, sub in pairs_df.groupby("target_feature"):
        cell_sub = cells_df[cells_df["target_feature"] == target_feature]
        disagree_sub = sub[sub["disagreement_h3"]]
        tied_sub = sub[~sub["disagreement_h3"]]

        rows.append(
            {
                "target_feature": target_feature,
                "n_pairs": int(len(sub)),
                "n_cells": int(len(cell_sub)),
                "disagreement_h3_rate": float(sub["disagreement_h3"].mean()),
                "present_only_h3_rate": float(sub["present_only_h3"].mean()),
                "absent_only_h3_rate": float(sub["absent_only_h3"].mean()),
                "both_pass_h3_rate": float(sub["both_pass_h3"].mean()),
                "both_fail_h3_rate": float(sub["both_fail_h3"].mean()),
                "present_H3_rate": float(sub["present_H3"].mean()),
                "absent_H3_rate": float(sub["absent_H3"].mean()),
                "present_minus_absent_H3_rate": float(sub["present_H3"].mean() - sub["absent_H3"].mean()),
                "edge_sensitive_cell_rate": float((cell_sub["cell_state"] == "edge_sensitive").mean()),
                "all_pass_tied_cell_rate": float((cell_sub["cell_state"] == "all_pass_tied").mean()),
                "all_fail_tied_cell_rate": float((cell_sub["cell_state"] == "all_fail_tied").mean()),
                "mixed_but_tied_cell_rate": float((cell_sub["cell_state"] == "mixed_but_tied").mean()),
                "disagreement_mean_baseline_safe_midpoint": (
                    float(disagree_sub["baseline_safe_midpoint"].mean()) if len(disagree_sub) else None
                ),
                "tied_mean_baseline_safe_midpoint": (
                    float(tied_sub["baseline_safe_midpoint"].mean()) if len(tied_sub) else None
                ),
                "disagreement_mean_abs_safe_gain_gap": (
                    float(disagree_sub["abs_safe_gain_gap"].mean()) if len(disagree_sub) else None
                ),
                "tied_mean_abs_safe_gain_gap": (
                    float(tied_sub["abs_safe_gain_gap"].mean()) if len(tied_sub) else None
                ),
            }
        )
    return pd.DataFrame(rows).sort_values("target_feature").reset_index(drop=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage 0l edge-effect activation diagnostics")
    parser.add_argument(
        "--in-csv",
        type=Path,
        default=Path("experiments/stage0k_focused_edge_replication/results/constructed_pair_evaluations.csv"),
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("experiments/stage0l_edge_effect_activation/results"),
    )
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    pairs_df = load_pair_table(args.in_csv)
    cells_df = summarize_cells(pairs_df)
    features_df = summarize_features(pairs_df, cells_df)

    pairs_df.to_csv(out / "pair_activation_table.csv", index=False)
    cells_df.to_csv(out / "cell_activation_summary.csv", index=False)
    features_df.to_csv(out / "feature_activation_summary.csv", index=False)

    summary = {
        "input_csv": str(args.in_csv),
        "n_pairs": int(len(pairs_df)),
        "n_cells": int(len(cells_df)),
        "n_features": int(len(features_df)),
        "features": features_df["target_feature"].tolist(),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

