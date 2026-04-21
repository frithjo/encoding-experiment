"""Hand-specified K (compatibility) and R (resistance) for Stage 0 — frozen specification."""

from __future__ import annotations

import numpy as np

STATEMENTS: list[str] = [
    "S0: ordinary_help_request",
    "S1: ambiguous_capability_request",
    "S2: potentially_unsafe_request",
    "S3: fictional_or_roleplay_reframe",
    "S4: request_assessed_as_benign",
    "S5: request_assessed_as_ambiguous",
    "S6: request_assessed_as_unsafe",
    "S7: direct_helpful_answer",
    "S8: ask_clarifying_question",
    "S9: brief_refusal",
    "S10: safe_alternative",
    "S11: high_level_explanation",
    "S12: actionable_unsafe_detail",
    "S13: boundary_maintained_after_reframe",
    "S14: user_correction_or_new_constraint",
    "S15: update_active_world_signature",
]


def build_idx() -> dict[str, int]:
    return {s.split(":")[0]: i for i, s in enumerate(STATEMENTS)}


def build_K_R() -> tuple[np.ndarray, np.ndarray, dict[str, int]]:
    idx = build_idx()
    n = len(STATEMENTS)
    K = np.zeros((n, n), dtype=np.float64)
    R = np.zeros((n, n), dtype=np.float64)

    def setK(src: str, dst: str, val: float) -> None:
        K[idx[src], idx[dst]] = val

    def setR(src: str, dst: str, val: float) -> None:
        R[idx[src], idx[dst]] = val

    # Base continuation tendencies
    setK("S0", "S4", 2.5)
    setK("S1", "S5", 2.2)
    setK("S2", "S6", 2.2)
    setK("S3", "S13", 1.6)

    setK("S4", "S7", 2.8)
    setK("S4", "S11", 1.2)
    setK("S5", "S8", 2.3)
    setK("S5", "S11", 1.0)
    setK("S5", "S7", 0.7)
    setK("S6", "S9", 1.7)
    setK("S6", "S10", 1.5)
    setK("S6", "S11", 1.1)
    setK("S6", "S12", 0.9)
    setK("S13", "S9", 1.4)
    setK("S13", "S10", 1.4)
    setK("S13", "S11", 0.8)
    setK("S14", "S15", 2.4)
    setK("S15", "S8", 1.0)
    setK("S15", "S7", 1.0)
    setK("S15", "S9", 1.0)
    setK("S15", "S10", 1.0)

    setK("S8", "S14", 1.5)
    setK("S9", "S10", 2.0)
    setK("S9", "S11", 1.0)
    setK("S10", "S11", 1.4)
    setK("S11", "S7", 0.7)

    # Resistance profile
    setR("S6", "S12", 5.0)
    setR("S6", "S7", 2.5)
    setR("S6", "S9", -1.4)
    setR("S6", "S10", -1.2)
    setR("S6", "S11", -0.8)
    setR("S6", "S8", -0.4)

    setR("S5", "S7", 1.2)
    setR("S5", "S8", -1.0)
    setR("S5", "S11", -0.3)
    setR("S5", "S12", 3.5)

    setR("S13", "S12", 4.0)
    setR("S13", "S9", -1.0)
    setR("S13", "S10", -1.2)
    setR("S13", "S11", -0.5)

    setR("S2", "S6", -1.2)
    setR("S2", "S5", -0.4)
    setR("S2", "S7", 2.0)
    setR("S2", "S12", 4.0)

    setR("S0", "S4", -0.1)
    setR("S0", "S7", -0.1)
    setR("S4", "S7", -0.1)
    setR("S4", "S8", 0.2)
    setR("S4", "S9", 0.5)
    setR("S4", "S12", 3.5)

    setR("S14", "S15", -0.7)
    setR("S15", "S8", -0.2)

    return K, R, idx
