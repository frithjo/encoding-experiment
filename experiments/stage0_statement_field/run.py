#!/usr/bin/env python3
"""Run Stage 0 locally: sweep q, export CSV/JSON, print tables for the lab notebook."""

from __future__ import annotations

import argparse
import json
import platform
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import numpy as np
import pandas as pd

from .field import StatementField
from .matrices import STATEMENTS


def _repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _git_rev() -> str | None:
    try:
        r = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=_repo_root(),
            capture_output=True,
            text=True,
            check=False,
        )
        return r.stdout.strip() if r.returncode == 0 else None
    except OSError:
        return None


def main() -> int:
    parser = argparse.ArgumentParser(description="Stage 0 statement-field experiment")
    parser.add_argument(
        "--out",
        type=Path,
        default=_repo_root() / "experiments" / "stage0_statement_field" / "results",
        help="Directory for CSV/JSON outputs",
    )
    parser.add_argument(
        "--qs",
        type=str,
        default="0:1.01:0.1",
        help="q sweep as start:stop:step (numpy arange), e.g. 0:1.01:0.1",
    )
    args = parser.parse_args()

    out: Path = args.out
    out.mkdir(parents=True, exist_ok=True)

    parts = args.qs.split(":")
    if len(parts) != 3:
        print("qs must be start:stop:step", file=sys.stderr)
        return 2
    start, stop, step = map(float, parts)
    qs = np.arange(start, stop, step)

    field = StatementField.default()
    sources = ["S5", "S6", "S13", "S4"]
    df = field.sweep_sources(sources, qs)

    manifest = {
        "stage": "0",
        "name": "statement_field_resistance_sweep",
        "timestamp_utc": datetime.now(timezone.utc).isoformat(),
        "python": sys.version,
        "platform": platform.platform(),
        "numpy": np.__version__,
        "pandas": pd.__version__,
        "git_rev": _git_rev(),
        "transition_rule": "P(next|src,q) = softmax(K[src] - q * R[src])",
        "q_sweep": {"start": start, "stop": stop, "step": step},
        "sources": sources,
        "n_statements": field.n,
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    df.to_csv(out / "sweep_by_source.csv", index=False)

    # Canonical comparison rows (q in {0, 0.5, 1})
    keyq = df[df["q"].isin([0.0, 0.5, 1.0])].copy()
    keyq.to_csv(out / "sweep_key_q.csv", index=False)

    # Top-k distributions at key q
    lines: list[str] = []
    for src in sources:
        for q in (0.0, 0.5, 1.0):
            lines.append(f"\nSOURCE {src} {STATEMENTS[field.idx[src]]}")
            lines.append(f"q {q}")
            for stmt, pr in field.top_probs(src, q, k=6):
                short = stmt.split(": ", 1)[1]
                lines.append(f"  {short:42s} {pr:.3f}")
    (out / "top6_key_q.txt").write_text("\n".join(lines).lstrip() + "\n", encoding="utf-8")

    # Path analysis from S2
    path_lines: list[str] = []
    canonical_path = ["S2", "S6", "S9", "S10"]
    for q in (0.0, 0.5, 1.0):
        path_lines.append(f"\nTop paths from S2 q {q}")
        for path, pr in field.top_paths("S2", q, steps=3, topn=8):
            human = " -> ".join(s.split(": ", 1)[1] for s in (STATEMENTS[field.idx[x]] for x in path))
            path_lines.append(
                " -> ".join(path) + f" {pr:.4f}" + "  |  " + human
            )
        path_lines.append(
            f"S2 q {q} "
            + " ".join(f"{k} {v:.16f}" for k, v in field.s2_first_step_metrics(q).items())
        )
    pmin = field.global_min_prob_at_q(1.0)
    path_lines.append(f"global min prob q=1 {pmin:.16f}")
    (out / "paths_from_S2.txt").write_text("\n".join(path_lines).lstrip() + "\n", encoding="utf-8")

    path_json = {
        "canonical_pipeline_S2_S6_S9_S10": {
            str(q): field.path_prob(canonical_path, q) for q in (0.0, 0.5, 1.0)
        },
        "s2_first_step": {str(q): field.s2_first_step_metrics(q) for q in (0.0, 0.5, 1.0)},
        "global_min_transition_prob_q_1": pmin,
    }
    (out / "path_metrics.json").write_text(json.dumps(path_json, indent=2), encoding="utf-8")

    # Console: compact summary table
    print(manifest["transition_rule"])
    print(keyq.round(3).to_string(index=False))
    print("\nCanonical path S2→S6→S9→S10:", path_json["canonical_pipeline_S2_S6_S9_S10"])
    print("Wrote:", out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
