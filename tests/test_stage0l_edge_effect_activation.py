from __future__ import annotations

from pathlib import Path

import pandas as pd

from experiments.stage0l_edge_effect_activation.run import (
    load_pair_table,
    summarize_cells,
    summarize_features,
)


def test_activation_state_classification(tmp_path: Path) -> None:
    csv_path = tmp_path / "pairs.csv"
    pd.DataFrame(
        [
            {
                "learning_seed": 0,
                "n_samples_per_state": 600,
                "template_seed": 1,
                "target_feature": "edge__foo__bar",
                "baseline_safe_present": 0.5,
                "baseline_safe_absent": 0.4,
                "safe_gain_present": 0.2,
                "safe_gain_absent": 0.1,
                "risky_drop_present": 0.3,
                "risky_drop_absent": 0.2,
                "present_H3": True,
                "absent_H3": False,
            },
            {
                "learning_seed": 0,
                "n_samples_per_state": 600,
                "template_seed": 2,
                "target_feature": "edge__foo__bar",
                "baseline_safe_present": 0.6,
                "baseline_safe_absent": 0.6,
                "safe_gain_present": 0.1,
                "safe_gain_absent": 0.1,
                "risky_drop_present": 0.2,
                "risky_drop_absent": 0.2,
                "present_H3": False,
                "absent_H3": False,
            },
            {
                "learning_seed": 1,
                "n_samples_per_state": 600,
                "template_seed": 3,
                "target_feature": "edge__foo__bar",
                "baseline_safe_present": 0.9,
                "baseline_safe_absent": 0.9,
                "safe_gain_present": 0.0,
                "safe_gain_absent": 0.0,
                "risky_drop_present": 0.1,
                "risky_drop_absent": 0.1,
                "present_H3": True,
                "absent_H3": True,
            },
        ]
    ).to_csv(csv_path, index=False)

    pairs_df = load_pair_table(csv_path)
    cells_df = summarize_cells(pairs_df)
    features_df = summarize_features(pairs_df, cells_df)

    cell0 = cells_df[cells_df["learning_seed"] == 0].iloc[0]
    assert cell0["cell_state"] == "edge_sensitive"
    assert cell0["present_only_h3_rate"] == 0.5
    assert cell0["both_fail_h3_rate"] == 0.5

    cell1 = cells_df[cells_df["learning_seed"] == 1].iloc[0]
    assert cell1["cell_state"] == "all_pass_tied"

    feat = features_df.iloc[0]
    assert feat["n_pairs"] == 3
    assert feat["n_cells"] == 2

