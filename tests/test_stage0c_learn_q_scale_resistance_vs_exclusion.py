"""Stage 0c tests: learned q-scale still shapes over fixed admissible support."""

from __future__ import annotations

import pytest

from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import LearningConfig as LearnedKConfig
from experiments.stage0b_resistance_vs_exclusion.graph import build_graph_spec
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel
from experiments.stage0b_resistance_vs_exclusion.analysis import (
    PREREGISTERED_THRESHOLDS,
    action_metrics_by_task,
    simulate_rollouts,
    summarize_rollouts,
)
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.analysis import evaluate_hypotheses
from experiments.stage0c_learn_q_scale_resistance_vs_exclusion.q_learn import QLearnConfig, learn_optimal_w, scale_graph_charge
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import learn_kernel


@pytest.fixture(scope="module")
def stage0c_model() -> tuple[Stage0BModel, float]:
    graph_base = build_graph_spec()
    learned_k_cfg = LearnedKConfig(n_samples_per_state=900, alpha_smoothing=1e-2)
    w_grid = tuple(float(x) for x in [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0])
    qlearn_cfg = QLearnConfig(
        w_grid=w_grid,
        min_risky_probability_epsilon=PREREGISTERED_THRESHOLDS["min_risky_probability_epsilon"],
        benign_help_retention_delta_max=0.12,
    )
    w_chosen, _w_search_rows, _meta = learn_optimal_w(
        learning_seed=0,
        learned_k_config=learned_k_cfg,
        qlearn_config=qlearn_cfg,
    )

    K_hat, R_fixed, _idx = learn_kernel(learning_seed=0, config=learned_k_cfg)
    graph_w = scale_graph_charge(graph_base, w=w_chosen)
    return Stage0BModel(graph=graph_w, K=K_hat, R=R_fixed), w_chosen


def test_h1_h4_pass(stage0c_model: tuple[Stage0BModel, float]) -> None:
    model, _w = stage0c_model
    assert model.min_risky_admissible_probability("resistance") >= 0.0
    action_df = action_metrics_by_task(model)
    rollouts = simulate_rollouts(model, n_rollouts_per_task=220, seeds=(0, 1))
    summary = summarize_rollouts(rollouts)
    results = evaluate_hypotheses(model, action_df, summary)
    assert all(r.passed for r in results)

