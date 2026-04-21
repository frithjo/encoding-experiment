from __future__ import annotations

from pathlib import Path
import json
import os
from typing import Any

from .models import RecipeRecord, RunRecord


def default_ui_home() -> Path:
    env_path = os.environ.get("LARQL_UI_HOME")
    if env_path:
        return Path(env_path).expanduser()
    return Path.home() / ".local" / "state" / "larql-ui"


class UiStore:
    def __init__(self, base_path: Path | None = None) -> None:
        self.base_path = (base_path or default_ui_home()).expanduser()
        self.base_path.mkdir(parents=True, exist_ok=True)
        self.recipes_path = self.base_path / "recipes.json"
        self.runs_path = self.base_path / "runs.json"
        self.settings_path = self.base_path / "settings.json"

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

    def current_workspace_path(self) -> str | None:
        settings = self._read_json(self.settings_path, {})
        return settings.get("current_workspace_path")

    def set_current_workspace_path(self, path: str) -> None:
        settings = self._read_json(self.settings_path, {})
        settings["current_workspace_path"] = path
        self._write_json(self.settings_path, settings)

    def list_recipes(self) -> list[RecipeRecord]:
        payload = self._read_json(self.recipes_path, [])
        return [RecipeRecord.from_dict(item) for item in payload]

    def get_recipe(self, recipe_id: str) -> RecipeRecord | None:
        for recipe in self.list_recipes():
            if recipe.id == recipe_id:
                return recipe
        return None

    def save_recipe(self, recipe: RecipeRecord) -> None:
        recipes = self.list_recipes()
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

    def delete_recipe(self, recipe_id: str) -> None:
        recipes = [item for item in self.list_recipes() if item.id != recipe_id]
        self._write_json(self.recipes_path, [item.to_dict() for item in recipes])

    def list_runs(self) -> list[RunRecord]:
        payload = self._read_json(self.runs_path, [])
        return [RunRecord.from_dict(item) for item in payload]

    def get_run(self, run_id: str) -> RunRecord | None:
        for run in self.list_runs():
            if run.id == run_id:
                return run
        return None

    def save_run(self, run: RunRecord, limit: int = 250) -> None:
        runs = [item for item in self.list_runs() if item.id != run.id]
        runs.insert(0, run)
        self._write_json(self.runs_path, [item.to_dict() for item in runs[:limit]])
