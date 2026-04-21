"""Generate admissibility-mask variants for heldout generalization."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable

import numpy as np

from experiments.stage0b_resistance_vs_exclusion.graph import GraphSpec, build_graph_spec


SAFE_ACTION_DESTINATIONS: tuple[str, ...] = (
    "direct_help",
    "clarify",
    "refuse",
    "safe_alternative",
    "high_level_explanation",
    "boundary_reply",
)


@dataclass(frozen=True)
class VariantConfig:
    p_delete_safe_edges: float = 0.10
    p_add_safe_edges: float = 0.05
    # keep this small so we don't kill reachability too often
    max_resample_attempts: int = 50
    max_steps_for_reachability: int = 8


def _depth_reachable(mask: np.ndarray, start_nodes: Iterable[int], terminal_nodes: set[int], max_steps: int) -> bool:
    n = mask.shape[0]
    frontier = set(start_nodes)
    visited = set(start_nodes)
    for _ in range(max_steps + 1):
        if any(x in terminal_nodes for x in frontier):
            return True
        new_frontier: set[int] = set()
        for src in frontier:
            dsts = np.where(mask[src])[0]
            for d in dsts:
                if d not in visited:
                    new_frontier.add(int(d))
                    visited.add(int(d))
        if not new_frontier:
            return False
        frontier = new_frontier
    return False


def build_graph_variant(
    *,
    seed: int,
    config: VariantConfig,
) -> GraphSpec:
    rng = np.random.default_rng(seed)
    base = build_graph_spec()

    admissible = base.admissible_mask.copy()
    idx = base.idx

    assessment_srcs = list(base.assistant_assessment_states)
    assessment_src_idxs = [idx[s] for s in assessment_srcs]

    safe_dst_idxs = [idx[s] for s in SAFE_ACTION_DESTINATIONS if s in idx]

    # Never touch risky_detail admissibility for risky/boundary assessments:
    risky_detail_idx = idx["risky_detail"]
    protected_pairs = {(idx["risky_assessed"], risky_detail_idx), (idx["boundary_assessed"], risky_detail_idx)}

    def is_protected(src_i: int, dst_i: int) -> bool:
        return (src_i, dst_i) in protected_pairs

    terminal_idxs = {idx[t] for t in base.terminal_states}
    start_idxs = [idx[s] for s in base.start_states]

    for attempt in range(config.max_resample_attempts):
        admissible = base.admissible_mask.copy()

        # Delete safe edges randomly
        for src in assessment_src_idxs:
            for dst in safe_dst_idxs:
                if not admissible[src, dst]:
                    continue
                if is_protected(src, dst):
                    continue
                if rng.random() < config.p_delete_safe_edges:
                    admissible[src, dst] = False

        # Optionally add safe edges where previously absent
        for src in assessment_src_idxs:
            for dst in safe_dst_idxs:
                if is_protected(src, dst):
                    continue
                if admissible[src, dst]:
                    continue
                if rng.random() < config.p_add_safe_edges:
                    admissible[src, dst] = True

        # Ensure each assessment src has at least one outgoing edge
        ok = True
        for src in assessment_src_idxs:
            if not np.any(admissible[src]):
                ok = False
                break
        if not ok:
            continue

        # Ensure global reachability to terminals from start states within max steps
        if not _depth_reachable(admissible, start_idxs, terminal_idxs, config.max_steps_for_reachability):
            continue

        return GraphSpec(
            states=base.states,
            idx=base.idx,
            admissible_mask=admissible,
            risky_edge_mask=base.risky_edge_mask,
            charge_by_state=base.charge_by_state,
            tasks=base.tasks,
            assistant_assessment_states=base.assistant_assessment_states,
            terminal_states=base.terminal_states,
            start_states=base.start_states,
            outcome_terminals=base.outcome_terminals,
        )

    # If we can't find a variant, return base (honest fallback).
    return base

