"""Stage 0b regression tests."""

from __future__ import annotations

import pytest

from experiments.stage0b_resistance_vs_exclusion.analysis import (
    PREREGISTERED_THRESHOLDS,
    action_metrics_by_task,
    evaluate_hypotheses,
    robustness_result,
    robustness_sweep,
    simulate_rollouts,
    summarize_rollouts,
)
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel


@pytest.fixture(scope="module")
def model() -> Stage0BModel:
    return Stage0BModel.default()


def test_support_invariance_except_exclusion(model: Stage0BModel) -> None:
    for state in model.graph.assistant_assessment_states:
        baseline = model.support(state, "baseline")
        resistance = model.support(state, "resistance")
        exclusion = model.support(state, "exclusion")
        assert baseline == resistance
        if state in {"risky_assessed", "boundary_assessed"}:
            assert "risky_detail" in baseline
            assert "risky_detail" not in exclusion
            assert exclusion == baseline - {"risky_detail"}
        else:
            assert exclusion == baseline


def test_risky_edges_remain_nonzero_under_resistance(model: Stage0BModel) -> None:
    min_prob = model.min_risky_admissible_probability("resistance")
    assert min_prob >= PREREGISTERED_THRESHOLDS["min_risky_probability_epsilon"]


def test_risky_action_mass_drops(model: Stage0BModel) -> None:
    action_df = action_metrics_by_task(model)
    risky = action_df[action_df["context_class"] == "risky"]
    baseline = float(risky[risky["condition"] == "baseline"]["risky_action_mass"].mean())
    resistance = float(risky[risky["condition"] == "resistance"]["risky_action_mass"].mean())
    assert resistance <= PREREGISTERED_THRESHOLDS["risky_mass_ratio_max"] * baseline


def test_hypotheses_h1_to_h4_pass(model: Stage0BModel) -> None:
    action_df = action_metrics_by_task(model)
    rollouts = simulate_rollouts(model, n_rollouts_per_task=600, seeds=(0, 1, 2))
    summary = summarize_rollouts(rollouts)
    results = evaluate_hypotheses(model, action_df, summary)
    assert all(result.passed for result in results)


def test_small_robustness_sweep_passes(model: Stage0BModel) -> None:
    robust_df = robustness_sweep(model, n_draws=6, n_rollouts_per_task=250)
    h5 = robustness_result(robust_df)
    assert h5.passed
