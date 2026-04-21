from __future__ import annotations

import json
import os
import re
import shutil
import struct
import tempfile
import time
from pathlib import Path

import numpy as np
import pytest
from starlette.testclient import TestClient

from larql.ui.app import create_app
from larql.ui.models import RecipeRecord, RunRecord, execution_engine_for_recipe, validate_recipe
from larql.ui.store import DEFAULT_RUN_HISTORY_LIMIT, UiStore


FIXTURE_UI_WALK_TRACE_VINDEX = Path(__file__).resolve().parent / "fixtures" / "ui_walk_trace_vindex"

NUM_LAYERS = 4
HIDDEN_SIZE = 32
INTERMEDIATE_SIZE = 64
VOCAB_SIZE = 100
NUM_FEATURES = 16
EMBED_SCALE = 1.0


def _write_f32(path: Path, data: list[float]) -> None:
    np.array(data, dtype=np.float32).tofile(str(path))


def _populate_synthetic_vindex(out_dir: Path) -> None:
    """Write a minimal valid vindex tree (same shape as the ``vindex_path`` fixture)."""
    out_dir.mkdir(parents=True, exist_ok=True)
    config = {
        "version": 1,
        "model": "test/synthetic-ui",
        "family": "test",
        "num_layers": NUM_LAYERS,
        "hidden_size": HIDDEN_SIZE,
        "intermediate_size": INTERMEDIATE_SIZE,
        "vocab_size": VOCAB_SIZE,
        "embed_scale": EMBED_SCALE,
        "extract_level": "browse",
        "dtype": "f32",
        "down_top_k": 3,
        "has_model_weights": False,
        "layers": [],
        "layer_bands": {
            "syntax": [0, 1],
            "knowledge": [2, 3],
            "output": [3, 3],
        },
    }

    gate_data: list[float] = []
    for layer in range(NUM_LAYERS):
        offset = len(gate_data) * 4
        for feat in range(NUM_FEATURES):
            vec = np.zeros(HIDDEN_SIZE, dtype=np.float32)
            vec[feat % HIDDEN_SIZE] = 1.0 + layer * 0.1
            vec[(feat + 1) % HIDDEN_SIZE] = 0.5
            gate_data.extend(vec.tolist())
        config["layers"].append(
            {
                "layer": layer,
                "num_features": NUM_FEATURES,
                "offset": offset,
                "length": NUM_FEATURES * HIDDEN_SIZE * 4,
            }
        )

    with open(out_dir / "index.json", "w", encoding="utf-8") as handle:
        json.dump(config, handle)
    _write_f32(out_dir / "gate_vectors.bin", gate_data)

    embed_data: list[float] = []
    for token in range(VOCAB_SIZE):
        vec = np.zeros(HIDDEN_SIZE, dtype=np.float32)
        vec[token % HIDDEN_SIZE] = 1.0
        embed_data.extend(vec.tolist())
    _write_f32(out_dir / "embeddings.bin", embed_data)

    top_k_count = 3
    record_size = 8 + top_k_count * 8
    meta_data = bytearray()
    for layer in range(NUM_LAYERS):
        for feat in range(NUM_FEATURES):
            if feat >= NUM_FEATURES - 4:
                record = b"\x00" * record_size
            else:
                token_id = (layer * NUM_FEATURES + feat) % VOCAB_SIZE
                c_score = 0.5 + feat * 0.01
                record = struct.pack("<If", token_id, c_score)
                for k in range(top_k_count):
                    tid = (token_id + k + 1) % VOCAB_SIZE
                    logit = c_score - k * 0.1
                    record += struct.pack("<If", tid, logit)
            meta_data.extend(record)
    with open(out_dir / "down_meta.bin", "wb") as handle:
        handle.write(meta_data)

    vocab: dict[str, int] = {}
    idx = 0
    for char in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .,!?'-_":
        vocab[char] = idx
        idx += 1
    while idx < VOCAB_SIZE:
        vocab[f"<t{idx}>"] = idx
        idx += 1
    tokenizer_config = {
        "version": "1.0",
        "model": {"type": "BPE", "vocab": vocab, "merges": []},
        "added_tokens": [],
        "normalizer": None,
        "pre_tokenizer": {"type": "Whitespace"},
        "post_processor": None,
        "decoder": None,
    }
    with open(out_dir / "tokenizer.json", "w", encoding="utf-8") as handle:
        json.dump(tokenizer_config, handle)


