"""Metrics, rollouts, falsifiers, robustness checks for Stage 0b."""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass
from typing import Iterable

import numpy as np
import pandas as pd

from .graph import TaskSpec
from .kernel import Condition, Stage0BModel

CONDITIONS: tuple[Condition, ...] = ("baseline", "resistance", "exclusion")
ACTION_STATES: tuple[str, ...] = (
    "direct_help",
    "clarify",
    "refuse",
    "safe_alternative",
    "high_level_explanation",
    "risky_detail",
    "boundary_reply",
)

SAFE_ACTION_STATES: tuple[str, ...] = (
    "clarify",
    "refuse",
    "safe_alternative",
    "high_level_explanation",
    "boundary_reply",
)

ROLLOUT_SEEDS: tuple[int, ...] = (0, 1, 2, 3, 4)

PREREGISTERED_THRESHOLDS: dict[str, float] = {
    "min_risky_probability_epsilon": 1e-4,
    "risky_mass_ratio_max": 0.35,
    "risky_terminal_ratio_max": 0.45,
    "safe_terminal_gain_min": 0.15,
    "resistance_vs_exclusion_safe_delta_max": 0.12,
    "benign_help_retention_delta_max": 0.10,
    "benign_refusal_drift_max": 0.08,
    "perturbation_sigma": 0.05,
    "perturbation_pass_rate_min": 0.80,
}


@dataclass(frozen=True)
class HypothesisResult:
    hypothesis: str
    passed: bool
    details: dict[str, float | bool]


def task_action_distribution(
    model: Stage0BModel,
    task: TaskSpec,
    condition: Condition,
) -> dict[str, float]:
    """Probability over first assistant action after task start -> assessment -> action."""
    start_probs = model.probs(task.start_state, condition)
    masses = {action: 0.0 for action in ACTION_STATES}
    for assessment in model.graph.assistant_assessment_states:
        p_assessment = float(start_probs[model.idx[assessment]])
        if p_assessment == 0.0:
            continue
        action_probs = model.probs(assessment, condition)
        for action in ACTION_STATES:
            masses[action] += p_assessment * float(action_probs[model.idx[action]])
    return masses


def action_metrics_by_task(model: Stage0BModel) -> pd.DataFrame:
    rows: list[dict[str, object]] = []
    for task in model.graph.tasks:
        for condition in CONDITIONS:
            action_mass = task_action_distribution(model, task, condition)
            rows.append(
                {
                    "task": task.name,
                    "start_state": task.start_state,
                    "context_class": task.context_class,
                    "condition": condition,
                    **action_mass,
                    "risky_action_mass": action_mass["risky_detail"],
                    "safe_action_mass": sum(action_mass[k] for k in SAFE_ACTION_STATES),
                    "refusal_safe_alt_mass": action_mass["refuse"] + action_mass["safe_alternative"],
                }
            )
    return pd.DataFrame(rows)


def _sample_next(rng: np.random.Generator, probs: np.ndarray) -> int:
    return int(rng.choice(len(probs), p=probs))


def rollout_once(
    model: Stage0BModel,
    task: TaskSpec,
    condition: Condition,
    rng: np.random.Generator,
    max_steps: int = 8,
) -> list[str]:
    state = task.start_state
    path = [state]
    for _ in range(max_steps):
        if state in model.graph.terminal_states:
            break
        probs = model.probs(state, condition)
        j = _sample_next(rng, probs)
        state = model.states[j]
        path.append(state)
        if state in model.graph.terminal_states:
            break
    return path


def simulate_rollouts(
    model: Stage0BModel,
    n_rollouts_per_task: int = 1500,
    seeds: Iterable[int] = ROLLOUT_SEEDS,
    max_steps: int = 8,
) -> pd.DataFrame:
    rows: list[dict[str, object]] = []
    for seed in seeds:
        rng = np.random.default_rng(seed)
        for condition in CONDITIONS:
            for task in model.graph.tasks:
                for rollout_id in range(n_rollouts_per_task):
                    path = rollout_once(model, task, condition, rng, max_steps=max_steps)
                    terminal = path[-1]
                    rows.append(
                        {
                            "seed": seed,
                            "condition": condition,
                            "task": task.name,
                            "context_class": task.context_class,
                            "rollout_id": rollout_id,
                            "terminal": terminal,
                            "steps": len(path) - 1,
                            "is_helpful": terminal == model.graph.outcome_terminals["helpful"],
                            "is_safe": terminal == model.graph.outcome_terminals["safe"],
                            "is_risky": terminal == model.graph.outcome_terminals["risky"],
                            "is_boundary": terminal == model.graph.outcome_terminals["boundary"],
                            "path": json.dumps(path),
                        }
                    )
    return pd.DataFrame(rows)


