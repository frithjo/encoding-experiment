from __future__ import annotations

from pathlib import Path

import pandas as pd

from experiments.stage0m_activation_band_replication.run import (
    _feature_to_edge,
    load_activation_cells,
    summarize_features,
    summarize_replication,
)


def test_feature_to_edge() -> None:
    assert _feature_to_edge("edge__foo__bar") == ("foo", "bar")


def test_replication_summary_sign_match(tmp_path: Path) -> None:
    activation_csv = tmp_path / "cells.csv"
    pd.DataFrame(
        [
            {
                "target_feature": "edge__foo__bar",
                "learning_seed": 0,
                "n_samples_per_state": 600,
                "cell_state": "edge_sensitive",
                "present_minus_absent_H3_rate": 0.25,
            },
            {
                "target_feature": "edge__foo__bar",
                "learning_seed": 1,
                "n_samples_per_state": 600,
                "cell_state": "all_fail_tied",
                "present_minus_absent_H3_rate": 0.0,
            },
        ]
    ).to_csv(activation_csv, index=False)

    selected = load_activation_cells(activation_csv)
    assert len(selected) == 1
    assert int(selected.iloc[0]["original_sign"]) == 1

    pair_df = pd.DataFrame(
        [
            {
                "target_feature": "edge__foo__bar",
                "learning_seed": 0,
                "n_samples_per_state": 600,
                "template_seed": 3000,
                "present_H3": True,
                "absent_H3": False,
                "baseline_safe_present_minus_absent": 0.0,
                "safe_gain_present_minus_absent": 0.1,
                "risky_drop_present_minus_absent": 0.0,
                "original_present_minus_absent_H3_rate": 0.25,
                "original_sign": 1,
            },
            {
                "target_feature": "edge__foo__bar",
                "learning_seed": 0,
                "n_samples_per_state": 600,
                "template_seed": 3001,
                "present_H3": False,
                "absent_H3": False,
                "baseline_safe_present_minus_absent": 0.0,
                "safe_gain_present_minus_absent": 0.0,
                "risky_drop_present_minus_absent": 0.0,
                "original_present_minus_absent_H3_rate": 0.25,
                "original_sign": 1,
            },
        ]
    )

    cell_df = summarize_replication(pair_df)
    feature_df = summarize_features(cell_df)

    assert len(cell_df) == 1
    assert bool(cell_df.iloc[0]["sign_match"]) is True
    assert float(cell_df.iloc[0]["replicate_present_minus_absent_H3_rate"]) == 0.5
    assert len(feature_df) == 1
    assert float(feature_df.iloc[0]["sign_match_rate"]) == 1.0

