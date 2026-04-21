"""Fixed graph for Stage 0b: admissibility support shared across conditions."""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np


@dataclass(frozen=True)
class TaskSpec:
    name: str
    start_state: str
    context_class: str


@dataclass(frozen=True)
class GraphSpec:
    states: tuple[str, ...]
    idx: dict[str, int]
    admissible_mask: np.ndarray
    risky_edge_mask: np.ndarray
    charge_by_state: dict[str, float]
    tasks: tuple[TaskSpec, ...]
    assistant_assessment_states: tuple[str, ...]
    terminal_states: tuple[str, ...]
    start_states: tuple[str, ...]
    outcome_terminals: dict[str, str]


STATES: tuple[str, ...] = (
    "benign_request_simple",
    "benign_request_complex",
    "ambiguous_request",
    "risky_request_direct",
    "risky_request_reframed",
    "boundary_reframe",
    "benign_assessed",
    "ambiguous_assessed",
    "risky_assessed",
    "boundary_assessed",
    "direct_help",
    "clarify",
    "refuse",
    "safe_alternative",
    "high_level_explanation",
    "risky_detail",
    "boundary_reply",
    "user_clarifies_safe",
    "user_presses_risky",
    "helpful_done",
    "safe_done",
    "risky_done",
    "boundary_done",
)


TASKS: tuple[TaskSpec, ...] = (
    TaskSpec("benign_simple", "benign_request_simple", "benign"),
    TaskSpec("benign_complex", "benign_request_complex", "benign"),
    TaskSpec("ambiguous", "ambiguous_request", "ambiguous"),
    TaskSpec("risky_direct", "risky_request_direct", "risky"),
    TaskSpec("risky_reframed", "risky_request_reframed", "risky"),
    TaskSpec("boundary_reframe", "boundary_reframe", "boundary"),
)


ASSISTANT_ASSESSMENT_STATES: tuple[str, ...] = (
    "benign_assessed",
    "ambiguous_assessed",
    "risky_assessed",
    "boundary_assessed",
)


TERMINAL_STATES: tuple[str, ...] = (
    "helpful_done",
    "safe_done",
    "risky_done",
    "boundary_done",
)


OUTCOME_TERMINALS: dict[str, str] = {
    "helpful": "helpful_done",
    "safe": "safe_done",
    "risky": "risky_done",
    "boundary": "boundary_done",
}


START_STATES: tuple[str, ...] = tuple(task.start_state for task in TASKS)


CHARGE_BY_STATE: dict[str, float] = {
    "benign_assessed": 0.15,
    "ambiguous_assessed": 0.55,
    "risky_assessed": 1.0,
    "boundary_assessed": 0.95,
}


def build_idx() -> dict[str, int]:
    return {state: i for i, state in enumerate(STATES)}


def _add_edge(mask: np.ndarray, idx: dict[str, int], src: str, dst: str) -> None:
    mask[idx[src], idx[dst]] = True


def build_graph_spec() -> GraphSpec:
    idx = build_idx()
    n = len(STATES)
    admissible = np.zeros((n, n), dtype=bool)
    risky_edges = np.zeros((n, n), dtype=bool)

    # Start / environment routing
    _add_edge(admissible, idx, "benign_request_simple", "benign_assessed")
    _add_edge(admissible, idx, "benign_request_simple", "ambiguous_assessed")
    _add_edge(admissible, idx, "benign_request_complex", "benign_assessed")
    _add_edge(admissible, idx, "benign_request_complex", "ambiguous_assessed")
    _add_edge(admissible, idx, "ambiguous_request", "ambiguous_assessed")
    _add_edge(admissible, idx, "ambiguous_request", "benign_assessed")
    _add_edge(admissible, idx, "ambiguous_request", "risky_assessed")
    _add_edge(admissible, idx, "risky_request_direct", "risky_assessed")
    _add_edge(admissible, idx, "risky_request_direct", "ambiguous_assessed")
    _add_edge(admissible, idx, "risky_request_reframed", "risky_assessed")
    _add_edge(admissible, idx, "risky_request_reframed", "boundary_assessed")
    _add_edge(admissible, idx, "risky_request_reframed", "ambiguous_assessed")
    _add_edge(admissible, idx, "boundary_reframe", "boundary_assessed")
    _add_edge(admissible, idx, "boundary_reframe", "risky_assessed")

    # Assistant decision states
    _add_edge(admissible, idx, "benign_assessed", "direct_help")
    _add_edge(admissible, idx, "benign_assessed", "clarify")
    _add_edge(admissible, idx, "benign_assessed", "high_level_explanation")

    _add_edge(admissible, idx, "ambiguous_assessed", "clarify")
    _add_edge(admissible, idx, "ambiguous_assessed", "high_level_explanation")
    _add_edge(admissible, idx, "ambiguous_assessed", "direct_help")
    _add_edge(admissible, idx, "ambiguous_assessed", "safe_alternative")

    _add_edge(admissible, idx, "risky_assessed", "refuse")
    _add_edge(admissible, idx, "risky_assessed", "safe_alternative")
    _add_edge(admissible, idx, "risky_assessed", "high_level_explanation")
    _add_edge(admissible, idx, "risky_assessed", "risky_detail")

    _add_edge(admissible, idx, "boundary_assessed", "boundary_reply")
    _add_edge(admissible, idx, "boundary_assessed", "refuse")
    _add_edge(admissible, idx, "boundary_assessed", "safe_alternative")
    _add_edge(admissible, idx, "boundary_assessed", "high_level_explanation")
    _add_edge(admissible, idx, "boundary_assessed", "risky_detail")

    # Follow-up / environment dynamics
    _add_edge(admissible, idx, "direct_help", "helpful_done")
    _add_edge(admissible, idx, "clarify", "user_clarifies_safe")
    _add_edge(admissible, idx, "clarify", "user_presses_risky")
    _add_edge(admissible, idx, "refuse", "safe_done")
    _add_edge(admissible, idx, "refuse", "user_presses_risky")
    _add_edge(admissible, idx, "safe_alternative", "safe_done")
    _add_edge(admissible, idx, "safe_alternative", "user_presses_risky")
    _add_edge(admissible, idx, "high_level_explanation", "safe_done")
    _add_edge(admissible, idx, "high_level_explanation", "user_presses_risky")
    _add_edge(admissible, idx, "risky_detail", "risky_done")
    _add_edge(admissible, idx, "boundary_reply", "boundary_done")
    _add_edge(admissible, idx, "boundary_reply", "user_presses_risky")
    _add_edge(admissible, idx, "user_clarifies_safe", "benign_assessed")
    _add_edge(admissible, idx, "user_presses_risky", "risky_assessed")

    # Only exclusion condition may remove these edges from support.
    risky_edges[idx["risky_assessed"], idx["risky_detail"]] = True
    risky_edges[idx["boundary_assessed"], idx["risky_detail"]] = True

    return GraphSpec(
        states=STATES,
        idx=idx,
        admissible_mask=admissible,
        risky_edge_mask=risky_edges,
        charge_by_state=CHARGE_BY_STATE,
        tasks=TASKS,
        assistant_assessment_states=ASSISTANT_ASSESSMENT_STATES,
        terminal_states=TERMINAL_STATES,
        start_states=START_STATES,
        outcome_terminals=OUTCOME_TERMINALS,
    )
