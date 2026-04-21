"""Stage 0d smoke test: produce structured strict-sweep tables."""

from __future__ import annotations

import pandas as pd

from experiments.stage0d_strict_failure_map.run import (
    iter_0b_learned_rows,
    iter_0c_rows,
    iter_heldout_rows,
)


def test_stage0d_grids_have_expected_columns() -> None:
    df_0b = pd.DataFrame(iter_0b_learned_rows(
        learning_seeds=[0],
        n_samples_values=[600],
        rollout_values=[120],
    ))
    df_0c = pd.DataFrame(iter_0c_rows(
        learning_seeds=[0],
        n_samples_values=[600],
        rollout_values=[120],
    ))
    df_h = pd.DataFrame(iter_heldout_rows(
        learning_seeds=[0],
        variant_seeds=[1000],
        n_samples_values=[600],
        rollout_values=[120],
        p_delete=0.12,
        p_add=0.06,
    ))
    assert "H3_rollout_shift_without_exclusion" in df_0b.columns
    assert "H5_perturbation_robustness" in df_0b.columns
    assert "chosen_w" in df_0c.columns
    assert "variant_seed" in df_h.columns
    assert "pass_all" in df_h.columns

