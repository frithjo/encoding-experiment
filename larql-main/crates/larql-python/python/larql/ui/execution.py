from __future__ import annotations

from time import perf_counter
from typing import Any

import larql

from .models import RecipeRecord, RunRecord, execution_engine_for_recipe
from .runtime_cache import LarqlRuntimeCache
from .store import UiStore
from .workspace import WorkspaceManager


def describe_edge_to_dict(edge: Any) -> dict[str, Any]:
    return {
        "relation": edge.relation,
        "target": edge.target,
        "gate_score": edge.gate_score,
        "layer": edge.layer,
        "feature": edge.feature,
        "source": edge.source,
        "confidence": edge.confidence,
        "also": list(edge.also),
    }


def predictions_to_raw(preds: list[tuple[str, float]]) -> list[dict[str, Any]]:
    return [{"token": token, "probability": float(prob)} for token, prob in preds]


def residual_trace_to_raw(t: Any) -> dict[str, Any]:
    """Serialize `larql.ResidualTrace` into JSON-safe dict for `RunRecord.raw`."""
    summaries = t.summary()
    return {
        "prompt": t.prompt,
        "tokens": list(t.tokens),
        "n_layers": t.n_layers,
        "hidden_size": t.hidden_size,
        "n_nodes": t.n_nodes,
        "layer_summaries": [
            {
                "layer": s.layer,
                "residual_norm": s.residual_norm,
                "attn_delta_norm": s.attn_delta_norm,
                "ffn_delta_norm": s.ffn_delta_norm,
                "top1_token": s.top1_token,
                "top1_prob": s.top1_prob,
            }
            for s in summaries
        ],
    }


