"""Heldout generalization sanity tests."""

from __future__ import annotations

import pandas as pd

from experiments.stage0b_heldout_generalization.graph_variants import VariantConfig, build_graph_variant
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import LearningConfig as LearnedKConfig, learn_kernel
from experiments.stage0b_resistance_vs_exclusion.analysis import action_metrics_by_task, evaluate_hypotheses, simulate_rollouts, summarize_rollouts
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel


def test_variant_generator_protects_risky_detail_edges() -> None:
    graph = build_graph_variant(seed=42, config=VariantConfig(p_delete_safe_edges=0.4, p_add_safe_edges=0.0, max_resample_attempts=10))
    idx = graph.idx
    assert graph.admissible_mask[idx["risky_assessed"], idx["risky_detail"]]  # protected
    assert graph.admissible_mask[idx["boundary_assessed"], idx["risky_detail"]]  # protected


def test_heldout_runs_smoke(tmp_path) -> None:
    variant_cfg = VariantConfig(p_delete_safe_edges=0.15, p_add_safe_edges=0.0, max_resample_attempts=20)
    graph_v = build_graph_variant(seed=5, config=variant_cfg)

    learned_cfg = LearnedKConfig(n_samples_per_state=600, alpha_smoothing=1e-2)
    K_hat, R_fixed, _idx = learn_kernel(learning_seed=0, config=learned_cfg)
    model = Stage0BModel(graph=graph_v, K=K_hat, R=R_fixed)

    action_df = action_metrics_by_task(model)
    rollouts = simulate_rollouts(model, n_rollouts_per_task=80, seeds=(0,), max_steps=8)
    summary = summarize_rollouts(rollouts)
    results = evaluate_hypotheses(model, action_df, summary)
    df = pd.DataFrame([{"hypothesis": r.hypothesis, "passed": r.passed} for r in results])
    assert set(df["hypothesis"]) == {"H1_risky_mass_drop", "H2_nonzero_risky_support", "H3_rollout_shift_without_exclusion", "H4_benign_help_retained"}

