"""JSON /api/* routes from plan.md — complement page forms and enable fetch-based clients."""

from __future__ import annotations

import asyncio
import json
from dataclasses import replace
from typing import Any, Callable

import larql
from starlette import status
from starlette.requests import Request
from starlette.responses import JSONResponse, Response

from .execution import UiExecutor
from .models import RecipeRecord, RunRecord, validate_recipe
from .store import UiStore
from .workspace import WorkspaceError, WorkspaceManager


async def _read_json(request: Request) -> dict[str, Any]:
    if not request.headers.get("content-type", "").startswith("application/json"):
        return {}
    try:
        body = await request.body()
        raw = json.loads(body.decode("utf-8") or "{}")
    except (json.JSONDecodeError, UnicodeError):
        return {}
    return raw if isinstance(raw, dict) else {}


def _relations_payload(vindex: Any) -> list[dict[str, Any]]:
    out: list[dict[str, Any]] = []
    for rel in vindex.relations():
        out.append(
            {
                "name": rel.name,
                "count": rel.count,
                "cluster_id": rel.cluster_id,
            }
        )
    return out


def _schedule_async(app_holder: list[Any], coro_factory: Callable[[], Any]) -> None:
    """Run coroutine on the current event loop; retain strong ref via app.state."""
    app = app_holder[0]
    if not hasattr(app.state, "_ui_bg_tasks"):
        app.state._ui_bg_tasks = set()  # type: ignore[attr-defined]
    task = asyncio.create_task(coro_factory())
    app.state._ui_bg_tasks.add(task)  # type: ignore[attr-defined]

    def _done(t: asyncio.Task) -> None:
        app.state._ui_bg_tasks.discard(t)  # type: ignore[attr-defined]
        if t.cancelled() or t.exception() is None:
            return
        exc = t.exception()
        if exc is not None:
            print(f"[larql-ui] background task failed: {exc!r}")  # noqa: T201

    task.add_done_callback(_done)


