#!/usr/bin/env python3
"""Heldout generalization: evaluate Stage 0b-learned over admissibility-mask variants."""

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
import pandas as pd

from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import LearningConfig as LearnedKConfig
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import learn_kernel
from experiments.stage0b_resistance_vs_exclusion.analysis import (
    action_metrics_by_task,
    evaluate_hypotheses,
    simulate_rollouts,
    summarize_rollouts,
    PREREGISTERED_THRESHOLDS,
)
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel

from .graph_variants import VariantConfig, build_graph_variant


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
    parser = argparse.ArgumentParser(description="Stage 0b heldout generalization")
    parser.add_argument("--out", type=Path, default=_repo_root() / "experiments" / "stage0b_heldout_generalization" / "results")
    parser.add_argument("--learning-seed", type=int, default=0)
    parser.add_argument("--n-samples-per-state", type=int, default=1200)
    parser.add_argument("--alpha-smoothing", type=float, default=1e-2)
    parser.add_argument("--n-variants", type=int, default=12)
    parser.add_argument("--rollouts-per-task", type=int, default=180)
    parser.add_argument("--p-delete", type=float, default=0.12)
    parser.add_argument("--p-add", type=float, default=0.06)
    args = parser.parse_args()

    out: Path = args.out
    out.mkdir(parents=True, exist_ok=True)

    learned_k_cfg = LearnedKConfig(n_samples_per_state=args.n_samples_per_state, alpha_smoothing=args.alpha_smoothing)
    K_hat, R_fixed, idx = learn_kernel(learning_seed=args.learning_seed, config=learned_k_cfg)

    variant_cfg = VariantConfig(
        p_delete_safe_edges=args.p_delete,
        p_add_safe_edges=args.p_add,
        max_resample_attempts=60,
    )

    rows: list[dict[str, object]] = []
    for v in range(args.n_variants):
        graph_v = build_graph_variant(seed=v + 1000, config=variant_cfg)
        model = Stage0BModel(graph=graph_v, K=K_hat, R=R_fixed)

        action_df = action_metrics_by_task(model)
        rollouts = simulate_rollouts(model, n_rollouts_per_task=args.rollouts_per_task, seeds=(0,), max_steps=8)
        summary = summarize_rollouts(rollouts)
        results = evaluate_hypotheses(model, action_df, summary)

        row: dict[str, object] = {
            "variant_seed": v + 1000,
            "p_delete": args.p_delete,
            "p_add": args.p_add,
            "n_admissible_edges": int(graph_v.admissible_mask.sum()),
            "min_risky_probability_under_resistance": float(model.min_risky_admissible_probability("resistance")),
            "pass_all": all(r.passed for r in results),
        }
        for r in results:
            row[r.hypothesis] = r.passed
        rows.append(row)

    df = pd.DataFrame(rows).sort_values("variant_seed")
    df.to_csv(out / "heldout_variant_hypotheses.csv", index=False)

    manifest = {
        "stage": "heldout_0b",
        "name": "evaluate_stage0b_learned_on_graph_admissibility_variants",
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "python": sys.version,
        "platform": platform.platform(),
        "git_rev": _git_rev(),
        "learning_seed": args.learning_seed,
        "learning_config": asdict(learned_k_cfg),
        "n_variants": args.n_variants,
        "rollouts_per_task": args.rollouts_per_task,
        "variant_config": asdict(variant_cfg),
        "thresholds": PREREGISTERED_THRESHOLDS,
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")

    print("Heldout generalization variants:", args.n_variants)
    print(df["pass_all"].value_counts(dropna=False).to_string())
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

