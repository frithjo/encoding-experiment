"""Learn baseline kernel K from synthetic traces on the fixed admissibility graph.

Stage 0b-learned keeps:
- the admissibility graph (what edges are structurally available)
- the resistance field R (structural discouragement)
and only changes:
- the baseline continuation kernel K, learned from synthetic traces
  using the baseline policy (q=0).
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np

from experiments.stage0b_resistance_vs_exclusion.kernel import masked_softmax
from experiments.stage0b_resistance_vs_exclusion.graph import build_graph_spec


@dataclass(frozen=True)
class LearningConfig:
    n_samples_per_state: int = 4000
    alpha_smoothing: float = 1e-2
    k_true_scale: float = 1.0


def build_r_fixed() -> np.ndarray:
    """Extract the fixed resistance matrix R from the stage0b kernel definition."""
    from experiments.stage0b_resistance_vs_exclusion.kernel import build_K_R  # local import

    graph = build_graph_spec()
    _, R = build_K_R(graph)
    return R


def generate_k_true(
    *,
    rng: np.random.Generator,
    graph_admissible_mask: np.ndarray,
    scale: float,
) -> np.ndarray:
    """Generate a latent baseline kernel K_true with random weights on admissible edges."""
    n_states = graph_admissible_mask.shape[0]
    K_true = np.zeros((n_states, n_states), dtype=np.float64)
    idxs = np.argwhere(graph_admissible_mask)
    # K_true[src,dst] for admissible edges; dense logits are okay because inadmissible masked later.
    K_true[idxs[:, 0], idxs[:, 1]] = rng.normal(0.0, scale, size=idxs.shape[0])
    return K_true


def sample_traces(
    *,
    rng: np.random.Generator,
    graph_admissible_mask: np.ndarray,
    K_true: np.ndarray,
    n_samples_per_state: int,
) -> np.ndarray:
    """Return transition counts C[src,dst] sampled from the baseline policy softmax(K_true) over admissible edges."""
    n_states = graph_admissible_mask.shape[0]
    counts = np.zeros((n_states, n_states), dtype=np.int64)
    for src in range(n_states):
        allowed = graph_admissible_mask[src]
        allowed_idxs = np.where(allowed)[0]
        if allowed_idxs.size == 0:
            continue
        logits = K_true[src].copy()
        probs = masked_softmax(logits, allowed)
        # sample dst under baseline policy
        dst_samples = rng.choice(allowed_idxs, size=n_samples_per_state, p=probs[allowed_idxs])
        # accumulate counts
        for dst in dst_samples:
            counts[src, dst] += 1
    return counts


def estimate_k_from_counts(counts: np.ndarray, admissible_mask: np.ndarray, alpha: float) -> np.ndarray:
    """Estimate K_hat = log(count + alpha) on admissible edges (others set to 0)."""
    K_hat = np.zeros_like(counts, dtype=np.float64)
    idxs = np.argwhere(admissible_mask)
    K_hat[idxs[:, 0], idxs[:, 1]] = np.log(counts[idxs[:, 0], idxs[:, 1]].astype(np.float64) + alpha)
    return K_hat


def learn_kernel(
    *,
    learning_seed: int,
    config: LearningConfig,
) -> tuple[np.ndarray, np.ndarray, dict[str, int]]:
    """Learn K_hat from synthetic traces; return (K_hat, R_fixed, idx)."""
    graph = build_graph_spec()
    rng = np.random.default_rng(learning_seed)
    R_fixed = build_r_fixed()

    K_true = generate_k_true(
        rng=rng,
        graph_admissible_mask=graph.admissible_mask,
        scale=config.k_true_scale,
    )
    counts = sample_traces(
        rng=rng,
        graph_admissible_mask=graph.admissible_mask,
        K_true=K_true,
        n_samples_per_state=config.n_samples_per_state,
    )
    K_hat = estimate_k_from_counts(counts, graph.admissible_mask, config.alpha_smoothing)
    return K_hat, R_fixed, graph.idx

