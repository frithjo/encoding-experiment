#!/usr/bin/env python3
"""Construct paired graph twins that differ only on one targeted edge.

Purpose:
- hold broader support geometry fixed by construction
- toggle exactly one safe edge
- evaluate whether that edge changes strict H3 outcomes

This is stronger than correlational ranking over random heldout variants,
but it is still a controlled toy setting with calibration-dependent costs.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import pandas as pd

from experiments.stage0b_heldout_generalization.graph_variants import VariantConfig
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
from experiments.stage0b_resistance_vs_exclusion.graph import GraphSpec, build_graph_spec
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel

SAFE_ACTION_DESTINATIONS: tuple[str, ...] = (
    "direct_help",
    "clarify",
    "refuse",
    "safe_alternative",
    "high_level_explanation",
    "boundary_reply",
)

PROTECTED_RISKY_EDGES: tuple[tuple[str, str], ...] = (
    ("risky_assessed", "risky_detail"),
    ("boundary_assessed", "risky_detail"),
)

DEFAULT_TARGET_EDGES: tuple[tuple[str, str], ...] = (
    ("boundary_assessed", "high_level_explanation"),
    ("risky_assessed", "refuse"),
    ("ambiguous_assessed", "clarify"),
    ("benign_assessed", "clarify"),
)


def _depth_reachable(mask: np.ndarray, start_nodes: list[int], terminal_nodes: set[int], max_steps: int) -> bool:
    frontier = set(start_nodes)
    visited = set(start_nodes)
    for _ in range(max_steps + 1):
        if any(node in terminal_nodes for node in frontier):
            return True
        next_frontier: set[int] = set()
        for src in frontier:
            for dst in np.where(mask[src])[0]:
                dst_i = int(dst)
                if dst_i in visited:
                    continue
                visited.add(dst_i)
                next_frontier.add(dst_i)
        if not next_frontier:
            return False
        frontier = next_frontier
    return False


def _graph_with_mask(base: GraphSpec, admissible_mask: np.ndarray) -> GraphSpec:
    return GraphSpec(
        states=base.states,
        idx=base.idx,
        admissible_mask=admissible_mask,
        risky_edge_mask=base.risky_edge_mask,
        charge_by_state=base.charge_by_state,
        tasks=base.tasks,
        assistant_assessment_states=base.assistant_assessment_states,
        terminal_states=base.terminal_states,
        start_states=base.start_states,
        outcome_terminals=base.outcome_terminals,
    )


def _pair_is_valid(base: GraphSpec, admissible_common: np.ndarray, src_i: int, dst_i: int, max_steps: int) -> bool:
    terminal_idxs = {base.idx[s] for s in base.terminal_states}
    start_idxs = [base.idx[s] for s in base.start_states]

    for target_present in (False, True):
        mask = admissible_common.copy()
        mask[src_i, dst_i] = target_present
        for state in base.assistant_assessment_states:
            if not np.any(mask[base.idx[state]]):
                return False
        if not _depth_reachable(mask, start_idxs, terminal_idxs, max_steps):
            return False
    return True


def build_targeted_pair(
    *,
    template_seed: int,
    target_src: str,
    target_dst: str,
    config: VariantConfig,
) -> tuple[GraphSpec, GraphSpec]:
    """Return (target_present_graph, target_absent_graph)."""
    rng = np.random.default_rng(template_seed)
    base = build_graph_spec()
    idx = base.idx
    src_i = idx[target_src]
    dst_i = idx[target_dst]

    assessment_src_idxs = [idx[s] for s in base.assistant_assessment_states]
    safe_dst_idxs = [idx[s] for s in SAFE_ACTION_DESTINATIONS if s in idx]
    protected_pairs = {(idx[s], idx[d]) for s, d in PROTECTED_RISKY_EDGES}
    target_pair = (src_i, dst_i)

    for _ in range(config.max_resample_attempts):
        admissible = base.admissible_mask.copy()

        for src in assessment_src_idxs:
            for dst in safe_dst_idxs:
                pair = (src, dst)
                if pair == target_pair or pair in protected_pairs or not admissible[src, dst]:
                    continue
                if rng.random() < config.p_delete_safe_edges:
                    admissible[src, dst] = False

        for src in assessment_src_idxs:
            for dst in safe_dst_idxs:
                pair = (src, dst)
                if pair == target_pair or pair in protected_pairs or admissible[src, dst]:
                    continue
                if rng.random() < config.p_add_safe_edges:
                    admissible[src, dst] = True

        if not _pair_is_valid(base, admissible, src_i, dst_i, config.max_steps_for_reachability):
            continue

        present_mask = admissible.copy()
        absent_mask = admissible.copy()
        present_mask[src_i, dst_i] = True
        absent_mask[src_i, dst_i] = False
        return _graph_with_mask(base, present_mask), _graph_with_mask(base, absent_mask)

    fallback_present = base.admissible_mask.copy()
    fallback_absent = base.admissible_mask.copy()
    fallback_present[src_i, dst_i] = True
    fallback_absent[src_i, dst_i] = False
    return _graph_with_mask(base, fallback_present), _graph_with_mask(base, fallback_absent)


def strict_eval_for_graph(
    *,
    graph: GraphSpec,
    K_hat: np.ndarray,
    R_fixed: np.ndarray,
    eval_rollouts: int,
) -> dict[str, object]:
    model = Stage0BModel(graph=graph, K=K_hat, R=R_fixed)
    action_df = action_metrics_by_task(model)
    rollouts = simulate_rollouts(model, n_rollouts_per_task=eval_rollouts, seeds=(0,), max_steps=8)
    summary = summarize_rollouts(rollouts)
    results = evaluate_hypotheses(model, action_df, summary)

    risky = summary[summary["context_class"] == "risky"].set_index("condition")
    baseline_safe = float(risky.loc["baseline", "safe_or_boundary_rate"])
    resistance_safe = float(risky.loc["resistance", "safe_or_boundary_rate"])
    baseline_risky = float(risky.loc["baseline", "risky_rate"])
    resistance_risky = float(risky.loc["resistance", "risky_rate"])

    row: dict[str, object] = {
        "n_admissible_edges": int(graph.admissible_mask.sum()),
        "baseline_safe_or_boundary_rate": baseline_safe,
        "resistance_safe_or_boundary_rate": resistance_safe,
        "baseline_risky_terminal_rate": baseline_risky,
        "resistance_risky_terminal_rate": resistance_risky,
        "safe_gain": resistance_safe - baseline_safe,
        "risky_drop": baseline_risky - resistance_risky,
    }
    for result in results:
        row[result.hypothesis] = result.passed
        for k, v in result.details.items():
            row[f"{result.hypothesis}__{k}"] = v
    row["pass_all"] = all(r.passed for r in results)
    return row


def evaluate_constructed_pairs(
    *,
    learning_seed: int,
    n_samples_per_state: int,
    template_seeds: list[int],
    target_src: str,
    target_dst: str,
    p_delete: float,
    p_add: float,
    eval_rollouts: int,
) -> pd.DataFrame:
    learned_cfg = LearnedKConfig(n_samples_per_state=n_samples_per_state, alpha_smoothing=1e-2)
    K_hat, R_fixed, _ = learn_kernel(learning_seed=learning_seed, config=learned_cfg)

    variant_cfg = VariantConfig(
        p_delete_safe_edges=p_delete,
        p_add_safe_edges=p_add,
        max_resample_attempts=60,
        max_steps_for_reachability=8,
    )

    rows: list[dict[str, object]] = []
    for template_seed in template_seeds:
        graph_present, graph_absent = build_targeted_pair(
            template_seed=template_seed,
            target_src=target_src,
            target_dst=target_dst,
            config=variant_cfg,
        )
        present = strict_eval_for_graph(
            graph=graph_present,
            K_hat=K_hat,
            R_fixed=R_fixed,
            eval_rollouts=eval_rollouts,
        )
        absent = strict_eval_for_graph(
            graph=graph_absent,
            K_hat=K_hat,
            R_fixed=R_fixed,
            eval_rollouts=eval_rollouts,
        )

        rows.append(
            {
                "learning_seed": learning_seed,
                "n_samples_per_state": n_samples_per_state,
                "template_seed": template_seed,
                "target_src": target_src,
                "target_dst": target_dst,
                "target_feature": f"edge__{target_src}__{target_dst}",
                "present_n_admissible_edges": int(present["n_admissible_edges"]),
                "absent_n_admissible_edges": int(absent["n_admissible_edges"]),
                "baseline_safe_present": float(present["baseline_safe_or_boundary_rate"]),
                "baseline_safe_absent": float(absent["baseline_safe_or_boundary_rate"]),
                "baseline_safe_present_minus_absent": float(
                    present["baseline_safe_or_boundary_rate"] - absent["baseline_safe_or_boundary_rate"]
                ),
                "safe_gain_present": float(present["safe_gain"]),
                "safe_gain_absent": float(absent["safe_gain"]),
                "safe_gain_present_minus_absent": float(present["safe_gain"] - absent["safe_gain"]),
                "risky_drop_present": float(present["risky_drop"]),
                "risky_drop_absent": float(absent["risky_drop"]),
                "risky_drop_present_minus_absent": float(present["risky_drop"] - absent["risky_drop"]),
                "present_H1": bool(present["H1_risky_mass_drop"]),
                "absent_H1": bool(absent["H1_risky_mass_drop"]),
                "present_H2": bool(present["H2_nonzero_risky_support"]),
                "absent_H2": bool(absent["H2_nonzero_risky_support"]),
                "present_H3": bool(present["H3_rollout_shift_without_exclusion"]),
                "absent_H3": bool(absent["H3_rollout_shift_without_exclusion"]),
                "present_H4": bool(present["H4_benign_help_retained"]),
                "absent_H4": bool(absent["H4_benign_help_retained"]),
                "present_pass_all": bool(present["pass_all"]),
                "absent_pass_all": bool(absent["pass_all"]),
                "present_resistance_risky_terminal": float(present["resistance_risky_terminal_rate"]),
                "absent_resistance_risky_terminal": float(absent["resistance_risky_terminal_rate"]),
            }
        )
    return pd.DataFrame(rows)


def parse_target_edges(target_edges_arg: str | None, target_src: str | None, target_dst: str | None) -> list[tuple[str, str]]:
    if target_edges_arg:
        edges: list[tuple[str, str]] = []
        for item in target_edges_arg.split(","):
            item = item.strip()
            if not item:
                continue
            if "->" not in item:
                raise ValueError(f"invalid target edge specification: {item!r}")
            src, dst = [part.strip() for part in item.split("->", 1)]
            if not src or not dst:
                raise ValueError(f"invalid target edge specification: {item!r}")
            edges.append((src, dst))
        if not edges:
            raise ValueError("no target edges parsed")
        return edges

    if target_src and target_dst:
        return [(target_src, target_dst)]

    return list(DEFAULT_TARGET_EDGES)


def summarize_pairs(pairs_df: pd.DataFrame) -> pd.DataFrame:
    if len(pairs_df) == 0:
        return pd.DataFrame()

    grouped = (
        pairs_df.groupby(["target_feature", "learning_seed", "n_samples_per_state"], as_index=False)
        .agg(
            n_pairs=("template_seed", "count"),
            present_H3_rate=("present_H3", "mean"),
            absent_H3_rate=("absent_H3", "mean"),
            present_pass_all_rate=("present_pass_all", "mean"),
            absent_pass_all_rate=("absent_pass_all", "mean"),
            mean_baseline_safe_present_minus_absent=("baseline_safe_present_minus_absent", "mean"),
            mean_safe_gain_present_minus_absent=("safe_gain_present_minus_absent", "mean"),
            mean_risky_drop_present_minus_absent=("risky_drop_present_minus_absent", "mean"),
        )
        .sort_values(["target_feature", "learning_seed", "n_samples_per_state"])
    )
    grouped["present_minus_absent_H3_rate"] = grouped["present_H3_rate"] - grouped["absent_H3_rate"]
    grouped["present_minus_absent_pass_all_rate"] = (
        grouped["present_pass_all_rate"] - grouped["absent_pass_all_rate"]
    )
    return grouped


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage 0i constructed targeted edge pairs")
    parser.add_argument("--out", type=Path, default=Path("experiments/stage0i_targeted_edge_pairs/results"))
    parser.add_argument("--learning-seeds", type=str, default="0,1")
    parser.add_argument("--n-samples-values", type=str, default="600,1200")
    parser.add_argument("--template-seeds", type=str, default="2000,2001,2002,2003,2004,2005,2006,2007,2008,2009")
    parser.add_argument("--target-edges", type=str, default=None)
    parser.add_argument("--target-src", type=str, default=None)
    parser.add_argument("--target-dst", type=str, default=None)
    parser.add_argument("--p-delete", type=float, default=0.12)
    parser.add_argument("--p-add", type=float, default=0.06)
    parser.add_argument("--eval-rollouts", type=int, default=360)
    args = parser.parse_args()

    out = args.out
    out.mkdir(parents=True, exist_ok=True)

    learning_seeds = [int(x) for x in args.learning_seeds.split(",") if x]
    n_samples_values = [int(x) for x in args.n_samples_values.split(",") if x]
    template_seeds = [int(x) for x in args.template_seeds.split(",") if x]
    target_edges = parse_target_edges(args.target_edges, args.target_src, args.target_dst)

    pair_tables: list[pd.DataFrame] = []
    for target_src, target_dst in target_edges:
        for learning_seed in learning_seeds:
            for n_samples in n_samples_values:
                pair_tables.append(
                    evaluate_constructed_pairs(
                        learning_seed=learning_seed,
                        n_samples_per_state=n_samples,
                        template_seeds=template_seeds,
                        target_src=target_src,
                        target_dst=target_dst,
                        p_delete=args.p_delete,
                        p_add=args.p_add,
                        eval_rollouts=args.eval_rollouts,
                    )
                )

    pairs_df = pd.concat(pair_tables, ignore_index=True) if pair_tables else pd.DataFrame()
    summary_df = summarize_pairs(pairs_df)

    pairs_df.to_csv(out / "constructed_pair_evaluations.csv", index=False)
    summary_df.to_csv(out / "constructed_pair_summary.csv", index=False)

    summary = {
        "target_features": [f"edge__{src}__{dst}" for src, dst in target_edges],
        "learning_seeds": learning_seeds,
        "n_samples_values": n_samples_values,
        "template_count": len(template_seeds),
        "total_pairs": int(len(pairs_df)),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    print("Artifacts:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

