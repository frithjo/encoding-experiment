"""Stage 0c: learn a resistance intensity scalar q-scale (w) on fixed graph support.

We already have a learned baseline kernel K_hat from traces (via stage0b-learned).
We keep the same resistance shape R but scale charges by w:
  q(state) = w * charge_by_state_base(state)
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np

from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import LearningConfig as LearnedKConfig
from experiments.stage0b_learned_kernel_resistance_vs_exclusion.learn import learn_kernel as learn_K_hat
from experiments.stage0b_resistance_vs_exclusion.graph import build_graph_spec
from experiments.stage0b_resistance_vs_exclusion.kernel import Stage0BModel
from experiments.stage0b_resistance_vs_exclusion.analysis import action_metrics_by_task


@dataclass(frozen=True)
class QLearnConfig:
    w_grid: tuple[float, ...]
    min_risky_probability_epsilon: float = 1e-4
    benign_help_retention_delta_max: float = 0.12
    # choose w that minimizes risky action mass subject to constraints
    selection_num_rollout_samples: int = 0  # currently selection uses action metrics only


def scale_graph_charge(graph, w: float):
    # GraphSpec is frozen dataclass; create new instance with updated charge dict.
    return type(graph)(
        states=graph.states,
        idx=graph.idx,
        admissible_mask=graph.admissible_mask,
        risky_edge_mask=graph.risky_edge_mask,
        charge_by_state={k: v * w for k, v in graph.charge_by_state.items()},
        tasks=graph.tasks,
        assistant_assessment_states=graph.assistant_assessment_states,
        terminal_states=graph.terminal_states,
        start_states=graph.start_states,
        outcome_terminals=graph.outcome_terminals,
    )


def learn_optimal_w(
    *,
    learning_seed: int,
    learned_k_config: LearnedKConfig,
    qlearn_config: QLearnConfig,
):
    graph_base = build_graph_spec()
    K_hat, R_fixed, idx = learn_K_hat(learning_seed=learning_seed, config=learned_k_config)

    # Baseline/exclusion proxies: baseline ignores q, exclusion masks risky_detail.
    baseline_graph_any = scale_graph_charge(graph_base, w=1.0)
    baseline_model = Stage0BModel(graph=baseline_graph_any, K=K_hat, R=R_fixed)
    action_df_any = action_metrics_by_task(baseline_model)
    risky_rows_base = action_df_any[(action_df_any["context_class"] == "risky") & (action_df_any["condition"] == "baseline")]
    risky_rows_exc = action_df_any[(action_df_any["context_class"] == "risky") & (action_df_any["condition"] == "exclusion")]
    benign_rows_base = action_df_any[(action_df_any["context_class"] == "benign") & (action_df_any["condition"] == "baseline")]

    baseline_benign_help_proxy = float(benign_rows_base["direct_help"].mean())
    baseline_safe_proxy = float(risky_rows_base["safe_action_mass"].mean())
    exclusion_safe_proxy = float(risky_rows_exc["safe_action_mass"].mean())
    baseline_risky_action_proxy = float(risky_rows_base["risky_action_mass"].mean())

    w_rows = []
    for w in qlearn_config.w_grid:
        graph_w = scale_graph_charge(graph_base, w)
        model = Stage0BModel(graph=graph_w, K=K_hat, R=R_fixed)

        # min risky admissible probability under resistance
        min_risky_prob = model.min_risky_admissible_probability("resistance")

        # approximate risky action mass by proxy: risky terminal edge mass is modeled via risky_detail? We use action metrics later.
        # Here we compute raw risky_detail probability on risky_assessed and boundary_assessed.
        risky_detail_probs = []
        for src in ["risky_assessed", "boundary_assessed"]:
            risky_detail_probs.append(model.edge_prob(src, "risky_detail", "resistance"))
        risky_action_mass_proxy = float(np.mean(risky_detail_probs))

        # benign helpful retention constraint: we proxy via direct_help probability on benign_assessed.
        benign_help_proxy = model.edge_prob("benign_assessed", "direct_help", "resistance")

        # proxy safe_action_mass under resistance on risky tasks (should track safe terminal gain)
        action_df_w = action_metrics_by_task(model)
        risky_w = action_df_w[(action_df_w["context_class"] == "risky") & (action_df_w["condition"] == "resistance")]
        resistance_safe_proxy = float(risky_w["safe_action_mass"].mean())
        resistance_risky_action_mass = float(risky_w["risky_action_mass"].mean())

        w_rows.append(
            {
                "w": w,
                "min_risky_probability_under_resistance": float(min_risky_prob),
                "risky_action_mass_proxy": risky_action_mass_proxy,
                "benign_help_proxy": float(benign_help_proxy),
                "benign_help_delta": float(benign_help_proxy - baseline_benign_help_proxy),
                "risky_action_proxy_ratio": (
                    float(risky_action_mass_proxy / baseline_risky_action_proxy)
                    if baseline_risky_action_proxy > 0
                    else 0.0
                ),
                "resistance_safe_action_mass": resistance_safe_proxy,
                "resistance_risky_action_mass": resistance_risky_action_mass,
                "safe_gap_to_exclusion": abs(resistance_safe_proxy - exclusion_safe_proxy),
            }
        )

    # select among eligible w based on constraints
    eligible = [
        r
        for r in w_rows
        if r["min_risky_probability_under_resistance"] >= qlearn_config.min_risky_probability_epsilon
        and r["benign_help_proxy"] >= baseline_benign_help_proxy - qlearn_config.benign_help_retention_delta_max
    ]
    if not eligible:
        # fall back to smallest w (least resistance), but will likely fail H2; still honest.
        chosen = min(w_rows, key=lambda r: r["w"])
        return float(chosen["w"]), w_rows, {"K_hat_shape": tuple(K_hat.shape)}

    # Prefer w that matches exclusion on safe mass while keeping risky mass low.
    chosen = min(
        eligible,
        key=lambda r: (
            r["safe_gap_to_exclusion"],
            r["resistance_risky_action_mass"],
        ),
    )
    return float(chosen["w"]), w_rows, {"K_hat_shape": tuple(K_hat.shape)}

