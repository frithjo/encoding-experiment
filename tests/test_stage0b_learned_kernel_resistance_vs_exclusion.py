"""Stage 0b-learned regression tests (learn K then test resistance vs exclusion)."""

from __future__ import annotations

import pytest

from experiments.stage0b_learned_kernel_resistance_vs_exclusion.analysis import (
    PREREGISTERED_THRESHOLDS,
    evaluate_hypotheses,
    robustness_result,
    robustness_sweep,
    action_metrics_by_task,
    simulate_rollouts,
    summarize_rollouts,
)
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import LearningConfig, learn_kernel
from experiments.stage0b_resistance_vs_exclusion.graph import build_graph_spec
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel


@pytest.fixture(scope="module")
def learned_model() -> Stage0BModel:
    graph = build_graph_spec()
    cfg = LearningConfig(n_samples_per_state=1200, alpha_smoothing=1e-2)
    K_hat, R_fixed, _idx = learn_kernel(learning_seed=0, config=cfg)
    return Stage0BModel(graph=graph, K=K_hat, R=R_fixed)


def test_support_invariance_except_exclusion(learned_model: Stage0BModel) -> None:
    for state in learned_model.graph.assistant_assessment_states:
        baseline = learned_model.support(state, "baseline")
        resistance = learned_model.support(state, "resistance")
        exclusion = learned_model.support(state, "exclusion")
        assert baseline == resistance
        if state in {"risky_assessed", "boundary_assessed"}:
            assert "risky_detail" in baseline
            assert "risky_detail" not in exclusion
            assert exclusion == baseline - {"risky_detail"}
        else:
            assert exclusion == baseline


def test_h1_h4_hold_for_learned_model(learned_model: Stage0BModel) -> None:
    action_df = action_metrics_by_task(learned_model)
    rollouts = simulate_rollouts(learned_model, n_rollouts_per_task=300, seeds=(0, 1, 2))
    rollout_summary = summarize_rollouts(rollouts)
    results = evaluate_hypotheses(learned_model, action_df, rollout_summary)
    assert all(r.passed for r in results)


def test_h5_robustness_small_sweep(learned_model: Stage0BModel) -> None:
    robust_df = robustness_sweep(learned_model, n_draws=4, n_rollouts_per_task=200, sigma=PREREGISTERED_THRESHOLDS["perturbation_sigma"])
    h5 = robustness_result(robust_df)
    assert h5.passed

