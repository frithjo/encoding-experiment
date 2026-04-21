from __future__ import annotations

from experiments.stage0b_heldout_generalization.graph_variants import VariantConfig
from experiments.stage0i_targeted_edge_pairs.run import (
    build_targeted_pair,
    evaluate_constructed_pairs,
    parse_target_edges,
)


def test_build_targeted_pair_only_differs_on_target_edge() -> None:
    present, absent = build_targeted_pair(
        template_seed=2000,
        target_src="boundary_assessed",
        target_dst="high_level_explanation",
        config=VariantConfig(
            p_delete_safe_edges=0.12,
            p_add_safe_edges=0.06,
            max_resample_attempts=10,
            max_steps_for_reachability=8,
        ),
    )
    src_i = present.idx["boundary_assessed"]
    dst_i = present.idx["high_level_explanation"]

    diff = present.admissible_mask != absent.admissible_mask
    assert int(diff.sum()) == 1
    assert present.admissible_mask[src_i, dst_i]
    assert not absent.admissible_mask[src_i, dst_i]


def test_evaluate_constructed_pairs_columns() -> None:
    df = evaluate_constructed_pairs(
        learning_seed=0,
        n_samples_per_state=300,
        template_seeds=[2000, 2001],
        target_src="boundary_assessed",
        target_dst="high_level_explanation",
        p_delete=0.12,
        p_add=0.06,
        eval_rollouts=60,
    )

    assert len(df) == 2
    required = {
        "target_feature",
        "baseline_safe_present_minus_absent",
        "safe_gain_present_minus_absent",
        "present_H3",
        "absent_H3",
        "present_pass_all",
        "absent_pass_all",
    }
    assert required.issubset(df.columns)


def test_parse_target_edges_supports_multiple_entries() -> None:
    edges = parse_target_edges(
        "risky_assessed->refuse, ambiguous_assessed->clarify",
        None,
        None,
    )
    assert edges == [
        ("risky_assessed", "refuse"),
        ("ambiguous_assessed", "clarify"),
    ]

