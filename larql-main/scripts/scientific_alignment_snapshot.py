#!/usr/bin/env python3
"""Generate a reproducible scientific-alignment snapshot from repo evidence."""

from __future__ import annotations

import argparse
import json
import os
import re
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path


@dataclass
class Snapshot:
    generated_at_utc: str
    lql_first_path: dict
    strict_contract: dict
    response_surface: dict
    historical_docs: dict
    ui_test_payload: dict
    ui_transport_fallback: dict
    storage_footprint: dict


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def dir_size_bytes(path: Path) -> int:
    if not path.exists():
        return 0
    if path.is_file():
        try:
            return path.stat().st_size
        except OSError:
            return 0

    total = 0
    for root, _dirs, files in os.walk(path, followlinks=False):
        for filename in files:
            file_path = Path(root) / filename
            try:
                total += file_path.stat().st_size
            except OSError:
                continue
    return total


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        default="docs/scientific-alignment/snapshot-latest.json",
        help="Path for JSON snapshot output",
    )
    parser.add_argument(
        "--history-dir",
        default="docs/scientific-alignment/history",
        help="Directory for timestamped snapshot copies",
    )
    parser.add_argument(
        "--no-history",
        action="store_true",
        help="Do not write a timestamped history copy",
    )
    parser.add_argument(
        "--print-summary",
        action="store_true",
        help="Print compact metric summary to stdout",
    )
    args = parser.parse_args()

    repo = Path(__file__).resolve().parent.parent
    core = read(repo / "crates/larql-workbench-core/src/lib.rs")
    leptos = read(repo / "crates/larql-leptos/src/lib.rs")
    analysis = read(repo / "crates/larql-inference/src/analysis.rs")
    migration_guide = read(repo / "docs/leptos-migration-guide.md")
    ui_readme = read(repo / "docs/ui/README.md")
    test_ui = read(repo / "crates/larql-leptos/test_ui.sh")

    lql_first = {
        "use_remote_present": "USE REMOTE" in core,
        "analyze_infer_statement_present": "ANALYZE INFER" in core,
        "direct_http_analyze_in_core": "/v1/analyze-infer" in core,
    }

    strict_contract_keys = [
        "MissingAnalysisMode",
        "InvalidAnalysisMode",
        "MissingFactProbeSpans",
        "MissingWorkflowProbeFalseSpans",
        "InvalidTopK",
    ]
    strict_contract = {
        "guards_present": {
            key: (key in core) for key in strict_contract_keys
        },
        "guard_count": sum(1 for key in strict_contract_keys if key in core),
        "analyze_infer_guard_tests": len(
            re.findall(r"fn\s+run_analyze_infer_", core)
        ),
    }

    full_fields = [
        "attention",
        "logit_lens",
        "head_dla",
        "num_layers",
        "seq_len",
        "tokens",
        "strings",
        "predictions",
        "generation_trace",
        "token_analysis",
        "analysis_summary",
        "ridge_by_layer",
    ]
    response_surface = {
        "analysis_result_full_fields_present": {
            field: (f"pub {field}:" in analysis) for field in full_fields
        },
        "analysis_result_field_count": sum(
            1 for field in full_fields if f"pub {field}:" in analysis
        ),
        "leptos_struct_field_count": sum(
            1 for field in full_fields if re.search(rf"\b{field}:", leptos)
        ),
    }

    historical_docs = {
        "migration_guide_historical_flag": "Historical Proof of Concept"
        in migration_guide,
        "migration_guide_historical_note": "Historical note:" in migration_guide,
        "ui_invariant_present": "One experiment engine, one language surface, one transport surface."
        in ui_readme,
    }

    payload_match = re.search(r"STRICT_ANALYSIS_PAYLOAD='([^']+)'", test_ui)
    payload_obj = json.loads(payload_match.group(1)) if payload_match else {}
    ui_test_payload = {
        "strict_payload_present": payload_match is not None,
        "mode": payload_obj.get("mode"),
        "truth_spans_count": len(payload_obj.get("truth_spans", [])),
        "false_spans_count": len(payload_obj.get("materially_false_spans", [])),
        "coherence_markers_count": len(payload_obj.get("coherence_markers", [])),
        "max_generated_tokens": payload_obj.get("max_generated_tokens"),
    }

    ui_transport_fallback = {
        "tauri_runtime_guard_count": len(re.findall(r"if\s+!has_tauri_runtime\(\)", leptos)),
        "http_lql_endpoint_present": "/api/lql/query" in leptos,
        "http_describe_endpoint_present": "/api/explorer/describe" in leptos,
        "http_analyze_endpoint_present": "/v1/analyze-infer" in leptos,
    }

    tracked_dirs = [
        "apps",
        "target",
        "data",
        "attention-decoupling-proof",
        "docs/scientific-alignment/history",
    ]
    tracked_bytes = {
        path: dir_size_bytes(repo / path) for path in tracked_dirs
    }
    tracked_gib = {
        path: round(num_bytes / (1024 ** 3), 3) for path, num_bytes in tracked_bytes.items()
    }
    largest_path, largest_bytes = max(
        tracked_bytes.items(), key=lambda item: item[1], default=("none", 0)
    )
    storage_footprint = {
        "tracked_dir_bytes": tracked_bytes,
        "tracked_dir_gib": tracked_gib,
        "tracked_total_bytes": sum(tracked_bytes.values()),
        "tracked_total_gib": round(sum(tracked_bytes.values()) / (1024 ** 3), 3),
        "largest_tracked_dir": largest_path,
        "largest_tracked_dir_bytes": largest_bytes,
    }

    snapshot = Snapshot(
        generated_at_utc=datetime.now(timezone.utc).isoformat(),
        lql_first_path=lql_first,
        strict_contract=strict_contract,
        response_surface=response_surface,
        historical_docs=historical_docs,
        ui_test_payload=ui_test_payload,
        ui_transport_fallback=ui_transport_fallback,
        storage_footprint=storage_footprint,
    )

    out_path = repo / args.output
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(snapshot.__dict__, indent=2), encoding="utf-8")

    if not args.no_history:
        ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        history_dir = repo / args.history_dir
        history_dir.mkdir(parents=True, exist_ok=True)
        history_path = history_dir / f"snapshot-{ts}.json"
        history_path.write_text(json.dumps(snapshot.__dict__, indent=2), encoding="utf-8")

    if args.print_summary:
        summary = {
            "lql_first": snapshot.lql_first_path["use_remote_present"]
            and snapshot.lql_first_path["analyze_infer_statement_present"]
            and not snapshot.lql_first_path["direct_http_analyze_in_core"],
            "strict_guard_count": snapshot.strict_contract["guard_count"],
            "strict_guard_tests": snapshot.strict_contract["analyze_infer_guard_tests"],
            "analysis_result_fields": snapshot.response_surface["analysis_result_field_count"],
            "leptos_result_fields": snapshot.response_surface["leptos_struct_field_count"],
            "tauri_runtime_guard_count": snapshot.ui_transport_fallback["tauri_runtime_guard_count"],
            "tracked_total_gib": snapshot.storage_footprint["tracked_total_gib"],
            "largest_tracked_dir": snapshot.storage_footprint["largest_tracked_dir"],
            "largest_tracked_dir_gib": round(
                snapshot.storage_footprint["largest_tracked_dir_bytes"] / (1024 ** 3), 3
            ),
        }
        print(json.dumps(summary, indent=2))
    else:
        print(out_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
