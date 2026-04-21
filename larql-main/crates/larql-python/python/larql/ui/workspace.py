from __future__ import annotations

import json
from importlib.util import find_spec
from pathlib import Path
from typing import Any

import larql

from .models import WorkspaceSummary, slugify
from .store import UiStore

RELATION_LABEL_PREVIEW_CAP = 60
TOP_TOKENS_PER_RELATION = 8
_WORKSPACE_CACHE_VERSION = 5
PROBE_RELATION_PREVIEW_CAP = 60


class WorkspaceError(RuntimeError):
    pass


class WorkspaceManager:
    def __init__(self, store: UiStore) -> None:
        self.store = store
        self._cache: dict[str, WorkspaceSummary] = {}

    def get_current(self) -> WorkspaceSummary | None:
        current_path = self.store.current_workspace_path()
        if not current_path:
            return None
        return self.open(current_path)

    def drop_workspace_cache(self, path: str | None = None) -> None:
        """Drop cached WorkspaceSummary so the next open() reloads from disk."""
        if path is None:
            self._cache.clear()
            return
        norm_path = str(Path(path).expanduser().resolve())
        cache_key = f"{_WORKSPACE_CACHE_VERSION}:{norm_path}"
        self._cache.pop(cache_key, None)

    def open(self, path: str) -> WorkspaceSummary:
        norm_path = str(Path(path).expanduser().resolve())
        cache_key = f"{_WORKSPACE_CACHE_VERSION}:{norm_path}"
        if cache_key in self._cache:
            return self._cache[cache_key]

        workspace_dir = Path(norm_path)
        if not workspace_dir.exists():
            raise WorkspaceError(f"Workspace path does not exist: {norm_path}")
        if not workspace_dir.is_dir():
            raise WorkspaceError(f"Workspace path is not directory: {norm_path}")
        index_path = workspace_dir / "index.json"
        if not index_path.exists():
            raise WorkspaceError(f"Missing index.json in workspace: {norm_path}")

        try:
            with index_path.open("r", encoding="utf-8") as handle:
                index_config = json.load(handle)
        except json.JSONDecodeError as exc:
            raise WorkspaceError(f"Invalid index.json: {exc}") from exc

        try:
            vindex = larql.load(norm_path)
        except Exception as exc:  # pragma: no cover - depends on native binding failures
            raise WorkspaceError(str(exc)) from exc

        stats = vindex.stats()
        relations = vindex.relations()
        layer_bands = vindex.layer_bands()

        num_clusters = int(stats.get("num_clusters", 0) or 0)
        num_probe_labels = int(stats.get("num_probe_labels", 0) or 0)

        relation_count = len(relations)
        relation_labels: list[dict[str, Any]] = []
        for rel in relations[:RELATION_LABEL_PREVIEW_CAP]:
            tops = list(rel.top_tokens) if rel.top_tokens else []
            relation_labels.append(
                {
                    "name": rel.name,
                    "count": rel.count,
                    "cluster_id": rel.cluster_id,
                    "top_tokens": tops[:TOP_TOKENS_PER_RELATION],
                }
            )

        probe_rels = vindex.probe_relations()
        probe_relation_name_count = len(probe_rels)
        probe_relation_labels: list[dict[str, Any]] = []
        for pr in probe_rels[:PROBE_RELATION_PREVIEW_CAP]:
            probe_relation_labels.append({"name": pr.name, "count": pr.count})

        mlx_available = find_spec("mlx") is not None and find_spec("mlx_lm") is not None
        has_model_weights = bool(index_config.get("has_model_weights", False))
        has_relation_labels = relation_count > 0 or num_probe_labels > 0
        has_layer_bands = layer_bands is not None
        warnings: list[str] = []
        if not has_model_weights:
            warnings.append("No model weights. Browse and LQL ready. Inference and trace disabled.")
        if relation_count == 0 and num_probe_labels == 0:
            warnings.append(
                "No cluster relation catalogue (relations() empty) and no probe labels — describe edges may be sparse."
            )
        elif relation_count == 0 and num_probe_labels > 0:
            warnings.append(
                "No cluster catalogue (relations() empty); probe labels present — describe may still show relations with source=probe."
            )
        if not mlx_available:
            warnings.append("MLX deps missing. MLX generation disabled.")

        summary = WorkspaceSummary(
            id=slugify(workspace_dir.name),
            path=norm_path,
            display_name=workspace_dir.name,
            model=str(stats.get("model", workspace_dir.name)),
            family=str(stats.get("family", "unknown")),
            num_layers=int(stats.get("num_layers", 0)),
            hidden_size=int(stats.get("hidden_size", 0)),
            vocab_size=int(stats.get("vocab_size", 0)),
            extract_level=index_config.get("extract_level"),
            has_model_weights=has_model_weights,
            has_relation_labels=has_relation_labels,
            has_layer_bands=has_layer_bands,
            supports_infer=has_model_weights,
            supports_trace=has_model_weights,
            supports_mlx=mlx_available and has_model_weights,
            supports_streaming=mlx_available and has_model_weights,
            supports_walk_ffn=mlx_available and has_model_weights,
            warnings=warnings,
            relation_count=relation_count,
            relation_labels=relation_labels,
            num_clusters=num_clusters,
            num_probe_labels=num_probe_labels,
            probe_relation_name_count=probe_relation_name_count,
            probe_relation_labels=probe_relation_labels,
        )
        self._cache[cache_key] = summary
        self.store.set_current_workspace_path(norm_path)
        return summary
