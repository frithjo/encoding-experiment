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
_WORKSPACE_CACHE_VERSION = 6
PROBE_RELATION_PREVIEW_CAP = 60
_WALK_MODEL_PROBE_TOP_K = 64

try:
    from larql._native import inspect_workspace_for_ui as _inspect_workspace_native
except ImportError:
    _inspect_workspace_native = None


class WorkspaceError(RuntimeError):
    pass


def _workspace_summary_from_native_dict(raw: dict[str, Any]) -> WorkspaceSummary:
    """Build `WorkspaceSummary` from `inspect_workspace_for_ui` (Rust) output."""
    el = raw.get("extract_level")
    return WorkspaceSummary(
        id=str(raw["id"]),
        path=str(raw["path"]),
        display_name=str(raw["display_name"]),
        model=str(raw["model"]),
        family=str(raw["family"]),
        num_layers=int(raw["num_layers"]),
        hidden_size=int(raw["hidden_size"]),
        vocab_size=int(raw["vocab_size"]),
        extract_level=str(el) if el is not None else None,
        has_model_weights=bool(raw["has_model_weights"]),
        has_relation_labels=bool(raw["has_relation_labels"]),
        has_layer_bands=bool(raw["has_layer_bands"]),
        supports_infer=bool(raw["supports_infer"]),
        supports_trace=bool(raw["supports_trace"]),
        supports_mlx=bool(raw["supports_mlx"]),
        supports_streaming=bool(raw["supports_streaming"]),
        supports_walk_ffn=bool(raw["supports_walk_ffn"]),
        supports_lql=bool(raw["supports_lql"]),
        supports_walk_model=bool(raw["supports_walk_model"]),
        warnings=list(raw["warnings"]),
        relation_count=int(raw["relation_count"]),
        relation_labels=list(raw["relation_labels"]),
        num_clusters=int(raw["num_clusters"]),
        num_probe_labels=int(raw["num_probe_labels"]),
        probe_relation_name_count=int(raw["probe_relation_name_count"]),
        probe_relation_labels=list(raw["probe_relation_labels"]),
    )


def _load_workspace_python(norm_path: str, workspace_dir: Path) -> WorkspaceSummary:
    """Fallback when native `inspect_workspace_for_ui` is unavailable — keep Python path minimal."""
    index_path = workspace_dir / "index.json"
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

    supports_lql = False
    lql_probe_error: str | None = None
    try:
        sess = larql.session(norm_path)
        _ = sess.query("STATS")
        supports_lql = True
        del sess
    except Exception as exc:  # pragma: no cover - rare native/session failures
        lql_probe_error = str(exc)

    supports_walk_model = False
    walk_model_error: str | None = None
    if has_model_weights:
        try:
            wm = larql.WalkModel(norm_path, top_k=_WALK_MODEL_PROBE_TOP_K)
            supports_walk_model = True
            del wm
        except Exception as exc:  # pragma: no cover - mmap / tokenizer edge cases
            walk_model_error = str(exc)

    warnings: list[str] = []
    if not supports_lql:
        detail = lql_probe_error or "unknown error"
        warnings.append(f"LQL session probe failed: {detail}")
    if has_model_weights and not supports_walk_model:
        detail = walk_model_error or "unknown error"
        warnings.append(f"WalkModel load failed: {detail}")
    if not has_model_weights:
        if supports_lql:
            warnings.append(
                "No model weights. Browse and LQL ready. Inference, trace, and mmap WalkModel disabled."
            )
        else:
            warnings.append(
                "No model weights. Inference, trace, and mmap WalkModel disabled."
            )
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

    return WorkspaceSummary(
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
        supports_lql=supports_lql,
        supports_walk_model=supports_walk_model,
        warnings=warnings,
        relation_count=relation_count,
        relation_labels=relation_labels,
        num_clusters=num_clusters,
        num_probe_labels=num_probe_labels,
        probe_relation_name_count=probe_relation_name_count,
        probe_relation_labels=probe_relation_labels,
    )


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
            summary = self._cache[cache_key]
            self.store.set_current_workspace_path(norm_path)
            return summary

        workspace_dir = Path(norm_path)
        if not workspace_dir.exists():
            raise WorkspaceError(f"Workspace path does not exist: {norm_path}")
        if not workspace_dir.is_dir():
            raise WorkspaceError(f"Workspace path is not directory: {norm_path}")
        if not (workspace_dir / "index.json").exists():
            raise WorkspaceError(f"Missing index.json in workspace: {norm_path}")

        if _inspect_workspace_native is not None:
            mlx_available = find_spec("mlx") is not None and find_spec("mlx_lm") is not None
            try:
                raw = _inspect_workspace_native(norm_path, mlx_available)
            except Exception as exc:
                raise WorkspaceError(str(exc)) from exc
            summary = _workspace_summary_from_native_dict(raw)
        else:
            summary = _load_workspace_python(norm_path, workspace_dir)

        self._cache[cache_key] = summary
        self.store.set_current_workspace_path(norm_path)
        return summary
