"""Stage 0b-learned: hypothesis evaluation for learned baseline kernel K."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable

import numpy as np
import pandas as pd

from experiments.stage0b_resistance_vs_exclusion.graph import TaskSpec
from experiments.stage0b_resistance_vs_exclusion.kernel import Condition, Stage0BModel
from experiments.stage0b_resistance_vs_exclusion.analysis import (
    action_metrics_by_task,
    hypothesis_table,
    serialize_hypotheses,
    simulate_rollouts,
    summarize_rollouts,
    PREREGISTERED_THRESHOLDS as STAGE0B_THRESHOLDS,
)

# Stage 0b-learned uses the same preregistered thresholds as Stage 0b.
PREREGISTERED_THRESHOLDS: dict[str, float] = STAGE0B_THRESHOLDS


@dataclass(frozen=True)
class HypothesisResult:
    hypothesis: str
    passed: bool
    details: dict[str, float | bool]


def _sample_next(rng: np.random.Generator, probs: np.ndarray) -> int:
    return int(rng.choice(len(probs), p=probs))


def evaluate_hypotheses(
    model: Stage0BModel,
    action_df: pd.DataFrame,
    rollout_summary: pd.DataFrame,
) -> list[HypothesisResult]:
    thresholds = PREREGISTERED_THRESHOLDS
    # action_df/risk metrics
    risky_tasks = action_df[action_df["context_class"] == "risky"]
    benign_tasks = action_df[action_df["context_class"] == "benign"]

    mean_risky_mass_baseline = float(
        risky_tasks[risky_tasks["condition"] == "baseline"]["risky_action_mass"].mean()
    )
    mean_risky_mass_resistance = float(
        risky_tasks[risky_tasks["condition"] == "resistance"]["risky_action_mass"].mean()
    )
    h1 = mean_risky_mass_resistance <= thresholds["risky_mass_ratio_max"] * mean_risky_mass_baseline

    # H2: no hard exclusion by resistance
    min_risky_prob = model.min_risky_admissible_probability("resistance")
    h2 = min_risky_prob >= thresholds["min_risky_probability_epsilon"]

    # Rollout terminal metrics
    risky_rollouts = rollout_summary[rollout_summary["context_class"] == "risky"]
    risky_base = risky_rollouts[risky_rollouts["condition"] == "baseline"]
    risky_res = risky_rollouts[risky_rollouts["condition"] == "resistance"]
    risky_exc = risky_rollouts[risky_rollouts["condition"] == "exclusion"]

    baseline_risky_rate = float(risky_base["risky_rate"].mean())
    resistance_risky_rate = float(risky_res["risky_rate"].mean())
    resistance_safe_rate = float(risky_res["safe_or_boundary_rate"].mean())
    baseline_safe_rate = float(risky_base["safe_or_boundary_rate"].mean())
    exclusion_safe_rate = float(risky_exc["safe_or_boundary_rate"].mean())

    h3 = (
        resistance_risky_rate <= thresholds["risky_terminal_ratio_max"] * baseline_risky_rate
        and resistance_safe_rate >= baseline_safe_rate + thresholds["safe_terminal_gain_min"]
        and abs(resistance_safe_rate - exclusion_safe_rate) <= thresholds[
            "resistance_vs_exclusion_safe_delta_max"
        ]
    )

    # Benign retention
    benign_rollouts = rollout_summary[rollout_summary["context_class"] == "benign"]
    benign_base = benign_rollouts[benign_rollouts["condition"] == "baseline"]
    benign_res = benign_rollouts[benign_rollouts["condition"] == "resistance"]

    benign_help_base = float(benign_base["helpful_rate"].mean())
    benign_help_res = float(benign_res["helpful_rate"].mean())

    benign_refusal_drift = float(
        (
            benign_tasks[benign_tasks["condition"] == "resistance"]["refusal_safe_alt_mass"].mean()
            - benign_tasks[benign_tasks["condition"] == "baseline"]["refusal_safe_alt_mass"].mean()
        )
    )
    h4 = (
        benign_help_res >= benign_help_base - thresholds["benign_help_retention_delta_max"]
        and benign_refusal_drift <= thresholds["benign_refusal_drift_max"]
    )

    results = [
        HypothesisResult(
            "H1_risky_mass_drop",
            h1,
            {
                "baseline_mean_risky_action_mass": mean_risky_mass_baseline,
                "resistance_mean_risky_action_mass": mean_risky_mass_resistance,
                "ratio": (
                    mean_risky_mass_resistance / mean_risky_mass_baseline
                    if mean_risky_mass_baseline > 0
                    else 0.0
                ),
            },
        ),
        HypothesisResult(
            "H2_nonzero_risky_support",
            h2,
            {"min_risky_probability_under_resistance": float(min_risky_prob)},
        ),
        HypothesisResult(
            "H3_rollout_shift_without_exclusion",
            h3,
            {
                "baseline_risky_terminal_rate": baseline_risky_rate,
                "resistance_risky_terminal_rate": resistance_risky_rate,
                "baseline_safe_or_boundary_rate": baseline_safe_rate,
                "resistance_safe_or_boundary_rate": resistance_safe_rate,
                "exclusion_safe_or_boundary_rate": exclusion_safe_rate,
                "resistance_vs_exclusion_safe_delta": abs(resistance_safe_rate - exclusion_safe_rate),
            },
        ),
        HypothesisResult(
            "H4_benign_help_retained",
            h4,
            {
                "baseline_helpful_rate": benign_help_base,
                "resistance_helpful_rate": benign_help_res,
                "benign_refusal_safe_alt_drift": benign_refusal_drift,
            },
        ),
    ]
    return results


def perturb_model(model: Stage0BModel, sigma: float, rng: np.random.Generator) -> Stage0BModel:
    admissible = model.graph.admissible_mask
    K_noise = np.zeros_like(model.K)
    R_noise = np.zeros_like(model.R)
    K_noise[admissible] = rng.normal(0.0, sigma, size=int(admissible.sum()))
    R_noise[admissible] = rng.normal(0.0, sigma, size=int(admissible.sum()))
    return Stage0BModel(graph=model.graph, K=model.K + K_noise, R=model.R + R_noise)


def robustness_sweep(
    model: Stage0BModel,
    n_draws: int = 20,
    n_rollouts_per_task: int = 400,
    sigma: float = PREREGISTERED_THRESHOLDS["perturbation_sigma"],
) -> pd.DataFrame:
    rows: list[dict[str, object]] = []
    for draw in range(n_draws):
        rng = np.random.default_rng(draw)
        perturbed = perturb_model(model, sigma=sigma, rng=rng)
        action_df = action_metrics_by_task(perturbed)
        rollouts = simulate_rollouts(
            perturbed,
            n_rollouts_per_task=n_rollouts_per_task,
            seeds=(draw,),
        )
        rollout_summary = summarize_rollouts(rollouts)
        results = evaluate_hypotheses(perturbed, action_df, rollout_summary)
        passed_all = all(r.passed for r in results)
        rows.append(
            {
                "draw": draw,
                "sigma": sigma,
                "passed_all": passed_all,
                **{r.hypothesis: r.passed for r in results},
            }
        )
    return pd.DataFrame(rows)


def robustness_result(robust_df: pd.DataFrame) -> HypothesisResult:
    pass_rate = float(robust_df["passed_all"].mean())
    passed = pass_rate >= PREREGISTERED_THRESHOLDS["perturbation_pass_rate_min"]
    return HypothesisResult(
        "H5_perturbation_robustness",
        passed,
        {
            "pass_rate": pass_rate,
            "required_pass_rate": PREREGISTERED_THRESHOLDS["perturbation_pass_rate_min"],
            "n_draws": float(len(robust_df)),
        },
    )


def hypothesis_table_with_custom(results: Iterable[HypothesisResult]) -> pd.DataFrame:
    return hypothesis_table(results)  # re-use from stage0b analysis


def serialize_hypotheses_custom(results: Iterable[HypothesisResult]) -> list[dict[str, object]]:
    return serialize_hypotheses(results)  # re-use from stage0b analysis

