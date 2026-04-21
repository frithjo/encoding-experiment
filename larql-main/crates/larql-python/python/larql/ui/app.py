from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any, Mapping

from starlette import status
from starlette.applications import Starlette
from starlette.middleware import Middleware
from starlette.middleware.base import BaseHTTPMiddleware
from starlette.requests import Request
from starlette.responses import RedirectResponse, Response
from starlette.routing import Mount, Route
from starlette.staticfiles import StaticFiles
from starlette.templating import Jinja2Templates

from .api import build_api_routes
from .execution import UiExecutor
from .models import RecipeRecord, WorkspaceSummary, render_template, template_fields, validate_recipe
from .runtime_cache import LarqlRuntimeCache, workspace_paths_differ
from .store import UiStore
from .workspace import WorkspaceError, WorkspaceManager

# Primary nav — stable tuple reused for every page (avoid per-request alloc).
NAV_ITEMS = (
    ("workspace", "Workspace", "/workspace"),
    ("studio", "Studio", "/studio"),
    ("explorer", "Explorer", "/explorer"),
    ("lql", "LQL", "/lql"),
    ("trace", "Trace", "/trace"),
    ("recipes", "Recipes", "/recipes"),
    ("runs", "Runs", "/runs"),
)

# Sentinel: `render()` loads workspace summary unless caller passes a pre-fetched value.
_SHELL_WS_FETCH = object()


class _StaticCacheControlMiddleware(BaseHTTPMiddleware):
    """Cheap Cache-Control for hashed UI assets (CSS); reduces repeat downloads in the terminal browser."""

    async def dispatch(self, request: Request, call_next: Any) -> Response:
        response = await call_next(request)
        if request.url.path.startswith("/static/") and response.status_code < 400:
            if "cache-control" not in response.headers:
                response.headers["cache-control"] = "public, max-age=86400"
        return response


def _form_positive_int(form: Mapping[str, Any], key: str, default: int) -> int:
    try:
        return max(1, int(str(form.get(key, default)).strip()))
    except ValueError:
        return default


def _studio_engine_flags(workspace: WorkspaceSummary | None) -> dict[str, bool]:
    """Studio template flags for mmap / LQL engines (terminal viewport capability gating)."""
    if workspace is None:
        return {"supports_infer": False, "supports_walk_model": False}
    return {
        "supports_infer": workspace.supports_infer,
        "supports_walk_model": workspace.supports_walk_model,
    }


def _studio_resolve_engine(*, form_engine: str, recipe: RecipeRecord | None) -> str:
    raw = form_engine.strip()
    if recipe is not None:
        return recipe.effective_engine(raw or None)
    return raw or "describe"