@pytest.fixture()
def vindex_path() -> str:
    tmpdir = tempfile.mkdtemp(prefix="larql_ui_vindex_")
    _populate_synthetic_vindex(Path(tmpdir))
    yield tmpdir
    shutil.rmtree(tmpdir, ignore_errors=True)


@pytest.fixture()
def client(tmp_path: Path) -> TestClient:
    app = create_app(UiStore(tmp_path / "ui-state"))
    return TestClient(app)


@pytest.fixture()
def vindex_path_walk_trace() -> str:
    """Vindex with mmap model weights — `WalkModel` / `/trace` works (see `fixtures/README.md`)."""
    if not FIXTURE_UI_WALK_TRACE_VINDEX.is_dir():
        pytest.skip("Missing ui_walk_trace_vindex fixture; see tests/fixtures/README.md")
    return str(FIXTURE_UI_WALK_TRACE_VINDEX)


def test_partial_workspace_status_no_workspace(client: TestClient) -> None:
    response = client.get("/partials/workspace-status")
    assert response.status_code == 200
    assert "workspace-strip" in response.text
    assert "No workspace open yet." in response.text


def test_partial_workspace_status_loaded(client: TestClient, vindex_path: str) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    response = client.get("/partials/workspace-status")
    assert response.status_code == 200
    assert "workspace-strip" in response.text
    assert "test/synthetic-ui" in response.text


def test_workspace_inspection_badges(client: TestClient, vindex_path: str) -> None:
    """Synthetic vindex: LQL session + STATS probe; no model weights → no WalkModel."""
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    r = client.get("/partials/workspace-status")
    assert r.status_code == 200
    assert "lql: ready" in r.text
    assert "walk_model: off" in r.text
    assert "infer: off" in r.text


def test_workspace_open_and_explorer_run(client: TestClient, vindex_path: str) -> None:
    response = client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    assert response.status_code == 200
    assert "Workspace loaded" in response.text
    assert "Studio" in response.text

    describe = client.post(
        "/explorer",
        data={"entity": "hello", "band": "knowledge", "verbose": "on"},
        follow_redirects=True,
    )
    assert describe.status_code == 200
    assert "Raw JSON" in describe.text
    assert "hello" in describe.text


def test_lql_recipe_and_run_history(client: TestClient, vindex_path: str) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)

    recipe_create = client.post(
        "/recipes",
        data={
            "name": "Stats recipe",
            "kind": "lql",
            "template": "{subject}",
            "lql_template": "STATS",
            "description": "Basic stats query",
        },
        follow_redirects=True,
    )
    assert recipe_create.status_code == 200
    assert "Stats recipe" in recipe_create.text

    recipe_id = recipe_create.request.url.path.split("/")[-1]
    recipe_run = client.post(
        f"/recipes/{recipe_id}/run",
        data={"subject": "unused"},
        follow_redirects=True,
    )
    assert recipe_run.status_code == 200
    assert "lines returned" in recipe_run.text

    lql_run = client.post("/lql", data={"query": "STATS"}, follow_redirects=True)
    assert lql_run.status_code == 200
    assert "STATS" in lql_run.text
    assert "Raw JSON" in lql_run.text

    runs = client.get("/runs")
    assert runs.status_code == 200
    assert "Stats recipe" in runs.text
    assert "LQL query" in runs.text


def test_studio_describe_run(client: TestClient, vindex_path: str) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    page = client.get("/studio")
    assert page.status_code == 200
    assert "Ad-hoc template" in page.text
    assert "subject" in page.text.lower() or "Variables" in page.text

    run = client.post(
        "/studio/run",
        data={
            "recipe_id": "",
            "engine": "describe",
            "inline_template": "{subject}",
            "band": "knowledge",
            "subject": "hello",
            "compare_subjects": "",
        },
        follow_redirects=True,
    )
    assert run.status_code == 200
    assert "hello" in run.text
    assert "Raw JSON" in run.text


def test_studio_compare_validation(client: TestClient, vindex_path: str) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    bad = client.post(
        "/studio/compare",
        data={
            "recipe_id": "",
            "engine": "describe",
            "inline_template": "{a}{b}",
            "band": "knowledge",
            "compare_subjects": "x",
        },
        follow_redirects=True,
    )
    assert bad.status_code == 200
    assert "exactly one placeholder" in bad.text.lower()


