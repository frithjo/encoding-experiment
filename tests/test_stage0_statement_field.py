"""Regression tests: Stage 0 numerics locked to the archived notebook transcript."""

import numpy as np
import pytest

from experiments.stage0_statement_field.field import StatementField


@pytest.fixture
def field() -> StatementField:
    return StatementField.default()


def test_s6_main_table(field: StatementField) -> None:
    """Archived table for S6 (unsafe assessed)."""
    rows = {round(q, 1): field.probs_from("S6", q) for q in (0.0, 0.5, 1.0)}
    g = field.group_definitions()

    def mass(p, name: str) -> float:
        return float(np.sum(p[g[name]]))

    # q=0
    p0 = rows[0.0]
    assert mass(p0, "safe_region") == pytest.approx(0.509, abs=1e-3)
    assert mass(p0, "risky_region") == pytest.approx(0.126, abs=1e-3)
    assert mass(p0, "refuse") == pytest.approx(0.200, abs=1e-3)
    assert mass(p0, "safe_alt") == pytest.approx(0.163, abs=1e-3)
    assert mass(p0, "high_level") == pytest.approx(0.110, abs=1e-3)
    assert mass(p0, "direct_help") == pytest.approx(0.036, abs=1e-3)
    assert mass(p0, "unsafe_detail") == pytest.approx(0.090, abs=1e-3)

    p1 = rows[1.0]
    assert mass(p1, "safe_region") == pytest.approx(0.818, abs=1e-3)
    assert mass(p1, "risky_region") == pytest.approx(0.002, abs=1e-3)
    assert mass(p1, "refuse") == pytest.approx(0.401, abs=1e-3)
    assert mass(p1, "direct_help") == pytest.approx(0.001, abs=1e-3)
    assert mass(p1, "unsafe_detail") == pytest.approx(0.000, abs=1e-3)


def test_s5_ambiguous_clarify(field: StatementField) -> None:
    p0 = field.probs_from("S5", 0.0)
    p1 = field.probs_from("S5", 1.0)
    ix = field.idx
    assert float(p0[ix["S8"]]) == pytest.approx(0.360, abs=1e-3)
    assert float(p1[ix["S8"]]) == pytest.approx(0.624, abs=1e-3)


def test_s4_benign_direct_help(field: StatementField) -> None:
    ix = field.idx
    p0 = field.probs_from("S4", 0.0)
    p1 = field.probs_from("S4", 1.0)
    assert float(p0[ix["S7"]]) == pytest.approx(0.487, abs=1e-3)
    assert float(p1[ix["S7"]]) == pytest.approx(0.535, abs=1e-3)


def test_canonical_path_from_s2(field: StatementField) -> None:
    path = ["S2", "S6", "S9", "S10"]
    assert field.path_prob(path, 0.0) == pytest.approx(0.0230, abs=1e-4)
    assert field.path_prob(path, 0.5) == pytest.approx(0.0521, abs=1e-4)
    assert field.path_prob(path, 1.0) == pytest.approx(0.0845, abs=1e-4)


def test_no_hard_zero_probabilities(field: StatementField) -> None:
    """Every transition should remain possible at q=1 (toy claim: no banned edges)."""
    pmin = field.global_min_prob_at_q(1.0)
    assert pmin > 0.0
