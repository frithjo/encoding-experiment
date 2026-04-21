#!/usr/bin/env python3
"""Structural diagnostics: connect heldout H3 success/failure to graph support features."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import pandas as pd

from experiments.stage0b_heldout_generalization.graph_variants import VariantConfig, build_graph_variant


FEATURE_EDGES: tuple[tuple[str, str], ...] = (
    ("risky_assessed", "refuse"),
    ("risky_assessed", "safe_alternative"),
    ("risky_assessed", "high_level_explanation"),
    ("risky_assessed", "risky_detail"),
    ("boundary_assessed", "boundary_reply"),
    ("boundary_assessed", "refuse"),
    ("boundary_assessed", "safe_alternative"),
    ("boundary_assessed", "high_level_explanation"),
    ("boundary_assessed", "risky_detail"),
    ("ambiguous_assessed", "clarify"),
    ("ambiguous_assessed", "high_level_explanation"),
    ("ambiguous_assessed", "direct_help"),
    ("ambiguous_assessed", "safe_alternative"),
    ("benign_assessed", "direct_help"),
    ("benign_assessed", "clarify"),
    ("benign_assessed", "high_level_explanation"),
)


def _edge_present(graph, src: str, dst: str) -> int:
    return int(graph.admissible_mask[graph.idx[src], graph.idx[dst]])


def _outdegree(graph, src: str) -> int:
    return int(graph.admissible_mask[graph.idx[src]].sum())


def _point_biserial(feature: pd.Series, label: pd.Series) -> float:
    x = feature.astype(float).to_numpy()
    y = label.astype(float).to_numpy()
    if np.std(x) == 0.0 or np.std(y) == 0.0:
        return 0.0
    return float(np.corrcoef(x, y)[0, 1])


def variant_feature_row(*, variant_seed: int, p_delete: float, p_add: float) -> dict[str, object]:
    graph = build_graph_variant(
        seed=variant_seed,
        config=VariantConfig(
            p_delete_safe_edges=p_delete,
            p_add_safe_edges=p_add,
            max_resample_attempts=60,
        ),
    )
    row: dict[str, object] = {
        "variant_seed": variant_seed,
        "n_admissible_edges": int(graph.admissible_mask.sum()),
        "outdeg_benign": _outdegree(graph, "benign_assessed"),
        "outdeg_ambiguous": _outdegree(graph, "ambiguous_assessed"),
        "outdeg_risky": _outdegree(graph, "risky_assessed"),
        "outdeg_boundary": _outdegree(graph, "boundary_assessed"),
        "safe_count_risky": sum(
            _edge_present(graph, "risky_assessed", dst)
            for dst in ("refuse", "safe_alternative", "high_level_explanation")
        ),
        "safe_count_boundary": sum(
            _edge_present(graph, "boundary_assessed", dst)
            for dst in ("boundary_reply", "refuse", "safe_alternative", "high_level_explanation")
        ),
        "total_safe_count_assessment": sum(
            _edge_present(graph, src, dst)
            for src, dst in FEATURE_EDGES
            if dst != "risky_detail"
        ),
    }
    for src, dst in FEATURE_EDGES:
        row[f"edge__{src}__{dst}"] = _edge_present(graph, src, dst)
    return row


def main() -> int:
    parser = argparse.ArgumentParser(description="Structural diagnostics for heldout variants")
    parser.add_argument(
        "--heldout-grid",
        type=Path,
        default=Path("experiments/stage0d_strict_failure_map/results/strict_heldout_grid.csv"),
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("experiments/stage0e_structural_diagnostics/results"),
    )
    parser.add_argument("--p-delete", type=float, default=0.12)
    parser.add_argument("--p-add", type=float, default=0.06)
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    heldout = pd.read_csv(args.heldout_grid)
    variant_seeds = sorted(heldout["variant_seed"].unique().tolist())

    feature_rows = [
        variant_feature_row(variant_seed=int(seed), p_delete=args.p_delete, p_add=args.p_add)
        for seed in variant_seeds
    ]
    feature_df = pd.DataFrame(feature_rows)

    merged = heldout.merge(feature_df, on=["variant_seed", "n_admissible_edges"], how="left")
    merged.to_csv(out / "heldout_structural_features_joined.csv", index=False)

    # Aggregate by variant and by H3 outcome
    variant_summary = (
        merged.groupby("variant_seed", as_index=False)
        .agg(
            H3_pass_rate=("H3_rollout_shift_without_exclusion", "mean"),
            pass_all_rate=("pass_all", "mean"),
            n_admissible_edges=("n_admissible_edges", "first"),
            outdeg_benign=("outdeg_benign", "first"),
            outdeg_ambiguous=("outdeg_ambiguous", "first"),
            outdeg_risky=("outdeg_risky", "first"),
            outdeg_boundary=("outdeg_boundary", "first"),
            safe_count_risky=("safe_count_risky", "first"),
            safe_count_boundary=("safe_count_boundary", "first"),
            total_safe_count_assessment=("total_safe_count_assessment", "first"),
        )
        .sort_values("variant_seed")
    )
    variant_summary.to_csv(out / "variant_summary.csv", index=False)

    h3_group = (
        merged.groupby("H3_rollout_shift_without_exclusion", as_index=False)
        .agg(
            n_rows=("variant_seed", "count"),
            mean_n_admissible_edges=("n_admissible_edges", "mean"),
            mean_outdeg_risky=("outdeg_risky", "mean"),
            mean_outdeg_boundary=("outdeg_boundary", "mean"),
            mean_safe_count_risky=("safe_count_risky", "mean"),
            mean_safe_count_boundary=("safe_count_boundary", "mean"),
            mean_total_safe_count_assessment=("total_safe_count_assessment", "mean"),
        )
        .sort_values("H3_rollout_shift_without_exclusion")
    )
    h3_group.to_csv(out / "h3_group_summary.csv", index=False)

    edge_cols = [c for c in merged.columns if c.startswith("edge__")]
    edge_group = (
        merged.groupby("H3_rollout_shift_without_exclusion", as_index=False)[edge_cols]
        .mean()
        .sort_values("H3_rollout_shift_without_exclusion")
    )
    edge_group.to_csv(out / "h3_edge_presence_summary.csv", index=False)

    feature_cols = [
        "n_admissible_edges",
        "outdeg_benign",
        "outdeg_ambiguous",
        "outdeg_risky",
        "outdeg_boundary",
        "safe_count_risky",
        "safe_count_boundary",
        "total_safe_count_assessment",
    ] + edge_cols

    predict_rows: list[dict[str, object]] = []
    h3_label = merged["H3_rollout_shift_without_exclusion"].astype(int)
    for col in feature_cols:
        feature = merged[col]
        row: dict[str, object] = {
            "feature": col,
            "mean_when_h3_false": float(merged.loc[merged["H3_rollout_shift_without_exclusion"] == False, col].mean()),
            "mean_when_h3_true": float(merged.loc[merged["H3_rollout_shift_without_exclusion"] == True, col].mean()),
            "delta_true_minus_false": float(
                merged.loc[merged["H3_rollout_shift_without_exclusion"] == True, col].mean()
                - merged.loc[merged["H3_rollout_shift_without_exclusion"] == False, col].mean()
            ),
            "point_biserial_corr": _point_biserial(feature, h3_label),
            "is_binary": bool(set(pd.unique(feature)).issubset({0, 1})),
        }
        if row["is_binary"]:
            present = merged[merged[col] == 1]
            absent = merged[merged[col] == 0]
            row["h3_pass_rate_when_present"] = float(present["H3_rollout_shift_without_exclusion"].mean()) if len(present) else None
            row["h3_pass_rate_when_absent"] = float(absent["H3_rollout_shift_without_exclusion"].mean()) if len(absent) else None
            if row["h3_pass_rate_when_present"] is not None and row["h3_pass_rate_when_absent"] is not None:
                row["binary_h3_gap"] = float(row["h3_pass_rate_when_present"] - row["h3_pass_rate_when_absent"])
            else:
                row["binary_h3_gap"] = None
        else:
            row["h3_pass_rate_when_present"] = None
            row["h3_pass_rate_when_absent"] = None
            row["binary_h3_gap"] = None
        predict_rows.append(row)

    feature_predictiveness = pd.DataFrame(predict_rows)
    feature_predictiveness["abs_corr"] = feature_predictiveness["point_biserial_corr"].abs()
    feature_predictiveness.to_csv(
        out / "feature_predictiveness.csv",
        index=False,
    )
    feature_predictiveness.sort_values("abs_corr", ascending=False).to_csv(
        out / "feature_predictiveness_ranked.csv",
        index=False,
    )

    summary = {
        "variant_count": len(variant_seeds),
        "rows": int(len(merged)),
        "h3_pass_rate": float(merged["H3_rollout_shift_without_exclusion"].mean()),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")

    print(json.dumps(summary, indent=2))
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