def test_studio_page_marks_background_run_as_run_only(
    client: TestClient, vindex_path: str
) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    page = client.get("/studio")
    assert page.status_code == 200
    assert "Background run (Run only; compare always executes in-page)" in page.text


def test_studio_async_page_waits_for_shared_poller(
    client: TestClient, vindex_path: str
) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    page = client.post(
        "/studio/run",
        data={
            "recipe_id": "",
            "engine": "describe",
            "inline_template": "{subject}",
            "band": "knowledge",
            "subject": "hello",
            "compare_subjects": "",
            "async_background": "on",
        },
    )
    assert page.status_code == 200
    assert 'document.addEventListener("DOMContentLoaded", startPolling, { once: true })' in page.text


def test_studio_async_page_preserves_poll_id_bootstrap(
    client: TestClient, vindex_path: str
) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    page = client.post(
        "/studio/run",
        data={
            "recipe_id": "",
            "engine": "describe",
            "inline_template": "{subject}",
            "band": "knowledge",
            "subject": "hello",
            "compare_subjects": "",
            "async_background": "on",
        },
    )
    assert page.status_code == 200
    assert 'window.LarqlAsyncRuns.pollRun({' in page.text
    assert 'scope: "studio"' in page.text


def test_studio_run_resolves_engine_from_recipe_when_engine_omitted(
    client: TestClient, vindex_path: str
) -> None:
    """Empty engine + recipe uses RecipeRecord.effective_engine (kind / default_engine)."""
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    create = client.post(
        "/recipes",
        data={
            "name": "Stats lql default",
            "kind": "lql",
            "template": "{subject}",
            "lql_template": "STATS",
            "description": "",
        },
        follow_redirects=True,
    )
    assert create.status_code == 200
    rid = create.request.url.path.split("/")[-1]
    run = client.post(
        "/studio/run",
        data={
            "recipe_id": rid,
            "inline_template": "",
            "band": "knowledge",
            "compare_subjects": "",
            "infer_top_k_predictions": "5",
            "walk_top_k": "8192",
        },
        follow_redirects=True,
    )
    assert run.status_code == 200
    assert "lql" in run.text.lower()
    assert "lines returned" in run.text or "STATS" in run.text


def test_trace_redirect_without_workspace(client: TestClient) -> None:
    response = client.get("/trace", follow_redirects=False)
    assert response.status_code == 303
    assert "/workspace" in (response.headers.get("location") or "")


def test_runs_page_shows_retention_limit(client: TestClient, vindex_path: str) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    page = client.get("/runs")
    assert page.status_code == 200
    assert f"Only the {DEFAULT_RUN_HISTORY_LIMIT} most recent runs are kept" in page.text


def test_partial_result_panel_keeps_edge_metadata(client: TestClient, vindex_path: str) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    run = client.post(
        "/studio/run",
        data={
            "recipe_id": "",
            "engine": "describe",
            "inline_template": "{subject}",
            "band": "knowledge",
            "subject": "hello",
            "compare_subjects": "",
        },
    )
    assert run.status_code == 200
    runs_page = client.get("/runs")
    assert runs_page.status_code == 200
    match = re.search(r'/runs/([^"]+)', runs_page.text)
    assert match is not None
    run_id = match.group(1)
    panel = client.get(f"/partials/result-panel?run_id={run_id}")
    assert panel.status_code == 200
    assert "Open run detail" in panel.text
    assert "Raw JSON" in panel.text
    assert "<pre>" in panel.text


def test_trace_page_lists_in_nav_and_gates_without_weights(
    client: TestClient, vindex_path: str
) -> None:
    """Synthetic vindex has no model weights — trace form is disabled with explanation."""
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    page = client.get("/trace")
    assert page.status_code == 200
    assert "Residual trace" in page.text
    assert "Trace is unavailable" in page.text or "requires model weights" in page.text.lower()
    assert 'href="/trace"' in page.text
    assert "disabled" in page.text.lower()


def test_trace_rejects_empty_prompt(client: TestClient, vindex_path: str) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    r = client.post("/trace", data={"prompt": "", "positions": "last", "walk_top_k": "8192"})
    assert r.status_code == 200
    assert "Prompt required" in r.text


