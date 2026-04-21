from __future__ import annotations

import json
import os
import shutil
import struct
import tempfile
from pathlib import Path

import numpy as np
import pytest
from fastapi.testclient import TestClient

from larql.ui.app import create_app
from larql.ui.store import UiStore


NUM_LAYERS = 4
HIDDEN_SIZE = 32
INTERMEDIATE_SIZE = 64
VOCAB_SIZE = 100
NUM_FEATURES = 16
EMBED_SCALE = 1.0


def _write_f32(path: Path, data: list[float]) -> None:
    np.array(data, dtype=np.float32).tofile(str(path))


@pytest.fixture()
def vindex_path() -> str:
    tmpdir = tempfile.mkdtemp(prefix="larql_ui_vindex_")
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

    with open(os.path.join(tmpdir, "index.json"), "w", encoding="utf-8") as handle:
        json.dump(config, handle)
    _write_f32(Path(tmpdir) / "gate_vectors.bin", gate_data)

    embed_data: list[float] = []
    for token in range(VOCAB_SIZE):
        vec = np.zeros(HIDDEN_SIZE, dtype=np.float32)
        vec[token % HIDDEN_SIZE] = 1.0
        embed_data.extend(vec.tolist())
    _write_f32(Path(tmpdir) / "embeddings.bin", embed_data)

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
    with open(os.path.join(tmpdir, "down_meta.bin"), "wb") as handle:
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
    with open(os.path.join(tmpdir, "tokenizer.json"), "w", encoding="utf-8") as handle:
        json.dump(tokenizer_config, handle)

    yield tmpdir
    shutil.rmtree(tmpdir, ignore_errors=True)


@pytest.fixture()
def client(tmp_path: Path) -> TestClient:
    app = create_app(UiStore(tmp_path / "ui-state"))
    return TestClient(app)


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
