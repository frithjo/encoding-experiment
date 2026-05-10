# LARQL Python — agent notes

## Workbench UI (strict)

**Requirement:** The workbench UX is **only** for a **browser running in the terminal**, implemented as the first-party Rust binary `crates/larql-terminal-browser` (see `docs/ui/terminal-browser-spec.md`). The **display port is user-resizable** and may be **generous**; **high resolution** is a goal; **GPU acceleration** is allowed; **CPU/software fallback** where no GPU exists. It is **not** a desktop web app with a narrow mode. Do not implement or merge layout, navigation, or density choices that assume a full-width desktop window as the primary surface. A normal browser may be used for debugging the same URL; `docs/ui/terminal-browser-spec.md` still treats the terminal-embedded viewport as the sole normative target.

### Rust-first workbench logic

Prefer **Rust** (`crates/larql-python`, PyO3 `larql._native`) for anything that touches the engine, vindex, LQL, or mmap weights:

- Workspace inspection for the UI is `inspect_workspace_for_ui` in Rust (returns a dict consumed by `larql_ui.ui.models.WorkspaceSummary`). Keep Python to HTTP, templates, JSON store, and glue.
- MLX availability flags still require Python (`importlib.util.find_spec` for `mlx` / `mlx_lm`) because MLX is a Python stack; pass that boolean into native inspection.

The web workbench under `python/larql_ui/ui/` is specified in:

**`larql-main/docs/ui/terminal-browser-spec.md`** (repo-relative: `../../docs/ui/terminal-browser-spec.md` from this crate, or `larql-main/docs/ui/terminal-browser-spec.md` from the monorepo root).

That document is **normative**. Do not implement or merge workbench behavior that contradicts `docs/ui/terminal-browser-spec.md` without **amending `docs/ui/terminal-browser-spec.md` in the same change** (see **Normative compliance** in that file).

When adding routes, partials, API handlers, or execution flows for the UI, follow:

- **Routing Spec** (page routes, partial routes, internal API routes)
- **Rollout Plan** phases (what belongs in Phase 1 vs 2 vs 3)
- **Backend Architecture** (Starlette, Jinja, minimal JS — native `fetch` for partials; optional Alpine only if documented in `docs/ui/terminal-browser-spec.md`)
- **Terminal viewport UX constraints**

Bindings must remain aligned with **shipped** `larql` APIs (`larql.load`, `larql.session`, etc.), as stated at the top of `docs/ui/terminal-browser-spec.md`.