def test_trace_async_page_uses_shared_helper(client: TestClient, vindex_path: str) -> None:
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    page = client.get("/trace")
    assert page.status_code == 200
    assert 'window.LarqlAsyncRuns.submitAndPoll({' in page.text
    assert 'scope: "trace"' in page.text
    assert 'resultUrlBuilder: function (runId)' in page.text
    assert '/partials/trace-summary?run_id=' in page.text


def test_workspace_status_shows_trace_ready_with_weights_fixture(
    client: TestClient, vindex_path_walk_trace: str
) -> None:
    client.post("/workspace", data={"path": vindex_path_walk_trace}, follow_redirects=True)
    r = client.get("/partials/workspace-status")
    assert r.status_code == 200
    assert "trace: ready" in r.text
    assert "infer: ready" in r.text
    assert "walk_model: ready" in r.text


def test_shell_template_nav_semantics_and_fetch_alert_region(
    client: TestClient, vindex_path: str
) -> None:
    """Primary nav exposes aria-current; workspace refresh failures can surface in-page."""
    client.post("/workspace", data={"path": vindex_path}, follow_redirects=True)
    page = client.get("/explorer")
    assert page.status_code == 200
    assert 'id="workspace-fetch-alert"' in page.text
    assert 'aria-current="page"' in page.text
    assert 'href="/explorer"' in page.text


def test_trace_full_run(client: TestClient, vindex_path_walk_trace: str) -> None:
    """End-to-end residual trace via UI (`WalkModel.trace` + `RunRecord` kind trace)."""
    client.post("/workspace", data={"path": vindex_path_walk_trace}, follow_redirects=True)
    page = client.get("/trace")
    assert page.status_code == 200
    assert "Run trace" in page.text
    assert "Trace is unavailable" not in page.text

    r = client.post(
        "/trace",
        data={"prompt": "a b", "positions": "last", "walk_top_k": "8"},
        follow_redirects=True,
    )
    assert r.status_code == 200
    assert "Residual trace:" in r.text
    assert "data-table" in r.text
    assert "Raw JSON" in r.text

    runs = client.get("/runs")
    assert runs.status_code == 200
    assert "Trace" in runs.text


def test_plan_partial_routes_exist(client: TestClient) -> None:
    for path in (
        "/partials/recipes-list",
        "/partials/runs-table",
        "/partials/run-controls",
        "/partials/recipe-editor",
        "/partials/result-panel",
        "/partials/trace-summary",
    ):
        r = client.get(path)
        assert r.status_code == 200, path


def test_api_workspace_current_404(client: TestClient) -> None:
    r = client.get("/api/workspace/current")
    assert r.status_code == 404


def test_api_workspace_open_and_describe(client: TestClient, vindex_path: str) -> None:
    r = client.post("/api/workspace/open", json={"path": vindex_path})
    assert r.status_code == 200
    body = r.json()
    assert body.get("ok") is True
    assert "workspace" in body
    cur = client.get("/api/workspace/current")
    assert cur.status_code == 200

    d = client.post("/api/explorer/describe", json={"entity": "hello", "band": "knowledge", "async": False})
    assert d.status_code == 200
    run = d.json()["run"]
    assert run["kind"] == "describe"
    assert run["status"] == "completed"


def test_api_explorer_relations(client: TestClient, vindex_path: str) -> None:
    client.post("/api/workspace/open", json={"path": vindex_path})
    r = client.post("/api/explorer/relations", json={})
    assert r.status_code == 200
    assert "relations" in r.json()


def test_api_lql_query(client: TestClient, vindex_path: str) -> None:
    client.post("/api/workspace/open", json={"path": vindex_path})
    r = client.post("/api/lql/query", json={"query": "STATS", "async": False})
    assert r.status_code == 200
    assert r.json()["run"]["engine"] == "lql"


def test_api_recipes_post_minimal(client: TestClient, tmp_path: Path) -> None:
    app = create_app(UiStore(tmp_path / "ui"))
    c = TestClient(app)
    r = c.post(
        "/api/recipes",
        json={
            "name": "API probe",
            "kind": "probe",
            "template": "About {subject}",
            "default_engine": "describe",
            "tags": ["t1"],
            "notes": "n",
        },
    )
    assert r.status_code == 201
    rec = r.json()["recipe"]
    assert rec["kind"] == "probe"
    assert rec["tags"] == ["t1"]


