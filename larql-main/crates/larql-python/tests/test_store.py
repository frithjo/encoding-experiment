from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from larql_ui.ui.store import UiStore


def _minimal_recipe() -> list[dict[str, Any]]:
    return [
        {
            "id": "r1",
            "name": "R",
            "kind": "describe",
            "template": "{x}",
            "lql_template": None,
            "description": "",
            "created_at": "2020-01-01T00:00:00+00:00",
            "updated_at": "2020-01-01T00:00:00+00:00",
        }
    ]


def _minimal_run() -> list[dict[str, Any]]:
    return [
        {
            "id": "run-1",
            "kind": "describe",
            "title": "t",
            "workspace_path": "/w",
            "recipe_id": None,
            "engine": "describe",
            "status": "completed",
            "created_at": "2020-01-01T00:00:00+00:00",
            "duration_ms": 0,
            "summary": "",
            "input_text": "",
            "raw": {},
        }
    ]


def test_list_recipes_second_call_skips_read_json_when_mtime_unchanged(tmp_path: Path) -> None:
    """UiStore caches parsed recipes while recipes.json mtime is unchanged."""
    store = UiStore(tmp_path)
    store.recipes_path.write_text(json.dumps(_minimal_recipe()), encoding="utf-8")

    calls = 0
    orig = store._read_json

    def wrapped(path: Path, default: Any) -> Any:
        nonlocal calls
        calls += 1
        return orig(path, default)

    store._read_json = wrapped  # type: ignore[method-assign]

    a = store.list_recipes()
    b = store.list_recipes()
    assert calls == 1
    assert len(a) == len(b) == 1
    assert a[0].id == "r1"

    assert store.get_recipe("r1") is not None
    assert calls == 1


def test_list_runs_second_call_skips_read_json_when_mtime_unchanged(tmp_path: Path) -> None:
    store = UiStore(tmp_path)
    store.runs_path.write_text(json.dumps(_minimal_run()), encoding="utf-8")

    calls = 0
    orig = store._read_json

    def wrapped(path: Path, default: Any) -> Any:
        nonlocal calls
        calls += 1
        return orig(path, default)

    store._read_json = wrapped  # type: ignore[method-assign]

    store.list_runs()
    store.list_runs()
    assert calls == 1

    assert store.get_run("run-1") is not None
    assert calls == 1


def test_list_recipes_invalidates_cache_after_save(tmp_path: Path) -> None:
    store = UiStore(tmp_path)
    store.recipes_path.write_text(json.dumps(_minimal_recipe()), encoding="utf-8")

    calls = 0
    orig = store._read_json

    def wrapped(path: Path, default: Any) -> Any:
        nonlocal calls
        calls += 1
        return orig(path, default)

    store._read_json = wrapped  # type: ignore[method-assign]

    store.list_recipes()
    assert calls == 1

    from larql_ui.ui.models import RecipeRecord

    r = RecipeRecord(
        id="r2",
        name="N2",
        kind="describe",
        template="{y}",
        lql_template=None,
        description="",
        created_at="2020-01-02T00:00:00+00:00",
        updated_at="2020-01-02T00:00:00+00:00",
    )
    store.save_recipe(r)
    store.list_recipes()
    assert calls == 2
