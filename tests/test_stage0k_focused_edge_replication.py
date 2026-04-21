from __future__ import annotations

from pathlib import Path

import pandas as pd

from experiments.stage0k_focused_edge_replication.run import main


def test_stage0k_smoke_minimal_grid(tmp_path: Path, monkeypatch: object) -> None:
    """Tiny grid: one seed, one n_samples, two templates, both edges."""
    import sys

    out = tmp_path / "results"
    argv = [
        "stage0k",
        "--out",
        str(out),
        "--learning-seeds",
        "0",
        "--n-samples-values",
        "300",
        "--template-seeds",
        "2000,2001",
        "--eval-rollouts",
        "60",
    ]
    monkeypatch.setattr(sys, "argv", argv)
    code = main()
    assert code == 0
    df = pd.read_csv(out / "constructed_pair_evaluations.csv")
    assert len(df) == 4
    assert set(df["target_feature"].unique()) == {
        "edge__benign_assessed__clarify",
        "edge__risky_assessed__refuse",
    }
