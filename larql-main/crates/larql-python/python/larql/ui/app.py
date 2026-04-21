from __future__ import annotations

from pathlib import Path
import json
from typing import Any, Mapping

from fastapi import FastAPI, Request
from fastapi.responses import RedirectResponse
from fastapi.staticfiles import StaticFiles
from fastapi.templating import Jinja2Templates
from starlette import status

from .execution import UiExecutor
from .models import RecipeRecord, render_template, template_fields
from .store import UiStore
from .workspace import WorkspaceError, WorkspaceManager


def _form_positive_int(form: Mapping[str, Any], key: str, default: int) -> int:
    try:
        return max(1, int(str(form.get(key, default)).strip()))
    except ValueError:
        return default


def _studio_resolve_engine(*, form_engine: str, recipe: RecipeRecord | None) -> str:
    raw = form_engine.strip()
    if recipe is not None:
        return recipe.effective_engine(raw or None)
    return raw or "describe"


def create_app(store: UiStore | None = None) -> FastAPI:
    app = FastAPI(title="LARQL Workbench", version="0.1.0")
    base_dir = Path(__file__).resolve().parent
    templates = Jinja2Templates(directory=str(base_dir / "templates"))
    templates.env.filters["to_pretty_json"] = lambda value: json.dumps(value, indent=2, sort_keys=True)

    app.mount("/static", StaticFiles(directory=str(base_dir / "static")), name="static")

    app.state.store = store or UiStore()
    app.state.workspace_manager = WorkspaceManager(app.state.store)
    app.state.executor = UiExecutor(app.state.store, app.state.workspace_manager)
    app.state.templates = templates

    def render(
        request: Request,
        template_name: str,
        *,
        title: str,
        current_page: str,
        **context: object,
    ):
        workspace = None
        workspace_error = None
        try:
            workspace = app.state.workspace_manager.get_current()
        except WorkspaceError as exc:
            workspace_error = str(exc)

        nav_items = [
            ("workspace", "Workspace", "/workspace"),
            ("studio", "Studio", "/studio"),
            ("explorer", "Explorer", "/explorer"),
            ("lql", "LQL", "/lql"),
            ("recipes", "Recipes", "/recipes"),
            ("runs", "Runs", "/runs"),
        ]
        payload = {
            "request": request,
            "title": title,
            "current_page": current_page,
            "nav_items": nav_items,
            "current_workspace": workspace,
            "workspace_error": workspace_error,
            "notice": request.query_params.get("notice"),
            "error": request.query_params.get("error"),
        }
        payload.update(context)
        return templates.TemplateResponse(request, template_name, payload)

    def current_workspace_path() -> str | None:
        try:
            workspace = app.state.workspace_manager.get_current()
        except WorkspaceError:
            return None
        return workspace.path if workspace else None

    @app.get("/")
    async def index() -> RedirectResponse:
        target = "/studio" if current_workspace_path() else "/workspace"
        return RedirectResponse(target, status_code=status.HTTP_303_SEE_OTHER)

    @app.get("/workspace")
    async def workspace_page(request: Request):
        return render(request, "workspace.html", title="Workspace", current_page="workspace")

    @app.post("/workspace")
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
            app.state.workspace_manager.open(path)
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

    def _studio_variable_names(
        *,
        variables_form: dict[str, str] | None,
        inline_template: str,
    ) -> list[str]:
        if variables_form:
            return list(variables_form.keys())
        return template_fields(inline_template or "{subject}")

    @app.get("/studio")
    async def studio_page(request: Request):
        if not current_workspace_path():
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        recipes = app.state.store.list_recipes()
        workspace = app.state.workspace_manager.get_current()
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
            recipes=recipes,
            form_data=fd,
            variable_names=_studio_variable_names(
                variables_form=None,
                inline_template=str(fd["inline_template"]),
            ),
            variables_form=None,
            run=None,
            compare_runs=None,
            supports_infer=bool(workspace and workspace.supports_infer),
        )

    @app.post("/studio/run")
    async def studio_run(request: Request):
        workspace_path = current_workspace_path()
        if not workspace_path:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)

        form = await request.form()
        workspace = app.state.workspace_manager.get_current()
        recipe_id = str(form.get("recipe_id", "")).strip() or None
        form_engine = str(form.get("engine", "")).strip()
        band = str(form.get("band", "knowledge")).strip() or "knowledge"
        verbose = form.get("verbose") == "on"
        infer_top = _form_positive_int(form, "infer_top_k_predictions", 5)
        walk_k = _form_positive_int(form, "walk_top_k", 8192)

        recipe: RecipeRecord | None = None
        rendered: str
        title: str | None = None
        variables_form: dict[str, str] | None = None

        try:
            if recipe_id:
                recipe = app.state.store.get_recipe(recipe_id)
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

            run = app.state.executor.run_studio(
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
            recipes = app.state.store.list_recipes()
            return render(
                request,
                "studio.html",
                title="Studio",
                current_page="studio",
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
                supports_infer=bool(workspace and workspace.supports_infer),
                page_error=str(exc),
            )

        recipes = app.state.store.list_recipes()
        return render(
            request,
            "studio.html",
            title="Studio",
            current_page="studio",
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
            supports_infer=bool(workspace and workspace.supports_infer),
        )

    @app.post("/studio/compare")
    async def studio_compare(request: Request):
        workspace_path = current_workspace_path()
        if not workspace_path:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)

        form = await request.form()
        workspace = app.state.workspace_manager.get_current()
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
                recipe = app.state.store.get_recipe(recipe_id)
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
                    app.state.executor.run_studio(
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
            recipes = app.state.store.list_recipes()
            return render(
                request,
                "studio.html",
                title="Studio",
                current_page="studio",
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
                supports_infer=bool(workspace and workspace.supports_infer),
                page_error=str(exc),
            )

        recipes = app.state.store.list_recipes()
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
            supports_infer=bool(workspace and workspace.supports_infer),
        )

    @app.get("/explorer")
    async def explorer_page(request: Request):
        if not current_workspace_path():
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        return render(
            request,
            "explorer.html",
            title="Explorer",
            current_page="explorer",
            form_data={"entity": "", "band": "knowledge", "verbose": False},
            run=None,
        )

    @app.post("/explorer")
    async def explorer_describe(request: Request):
        workspace_path = current_workspace_path()
        if not workspace_path:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)

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
                form_data={"entity": entity, "band": band, "verbose": verbose},
                run=None,
                page_error="Entity required.",
            )

        try:
            run = app.state.executor.run_describe(
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
                form_data={"entity": entity, "band": band, "verbose": verbose},
                run=None,
                page_error=str(exc),
            )
        return render(
            request,
            "explorer.html",
            title="Explorer",
            current_page="explorer",
            form_data={"entity": entity, "band": band, "verbose": verbose},
            run=run,
        )

    @app.get("/lql")
    async def lql_page(request: Request):
        if not current_workspace_path():
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        return render(
            request,
            "lql.html",
            title="LQL",
            current_page="lql",
            query="STATS",
            run=None,
        )

    @app.post("/lql")
    async def lql_query(request: Request):
        workspace_path = current_workspace_path()
        if not workspace_path:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)

        form = await request.form()
        query = str(form.get("query", "")).strip()
        if not query:
            return render(
                request,
                "lql.html",
                title="LQL",
                current_page="lql",
                query=query,
                run=None,
                page_error="Query required.",
            )

        try:
            run = app.state.executor.run_lql(workspace_path=workspace_path, query=query)
        except Exception as exc:
            return render(
                request,
                "lql.html",
                title="LQL",
                current_page="lql",
                query=query,
                run=None,
                page_error=str(exc),
            )
        return render(
            request,
            "lql.html",
            title="LQL",
            current_page="lql",
            query=query,
            run=run,
        )

    @app.get("/recipes")
    async def recipes_page(request: Request):
        recipes = app.state.store.list_recipes()
        return render(
            request,
            "recipes.html",
            title="Recipes",
            current_page="recipes",
            recipes=recipes,
            page_error=None,
        )

    @app.post("/recipes")
    async def recipes_create(request: Request):
        form = await request.form()
        name = str(form.get("name", "")).strip()
        kind = str(form.get("kind", "describe")).strip() or "describe"
        template = str(form.get("template", "")).strip()
        lql_template = str(form.get("lql_template", "")).strip()
        description = str(form.get("description", "")).strip()
        default_engine = str(form.get("default_engine", "")).strip() or None
        infer_top = _form_positive_int(form, "infer_top_k_predictions", 5)
        walk_k = _form_positive_int(form, "walk_top_k", 8192)
        describe_band = str(form.get("describe_band", "knowledge")).strip() or "knowledge"
        describe_verbose = form.get("describe_verbose") == "on"

        page_error = None
        if not name:
            page_error = "Recipe name required."
        elif kind == "describe" and not template:
            page_error = "Describe recipe needs template."
        elif kind in ("infer", "walk_model") and not template:
            page_error = "Infer / WalkModel recipe needs prompt template."
        elif kind == "lql" and not (lql_template or template):
            page_error = "LQL recipe needs query template."

        if page_error:
            return render(
                request,
                "recipes.html",
                title="Recipes",
                current_page="recipes",
                recipes=app.state.store.list_recipes(),
                page_error=page_error,
                form_data={
                    "name": name,
                    "kind": kind,
                    "template": template,
                    "lql_template": lql_template,
                    "description": description,
                    "default_engine": default_engine or "",
                    "infer_top_k_predictions": infer_top,
                    "walk_top_k": walk_k,
                    "describe_band": describe_band,
                    "describe_verbose": describe_verbose,
                },
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
        )
        app.state.store.save_recipe(recipe)
        return RedirectResponse(
            f"/recipes/{recipe.id}?notice=Recipe+saved",
            status_code=status.HTTP_303_SEE_OTHER,
        )

    @app.get("/recipes/{recipe_id}")
    async def recipe_detail(request: Request, recipe_id: str):
        recipe = app.state.store.get_recipe(recipe_id)
        if recipe is None:
            return RedirectResponse("/recipes?error=Recipe+not+found", status_code=status.HTTP_303_SEE_OTHER)
        variables = {name: "" for name in recipe.variables}
        if "subject" in variables:
            variables["subject"] = "France"
        return render(
            request,
            "recipe_detail.html",
            title=recipe.name,
            current_page="recipes",
            recipe=recipe,
            variables=variables,
            rendered_text=None,
            run=None,
        )

    @app.post("/recipes/{recipe_id}/run")
    async def recipe_run(request: Request, recipe_id: str):
        workspace_path = current_workspace_path()
        if not workspace_path:
            return RedirectResponse("/workspace?error=Open+workspace+first", status_code=status.HTTP_303_SEE_OTHER)
        recipe = app.state.store.get_recipe(recipe_id)
        if recipe is None:
            return RedirectResponse("/recipes?error=Recipe+not+found", status_code=status.HTTP_303_SEE_OTHER)

        form = await request.form()
        variables = {name: str(form.get(name, "")).strip() for name in recipe.variables}
        try:
            rendered_text = recipe.rendered(variables)
            run = app.state.executor.run_recipe(
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
            recipe=recipe,
            variables=variables,
            rendered_text=rendered_text,
            run=run,
        )

    @app.post("/recipes/{recipe_id}/delete")
    async def recipe_delete(recipe_id: str):
        app.state.store.delete_recipe(recipe_id)
        return RedirectResponse("/recipes?notice=Recipe+deleted", status_code=status.HTTP_303_SEE_OTHER)

    @app.get("/runs")
    async def runs_page(request: Request):
        return render(
            request,
            "runs.html",
            title="Runs",
            current_page="runs",
            runs=app.state.store.list_runs(),
        )

    @app.get("/runs/{run_id}")
    async def run_detail(request: Request, run_id: str):
        run = app.state.store.get_run(run_id)
        if run is None:
            return RedirectResponse("/runs?error=Run+not+found", status_code=status.HTTP_303_SEE_OTHER)
        return render(
            request,
            "run_detail.html",
            title=run.title,
            current_page="runs",
            run=run,
        )

    return app