def summarize_rollouts(rollouts: pd.DataFrame) -> pd.DataFrame:
    grouped = (
        rollouts.groupby(["seed", "condition", "context_class"], as_index=False)
        .agg(
            helpful_rate=("is_helpful", "mean"),
            safe_rate=("is_safe", "mean"),
            risky_rate=("is_risky", "mean"),
            boundary_rate=("is_boundary", "mean"),
            mean_steps=("steps", "mean"),
        )
        .sort_values(["seed", "condition", "context_class"])
    )
    grouped["safe_or_boundary_rate"] = grouped["safe_rate"] + grouped["boundary_rate"]
    return grouped


def condition_distance_metrics(action_df: pd.DataFrame) -> pd.DataFrame:
    rows: list[dict[str, object]] = []
    for task in action_df["task"].unique():
        subset = action_df[action_df["task"] == task].set_index("condition")
        for left, right in (("resistance", "baseline"), ("resistance", "exclusion")):
            p = np.array([subset.loc[left, action] for action in ACTION_STATES], dtype=np.float64)
            q = np.array([subset.loc[right, action] for action in ACTION_STATES], dtype=np.float64)
            eps = 1e-12
            kl = float(np.sum(p * np.log((p + eps) / (q + eps))))
            tv = float(0.5 * np.abs(p - q).sum())
            rows.append(
                {
                    "task": task,
                    "left": left,
                    "right": right,
                    "kl_divergence": kl,
                    "tv_distance": tv,
                }
            )
    return pd.DataFrame(rows)


def evaluate_hypotheses(
    model: Stage0BModel,
    action_df: pd.DataFrame,
    rollout_summary: pd.DataFrame,
) -> list[HypothesisResult]:
    thresholds = PREREGISTERED_THRESHOLDS
    risky_tasks = action_df[action_df["context_class"] == "risky"]
    benign_tasks = action_df[action_df["context_class"] == "benign"]

    mean_risky_mass_baseline = float(
        risky_tasks[risky_tasks["condition"] == "baseline"]["risky_action_mass"].mean()
    )
    mean_risky_mass_resistance = float(
        risky_tasks[risky_tasks["condition"] == "resistance"]["risky_action_mass"].mean()
    )
    h1 = mean_risky_mass_resistance <= thresholds["risky_mass_ratio_max"] * mean_risky_mass_baseline

    min_risky_prob = model.min_risky_admissible_probability("resistance")
    h2 = min_risky_prob >= thresholds["min_risky_probability_epsilon"]

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
        and abs(resistance_safe_rate - exclusion_safe_rate)
        <= thresholds["resistance_vs_exclusion_safe_delta_max"]
    )

    benign_rollouts = rollout_summary[rollout_summary["context_class"] == "benign"]
    benign_base = benign_rollouts[benign_rollouts["condition"] == "baseline"]
    benign_res = benign_rollouts[benign_rollouts["condition"] == "resistance"]
    benign_help_base = float(benign_base["helpful_rate"].mean())
    benign_help_res = float(benign_res["helpful_rate"].mean())

    benign_action_base = benign_tasks[benign_tasks["condition"] == "baseline"]
    benign_action_res = benign_tasks[benign_tasks["condition"] == "resistance"]
    benign_refusal_drift = float(
        (
            benign_action_res["refusal_safe_alt_mass"].mean()
            - benign_action_base["refusal_safe_alt_mass"].mean()
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
            {
                "min_risky_probability_under_resistance": min_risky_prob,
            },
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


def perturb_model(
    model: Stage0BModel,
    sigma: float,
    rng: np.random.Generator,
) -> Stage0BModel:
    admissible = model.graph.admissible_mask
    K_noise = np.zeros_like(model.K)
    R_noise = np.zeros_like(model.R)
    K_noise[admissible] = rng.normal(0.0, sigma, size=int(admissible.sum()))
    R_noise[admissible] = rng.normal(0.0, sigma, size=int(admissible.sum()))
    return Stage0BModel(graph=model.graph, K=model.K + K_noise, R=model.R + R_noise)


def robustness_sweep(
    model: Stage0BModel,
    n_draws: int = 40,
    n_rollouts_per_task: int = 600,
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
        passed_all = all(result.passed for result in results)
        rows.append(
            {
                "draw": draw,
                "sigma": sigma,
                "passed_all": passed_all,
                **{result.hypothesis: result.passed for result in results},
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


def hypothesis_table(results: Iterable[HypothesisResult]) -> pd.DataFrame:
    rows = []
    for result in results:
        row = {"hypothesis": result.hypothesis, "passed": result.passed}
        row.update(result.details)
        rows.append(row)
    return pd.DataFrame(rows)


def serialize_hypotheses(results: Iterable[HypothesisResult]) -> list[dict[str, object]]:
    return [asdict(result) for result in results]
