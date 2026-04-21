"""Transition rule: P(next | current, q) = softmax(K[current] - q * R[current])."""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
import pandas as pd

from .matrices import STATEMENTS, build_K_R


def softmax(logits: np.ndarray, temp: float = 1.0) -> np.ndarray:
    x = np.asarray(logits, dtype=np.float64) / temp
    x = x - np.max(x)
    e = np.exp(x)
    return e / e.sum()


@dataclass(frozen=True)
class StatementField:
    """Toy statement field: K compatibility, R resistance, indexed statement labels."""

    K: np.ndarray
    R: np.ndarray
    idx: dict[str, int]

    @classmethod
    def default(cls) -> StatementField:
        K, R, idx = build_K_R()
        return cls(K=K, R=R, idx=idx)

    @property
    def n(self) -> int:
        return self.K.shape[0]

    def probs_from(self, src: str, q: float, temp: float = 1.0) -> np.ndarray:
        i = self.idx[src]
        return softmax(self.K[i] - q * self.R[i], temp=temp)

    def group_definitions(self) -> dict[str, list[int]]:
        ix = self.idx
        return {
            "direct_help": [ix["S7"]],
            "clarify": [ix["S8"]],
            "refuse": [ix["S9"]],
            "safe_alt": [ix["S10"]],
            "high_level": [ix["S11"]],
            "unsafe_detail": [ix["S12"]],
            "safe_region": [ix["S8"], ix["S9"], ix["S10"], ix["S11"]],
            "risky_region": [ix["S7"], ix["S12"]],
        }

    def group_mass(self, p: np.ndarray, group: str) -> float:
        g = self.group_definitions()[group]
        return float(np.sum(p[g]))

    def entropy_bits(self, p: np.ndarray) -> float:
        return float(-sum(pi * math.log2(pi) for pi in p if pi > 0))

    def sweep_sources(
        self,
        sources: list[str],
        qs: np.ndarray | list[float],
    ) -> pd.DataFrame:
        rows = []
        groups = self.group_definitions()
        for src in sources:
            for q in qs:
                p = self.probs_from(src, float(q))
                rows.append(
                    {
                        "source": src,
                        "q": round(float(q), 2),
                        "safe_region": self.group_mass(p, "safe_region"),
                        "risky_region": self.group_mass(p, "risky_region"),
                        "clarify": self.group_mass(p, "clarify"),
                        "refuse": self.group_mass(p, "refuse"),
                        "safe_alt": self.group_mass(p, "safe_alt"),
                        "high_level": self.group_mass(p, "high_level"),
                        "direct_help": self.group_mass(p, "direct_help"),
                        "unsafe_detail": self.group_mass(p, "unsafe_detail"),
                        "entropy_bits": self.entropy_bits(p),
                    }
                )
        return pd.DataFrame(rows)

    def top_probs(self, src: str, q: float, k: int = 6) -> list[tuple[str, float]]:
        p = self.probs_from(src, q)
        order = np.argsort(-p)[:k]
        return [(STATEMENTS[j], float(p[j])) for j in order]

    def path_prob(self, path: list[str], q: float) -> float:
        pr = 1.0
        for a, b in zip(path, path[1:]):
            pr *= float(self.probs_from(a, q)[self.idx[b]])
        return pr

    def top_paths(
        self,
        start: str,
        q: float,
        steps: int = 3,
        topn: int = 10,
        beam: int = 2000,
    ) -> list[tuple[list[str], float]]:
        """Greedy beam search over length-(steps+1) paths (matches archived experiment)."""
        paths: list[tuple[list[str], float]] = [([start], 1.0)]
        for _ in range(steps):
            new: list[tuple[list[str], float]] = []
            for path, pr in paths:
                p = self.probs_from(path[-1], q)
                for j in range(self.n):
                    sj = f"S{j}"
                    new.append((path + [sj], pr * float(p[j])))
            new.sort(key=lambda x: -x[1])
            paths = new[:beam]
        paths.sort(key=lambda x: -x[1])
        return [(path, pr) for path, pr in paths[:topn]]

    def global_min_prob_at_q(self, q: float) -> float:
        return min(float(self.probs_from(f"S{i}", q).min()) for i in range(self.n))

    def s2_first_step_metrics(self, q: float) -> dict[str, float]:
        p = self.probs_from("S2", q)
        ix = self.idx
        return {
            "unsafe_assess": float(p[ix["S6"]]),
            "ambig_assess": float(p[ix["S5"]]),
            "direct_help": float(p[ix["S7"]]),
            "unsafe_detail": float(p[ix["S12"]]),
            "safeish_assess_total": float(p[ix["S6"]] + p[ix["S5"]]),
        }
