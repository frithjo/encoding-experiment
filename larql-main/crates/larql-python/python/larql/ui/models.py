from __future__ import annotations

from dataclasses import MISSING, asdict, dataclass, field, fields
from datetime import datetime, timezone
from string import Formatter
from typing import Any
import re


def utc_now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def slugify(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.lower()).strip("-")
    return slug or "item"


def template_fields(template: str | None) -> list[str]:
    if not template:
        return []
    fields: list[str] = []
    for _, field_name, _, _ in Formatter().parse(template):
        if field_name and field_name not in fields:
            fields.append(field_name)
    return fields


def render_template(template: str, variables: dict[str, str]) -> str:
    missing = [name for name in template_fields(template) if not variables.get(name)]
    if missing:
        joined = ", ".join(missing)
        raise ValueError(f"Missing template variables: {joined}")
    try:
        return template.format_map(variables)
    except KeyError as exc:  # pragma: no cover - guarded above, kept for safety
        raise ValueError(f"Missing template variable: {exc.args[0]}") from exc


@dataclass(slots=True)
class WorkspaceSummary:
    id: str
    path: str
    display_name: str
    model: str
    family: str
    num_layers: int
    hidden_size: int
    vocab_size: int
    extract_level: str | None
    has_model_weights: bool
    has_relation_labels: bool
    has_layer_bands: bool
    supports_infer: bool
    supports_trace: bool
    supports_mlx: bool
    supports_streaming: bool
    supports_walk_ffn: bool
    warnings: list[str] = field(default_factory=list)
    # From vindex.relations() — cluster classifier labels (sorted by count desc); capped for UI.
    relation_count: int = 0
    relation_labels: list[dict[str, Any]] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(slots=True)
class RecipeRecord:
    id: str
    name: str
    kind: str
    template: str
    lql_template: str | None
    description: str
    created_at: str
    updated_at: str
    default_engine: str | None = None
    infer_top_k_predictions: int = 5
    walk_top_k: int = 8192
    describe_band: str = "knowledge"
    describe_verbose: bool = False

    @property
    def variables(self) -> list[str]:
        if self.kind == "lql" and self.lql_template:
            source = self.lql_template
        else:
            source = self.template
        return template_fields(source)

    def rendered(self, variables: dict[str, str]) -> str:
        source = self.lql_template if self.kind == "lql" and self.lql_template else self.template
        return render_template(source, variables)

    def effective_engine(self, override: str | None) -> str:
        if override and override.strip():
            return override.strip()
        if self.default_engine and self.default_engine.strip():
            return self.default_engine.strip()
        if self.kind == "lql":
            return "lql"
        if self.kind == "infer":
            return "infer"
        if self.kind == "walk_model":
            return "walk_model"
        return "describe"

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def create(
        cls,
        *,
        name: str,
        kind: str,
        template: str,
        lql_template: str | None = None,
        description: str = "",
        default_engine: str | None = None,
        infer_top_k_predictions: int = 5,
        walk_top_k: int = 8192,
        describe_band: str = "knowledge",
        describe_verbose: bool = False,
    ) -> "RecipeRecord":
        now = utc_now_iso()
        return cls(
            id=f"{slugify(name)}-{int(datetime.now(timezone.utc).timestamp())}",
            name=name.strip(),
            kind=kind,
            template=template.strip(),
            lql_template=lql_template.strip() if lql_template else None,
            description=description.strip(),
            created_at=now,
            updated_at=now,
            default_engine=default_engine,
            infer_top_k_predictions=infer_top_k_predictions,
            walk_top_k=walk_top_k,
            describe_band=describe_band,
            describe_verbose=describe_verbose,
        )

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "RecipeRecord":
        kwargs: dict[str, Any] = {}
        for f in fields(cls):
            if f.name in data:
                kwargs[f.name] = data[f.name]
            else:
                if f.default is not MISSING:
                    kwargs[f.name] = f.default
                elif f.default_factory is not MISSING:
                    kwargs[f.name] = f.default_factory()
                else:
                    raise KeyError(f"Missing required field {f.name!r} in recipe JSON")
        return cls(**kwargs)


@dataclass(slots=True)
class RunRecord:
    id: str
    kind: str
    title: str
    workspace_path: str
    recipe_id: str | None
    engine: str
    status: str
    created_at: str
    duration_ms: int
    summary: str
    input_text: str
    raw: dict[str, Any]

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def create(
        cls,
        *,
        kind: str,
        title: str,
        workspace_path: str,
        engine: str,
        summary: str,
        input_text: str,
        raw: dict[str, Any],
        duration_ms: int,
        recipe_id: str | None = None,
        status: str = "completed",
    ) -> "RunRecord":
        return cls(
            id=f"run-{int(datetime.now(timezone.utc).timestamp() * 1000)}",
            kind=kind,
            title=title,
            workspace_path=workspace_path,
            recipe_id=recipe_id,
            engine=engine,
            status=status,
            created_at=utc_now_iso(),
            duration_ms=duration_ms,
            summary=summary,
            input_text=input_text,
            raw=raw,
        )

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "RunRecord":
        return cls(**data)
