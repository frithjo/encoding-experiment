from __future__ import annotations

import json
import os
import sys
import threading
from pathlib import Path
from typing import Any

from .models import RecipeRecord, RunRecord


def _norm_workspace_path(path: str) -> str:
    return str(Path(path).expanduser().resolve())


def default_ui_home() -> Path:
    env_path = os.environ.get("LARQL_UI_HOME")
    if env_path:
        return Path(env_path).expanduser()
    return Path.home() / ".local" / "state" / "larql-ui"


class UiStore:
    """Persistent JSON store with in-process mtime caches to avoid re-parsing unchanged files."""

    def __init__(self, base_path: Path | None = None) -> None:
        self.base_path = (base_path or default_ui_home()).expanduser()
        self.base_path.mkdir(parents=True, exist_ok=True)
        self._file_lock = threading.Lock()
        self.recipes_path = self.base_path / "recipes.json"
        self.runs_path = self.base_path / "runs.json"
        self.settings_path = self.base_path / "settings.json"

        self._settings_mtime_ns: int | None = None
        self._settings_dict: dict[str, Any] | None = None

        self._recipes_mtime_ns: int | None = None
        self._recipes_list: list[RecipeRecord] | None = None

        self._runs_mtime_ns: int | None = None
        self._runs_list: list[RunRecord] | None = None

    def _read_json(self, path: Path, default: Any) -> Any:
        if not path.exists():
            return default
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)

    def _write_json(self, path: Path, payload: Any) -> None:
        tmp_path = path.with_suffix(path.suffix + ".tmp")
        with tmp_path.open("w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=2, sort_keys=True)
            handle.write("\n")
        tmp_path.replace(path)

    def _read_settings(self) -> dict[str, Any]:
        p = self.settings_path
        if not p.exists():
            self._settings_dict = {}
            self._settings_mtime_ns = None
            return {}
        m = p.stat().st_mtime_ns
        if self._settings_dict is not None and self._settings_mtime_ns == m:
            return self._settings_dict
        raw = self._read_json(p, {})
        data = raw if isinstance(raw, dict) else {}
        self._settings_dict = data
        self._settings_mtime_ns = m
        return data

    def _invalidate_settings_cache(self) -> None:
        self._settings_mtime_ns = None
        self._settings_dict = None

    def _invalidate_recipes_cache(self) -> None:
        self._recipes_mtime_ns = None
        self._recipes_list = None

    def _invalidate_runs_cache(self) -> None:
        self._runs_mtime_ns = None
        self._runs_list = None

    def current_workspace_path(self) -> str | None:
        return self._read_settings().get("current_workspace_path")

    def set_current_workspace_path(self, path: str) -> None:
        norm = _norm_workspace_path(path)
        with self._file_lock:
            settings = dict(self._read_settings())
            cur = settings.get("current_workspace_path")
            if cur is not None:
                try:
                    if _norm_workspace_path(str(cur)) == norm:
                        return
                except (OSError, ValueError):
                    pass
            settings["current_workspace_path"] = norm
            self._write_json(self.settings_path, settings)
            self._invalidate_settings_cache()

    def list_recipes(self) -> list[RecipeRecord]:
        p = self.recipes_path
        if not p.exists():
            self._recipes_list = []
            self._recipes_mtime_ns = None
            return []
        m = p.stat().st_mtime_ns
        if self._recipes_list is not None and self._recipes_mtime_ns == m:
            return self._recipes_list
        payload = self._read_json(p, [])
        out = [RecipeRecord.from_dict(item) for item in payload]
        self._recipes_list = out
        self._recipes_mtime_ns = m
        return out

    def get_recipe(self, recipe_id: str) -> RecipeRecord | None:
        for recipe in self.list_recipes():
            if recipe.id == recipe_id:
                return recipe
        return None

    def save_recipe(self, recipe: RecipeRecord) -> None:
        with self._file_lock:
            recipes = list(self.list_recipes())
            replaced = False
            for index, existing in enumerate(recipes):
                if existing.id == recipe.id:
                    recipes[index] = recipe
                    replaced = True
                    break
            if not replaced:
                recipes.append(recipe)
            recipes.sort(key=lambda item: item.updated_at, reverse=True)
            self._write_json(self.recipes_path, [item.to_dict() for item in recipes])
            self._invalidate_recipes_cache()

    def delete_recipe(self, recipe_id: str) -> None:
        with self._file_lock:
            recipes = [item for item in self.list_recipes() if item.id != recipe_id]
            self._write_json(self.recipes_path, [item.to_dict() for item in recipes])
            self._invalidate_recipes_cache()

    def list_runs(self) -> list[RunRecord]:
        p = self.runs_path
        if not p.exists():
            self._runs_list = []
            self._runs_mtime_ns = None
            return []
        m = p.stat().st_mtime_ns
        if self._runs_list is not None and self._runs_mtime_ns == m:
            return self._runs_list
        payload = self._read_json(p, [])
        out: list[RunRecord] = []
        if isinstance(payload, list):
            _debug = os.environ.get("LARQL_UI_DEBUG", "").lower() in ("1", "true", "yes")
            for item in payload:
                if not isinstance(item, dict):
                    if _debug:
                        print(f"[larql-ui] skipped non-dict run row: {item!r}", file=sys.stderr)
                    continue
                try:
                    out.append(RunRecord.from_dict(item))
                except (TypeError, ValueError) as exc:
                    if _debug:
                        print(f"[larql-ui] skipped bad run row: {item!r} ({exc})", file=sys.stderr)
                    continue
        self._runs_list = out
        self._runs_mtime_ns = m
        return out

    def get_run(self, run_id: str) -> RunRecord | None:
        for run in self.list_runs():
            if run.id == run_id:
                return run
        return None

    def save_run(self, run: RunRecord, limit: int = 250) -> None:
        with self._file_lock:
            runs = [item for item in self.list_runs() if item.id != run.id]
            runs.insert(0, run)
            self._write_json(self.runs_path, [item.to_dict() for item in runs[:limit]])
            self._invalidate_runs_cache()
