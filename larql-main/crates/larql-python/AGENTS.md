# LARQL Python — agent notes

## Workbench UI (strict)

The Carbonyl / web workbench under `python/larql/ui/` is specified in:

**`larql-main/plan.md`** (repo-relative: `../../plan.md` from this crate, or `larql-main/plan.md` from the monorepo root).

That document is **normative**. Do not implement or merge workbench behavior that contradicts `plan.md` without **amending `plan.md` in the same change** (see **Normative compliance** in that file).

When adding routes, partials, HTMX, API handlers, or execution flows for the UI, follow:

- **Routing Spec** (page routes, partial routes, internal API routes)
- **Rollout Plan** phases (what belongs in Phase 1 vs 2 vs 3)
- **Backend Architecture** (FastAPI, Jinja, HTMX, optional Alpine)
- **Carbonyl-Specific UX Constraints**

Bindings must remain aligned with **shipped** `larql` APIs (`larql.load`, `larql.session`, etc.), as stated at the top of `plan.md`.