def build_api_routes(
    *,
    app_holder: list[Any],
    ui_store: UiStore,
    workspace_manager: WorkspaceManager,
    executor: UiExecutor,
) -> list[Any]:
    from starlette.routing import Route

    async def api_workspace_open(request: Request) -> Response:
        data = await _read_json(request)
        path = str(data.get("path", "")).strip()
        if not path:
            return JSONResponse({"error": "path required"}, status_code=status.HTTP_400_BAD_REQUEST)
        try:
            workspace_manager.open(path)
        except WorkspaceError as exc:
            return JSONResponse({"error": str(exc)}, status_code=status.HTTP_400_BAD_REQUEST)
        try:
            summary = workspace_manager.get_current()
        except WorkspaceError as exc:
            return JSONResponse({"error": str(exc)}, status_code=status.HTTP_400_BAD_REQUEST)
        return JSONResponse({"ok": True, "workspace": summary.to_dict()})

    async def api_workspace_current(request: Request) -> Response:
        try:
            summary = workspace_manager.get_current()
        except WorkspaceError as exc:
            return JSONResponse({"error": str(exc)}, status_code=status.HTTP_404_NOT_FOUND)
        if summary is None:
            return JSONResponse({"error": "no workspace open"}, status_code=status.HTTP_404_NOT_FOUND)
        return JSONResponse({"workspace": summary.to_dict()})

    async def api_recipes_post(request: Request) -> Response:
        data = await _read_json(request)
        try:
            if data.get("id"):
                recipe = RecipeRecord.from_dict(data)
            else:
                name = str(data.get("name", "")).strip()
                kind = str(data.get("kind", "describe")).strip() or "describe"
                if not name:
                    return JSONResponse({"error": "name required"}, status_code=status.HTTP_400_BAD_REQUEST)
                recipe = RecipeRecord.create(
                    name=name,
                    kind=kind,
                    template=str(data.get("template", "{subject}")).strip(),
                    lql_template=str(data.get("lql_template", "")).strip() or None,
                    description=str(data.get("description", "")).strip(),
                    default_engine=str(data.get("default_engine", "")).strip() or None,
                    infer_top_k_predictions=int(data.get("infer_top_k_predictions", 5) or 5),
                    walk_top_k=int(data.get("walk_top_k", 8192) or 8192),
                    describe_band=str(data.get("describe_band", "knowledge")).strip() or "knowledge",
                    describe_verbose=bool(data.get("describe_verbose", False)),
                    tags=[str(t) for t in data.get("tags", [])] if isinstance(data.get("tags"), list) else [],
                    notes=str(data.get("notes", "")).strip(),
                    options=data.get("options") if isinstance(data.get("options"), dict) else {},
                    variable_defs=list(data["variable_defs"])
                    if isinstance(data.get("variable_defs"), list)
                    else [],
                )
        except (KeyError, TypeError, ValueError) as exc:
            return JSONResponse({"error": f"Invalid recipe: {exc}"}, status_code=status.HTTP_400_BAD_REQUEST)
        try:
            validate_recipe(recipe)
        except ValueError as exc:
            return JSONResponse({"error": str(exc)}, status_code=status.HTTP_400_BAD_REQUEST)
        ui_store.save_recipe(recipe)
        return JSONResponse({"recipe": recipe.to_dict()}, status_code=status.HTTP_201_CREATED)

    async def api_recipes_put(request: Request) -> Response:
        rid = request.path_params["recipe_id"]
        existing = ui_store.get_recipe(rid)
        if existing is None:
            return JSONResponse({"error": "not found"}, status_code=status.HTTP_404_NOT_FOUND)
        data = await _read_json(request)
        merged = existing.to_dict()
        merged.update(data)
        merged["id"] = existing.id
        try:
            recipe = RecipeRecord.from_dict(merged)
            validate_recipe(recipe)
        except (KeyError, TypeError, ValueError) as exc:
            return JSONResponse({"error": str(exc)}, status_code=status.HTTP_400_BAD_REQUEST)
        ui_store.save_recipe(recipe)
        return JSONResponse({"recipe": recipe.to_dict()})

    async def api_recipes_delete(request: Request) -> Response:
        rid = request.path_params["recipe_id"]
        if ui_store.get_recipe(rid) is None:
            return JSONResponse({"error": "not found"}, status_code=status.HTTP_404_NOT_FOUND)
        ui_store.delete_recipe(rid)
        return JSONResponse({"ok": True})

    async def api_runs_get(request: Request) -> Response:
        rid = request.path_params["run_id"]
        run = ui_store.get_run(rid)
        if run is None:
            return JSONResponse({"error": "not found"}, status_code=status.HTTP_404_NOT_FOUND)
        return JSONResponse({"run": run.to_dict()})

    async def api_runs_post(request: Request) -> Response:
        data = await _read_json(request)
        async_flag = bool(data.get("async"))
        try:
            ws_path = str(data.get("workspace_path") or "").strip() or ui_store.current_workspace_path()
        except Exception:
            ws_path = ui_store.current_workspace_path()
        if not ws_path:
            return JSONResponse({"error": "no workspace; open one or pass workspace_path"}, status_code=400)
        try:
            workspace_manager.open(ws_path)
        except WorkspaceError as exc:
            return JSONResponse({"error": str(exc)}, status_code=400)

        engine = str(data.get("engine", "")).strip()
        rendered = str(data.get("rendered", "")).strip()
        recipe_id = str(data.get("recipe_id", "")).strip() or None
        title = str(data.get("title", "")).strip() or None
        band = str(data.get("band", "knowledge")).strip() or "knowledge"
        verbose = bool(data.get("verbose", False))
        infer_top = int(data.get("infer_top_k_predictions", 5) or 5)
        walk_k = int(data.get("walk_top_k", 8192) or 8192)

        if not engine or not rendered:
            return JSONResponse({"error": "engine and rendered required"}, status_code=400)

        if not async_flag:

            def _go() -> RunRecord:
                return executor.run_studio(
                    workspace_path=ws_path,
                    engine=engine,
                    rendered=rendered,
                    recipe_id=recipe_id,
                    title=title,
                    band=band,
                    verbose=verbose,
                    infer_top_k_predictions=max(1, infer_top),
                    walk_top_k=max(1, walk_k),
                )

            try:
                run = await asyncio.to_thread(_go)
            except Exception as exc:
                return JSONResponse({"error": str(exc)}, status_code=500)
            return JSONResponse({"run": run.to_dict()})

        pending = RunRecord.create(
            kind="studio",
            title=title or f"Studio ({engine})",
            workspace_path=ws_path,
            recipe_id=recipe_id,
            engine=engine,
            status="pending",
            summary="Queued",
            input_text=rendered,
            raw={"async_job": True, "band": band, "verbose": verbose, "infer_top_k_predictions": infer_top, "walk_top_k": walk_k},
            duration_ms=0,
        )
        ui_store.save_run(pending)
        rid = pending.id

        async def _bg() -> None:
            cur = ui_store.get_run(rid)
            if cur:
                ui_store.save_run(replace(cur, status="running", summary="Running…"))
            try:

                def _work() -> RunRecord:
                    return executor.run_studio(
                        workspace_path=ws_path,
                        engine=engine,
                        rendered=rendered,
                        recipe_id=recipe_id,
                        title=title,
                        band=band,
                        verbose=verbose,
                        infer_top_k_predictions=max(1, infer_top),
                        walk_top_k=max(1, walk_k),
                        run_id=rid,
                    )

                await asyncio.to_thread(_work)
            except Exception as exc:
                old = ui_store.get_run(rid)
                if old:
                    ui_store.save_run(
                        replace(
                            old,
                            status="error",
                            summary=str(exc),
                            raw={**old.raw, "error": str(exc)},
                            duration_ms=0,
                        )
                    )

        _schedule_async(app_holder, _bg)
        return JSONResponse({"run_id": rid, "status": "pending"}, status_code=status.HTTP_202_ACCEPTED)

    async def api_runs_rerun(request: Request) -> Response:
        rid = request.path_params["run_id"]
        prev = ui_store.get_run(rid)
        if prev is None:
            return JSONResponse({"error": "not found"}, status_code=404)
        data = await _read_json(request)
        async_flag = bool(data.get("async"))
        ws_path = prev.workspace_path

        async def _rerun_sync() -> Response:
            try:
                if prev.kind == "studio":
                    run = executor.run_studio(
                        workspace_path=ws_path,
                        engine=prev.engine,
                        rendered=prev.input_text,
                        recipe_id=prev.recipe_id,
                        title=prev.title,
                        band=str(prev.raw.get("band", "knowledge")),
                        verbose=bool(prev.raw.get("verbose", False)),
                        infer_top_k_predictions=int(prev.raw.get("infer_top_k_predictions", 5)),
                        walk_top_k=int(prev.raw.get("walk_top_k", 8192)),
                    )
                    return JSONResponse({"run": run.to_dict()})
                if prev.engine == "describe":
                    raw = prev.raw
                    run = executor.run_describe(
                        workspace_path=ws_path,
                        entity=str(raw.get("entity", prev.input_text)),
                        band=str(raw.get("band", "knowledge")),
                        verbose=bool(raw.get("verbose", False)),
                        recipe_id=prev.recipe_id,
                        title=prev.title,
                    )
                elif prev.engine == "lql":
                    run = executor.run_lql(
                        workspace_path=ws_path,
                        query=str(prev.raw.get("query", prev.input_text)),
                        recipe_id=prev.recipe_id,
                        title=prev.title,
                    )
                elif prev.engine == "infer":
                    run = executor.run_infer(
                        workspace_path=ws_path,
                        prompt=str(prev.raw.get("prompt", prev.input_text)),
                        top_k_predictions=int(prev.raw.get("top_k_predictions", 5)),
                        recipe_id=prev.recipe_id,
                        title=prev.title,
                    )
                elif prev.engine == "walk_model":
                    run = executor.run_walk_model(
                        workspace_path=ws_path,
                        prompt=str(prev.raw.get("prompt", prev.input_text)),
                        top_k_predictions=int(prev.raw.get("top_k_predictions", 5)),
                        top_k_features=int(prev.raw.get("walk_top_k_features", 8192)),
                        recipe_id=prev.recipe_id,
                        title=prev.title,
                    )
                elif prev.engine == "trace":
                    run = executor.run_trace(
                        workspace_path=ws_path,
                        prompt=str(prev.raw.get("prompt", prev.input_text)),
                        positions=str(prev.raw.get("positions", "last")),
                        walk_top_k=int(prev.raw.get("walk_top_k_features", 8192)),
                        recipe_id=prev.recipe_id,
                        title=prev.title,
                    )
                else:
                    return JSONResponse({"error": f"cannot rerun engine {prev.engine}"}, status_code=400)
            except Exception as exc:
                return JSONResponse({"error": str(exc)}, status_code=500)
            return JSONResponse({"run": run.to_dict()})

        if not async_flag:
            return await _rerun_sync()

        pending = RunRecord.create(
            kind=prev.kind,
            title=f"Rerun: {prev.title}",
            workspace_path=ws_path,
            recipe_id=prev.recipe_id,
            engine=prev.engine,
            status="pending",
            summary="Queued (rerun)",
            input_text=prev.input_text,
            raw={**prev.raw, "rerun_of": prev.id},
            duration_ms=0,
        )
        ui_store.save_run(pending)
        new_id = pending.id

        async def _bg() -> None:
            try:
                if prev.kind == "studio":

                    def _w() -> RunRecord:
                        return executor.run_studio(
                            workspace_path=ws_path,
                            engine=prev.engine,
                            rendered=prev.input_text,
                            recipe_id=prev.recipe_id,
                            title=pending.title,
                            band=str(prev.raw.get("band", "knowledge")),
                            verbose=bool(prev.raw.get("verbose", False)),
                            infer_top_k_predictions=int(prev.raw.get("infer_top_k_predictions", 5)),
                            walk_top_k=int(prev.raw.get("walk_top_k", 8192)),
                            run_id=new_id,
                        )

                    await asyncio.to_thread(_w)
                elif prev.engine == "describe":
                    raw = prev.raw

                    def _w() -> RunRecord:
                        return executor.run_describe(
                            workspace_path=ws_path,
                            entity=str(raw.get("entity", prev.input_text)),
                            band=str(raw.get("band", "knowledge")),
                            verbose=bool(raw.get("verbose", False)),
                            recipe_id=prev.recipe_id,
                            title=pending.title,
                            run_id=new_id,
                        )

                    await asyncio.to_thread(_w)
                elif prev.engine == "lql":

                    def _w() -> RunRecord:
                        return executor.run_lql(
                            workspace_path=ws_path,
                            query=str(prev.raw.get("query", prev.input_text)),
                            recipe_id=prev.recipe_id,
                            title=pending.title,
                            run_id=new_id,
                        )

                    await asyncio.to_thread(_w)
                elif prev.engine == "infer":

                    def _w() -> RunRecord:
                        return executor.run_infer(
                            workspace_path=ws_path,
                            prompt=str(prev.raw.get("prompt", prev.input_text)),
                            top_k_predictions=int(prev.raw.get("top_k_predictions", 5)),
                            recipe_id=prev.recipe_id,
                            title=pending.title,
                            run_id=new_id,
                        )

                    await asyncio.to_thread(_w)
                elif prev.engine == "walk_model":

                    def _w() -> RunRecord:
                        return executor.run_walk_model(
                            workspace_path=ws_path,
                            prompt=str(prev.raw.get("prompt", prev.input_text)),
                            top_k_predictions=int(prev.raw.get("top_k_predictions", 5)),
                            top_k_features=int(prev.raw.get("walk_top_k_features", 8192)),
                            recipe_id=prev.recipe_id,
                            title=pending.title,
                            run_id=new_id,
                        )

                    await asyncio.to_thread(_w)
                elif prev.engine == "trace":

                    def _w() -> RunRecord:
                        return executor.run_trace(
                            workspace_path=ws_path,
                            prompt=str(prev.raw.get("prompt", prev.input_text)),
                            positions=str(prev.raw.get("positions", "last")),
                            walk_top_k=int(prev.raw.get("walk_top_k_features", 8192)),
                            recipe_id=prev.recipe_id,
                            title=pending.title,
                            run_id=new_id,
                        )

                    await asyncio.to_thread(_w)
                else:
                    raise RuntimeError(f"cannot rerun engine {prev.engine}")
            except Exception as exc:
                old = ui_store.get_run(new_id)
                if old:
                    ui_store.save_run(
                        replace(
                            old,
                            status="error",
                            summary=str(exc),
                            raw={**old.raw, "error": str(exc)},
                        )
                    )

        _schedule_async(app_holder, _bg)
        return JSONResponse({"run_id": new_id, "status": "pending"}, status_code=status.HTTP_202_ACCEPTED)

    async def api_explorer_describe(request: Request) -> Response:
        data = await _read_json(request)
        try:
            ws = workspace_manager.get_current()
        except WorkspaceError as exc:
            return JSONResponse({"error": str(exc)}, status_code=400)
        if ws is None:
            return JSONResponse({"error": "no workspace open"}, status_code=400)
        ws_path = ws.path
        entity = str(data.get("entity", "")).strip()
        if not entity:
            return JSONResponse({"error": "entity required"}, status_code=400)
        band = str(data.get("band", "knowledge")).strip() or "knowledge"
        verbose = bool(data.get("verbose", False))
        async_flag = bool(data.get("async"))

        if not async_flag:

            def _go() -> RunRecord:
                return executor.run_describe(
                    workspace_path=ws_path,
                    entity=entity,
                    band=band,
                    verbose=verbose,
                )

            try:
                run = await asyncio.to_thread(_go)
            except Exception as exc:
                return JSONResponse({"error": str(exc)}, status_code=500)
            return JSONResponse({"run": run.to_dict()})

        pending = RunRecord.create(
            kind="describe",
            title=f"Describe {entity}",
            workspace_path=ws_path,
            recipe_id=None,
            engine="describe",
            status="pending",
            summary="Queued",
            input_text=entity,
            raw={"async_job": True, "band": band, "verbose": verbose},
            duration_ms=0,
        )
        ui_store.save_run(pending)
        rid = pending.id

        async def _bg() -> None:
            try:

                def _w() -> RunRecord:
                    return executor.run_describe(
                        workspace_path=ws_path,
                        entity=entity,
                        band=band,
                        verbose=verbose,
                        run_id=rid,
                    )

                await asyncio.to_thread(_w)
            except Exception as exc:
                old = ui_store.get_run(rid)
                if old:
                    ui_store.save_run(
                        replace(old, status="error", summary=str(exc), raw={**old.raw, "error": str(exc)})
                    )

        _schedule_async(app_holder, _bg)
        return JSONResponse({"run_id": rid, "status": "pending"}, status_code=202)

    async def api_explorer_relations(request: Request) -> Response:
        try:
            ws = workspace_manager.get_current()
        except WorkspaceError as exc:
            return JSONResponse({"error": str(exc)}, status_code=400)
        if ws is None:
            return JSONResponse({"error": "no workspace open"}, status_code=400)
        try:

            def _go() -> list[dict[str, Any]]:
                workspace_manager.open(ws.path)
                v = larql.load(ws.path)
                return _relations_payload(v)

            rels = await asyncio.to_thread(_go)
        except Exception as exc:
            return JSONResponse({"error": str(exc)}, status_code=500)
        return JSONResponse({"relations": rels})

    async def api_lql_query(request: Request) -> Response:
        data = await _read_json(request)
        try:
            ws = workspace_manager.get_current()
        except WorkspaceError as exc:
            return JSONResponse({"error": str(exc)}, status_code=400)
        if ws is None:
            return JSONResponse({"error": "no workspace open"}, status_code=400)
        ws_path = ws.path
        query = str(data.get("query", "")).strip()
        if not query:
            return JSONResponse({"error": "query required"}, status_code=400)
        async_flag = bool(data.get("async"))

        if not async_flag:

            def _go() -> RunRecord:
                return executor.run_lql(workspace_path=ws_path, query=query)

            try:
                run = await asyncio.to_thread(_go)
            except Exception as exc:
                return JSONResponse({"error": str(exc)}, status_code=500)
            return JSONResponse({"run": run.to_dict()})

        pending = RunRecord.create(
            kind="lql",
            title="LQL query",
            workspace_path=ws_path,
            recipe_id=None,
            engine="lql",
            status="pending",
            summary="Queued",
            input_text=query,
            raw={"async_job": True},
            duration_ms=0,
        )
        ui_store.save_run(pending)
        rid = pending.id

        async def _bg() -> None:
            try:

                def _w() -> RunRecord:
                    return executor.run_lql(workspace_path=ws_path, query=query, run_id=rid)

                await asyncio.to_thread(_w)
            except Exception as exc:
                old = ui_store.get_run(rid)
                if old:
                    ui_store.save_run(
                        replace(old, status="error", summary=str(exc), raw={**old.raw, "error": str(exc)})
                    )

        _schedule_async(app_holder, _bg)
        return JSONResponse({"run_id": rid, "status": "pending"}, status_code=202)

    async def api_trace_run(request: Request) -> Response:
        data = await _read_json(request)
        try:
            ws = workspace_manager.get_current()
        except WorkspaceError as exc:
            return JSONResponse({"error": str(exc)}, status_code=400)
        if ws is None:
            return JSONResponse({"error": "no workspace open"}, status_code=400)
        if not ws.supports_trace:
            return JSONResponse({"error": "trace not available for this workspace"}, status_code=400)
        ws_path = ws.path
        prompt = str(data.get("prompt", "")).strip()
        if not prompt:
            return JSONResponse({"error": "prompt required"}, status_code=400)
        positions = str(data.get("positions", "last")).strip() or "last"
        walk_top_k = int(data.get("walk_top_k", 8192) or 8192)
        async_flag = bool(data.get("async"))

        if not async_flag:

            def _go() -> RunRecord:
                return executor.run_trace(
                    workspace_path=ws_path,
                    prompt=prompt,
                    positions=positions,
                    walk_top_k=max(1, walk_top_k),
                )

            try:
                run = await asyncio.to_thread(_go)
            except Exception as exc:
                return JSONResponse({"error": str(exc)}, status_code=500)
            return JSONResponse({"run": run.to_dict()})

        pending = RunRecord.create(
            kind="trace",
            title="Trace",
            workspace_path=ws_path,
            recipe_id=None,
            engine="trace",
            status="pending",
            summary="Queued",
            input_text=prompt,
            raw={"async_job": True, "positions": positions, "walk_top_k_features": max(1, walk_top_k)},
            duration_ms=0,
        )
        ui_store.save_run(pending)
        rid = pending.id

        async def _bg() -> None:
            try:

                def _w() -> RunRecord:
                    return executor.run_trace(
                        workspace_path=ws_path,
                        prompt=prompt,
                        positions=positions,
                        walk_top_k=max(1, walk_top_k),
                        run_id=rid,
                    )

                await asyncio.to_thread(_w)
            except Exception as exc:
                old = ui_store.get_run(rid)
                if old:
                    ui_store.save_run(
                        replace(old, status="error", summary=str(exc), raw={**old.raw, "error": str(exc)})
                    )

        _schedule_async(app_holder, _bg)
        return JSONResponse({"run_id": rid, "status": "pending"}, status_code=202)

    return [
        Route("/api/workspace/open", api_workspace_open, methods=["POST"]),
        Route("/api/workspace/current", api_workspace_current, methods=["GET"]),
        Route("/api/recipes", api_recipes_post, methods=["POST"]),
        Route("/api/recipes/{recipe_id}", api_recipes_put, methods=["PUT"]),
        Route("/api/recipes/{recipe_id}", api_recipes_delete, methods=["DELETE"]),
        Route("/api/runs", api_runs_post, methods=["POST"]),
        Route("/api/runs/{run_id}", api_runs_get, methods=["GET"]),
        Route("/api/runs/{run_id}/rerun", api_runs_rerun, methods=["POST"]),
        Route("/api/explorer/describe", api_explorer_describe, methods=["POST"]),
        Route("/api/explorer/relations", api_explorer_relations, methods=["POST"]),
        Route("/api/lql/query", api_lql_query, methods=["POST"]),
        Route("/api/trace/run", api_trace_run, methods=["POST"]),
    ]
