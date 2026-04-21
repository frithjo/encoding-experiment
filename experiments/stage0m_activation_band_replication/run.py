#!/usr/bin/env python3
"""Replicate Stage 0l edge-sensitive cells on fresh template seeds.

This stage selects only the `(target_feature, learning_seed, n_samples_per_state)`
cells that were classified as `edge_sensitive` in Stage 0l, then reruns those
same settings on a fresh template-seed range.

The question is not whether the pooled edge sign looks good. The question is:
does the sign observed in an activation-band cell survive when the template
family is refreshed while the learned-k regime is held fixed?
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import pandas as pd

from experiments.stage0i_targeted_edge_pairs.run import evaluate_constructed_pairs


def _parse_template_seeds(raw: str) -> list[int]:
    return [int(x) for x in raw.split(",") if x]


def _feature_to_edge(target_feature: str) -> tuple[str, str]:
    prefix = "edge__"
    if not target_feature.startswith(prefix):
        raise ValueError(f"unexpected target feature format: {target_feature!r}")
    src_dst = target_feature[len(prefix) :]
    src, dst = src_dst.split("__", 1)
    return src, dst


def load_activation_cells(path: Path) -> pd.DataFrame:
    df = pd.read_csv(path)
    selected = df[df["cell_state"] == "edge_sensitive"].copy()
    selected["original_sign"] = selected["present_minus_absent_H3_rate"].apply(
        lambda x: 1 if float(x) > 0.0 else -1
    )
    return selected.sort_values(["target_feature", "learning_seed", "n_samples_per_state"]).reset_index(drop=True)


def summarize_replication(replication_df: pd.DataFrame) -> pd.DataFrame:
    rows: list[dict[str, object]] = []
    group_cols = ["target_feature", "learning_seed", "n_samples_per_state"]

    for keys, sub in replication_df.groupby(group_cols):
        target_feature, learning_seed, n_samples = keys
        original_sign = int(sub["original_sign"].iloc[0])
        original_delta = float(sub["original_present_minus_absent_H3_rate"].iloc[0])
        replicate_present = float(sub["present_H3"].mean())
        replicate_absent = float(sub["absent_H3"].mean())
        replicate_delta = replicate_present - replicate_absent
        replicate_sign = 1 if replicate_delta > 0.0 else (-1 if replicate_delta < 0.0 else 0)

        rows.append(
            {
                "target_feature": target_feature,
                "learning_seed": int(learning_seed),
                "n_samples_per_state": int(n_samples),
                "n_replication_templates": int(len(sub)),
                "original_present_minus_absent_H3_rate": original_delta,
                "original_sign": original_sign,
                "replicate_present_H3_rate": replicate_present,
                "replicate_absent_H3_rate": replicate_absent,
                "replicate_present_minus_absent_H3_rate": replicate_delta,
                "replicate_sign": replicate_sign,
                "sign_match": bool(replicate_sign == original_sign),
                "present_only_replication_rate": float(((sub["present_H3"]) & (~sub["absent_H3"])).mean()),
                "absent_only_replication_rate": float(((~sub["present_H3"]) & (sub["absent_H3"])).mean()),
                "both_pass_replication_rate": float(((sub["present_H3"]) & (sub["absent_H3"])).mean()),
                "both_fail_replication_rate": float(((~sub["present_H3"]) & (~sub["absent_H3"])).mean()),
                "mean_baseline_safe_present_minus_absent": float(sub["baseline_safe_present_minus_absent"].mean()),
                "mean_safe_gain_present_minus_absent": float(sub["safe_gain_present_minus_absent"].mean()),
                "mean_risky_drop_present_minus_absent": float(sub["risky_drop_present_minus_absent"].mean()),
            }
        )

    return pd.DataFrame(rows).sort_values(group_cols).reset_index(drop=True)


def summarize_features(cell_replication_df: pd.DataFrame) -> pd.DataFrame:
    rows: list[dict[str, object]] = []
    for target_feature, sub in cell_replication_df.groupby("target_feature"):
        rows.append(
            {
                "target_feature": target_feature,
                "n_activation_cells": int(len(sub)),
                "sign_match_rate": float(sub["sign_match"].mean()),
                "mean_original_delta": float(sub["original_present_minus_absent_H3_rate"].mean()),
                "mean_replicate_delta": float(sub["replicate_present_minus_absent_H3_rate"].mean()),
                "positive_replicate_rate": float((sub["replicate_sign"] > 0).mean()),
                "negative_replicate_rate": float((sub["replicate_sign"] < 0).mean()),
                "zero_replicate_rate": float((sub["replicate_sign"] == 0).mean()),
            }
        )
    return pd.DataFrame(rows).sort_values("target_feature").reset_index(drop=True)


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage 0m activation-band replication")
    parser.add_argument(
        "--activation-cells-csv",
        type=Path,
        default=Path("experiments/stage0l_edge_effect_activation/results/cell_activation_summary.csv"),
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path("experiments/stage0m_activation_band_replication/results"),
    )
    parser.add_argument(
        "--template-seeds",
        type=str,
        default=",".join(str(x) for x in range(3000, 3032)),
    )
    parser.add_argument("--p-delete", type=float, default=0.12)
    parser.add_argument("--p-add", type=float, default=0.06)
    parser.add_argument("--eval-rollouts", type=int, default=360)
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    template_seeds = _parse_template_seeds(args.template_seeds)
    activation_cells = load_activation_cells(args.activation_cells_csv)

    meta = {
        "activation_cells_csv": str(args.activation_cells_csv),
        "template_seeds": template_seeds,
        "n_activation_cells": int(len(activation_cells)),
        "eval_rollouts": args.eval_rollouts,
        "p_delete": args.p_delete,
        "p_add": args.p_add,
    }
    (out / "protocol.json").write_text(json.dumps(meta, indent=2), encoding="utf-8")

    pair_csv = out / "replication_pair_results.csv"
    pair_csv.unlink(missing_ok=True)
    header_written = False

    pair_tables: list[pd.DataFrame] = []
    for _, cell in activation_cells.iterrows():
        target_feature = str(cell["target_feature"])
        target_src, target_dst = _feature_to_edge(target_feature)
        chunk = evaluate_constructed_pairs(
            learning_seed=int(cell["learning_seed"]),
            n_samples_per_state=int(cell["n_samples_per_state"]),
            template_seeds=template_seeds,
            target_src=target_src,
            target_dst=target_dst,
            p_delete=args.p_delete,
            p_add=args.p_add,
            eval_rollouts=args.eval_rollouts,
        )
        chunk["original_present_minus_absent_H3_rate"] = float(cell["present_minus_absent_H3_rate"])
        chunk["original_sign"] = int(cell["original_sign"])
        pair_tables.append(chunk)
        chunk.to_csv(pair_csv, mode="a", index=False, header=not header_written)
        header_written = True

    pair_df = pd.concat(pair_tables, ignore_index=True) if pair_tables else pd.DataFrame()
    cell_df = summarize_replication(pair_df)
    feature_df = summarize_features(cell_df)

    pair_df.to_csv(pair_csv, index=False)
    cell_df.to_csv(out / "replication_cell_summary.csv", index=False)
    feature_df.to_csv(out / "replication_feature_summary.csv", index=False)

    summary = {
        "n_activation_cells": int(len(activation_cells)),
        "n_replication_pairs": int(len(pair_df)),
        "n_replication_templates": len(template_seeds),
        "features": sorted(activation_cells["target_feature"].unique().tolist()),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