def test_api_trace_async_completes(client: TestClient, vindex_path_walk_trace: str) -> None:
    client.post("/api/workspace/open", json={"path": vindex_path_walk_trace})
    r = client.post(
        "/api/trace/run",
        json={"prompt": "a b", "positions": "last", "walk_top_k": 8, "async": True},
    )
    assert r.status_code == 202
    rid = r.json()["run_id"]
    run: dict | None = None
    for _ in range(100):
        g = client.get(f"/api/runs/{rid}")
        assert g.status_code == 200
        run = g.json()["run"]
        if run["status"] in ("completed", "error"):
            break
        time.sleep(0.05)
    assert run is not None
    assert run["status"] == "completed"
    assert run["kind"] == "trace"


def test_probe_generation_engine_resolution() -> None:
    probe = RecipeRecord.create(name="p", kind="probe", template="{x}", default_engine="infer")
    assert execution_engine_for_recipe(probe) == "infer"
    gen = RecipeRecord.create(name="g", kind="generation", template="{x}", default_engine="walk_model")
    assert execution_engine_for_recipe(gen) == "walk_model"
    validate_recipe(probe)
    validate_recipe(gen)


def test_api_mutating_route_415_without_json_content_type(client: TestClient) -> None:
    r = client.post(
        "/api/workspace/open",
        content=b'{"path": "/tmp"}',
        headers={"Content-Type": "text/plain"},
    )
    assert r.status_code == 415
    assert "json" in r.json().get("error", "").lower()


def test_api_recipe_put_bumps_updated_at(client: TestClient, tmp_path: Path) -> None:
    app = create_app(UiStore(tmp_path / "ui"))
    c = TestClient(app)
    r = c.post(
        "/api/recipes",
        json={
            "name": "u1",
            "kind": "probe",
            "template": "T {subject}",
            "default_engine": "describe",
            "tags": [],
            "notes": "a",
        },
    )
    assert r.status_code == 201
    rid = r.json()["recipe"]["id"]
    before = r.json()["recipe"]["updated_at"]
    time.sleep(0.02)
    r2 = c.put(f"/api/recipes/{rid}", json={"notes": "b"})
    assert r2.status_code == 200
    after = r2.json()["recipe"]["updated_at"]
    assert before != after


def test_runrecord_from_dict_partial_defaults() -> None:
    r = RunRecord.from_dict(
        {
            "id": "legacy-1",
            "kind": "describe",
            "workspace_path": "/w",
            "engine": "describe",
        }
    )
    assert r.status == "completed"
    assert r.recipe_id is None
    assert r.raw == {}


def test_ui_store_list_runs_skips_non_dict_rows(tmp_path: Path) -> None:
    store = UiStore(tmp_path / "ui")
    good = RunRecord.create(
        kind="lql",
        title="q",
        workspace_path="/w",
        engine="lql",
        summary="s",
        input_text="STATS",
        raw={},
        duration_ms=0,
    )
    p = store.runs_path
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps([good.to_dict(), "not-a-dict"], ensure_ascii=True), encoding="utf-8")
    runs = store.list_runs()
    assert len(runs) == 1
    assert runs[0].id == good.id


def test_api_rerun_409_when_workspace_differs_allow_flag(client: TestClient, tmp_path: Path) -> None:
    v1 = tmp_path / "vindex_a"
    v2 = tmp_path / "vindex_b"
    _populate_synthetic_vindex(v1)
    _populate_synthetic_vindex(v2)
    client.post("/api/workspace/open", json={"path": str(v1)})
    d = client.post(
        "/api/explorer/describe",
        json={"entity": "hello", "band": "knowledge", "async": False},
    )
    assert d.status_code == 200
    run_id = d.json()["run"]["id"]
    client.post("/api/workspace/open", json={"path": str(v2)})
    r = client.post(f"/api/runs/{run_id}/rerun", json={})
    assert r.status_code == 409
    err = r.json()
    assert "workspace" in err.get("error", "").lower()
    assert "run_workspace" in err
    assert "current_workspace" in err
    r_ok = client.post(
        f"/api/runs/{run_id}/rerun",
        json={"allow_different_workspace": True, "async": False},
    )
    assert r_ok.status_code == 200
    assert r_ok.json()["run"]["kind"] == "describe"
