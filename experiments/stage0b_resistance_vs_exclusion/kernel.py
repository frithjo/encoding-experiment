"""Stage 0b kernel: baseline, resistance, exclusion over fixed admissibility support."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

import numpy as np

from .graph import GraphSpec, build_graph_spec

Condition = Literal["baseline", "resistance", "exclusion"]


def masked_softmax(logits: np.ndarray, allowed_mask: np.ndarray) -> np.ndarray:
    masked = np.where(allowed_mask, logits, -np.inf)
    if not np.any(allowed_mask):
        raise ValueError("state has no admissible outgoing edges")
    max_logit = np.max(masked[allowed_mask])
    stable = masked - max_logit
    exp = np.where(allowed_mask, np.exp(stable), 0.0)
    return exp / exp.sum()


@dataclass(frozen=True)
class Stage0BModel:
    graph: GraphSpec
    K: np.ndarray
    R: np.ndarray

    @classmethod
    def default(cls) -> Stage0BModel:
        graph = build_graph_spec()
        K, R = build_K_R(graph)
        return cls(graph=graph, K=K, R=R)

    @property
    def states(self) -> tuple[str, ...]:
        return self.graph.states

    @property
    def idx(self) -> dict[str, int]:
        return self.graph.idx

    @property
    def n(self) -> int:
        return len(self.graph.states)

    def charge(self, state: str) -> float:
        return self.graph.charge_by_state.get(state, 0.0)

    def admissible_mask(self, state: str, condition: Condition) -> np.ndarray:
        i = self.idx[state]
        mask = self.graph.admissible_mask[i].copy()
        if condition == "exclusion":
            mask &= ~self.graph.risky_edge_mask[i]
        return mask

    def logits(self, state: str, condition: Condition) -> np.ndarray:
        i = self.idx[state]
        logits = self.K[i].copy()
        if condition == "resistance":
            logits = logits - self.charge(state) * self.R[i]
        return logits

    def probs(self, state: str, condition: Condition) -> np.ndarray:
        mask = self.admissible_mask(state, condition)
        return masked_softmax(self.logits(state, condition), mask)

    def support(self, state: str, condition: Condition) -> set[str]:
        mask = self.admissible_mask(state, condition)
        return {self.states[j] for j in np.where(mask)[0]}

    def edge_prob(self, src: str, dst: str, condition: Condition) -> float:
        return float(self.probs(src, condition)[self.idx[dst]])

    def min_risky_admissible_probability(self, condition: Condition) -> float:
        vals: list[float] = []
        for src in self.graph.assistant_assessment_states:
            i = self.idx[src]
            risky_dsts = np.where(self.graph.risky_edge_mask[i])[0]
            if risky_dsts.size == 0:
                continue
            p = self.probs(src, condition)
            vals.extend(float(p[j]) for j in risky_dsts)
        if not vals:
            raise ValueError("no risky admissible edges defined")
        return min(vals)

    def risky_support_nonzero(self) -> bool:
        return self.min_risky_admissible_probability("resistance") > 0.0


def build_K_R(graph: GraphSpec) -> tuple[np.ndarray, np.ndarray]:
    n = len(graph.states)
    idx = graph.idx
    K = np.zeros((n, n), dtype=np.float64)
    R = np.zeros((n, n), dtype=np.float64)

    def setK(src: str, dst: str, val: float) -> None:
        K[idx[src], idx[dst]] = val

    def setR(src: str, dst: str, val: float) -> None:
        R[idx[src], idx[dst]] = val

    # Start-state routing
    setK("benign_request_simple", "benign_assessed", 1.8)
    setK("benign_request_simple", "ambiguous_assessed", 0.7)
    setK("benign_request_complex", "benign_assessed", 1.5)
    setK("benign_request_complex", "ambiguous_assessed", 1.0)
    setK("ambiguous_request", "ambiguous_assessed", 1.6)
    setK("ambiguous_request", "benign_assessed", 0.9)
    setK("ambiguous_request", "risky_assessed", 0.8)
    setK("risky_request_direct", "risky_assessed", 1.5)
    setK("risky_request_direct", "ambiguous_assessed", 1.0)
    setK("risky_request_reframed", "risky_assessed", 1.3)
    setK("risky_request_reframed", "boundary_assessed", 1.1)
    setK("risky_request_reframed", "ambiguous_assessed", 0.8)
    setK("boundary_reframe", "boundary_assessed", 1.6)
    setK("boundary_reframe", "risky_assessed", 0.9)

    # Assistant policy states
    setK("benign_assessed", "direct_help", 1.9)
    setK("benign_assessed", "clarify", 0.9)
    setK("benign_assessed", "high_level_explanation", 0.7)

    setK("ambiguous_assessed", "clarify", 1.6)
    setK("ambiguous_assessed", "high_level_explanation", 1.1)
    setK("ambiguous_assessed", "direct_help", 1.0)
    setK("ambiguous_assessed", "safe_alternative", 0.8)

    setK("risky_assessed", "refuse", 1.3)
    setK("risky_assessed", "safe_alternative", 1.2)
    setK("risky_assessed", "high_level_explanation", 1.0)
    setK("risky_assessed", "risky_detail", 1.15)

    setK("boundary_assessed", "boundary_reply", 1.4)
    setK("boundary_assessed", "refuse", 1.2)
    setK("boundary_assessed", "safe_alternative", 1.1)
    setK("boundary_assessed", "high_level_explanation", 0.9)
    setK("boundary_assessed", "risky_detail", 1.05)

    # Follow-up / environment dynamics
    setK("direct_help", "helpful_done", 2.0)
    setK("clarify", "user_clarifies_safe", 1.3)
    setK("clarify", "user_presses_risky", 0.9)
    setK("refuse", "safe_done", 1.3)
    setK("refuse", "user_presses_risky", 0.7)
    setK("safe_alternative", "safe_done", 1.5)
    setK("safe_alternative", "user_presses_risky", 0.6)
    setK("high_level_explanation", "safe_done", 1.1)
    setK("high_level_explanation", "user_presses_risky", 0.9)
    setK("risky_detail", "risky_done", 2.0)
    setK("boundary_reply", "boundary_done", 1.4)
    setK("boundary_reply", "user_presses_risky", 0.6)
    setK("user_clarifies_safe", "benign_assessed", 1.7)
    setK("user_presses_risky", "risky_assessed", 1.7)

    # Resistance field: same admissible support, different weights only.
    setR("benign_assessed", "direct_help", 0.15)
    setR("benign_assessed", "clarify", -0.05)

    setR("ambiguous_assessed", "direct_help", 0.7)
    setR("ambiguous_assessed", "clarify", -0.5)
    setR("ambiguous_assessed", "safe_alternative", -0.2)
    setR("ambiguous_assessed", "high_level_explanation", -0.1)

    setR("risky_assessed", "risky_detail", 3.4)
    setR("risky_assessed", "refuse", -1.0)
    setR("risky_assessed", "safe_alternative", -0.8)
    setR("risky_assessed", "high_level_explanation", -0.3)

    setR("boundary_assessed", "risky_detail", 3.6)
    setR("boundary_assessed", "boundary_reply", -0.9)
    setR("boundary_assessed", "refuse", -0.7)
    setR("boundary_assessed", "safe_alternative", -0.5)
    setR("boundary_assessed", "high_level_explanation", -0.2)

    return K, R
