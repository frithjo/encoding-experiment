#!/usr/bin/env python3
"""Compare scientific-alignment snapshots and report regressions."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass
class CheckResult:
    name: str
    passed: bool
    baseline: Any
    candidate: Any
    rule: str


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def nested_get(payload: dict[str, Any], path: tuple[str, ...]) -> Any:
    current: Any = payload
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def compare_snapshots(baseline: dict[str, Any], candidate: dict[str, Any]) -> list[CheckResult]:
    checks: list[CheckResult] = []

    def non_decreasing(name: str, path: tuple[str, ...]) -> None:
        b = nested_get(baseline, path)
        c = nested_get(candidate, path)
        if b is None and c is None:
            passed = True
            rule = "skipped (missing in both); candidate >= baseline"
        elif b is None and isinstance(c, (int, float)):
            passed = True
            rule = "baseline missing (schema evolution); candidate is numeric"
        elif isinstance(b, (int, float)) and c is None:
            passed = False
            rule = "candidate field missing; expected candidate >= baseline"
        else:
            passed = isinstance(b, (int, float)) and isinstance(c, (int, float)) and c >= b
            rule = "candidate >= baseline"
        checks.append(
            CheckResult(
                name=name,
                passed=passed,
                baseline=b,
                candidate=c,
                rule=rule,
            )
        )

    def must_equal_true(name: str, path: tuple[str, ...]) -> None:
        b_raw = nested_get(baseline, path)
        c_raw = nested_get(candidate, path)
        b = bool(b_raw)
        c = bool(c_raw)
        if b_raw is None and c_raw is None:
            passed = True
            rule = "skipped (missing in both); baseline and candidate are true"
        elif b_raw is None and c:
            passed = True
            rule = "baseline missing (schema evolution); candidate is true"
        else:
            passed = b and c
            rule = "baseline and candidate are true"
        checks.append(
            CheckResult(
                name=name,
                passed=passed,
                baseline=b,
                candidate=c,
                rule=rule,
            )
        )

    def must_equal_false(name: str, path: tuple[str, ...]) -> None:
        b_raw = nested_get(baseline, path)
        c_raw = nested_get(candidate, path)
        b = bool(b_raw)
        c = bool(c_raw)
        if b_raw is None and c_raw is None:
            passed = True
            rule = "skipped (missing in both); baseline and candidate are false"
        elif b_raw is None and not c:
            passed = True
            rule = "baseline missing (schema evolution); candidate is false"
        else:
            passed = (not b) and (not c)
            rule = "baseline and candidate are false"
        checks.append(
            CheckResult(
                name=name,
                passed=passed,
                baseline=b,
                candidate=c,
                rule=rule,
            )
        )

    def less_or_equal_threshold(
        name: str, path: tuple[str, ...], threshold: int, threshold_label: str
    ) -> None:
        b = nested_get(baseline, path)
        c = nested_get(candidate, path)
        if c is None and b is None:
            passed = True
            rule = f"skipped (missing in both); candidate <= {threshold_label}"
        elif c is None and b is not None:
            passed = False
            rule = f"candidate field missing; expected <= {threshold_label}"
        else:
            passed = isinstance(c, (int, float)) and c <= threshold
            rule = f"candidate <= {threshold_label}"
        checks.append(
            CheckResult(
                name=name,
                passed=passed,
                baseline=b,
                candidate=c,
                rule=rule,
            )
        )

    must_equal_true(
        "lql_first.use_remote_present",
        ("lql_first_path", "use_remote_present"),
    )
    must_equal_true(
        "lql_first.analyze_infer_statement_present",
        ("lql_first_path", "analyze_infer_statement_present"),
    )
    must_equal_false(
        "lql_first.direct_http_analyze_in_core",
        ("lql_first_path", "direct_http_analyze_in_core"),
    )

    non_decreasing("strict_contract.guard_count", ("strict_contract", "guard_count"))
    non_decreasing(
        "strict_contract.analyze_infer_guard_tests",
        ("strict_contract", "analyze_infer_guard_tests"),
    )
    non_decreasing(
        "response_surface.analysis_result_field_count",
        ("response_surface", "analysis_result_field_count"),
    )
    non_decreasing(
        "response_surface.leptos_struct_field_count",
        ("response_surface", "leptos_struct_field_count"),
    )

    must_equal_true(
        "historical_docs.ui_invariant_present",
        ("historical_docs", "ui_invariant_present"),
    )
    must_equal_true(
        "ui_test_payload.strict_payload_present",
        ("ui_test_payload", "strict_payload_present"),
    )
    non_decreasing(
        "ui_transport_fallback.tauri_runtime_guard_count",
        ("ui_transport_fallback", "tauri_runtime_guard_count"),
    )
    must_equal_true(
        "ui_transport_fallback.http_lql_endpoint_present",
        ("ui_transport_fallback", "http_lql_endpoint_present"),
    )
    must_equal_true(
        "ui_transport_fallback.http_describe_endpoint_present",
        ("ui_transport_fallback", "http_describe_endpoint_present"),
    )
    must_equal_true(
        "ui_transport_fallback.http_analyze_endpoint_present",
        ("ui_transport_fallback", "http_analyze_endpoint_present"),
    )

    # Disk-footprint regression guards for CPU/Linux experiment runs.
    less_or_equal_threshold(
        "storage_footprint.apps_bytes_budget",
        ("storage_footprint", "tracked_dir_bytes", "apps"),
        8 * 1024**3,
        "8 GiB",
    )
    less_or_equal_threshold(
        "storage_footprint.target_bytes_budget",
        ("storage_footprint", "tracked_dir_bytes", "target"),
        8 * 1024**3,
        "8 GiB",
    )
    less_or_equal_threshold(
        "storage_footprint.data_bytes_budget",
        ("storage_footprint", "tracked_dir_bytes", "data"),
        6 * 1024**3,
        "6 GiB",
    )
    less_or_equal_threshold(
        "storage_footprint.attention_decoupling_bytes_budget",
        ("storage_footprint", "tracked_dir_bytes", "attention-decoupling-proof"),
        4 * 1024**3,
        "4 GiB",
    )
    less_or_equal_threshold(
        "storage_footprint.history_bytes_budget",
        ("storage_footprint", "tracked_dir_bytes", "docs/scientific-alignment/history"),
        200 * 1024**2,
        "200 MiB",
    )
    less_or_equal_threshold(
        "storage_footprint.tracked_total_bytes_budget",
        ("storage_footprint", "tracked_total_bytes"),
        30 * 1024**3,
        "30 GiB",
    )

    return checks


def select_auto_pair(history_dir: Path, latest_path: Path) -> tuple[Path, Path]:
    history = sorted(history_dir.glob("snapshot-*.json"))
    if len(history) >= 2:
        return history[-2], history[-1]
    if len(history) == 1 and latest_path.exists():
        return history[0], latest_path
    raise SystemExit(
        "not enough snapshots to compare; generate at least two history snapshots first"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--history-dir",
        default="docs/scientific-alignment/history",
        help="Directory containing timestamped snapshots",
    )
    parser.add_argument(
        "--latest",
        default="docs/scientific-alignment/snapshot-latest.json",
        help="Path to latest snapshot (used for fallback auto selection)",
    )
    parser.add_argument("--baseline", help="Explicit baseline snapshot path")
    parser.add_argument("--candidate", help="Explicit candidate snapshot path")
    parser.add_argument(
        "--output",
        default="docs/scientific-alignment/trend-latest.json",
        help="Path for trend comparison output",
    )
    parser.add_argument(
        "--print-summary",
        action="store_true",
        help="Print compact JSON summary to stdout",
    )
    args = parser.parse_args()

    repo = Path(__file__).resolve().parent.parent
    history_dir = repo / args.history_dir
    latest_path = repo / args.latest
    output_path = repo / args.output

    if args.baseline and args.candidate:
        baseline_path = repo / args.baseline
        candidate_path = repo / args.candidate
    else:
        baseline_path, candidate_path = select_auto_pair(history_dir, latest_path)

    baseline = load_json(baseline_path)
    candidate = load_json(candidate_path)
    checks = compare_snapshots(baseline, candidate)

    regressions = [
        {
            "name": check.name,
            "rule": check.rule,
            "baseline": check.baseline,
            "candidate": check.candidate,
        }
        for check in checks
        if not check.passed
    ]

    report = {
        "baseline_path": str(baseline_path.relative_to(repo)),
        "candidate_path": str(candidate_path.relative_to(repo)),
        "baseline_generated_at_utc": baseline.get("generated_at_utc"),
        "candidate_generated_at_utc": candidate.get("generated_at_utc"),
        "check_count": len(checks),
        "regression_count": len(regressions),
        "status": "pass" if not regressions else "fail",
        "regressions": regressions,
    }

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2), encoding="utf-8")

    if args.print_summary:
        print(json.dumps(report, indent=2))
    else:
        print(output_path)

    return 0 if not regressions else 2


if __name__ == "__main__":
    raise SystemExit(main())