def create_app(store: UiStore | None = None) -> Starlette:
    """ASGI app for the workbench — Starlette only (no FastAPI / Pydantic in the UI stack)."""
    base_dir = Path(__file__).resolve().parent
    templates = Jinja2Templates(directory=str(base_dir / "templates"))
    # Avoid stat-ing template files on every request unless explicitly debugging templates.
    templates.env.auto_reload = os.environ.get("LARQL_UI_DEBUG", "").lower() in ("1", "true", "yes")
    templates.env.filters["to_pretty_json"] = lambda value: json.dumps(
        value, indent=2, sort_keys=True, default=str
    )

    ui_store = store or UiStore()
    workspace_manager = WorkspaceManager(ui_store)
    runtime_cache = LarqlRuntimeCache()
    executor = UiExecutor(ui_store, workspace_manager, runtime_cache)

    def render(
        request: Request,
        template_name: str,
        *,
        title: str,
        current_page: str,
        shell_workspace: object = _SHELL_WS_FETCH,
        shell_workspace_error: str | None = None,
        **context: object,
    ):
        workspace = None
        workspace_error = None
        if shell_workspace is _SHELL_WS_FETCH:
            try:
                workspace = workspace_manager.get_current()
            except WorkspaceError as exc:
                workspace_error = str(exc)
        else:
            workspace = shell_workspace  # type: ignore[assignment]
            workspace_error = shell_workspace_error

        payload = {
            "request": request,
            "title": title,
            "current_page": current_page,
            "nav_items": NAV_ITEMS,
            "current_workspace": workspace,
            "workspace_error": workspace_error,
            "notice": request.query_params.get("notice"),
            "error": request.query_params.get("error"),
        }
        payload.update(context)
        return templates.TemplateResponse(request, template_name, payload)

    def current_workspace_path() -> str | None:
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return None
        return workspace.path if workspace else None

    def _studio_variable_names(
        *,
        variables_form: dict[str, str] | None,
        inline_template: str,
    ) -> list[str]:
        if variables_form:
            return list(variables_form.keys())
        return template_fields(inline_template or "{subject}")

    async def index(_request: Request) -> RedirectResponse:
        target = "/studio" if current_workspace_path() else "/workspace"
        return RedirectResponse(target, status_code=status.HTTP_303_SEE_OTHER)

    async def workspace_page(request: Request):
        return render(request, "workspace.html", title="Workspace", current_page="workspace")

    async def workspace_open(request: Request):
        form = await request.form()
        path = str(form.get("path", "")).strip()
        if not path:
            return render(
                request,
                "workspace.html",
                title="Workspace",
                current_page="workspace",
                form_path=path,
                page_error="Workspace path required.",
            )
        try:
            workspace_manager.open(path)
        except WorkspaceError as exc:
            return render(
                request,
                "workspace.html",
                title="Workspace",
                current_page="workspace",
                form_path=path,
                page_error=str(exc),
            )
        return RedirectResponse("/studio?notice=Workspace+loaded", status_code=status.HTTP_303_SEE_OTHER)

    async def partial_workspace_status(request: Request):
        cur = ui_store.current_workspace_path()
        if cur:
            workspace_manager.drop_workspace_cache(cur)
        workspace = None
        workspace_error = None
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError as exc:
            workspace_error = str(exc)
        return templates.TemplateResponse(
            request,
            "partials/workspace_status.html",
            {
                "request": request,
                "current_workspace": workspace,
                "workspace_error": workspace_error,
            },
        )

    async def partial_recipes_list(request: Request):
        recipes = ui_store.list_recipes()
        return templates.TemplateResponse(
            request,
            "partials/recipes_list.html",
            {"request": request, "recipes": recipes},
        )

    async def partial_runs_table(request: Request):
        runs = ui_store.list_runs()
        return templates.TemplateResponse(
            request,
            "partials/runs_table.html",
            {"request": request, "runs": runs},
        )

    async def partial_trace_summary(request: Request):
        run_id = str(request.query_params.get("run_id", "")).strip()
        run = ui_store.get_run(run_id) if run_id else None
        return templates.TemplateResponse(
            request,
            "partials/trace_summary.html",
            {"request": request, "run": run},
        )

    async def partial_result_panel(request: Request):
        run_id = str(request.query_params.get("run_id", "")).strip()
        run = ui_store.get_run(run_id) if run_id else None
        return templates.TemplateResponse(
            request,
            "partials/result_panel.html",
            {"request": request, "run": run},
        )

    async def partial_run_controls(request: Request):
        try:
            ws = workspace_manager.get_current()
        except WorkspaceError:
            ws = None
        return templates.TemplateResponse(
            request,
            "partials/run_controls.html",
            {"request": request, "workspace": ws},
        )

    async def partial_recipe_editor(request: Request):
        rid = str(request.query_params.get("recipe_id", "")).strip()
        recipe = ui_store.get_recipe(rid) if rid else None
        return templates.TemplateResponse(
            request,
            "partials/recipe_editor.html",
            {"request": request, "recipe": recipe},
        )

    async def studio_page(request: Request):
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        recipes = ui_store.list_recipes()
        fd = {
            "recipe_id": "",
            "engine": "describe",
            "inline_template": "{subject}",
            "band": "knowledge",
            "verbose": False,
            "infer_top_k_predictions": 5,
            "walk_top_k": 8192,
            "compare_subjects": "",
        }
        return render(
            request,
            "studio.html",
            title="Studio",
            current_page="studio",
            shell_workspace=workspace,
            recipes=recipes,
            form_data=fd,
            variable_names=_studio_variable_names(
                variables_form=None,
                inline_template=str(fd["inline_template"]),
            ),
            variables_form=None,
            run=None,
            compare_runs=None,
            **_studio_engine_flags(workspace),
        )

    async def studio_run(request: Request):
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        workspace_path = workspace.path

        form = await request.form()
        recipe_id = str(form.get("recipe_id", "")).strip() or None
        form_engine = str(form.get("engine", "")).strip()
        band = str(form.get("band", "knowledge")).strip() or "knowledge"
        verbose = form.get("verbose") == "on"
        infer_top = _form_positive_int(form, "infer_top_k_predictions", 5)
        walk_k = _form_positive_int(form, "walk_top_k", 8192)

        recipe: RecipeRecord | None = None
        variables_form: dict[str, str] | None = None

        try:
            if recipe_id:
                recipe = ui_store.get_recipe(recipe_id)
                if recipe is None:
                    raise ValueError("Recipe not found.")
                variables_form = {n: str(form.get(n, "")).strip() for n in recipe.variables}
                rendered = recipe.rendered(variables_form)
                title = recipe.name
            else:
                inline = str(form.get("inline_template", "")).strip()
                if not inline:
                    raise ValueError("Template required when no recipe selected.")
                names = template_fields(inline)
                variables_form = {n: str(form.get(n, "")).strip() for n in names}
                rendered = render_template(inline, variables_form)
                title = "Studio"

            engine = _studio_resolve_engine(form_engine=form_engine, recipe=recipe)

            run = executor.run_studio(
                workspace_path=workspace_path,
                engine=engine,
                rendered=rendered,
                recipe_id=recipe_id,
                title=title,
                band=band,
                verbose=verbose,
                infer_top_k_predictions=infer_top,
                walk_top_k=walk_k,
            )
        except Exception as exc:
            recipes = ui_store.list_recipes()
            return render(
                request,
                "studio.html",
                title="Studio",
                current_page="studio",
                shell_workspace=workspace,
                recipes=recipes,
                form_data={
                    "recipe_id": recipe_id or "",
                    "engine": form_engine or "describe",
                    "inline_template": str(form.get("inline_template", "")),
                    "band": band,
                    "verbose": verbose,
                    "infer_top_k_predictions": infer_top,
                    "walk_top_k": walk_k,
                    "compare_subjects": str(form.get("compare_subjects", "")),
                },
                variable_names=_studio_variable_names(
                    variables_form=variables_form,
                    inline_template=str(form.get("inline_template", "")),
                ),
                variables_form=variables_form,
                run=None,
                compare_runs=None,
                **_studio_engine_flags(workspace),
                page_error=str(exc),
            )

        recipes = ui_store.list_recipes()
        return render(
            request,
            "studio.html",
            title="Studio",
            current_page="studio",
            shell_workspace=workspace,
            recipes=recipes,
            form_data={
                "recipe_id": recipe_id or "",
                "engine": engine,
                "inline_template": str(form.get("inline_template", "")),
                "band": band,
                "verbose": verbose,
                "infer_top_k_predictions": infer_top,
                "walk_top_k": walk_k,
                "compare_subjects": str(form.get("compare_subjects", "")),
            },
            variable_names=_studio_variable_names(
                variables_form=variables_form,
                inline_template=str(form.get("inline_template", "")),
            ),
            variables_form=variables_form,
            run=run,
            compare_runs=None,
            **_studio_engine_flags(workspace),
        )

    async def studio_compare(request: Request):
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        workspace_path = workspace.path

        form = await request.form()
        recipe_id = str(form.get("recipe_id", "")).strip() or None
        form_engine = str(form.get("engine", "")).strip()
        band = str(form.get("band", "knowledge")).strip() or "knowledge"
        verbose = form.get("verbose") == "on"
        infer_top = _form_positive_int(form, "infer_top_k_predictions", 5)
        walk_k = _form_positive_int(form, "walk_top_k", 8192)
        raw_lines = str(form.get("compare_subjects", "")).splitlines()
        lines = [ln.strip() for ln in raw_lines if ln.strip()]

        recipe: RecipeRecord | None = None
        template_src: str = ""
        base_vars: dict[str, str] = {}

        try:
            if recipe_id:
                recipe = ui_store.get_recipe(recipe_id)
                if recipe is None:
                    raise ValueError("Recipe not found.")
                if recipe.kind == "lql" and recipe.lql_template:
                    template_src = recipe.lql_template
                else:
                    template_src = recipe.template
            else:
                inline = str(form.get("inline_template", "")).strip()
                if not inline:
                    raise ValueError("Template required when no recipe selected.")
                template_src = inline

            fields = template_fields(template_src)
            if len(fields) != 1:
                raise ValueError("Compare batch needs template with exactly one placeholder.")
            key = fields[0]
            if not lines:
                raise ValueError("Add one subject per line in compare box.")

            base_vars = {}
            if recipe_id and recipe is not None:
                for n in recipe.variables:
                    if n == key:
                        continue
                    base_vars[n] = str(form.get(n, "")).strip()
            else:
                for n in template_fields(template_src):
                    if n == key:
                        continue
                    base_vars[n] = str(form.get(n, "")).strip()

            engine = _studio_resolve_engine(form_engine=form_engine, recipe=recipe)

            compare_runs = []
            for line in lines:
                variables = dict(base_vars)
                variables[key] = line
                rendered = render_template(template_src, variables)
                compare_runs.append(
                    executor.run_studio(
                        workspace_path=workspace_path,
                        engine=engine,
                        rendered=rendered,
                        recipe_id=recipe_id,
                        title=(recipe.name + f" ({line})") if recipe else f"Studio ({line})",
                        band=band,
                        verbose=verbose,
                        infer_top_k_predictions=infer_top,
                        walk_top_k=walk_k,
                    )
                )
        except Exception as exc:
            recipes = ui_store.list_recipes()
            return render(
                request,
                "studio.html",
                title="Studio",
                current_page="studio",
                shell_workspace=workspace,
                recipes=recipes,
                form_data={
                    "recipe_id": recipe_id or "",
                    "engine": form_engine or "describe",
                    "inline_template": str(form.get("inline_template", "")),
                    "band": band,
                    "verbose": verbose,
                    "infer_top_k_predictions": infer_top,
                    "walk_top_k": walk_k,
                    "compare_subjects": str(form.get("compare_subjects", "")),
                },
                variable_names=_studio_variable_names(
                    variables_form=base_vars,
                    inline_template=str(form.get("inline_template", "")),
                ),
                variables_form=base_vars,
                run=None,
                compare_runs=None,
                **_studio_engine_flags(workspace),
                page_error=str(exc),
            )

        recipes = ui_store.list_recipes()
        compare_vars = dict(base_vars)
        if lines and template_src:
            tf = template_fields(template_src)
            if len(tf) == 1:
                compare_vars[tf[0]] = lines[0]
        return render(
            request,
            "studio.html",
            title="Studio",
            current_page="studio",
            shell_workspace=workspace,
            recipes=recipes,
            form_data={
                "recipe_id": recipe_id or "",
                "engine": engine,
                "inline_template": str(form.get("inline_template", "")),
                "band": band,
                "verbose": verbose,
                "infer_top_k_predictions": infer_top,
                "walk_top_k": walk_k,
                "compare_subjects": str(form.get("compare_subjects", "")),
            },
            variable_names=template_fields(template_src) if template_src else [],
            variables_form=compare_vars,
            run=None,
            compare_runs=compare_runs,
            **_studio_engine_flags(workspace),
        )

    async def explorer_page(request: Request):
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        return render(
            request,
            "explorer.html",
            title="Explorer",
            current_page="explorer",
            shell_workspace=workspace,
            form_data={"entity": "", "band": "knowledge", "verbose": False},
            run=None,
        )

    async def explorer_describe(request: Request):
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        workspace_path = workspace.path

        form = await request.form()
        entity = str(form.get("entity", "")).strip()
        band = str(form.get("band", "knowledge")).strip() or "knowledge"
        verbose = form.get("verbose") == "on"
        if not entity:
            return render(
                request,
                "explorer.html",
                title="Explorer",
                current_page="explorer",
                shell_workspace=workspace,
                form_data={"entity": entity, "band": band, "verbose": verbose},
                run=None,
                page_error="Entity required.",
            )

        try:
            run = executor.run_describe(
                workspace_path=workspace_path,
                entity=entity,
                band=band,
                verbose=verbose,
            )
        except Exception as exc:
            return render(
                request,
                "explorer.html",
                title="Explorer",
                current_page="explorer",
                shell_workspace=workspace,
                form_data={"entity": entity, "band": band, "verbose": verbose},
                run=None,
                page_error=str(exc),
            )
        return render(
            request,
            "explorer.html",
            title="Explorer",
            current_page="explorer",
            shell_workspace=workspace,
            form_data={"entity": entity, "band": band, "verbose": verbose},
            run=run,
        )

    async def lql_page(request: Request):
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        return render(
            request,
            "lql.html",
            title="LQL",
            current_page="lql",
            shell_workspace=workspace,
            query="STATS",
            run=None,
        )

    async def lql_query(request: Request):
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        workspace_path = workspace.path

        form = await request.form()
        query = str(form.get("query", "")).strip()
        if not query:
            return render(
                request,
                "lql.html",
                title="LQL",
                current_page="lql",
                shell_workspace=workspace,
                query=query,
                run=None,
                page_error="Query required.",
            )

        try:
            run = executor.run_lql(workspace_path=workspace_path, query=query)
        except Exception as exc:
            return render(
                request,
                "lql.html",
                title="LQL",
                current_page="lql",
                shell_workspace=workspace,
                query=query,
                run=None,
                page_error=str(exc),
            )
        return render(
            request,
            "lql.html",
            title="LQL",
            current_page="lql",
            shell_workspace=workspace,
            query=query,
            run=run,
        )

    async def trace_page(request: Request):
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        fd = {"prompt": "", "positions": "last", "walk_top_k": 8192}
        return render(
            request,
            "trace.html",
            title="Trace",
            current_page="trace",
            shell_workspace=workspace,
            form_data=fd,
            run=None,
            supports_trace=workspace.supports_trace,
        )

    async def trace_run(request: Request):
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        workspace_path = workspace.path
        form = await request.form()
        prompt = str(form.get("prompt", "")).strip()
        positions = str(form.get("positions", "last")).strip() or "last"
        walk_top_k = _form_positive_int(form, "walk_top_k", 8192)
        fd = {"prompt": prompt, "positions": positions, "walk_top_k": walk_top_k}

        if not prompt:
            return render(
                request,
                "trace.html",
                title="Trace",
                current_page="trace",
                shell_workspace=workspace,
                form_data=fd,
                run=None,
                supports_trace=workspace.supports_trace,
                page_error="Prompt required.",
            )

        try:
            run = executor.run_trace(
                workspace_path=workspace_path,
                prompt=prompt,
                positions=positions,
                walk_top_k=walk_top_k,
            )
        except Exception as exc:
            return render(
                request,
                "trace.html",
                title="Trace",
                current_page="trace",
                shell_workspace=workspace,
                form_data=fd,
                run=None,
                supports_trace=workspace.supports_trace,
                page_error=str(exc),
            )
        return render(
            request,
            "trace.html",
            title="Trace",
            current_page="trace",
            shell_workspace=workspace,
            form_data=fd,
            run=run,
            supports_trace=workspace.supports_trace,
        )

    async def recipes_page(request: Request):
        recipes = ui_store.list_recipes()
        try:
            sw = workspace_manager.get_current()
        except WorkspaceError as exc:
            return render(
                request,
                "recipes.html",
                title="Recipes",
                current_page="recipes",
                shell_workspace=None,
                shell_workspace_error=str(exc),
                recipes=recipes,
                page_error=None,
            )
        return render(
            request,
            "recipes.html",
            title="Recipes",
            current_page="recipes",
            shell_workspace=sw,
            recipes=recipes,
            page_error=None,
        )

    async def recipes_create(request: Request):
        form = await request.form()
        name = str(form.get("name", "")).strip()
        kind = str(form.get("kind", "describe")).strip() or "describe"
        template = str(form.get("template", "")).strip()
        lql_template = str(form.get("lql_template", "")).strip()
        description = str(form.get("description", "")).strip()
        notes = str(form.get("notes", "")).strip()
        tags_raw = str(form.get("tags", "")).strip()
        tags = [t.strip() for t in tags_raw.replace(";", ",").split(",") if t.strip()]
        default_engine = str(form.get("default_engine", "")).strip() or None
        infer_top = _form_positive_int(form, "infer_top_k_predictions", 5)
        walk_k = _form_positive_int(form, "walk_top_k", 8192)
        describe_band = str(form.get("describe_band", "knowledge")).strip() or "knowledge"
        describe_verbose = form.get("describe_verbose") == "on"

        def _form_snapshot() -> dict[str, Any]:
            return {
                "name": name,
                "kind": kind,
                "template": template,
                "lql_template": lql_template,
                "description": description,
                "notes": notes,
                "tags": tags_raw,
                "default_engine": default_engine or "",
                "infer_top_k_predictions": infer_top,
                "walk_top_k": walk_k,
                "describe_band": describe_band,
                "describe_verbose": describe_verbose,
            }

        page_error = None
        if not name:
            page_error = "Recipe name required."
        elif kind == "describe" and not template:
            page_error = "Describe recipe needs template."
        elif kind in ("infer", "walk_model", "probe", "generation") and not template:
            page_error = "This recipe kind needs a prompt template."
        elif kind == "lql" and not (lql_template or template):
            page_error = "LQL recipe needs query template."

        if page_error:
            try:
                sw = workspace_manager.get_current()
            except WorkspaceError as exc:
                return render(
                    request,
                    "recipes.html",
                    title="Recipes",
                    current_page="recipes",
                    shell_workspace=None,
                    shell_workspace_error=str(exc),
                    recipes=ui_store.list_recipes(),
                    page_error=page_error,
                    form_data=_form_snapshot(),
                )
            return render(
                request,
                "recipes.html",
                title="Recipes",
                current_page="recipes",
                shell_workspace=sw,
                recipes=ui_store.list_recipes(),
                page_error=page_error,
                form_data=_form_snapshot(),
            )

        recipe = RecipeRecord.create(
            name=name,
            kind=kind,
            template=template or "{subject}",
            lql_template=lql_template or None,
            description=description,
            default_engine=default_engine,
            infer_top_k_predictions=infer_top,
            walk_top_k=walk_k,
            describe_band=describe_band,
            describe_verbose=describe_verbose,
            tags=tags,
            notes=notes,
        )
        try:
            validate_recipe(recipe)
        except ValueError as exc:
            try:
                sw = workspace_manager.get_current()
            except WorkspaceError as err:
                return render(
                    request,
                    "recipes.html",
                    title="Recipes",
                    current_page="recipes",
                    shell_workspace=None,
                    shell_workspace_error=str(err),
                    recipes=ui_store.list_recipes(),
                    page_error=str(exc),
                    form_data=_form_snapshot(),
                )
            return render(
                request,
                "recipes.html",
                title="Recipes",
                current_page="recipes",
                shell_workspace=sw,
                recipes=ui_store.list_recipes(),
                page_error=str(exc),
                form_data=_form_snapshot(),
            )
        ui_store.save_recipe(recipe)
        return RedirectResponse(
            f"/recipes/{recipe.id}?notice=Recipe+saved",
            status_code=status.HTTP_303_SEE_OTHER,
        )

    async def recipe_detail(request: Request):
        recipe_id = request.path_params["recipe_id"]
        recipe = ui_store.get_recipe(recipe_id)
        if recipe is None:
            return RedirectResponse("/recipes?error=Recipe+not+found", status_code=status.HTTP_303_SEE_OTHER)
        variables = {name: "" for name in recipe.variables}
        if "subject" in variables:
            variables["subject"] = "France"
        try:
            sw = workspace_manager.get_current()
        except WorkspaceError as exc:
            return render(
                request,
                "recipe_detail.html",
                title=recipe.name,
                current_page="recipes",
                shell_workspace=None,
                shell_workspace_error=str(exc),
                recipe=recipe,
                variables=variables,
                rendered_text=None,
                run=None,
            )
        return render(
            request,
            "recipe_detail.html",
            title=recipe.name,
            current_page="recipes",
            shell_workspace=sw,
            recipe=recipe,
            variables=variables,
            rendered_text=None,
            run=None,
        )

    async def recipe_run(request: Request):
        recipe_id = request.path_params["recipe_id"]
        try:
            workspace = workspace_manager.get_current()
        except WorkspaceError:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        if workspace is None:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        workspace_path = workspace.path
        recipe = ui_store.get_recipe(recipe_id)
        if recipe is None:
            return RedirectResponse("/recipes?error=Recipe+not+found", status_code=status.HTTP_303_SEE_OTHER)

        form = await request.form()
        variables = {name: str(form.get(name, "")).strip() for name in recipe.variables}
        try:
            rendered_text = recipe.rendered(variables)
            run = executor.run_recipe(
                workspace_path=workspace_path,
                recipe=recipe,
                variables=variables,
            )
        except Exception as exc:
            return render(
                request,
                "recipe_detail.html",
                title=recipe.name,
                current_page="recipes",
                shell_workspace=workspace,
                recipe=recipe,
                variables=variables,
                rendered_text=None,
                run=None,
                page_error=str(exc),
            )

        return render(
            request,
            "recipe_detail.html",
            title=recipe.name,
            current_page="recipes",
            shell_workspace=workspace,
            recipe=recipe,
            variables=variables,
            rendered_text=rendered_text,
            run=run,
        )

    async def recipe_delete(request: Request):
        recipe_id = request.path_params["recipe_id"]
        ui_store.delete_recipe(recipe_id)
        return RedirectResponse("/recipes?notice=Recipe+deleted", status_code=status.HTTP_303_SEE_OTHER)

    async def runs_page(request: Request):
        try:
            sw = workspace_manager.get_current()
        except WorkspaceError as exc:
            return render(
                request,
                "runs.html",
                title="Runs",
                current_page="runs",
                shell_workspace=None,
                shell_workspace_error=str(exc),
                runs=ui_store.list_runs(),
            )
        return render(
            request,
            "runs.html",
            title="Runs",
            current_page="runs",
            shell_workspace=sw,
            runs=ui_store.list_runs(),
        )

    async def run_detail(request: Request):
        run_id = request.path_params["run_id"]
        run = ui_store.get_run(run_id)
        if run is None:
            return RedirectResponse("/runs?error=Run+not+found", status_code=status.HTTP_303_SEE_OTHER)
        try:
            sw = workspace_manager.get_current()
        except WorkspaceError as exc:
            return render(
                request,
                "run_detail.html",
                title=run.title,
                current_page="runs",
                shell_workspace=None,
                shell_workspace_error=str(exc),
                run=run,
            )
        return render(
            request,
            "run_detail.html",
            title=run.title,
            current_page="runs",
            shell_workspace=sw,
            run=run,
        )

    routes: list[Any] = [
        Mount("/static", StaticFiles(directory=str(base_dir / "static")), name="static"),
        Route("/", index, methods=["GET"]),
        Route("/workspace", workspace_page, methods=["GET"]),
        Route("/workspace", workspace_open, methods=["POST"]),
        Route("/partials/workspace-status", partial_workspace_status, methods=["GET"]),
        Route("/partials/recipes-list", partial_recipes_list, methods=["GET"]),
        Route("/partials/runs-table", partial_runs_table, methods=["GET"]),
        Route("/partials/trace-summary", partial_trace_summary, methods=["GET"]),
        Route("/partials/result-panel", partial_result_panel, methods=["GET"]),
        Route("/partials/run-controls", partial_run_controls, methods=["GET"]),
        Route("/partials/recipe-editor", partial_recipe_editor, methods=["GET"]),
        Route("/studio", studio_page, methods=["GET"]),
        Route("/studio/run", studio_run, methods=["POST"]),
        Route("/studio/compare", studio_compare, methods=["POST"]),
        Route("/explorer", explorer_page, methods=["GET"]),
        Route("/explorer", explorer_describe, methods=["POST"]),
        Route("/lql", lql_page, methods=["GET"]),
        Route("/lql", lql_query, methods=["POST"]),
        Route("/trace", trace_page, methods=["GET"]),
        Route("/trace", trace_run, methods=["POST"]),
        Route("/recipes", recipes_page, methods=["GET"]),
        Route("/recipes", recipes_create, methods=["POST"]),
        Route("/recipes/{recipe_id}/delete", recipe_delete, methods=["POST"]),
        Route("/recipes/{recipe_id}/run", recipe_run, methods=["POST"]),
        Route("/recipes/{recipe_id}", recipe_detail, methods=["GET"]),
        Route("/runs", runs_page, methods=["GET"]),
        Route("/runs/{run_id}", run_detail, methods=["GET"]),
    ]

    app_holder: list[Any] = []
    routes.extend(
        build_api_routes(
            app_holder=app_holder,
            ui_store=ui_store,
            workspace_manager=workspace_manager,
            executor=executor,
            runtime_cache=runtime_cache,
        )
    )

    app = Starlette(
        routes=routes,
        middleware=[Middleware(_StaticCacheControlMiddleware)],
    )
    app_holder.append(app)
    app.state.store = ui_store
    app.state.workspace_manager = workspace_manager
    app.state.executor = executor
    app.state.runtime_cache = runtime_cache
    app.state.templates = templates
    return app