class UiExecutor:
    def __init__(
        self,
        store: UiStore,
        workspace_manager: WorkspaceManager,
        runtime_cache: LarqlRuntimeCache,
    ) -> None:
        self.store = store
        self.workspace_manager = workspace_manager
        self._runtime_cache = runtime_cache

    def run_describe(
        self,
        *,
        workspace_path: str,
        entity: str,
        band: str = "knowledge",
        verbose: bool = False,
        recipe_id: str | None = None,
        title: str | None = None,
        run_id: str | None = None,
    ) -> RunRecord:
        self.workspace_manager.open(workspace_path)
        vindex = self._runtime_cache.vindex(workspace_path)
        started = perf_counter()
        edges = vindex.describe(entity, band=band, verbose=verbose)
        duration_ms = int((perf_counter() - started) * 1000)
        edge_dicts = [describe_edge_to_dict(edge) for edge in edges]
        summary = (
            f"{len(edge_dicts)} edges for {entity}"
            if edge_dicts
            else f"No edges found for {entity}"
        )
        run = RunRecord.create(
            kind="describe",
            title=title or f"Describe {entity}",
            workspace_path=workspace_path,
            recipe_id=recipe_id,
            engine="describe",
            summary=summary,
            input_text=entity,
            raw={
                "entity": entity,
                "band": band,
                "verbose": verbose,
                "edges": edge_dicts,
            },
            duration_ms=duration_ms,
            run_id=run_id,
        )
        self.store.save_run(run)
        return run

    def run_lql(
        self,
        *,
        workspace_path: str,
        query: str,
        recipe_id: str | None = None,
        title: str | None = None,
        run_id: str | None = None,
    ) -> RunRecord:
        summary = self.workspace_manager.open(workspace_path)
        if not summary.supports_lql:
            raise RuntimeError(
                "LQL session is not available for this workspace (session probe failed during inspection)."
            )
        session = self._runtime_cache.session(workspace_path)
        started = perf_counter()
        lines = session.query(query)
        duration_ms = int((perf_counter() - started) * 1000)
        summary = f"{len(lines)} lines returned" if lines else "Query returned no lines"
        run = RunRecord.create(
            kind="lql",
            title=title or "LQL query",
            workspace_path=workspace_path,
            recipe_id=recipe_id,
            engine="lql",
            summary=summary,
            input_text=query,
            raw={"query": query, "lines": lines},
            duration_ms=duration_ms,
            run_id=run_id,
        )
        self.store.save_run(run)
        return run

    def run_infer(
        self,
        *,
        workspace_path: str,
        prompt: str,
        top_k_predictions: int = 5,
        recipe_id: str | None = None,
        title: str | None = None,
        run_id: str | None = None,
    ) -> RunRecord:
        summary = self.workspace_manager.open(workspace_path)
        if not summary.supports_infer:
            raise RuntimeError(
                "Inference requires model weights. Extract vindex with --level all (or equivalent)."
            )
        vindex = self._runtime_cache.vindex(workspace_path)
        started = perf_counter()
        preds = vindex.infer(prompt, top_k_predictions=top_k_predictions)
        duration_ms = int((perf_counter() - started) * 1000)
        raw_preds = predictions_to_raw(preds)
        top = raw_preds[0] if raw_preds else None
        summary_text = (
            f"Top: {top['token']} ({top['probability']:.1%})" if top else "No predictions"
        )
        run = RunRecord.create(
            kind="infer",
            title=title or "Infer",
            workspace_path=workspace_path,
            recipe_id=recipe_id,
            engine="infer",
            summary=summary_text,
            input_text=prompt,
            raw={
                "prompt": prompt,
                "top_k_predictions": top_k_predictions,
                "predictions": raw_preds,
            },
            duration_ms=duration_ms,
            run_id=run_id,
        )
        self.store.save_run(run)
        return run

    def run_walk_model(
        self,
        *,
        workspace_path: str,
        prompt: str,
        top_k_predictions: int = 5,
        top_k_features: int = 8192,
        recipe_id: str | None = None,
        title: str | None = None,
        run_id: str | None = None,
    ) -> RunRecord:
        summary = self.workspace_manager.open(workspace_path)
        if not summary.supports_walk_model:
            raise RuntimeError(
                "WalkModel is not available for this workspace (load failed during inspection, or no weights)."
            )
        started = perf_counter()
        wm = larql.WalkModel(workspace_path, top_k=top_k_features)
        preds = wm.predict(prompt, top_k_predictions=top_k_predictions)
        duration_ms = int((perf_counter() - started) * 1000)
        raw_preds = predictions_to_raw(preds)
        top = raw_preds[0] if raw_preds else None
        summary_text = (
            f"Top: {top['token']} ({top['probability']:.1%})" if top else "No predictions"
        )
        run = RunRecord.create(
            kind="walk_model",
            title=title or "WalkModel",
            workspace_path=workspace_path,
            recipe_id=recipe_id,
            engine="walk_model",
            summary=summary_text,
            input_text=prompt,
            raw={
                "prompt": prompt,
                "top_k_predictions": top_k_predictions,
                "walk_top_k_features": top_k_features,
                "predictions": raw_preds,
            },
            duration_ms=duration_ms,
            run_id=run_id,
        )
        self.store.save_run(run)
        return run

    def run_trace(
        self,
        *,
        workspace_path: str,
        prompt: str,
        positions: str = "last",
        walk_top_k: int = 8192,
        recipe_id: str | None = None,
        title: str | None = None,
        run_id: str | None = None,
    ) -> RunRecord:
        summary = self.workspace_manager.open(workspace_path)
        if not summary.supports_trace:
            raise RuntimeError(
                "Trace requires model weights. Extract vindex with --level all (or equivalent)."
            )
        positions_norm = (positions or "last").strip().lower()
        if positions_norm not in ("last", "all"):
            raise ValueError('positions must be "last" or "all"')

        started = perf_counter()
        wm = larql.WalkModel(workspace_path, top_k=walk_top_k)
        rt = wm.trace(prompt, positions_norm)
        duration_ms = int((perf_counter() - started) * 1000)
        raw = residual_trace_to_raw(rt)
        raw["positions"] = positions_norm
        raw["walk_top_k_features"] = walk_top_k
        summary_text = (
            f"Residual trace: {raw['n_nodes']} nodes, {raw['n_layers']} layers, "
            f"{len(raw['tokens'])} tokens"
        )
        run = RunRecord.create(
            kind="trace",
            title=title or "Trace",
            workspace_path=workspace_path,
            recipe_id=recipe_id,
            engine="trace",
            summary=summary_text,
            input_text=prompt,
            raw=raw,
            duration_ms=duration_ms,
            run_id=run_id,
        )
        self.store.save_run(run)
        return run

    def run_recipe(
        self,
        *,
        workspace_path: str,
        recipe: RecipeRecord,
        variables: dict[str, str],
        run_id: str | None = None,
    ) -> RunRecord:
        rendered = recipe.rendered(variables)
        if recipe.kind in ("describe", "lql", "infer", "walk_model"):
            if recipe.kind == "describe":
                return self.run_describe(
                    workspace_path=workspace_path,
                    entity=rendered,
                    band=recipe.describe_band,
                    verbose=recipe.describe_verbose,
                    recipe_id=recipe.id,
                    title=recipe.name,
                    run_id=run_id,
                )
            if recipe.kind == "lql":
                return self.run_lql(
                    workspace_path=workspace_path,
                    query=rendered,
                    recipe_id=recipe.id,
                    title=recipe.name,
                    run_id=run_id,
                )
            if recipe.kind == "infer":
                return self.run_infer(
                    workspace_path=workspace_path,
                    prompt=rendered,
                    top_k_predictions=recipe.infer_top_k_predictions,
                    recipe_id=recipe.id,
                    title=recipe.name,
                    run_id=run_id,
                )
            return self.run_walk_model(
                workspace_path=workspace_path,
                prompt=rendered,
                top_k_predictions=recipe.infer_top_k_predictions,
                top_k_features=recipe.walk_top_k,
                recipe_id=recipe.id,
                title=recipe.name,
                run_id=run_id,
            )
        if recipe.kind in ("probe", "generation"):
            eng = execution_engine_for_recipe(recipe)
            return self.run_studio(
                workspace_path=workspace_path,
                engine=eng,
                rendered=rendered,
                recipe_id=recipe.id,
                title=recipe.name,
                band=recipe.describe_band,
                verbose=recipe.describe_verbose,
                infer_top_k_predictions=recipe.infer_top_k_predictions,
                walk_top_k=recipe.walk_top_k,
                run_id=run_id,
            )
        raise ValueError(f"Unsupported recipe kind: {recipe.kind}")

    def run_studio(
        self,
        *,
        workspace_path: str,
        engine: str,
        rendered: str,
        recipe_id: str | None = None,
        title: str | None = None,
        band: str = "knowledge",
        verbose: bool = False,
        infer_top_k_predictions: int = 5,
        walk_top_k: int = 8192,
        run_id: str | None = None,
    ) -> RunRecord:
        eng = engine.strip()
        if eng == "describe":
            return self.run_describe(
                workspace_path=workspace_path,
                entity=rendered,
                band=band,
                verbose=verbose,
                recipe_id=recipe_id,
                title=title,
                run_id=run_id,
            )
        if eng == "lql":
            return self.run_lql(
                workspace_path=workspace_path,
                query=rendered,
                recipe_id=recipe_id,
                title=title,
                run_id=run_id,
            )
        if eng == "infer":
            return self.run_infer(
                workspace_path=workspace_path,
                prompt=rendered,
                top_k_predictions=infer_top_k_predictions,
                recipe_id=recipe_id,
                title=title,
                run_id=run_id,
            )
        if eng == "walk_model":
            return self.run_walk_model(
                workspace_path=workspace_path,
                prompt=rendered,
                top_k_predictions=infer_top_k_predictions,
                top_k_features=walk_top_k,
                recipe_id=recipe_id,
                title=title,
                run_id=run_id,
            )
        raise ValueError(f"Unknown engine: {engine}")
