## Carbonyl UI Spec

This spec defines a **terminal-first web workbench** for LARQL recipes and prompt generation, rendered inside Carbonyl. The product should stay aligned with the **actual shipped Python bindings**, not the broader aspirational docs: the main execution surfaces today are `larql.load(...)`, `larql.session(...)`, `Vindex.describe(...)`, `Vindex.infer(...)`, `WalkModel.predict(...)`, `WalkModel.trace(...)`, and the optional MLX loaders in `larql.mlx`, `larql.streaming`, and `larql.walk_ffn`. The core design goal is a **simple, inspectable, keyboard-first HTML UI** that remains usable in a narrow terminal browser while still exposing serious LARQL capabilities.

### Grounding assumptions
- The UI runs locally against a local `.vindex`.
- The first release targets the Python package at `crates/larql-python/python/larql`.
- Carbonyl is Chromium-based, so modern HTML/CSS/JS work, but the UI should still behave like a restrained terminal app rather than a dense desktop dashboard.
- The first release should prefer **server-rendered HTML + HTMX**, with minimal JS and no SPA requirement.
- The UI should not present features as “ready” unless they are clearly supported by the current Python API.

A few implementation facts that matter directly to the spec:

```16:24:larql-main/crates/larql-python/python/larql/__init__.py
from larql._native import (
    Vindex,
    FeatureMeta,
    WalkHit,
    DescribeEdge,
    Relation,
    Session,
    WalkModel,
```

```52:83:larql-main/crates/larql-python/python/larql/__init__.py
def load(path: str, **kwargs) -> "Vindex":
    ...
    return load_vindex(path)

def session(path: str) -> "Session":
    ...
    return create_session(path)
```

```61:79:larql-main/crates/larql-python/src/session.rs
fn query(&mut self, lql: &str) -> PyResult<Vec<String>> {
    // Add semicolon if missing
    let input = if lql.trim_end().ends_with(';') {
        lql.to_string()
    } else {
        format!("{};", lql)
    };
```

```421:457:larql-main/crates/larql-python/src/vindex.rs
#[pyo3(signature = (residual, layers=None, top_k=5))]
fn walk(
    &self, residual: Vec<f32>, layers: Option<Vec<usize>>, top_k: usize
) -> Vec<PyWalkHit> {
    let arr = Array1::from_vec(residual);
    let layer_list = layers.unwrap_or_else(|| self.index.loaded_layers());
    let trace = self.index.walk(&arr, &layer_list, top_k);
```

That last point is key: **raw `Vindex.walk(...)` is residual-vector-first, not prompt-string-first**, so the text-facing prompt experience should route through LQL and inference flows rather than exposing `walk` as a plain text prompt box.

---

## Product Goals

### Primary goal
Create a UI that lets users:
- load a `.vindex`
- define and save reusable **recipes**
- render prompts from templates and variables
- run browse/query/generation/trace workflows
- inspect outputs in both human-readable and structured forms
- compare runs without dropping into Python scripts

### Secondary goals
- make advanced LARQL capabilities feel approachable
- preserve reproducibility of every run
- stay robust under large models and long-running inference
- remain legible and efficient inside Carbonyl

### Non-goals for v1
- full patch lifecycle UI
- compile/export UI
- remote server integration
- graph-canvas visualization
- mutation-first editing workflows
- notebook-style freeform documents

These are out because the current Python layer is not yet the right stable abstraction for them.

---

## Design Principles

1. **Terminal-first, not text-only**  
   Use HTML, forms, tabs, cards, and tables, but optimize for narrow widths, low visual noise, keyboard navigation, and stable layouts.

2. **One dominant task per screen**  
   Every page should have a clear primary action and one main result region.

3. **Structured before pretty**  
   Output should be inspectable, exportable, and reproducible before it is visually sophisticated.

4. **Progressive disclosure**  
   Default views should be brief and readable. Raw JSON, feature IDs, layers, and trace internals should be available but not forced.

5. **Capability-gated UI**  
   If a vindex lacks weights, inference buttons should disable. If MLX is unavailable, MLX engines should hide or show “Unavailable”.

6. **No fake affordances**  
   Do not show controls that imply features the Python API does not cleanly expose today.

---

## User Personas

### 1. Explorer
Wants to ask “What does this model know about X?” and browse relations quickly.

### 2. Recipe author
Wants to define prompt templates, run them repeatedly with different subjects, and compare outputs.

### 3. Investigator
Wants to inspect layer-level or trace-level internals and connect outputs to specific features/layers.

### 4. Power user
Wants raw LQL and low ceremony access to the underlying engine.

---

## Scope

### In scope
- local workspace loading for `.vindex`
- saved recipe system
- prompt rendering with variables
- browse/query/generation/trace workflows
- run history and result persistence
- Carbonyl-optimized layout and interaction design
- compare mode for repeated runs
- keyboard shortcuts and focus management

### Out of scope
- patch save/apply/remove UI
- compile/recompile/export UI
- remote vindex connections
- multi-user auth
- collaborative editing
- production deployment concerns beyond local app startup

---

## Information Architecture

The application should have four top-level work areas:

1. `Studio`
2. `Explorer`
3. `LQL`
4. `Trace`

There should also be two supporting surfaces:
- `Recipes`
- `Runs`

### Global shell
- top bar
- primary navigation
- workspace status strip
- main content region
- transient notification area

### Top bar contents
- app title: `LARQL Workbench`
- current workspace name
- capability badges
- shortcut hint
- global search / command field trigger

### Capability badges
Derived from workspace inspection:
- `browse`
- `infer`
- `trace`
- `mlx`
- `walk_ffn`
- `labels`

Badges should have plain text semantics, e.g.:
- `infer: ready`
- `trace: unavailable`
- `mlx: missing deps`

---

## Workspace Model

A workspace is a loaded `.vindex` plus discovered runtime capabilities.

### Workspace fields
- `id`
- `path`
- `display_name`
- `model`
- `family`
- `num_layers`
- `hidden_size`
- `vocab_size`
- `extract_level` if detectable
- `has_model_weights`
- `has_relation_labels`
- `has_layer_bands`
- `supports_infer`
- `supports_trace`
- `supports_mlx`
- `supports_streaming`
- `supports_walk_ffn`
- `warnings[]`

### Workspace inspection behavior
On load:
1. call `larql.load(path)`
2. call `vindex.stats()`
3. probe `layer_bands()`
4. probe `relations()` and classifier-dependent surfaces
5. detect whether MLX imports succeed
6. determine availability of `WalkModel` and inference weights

If inspection fails, the user gets a stable error page with:
- path
- failure summary
- likely cause
- next actions

---

## Recipe System

Recipes are the central UX abstraction. They are app-level objects, not core `larql` objects.

### Recipe types
Two first-class kinds:

#### 1. Probe recipe
For structured knowledge retrieval and text-backed browse workflows.
Examples:
- `DESCRIBE "{subject}"`
- prompt template `"The capital of {subject} is"` used in `INFER` or LQL `WALK`

#### 2. Generation recipe
For completion/generation workflows.
Examples:
- `"The capital of {subject} is"`
- `"Summarize the key facts about {subject} in one sentence"`

### Recipe schema
```json
{
  "id": "capital-probe",
  "name": "Capital Probe",
  "kind": "probe",
  "template": "The capital of {subject} is",
  "lql_template": null,
  "description": "Probe likely capital relation using a subject slot.",
  "variables": [
    {
      "name": "subject",
      "label": "Subject",
      "type": "text",
      "required": true,
      "default": "France"
    }
  ],
  "default_engine": "infer",
  "options": {
    "band": "knowledge",
    "top_k": 5,
    "top_k_predictions": 5,
    "max_tokens": 16,
    "trace": false,
    "layers": null
  },
  "tags": ["geography", "relation"],
  "notes": ""
}
```

### Supported template fields
- plain text template using `{subject}` and future variables
- optional `lql_template` for LQL-native recipes
- default execution engine
- default tuning parameters
- tags and notes
- display metadata

### Template validation
- variables must be declared
- unsupported placeholders must fail validation
- empty templates disallowed
- warn if rendered prompt exceeds configured display threshold
- for LQL templates, validate against placeholder expansion and likely semicolon rules

### Why keep both `template` and `lql_template`
Because some workflows are best expressed as:
- prompt text
- others as raw LQL statements

The UI should not force one representation onto both.

---

## Engine Model

The user runs a recipe through one engine.

### Engines in v1
- `describe`
- `lql`
- `infer`
- `walk_model`
- `trace`
- `mlx_dense`
- `mlx_streaming`
- `mlx_walk_ffn`

### Engine semantics

#### `describe`
Calls `Vindex.describe(...)` with entity text and options.

Use for:
- entity browse
- quick fact lookup
- low-latency knowledge inspection

#### `lql`
Calls `Session.query(...)`.

Use for:
- `DESCRIBE`
- `WALK`
- `STATS`
- raw LQL snippets
- expert workflows

#### `infer`
Calls `Vindex.infer(...)`.

Use for:
- lightweight next-token prediction
- recipe prompts
- compare against structured browse results

#### `walk_model`
Calls `WalkModel.predict(...)`.

Use for:
- explicit mmap-backed inference path
- comparison against `Vindex.infer`

#### `trace`
Calls `WalkModel.trace(...)` or `Vindex.infer_trace(...)`.

Use for:
- layer-level investigation
- answer trajectory
- residual decomposition

#### `mlx_dense`
Calls `larql.mlx.load(...)` then generation.

#### `mlx_streaming`
Calls `larql.streaming.load(...)` then generation.

#### `mlx_walk_ffn`
Calls `larql.walk_ffn.load(...)` then generation.

### Engine availability rules
- `describe`: always if workspace loads
- `lql`: always if session loads
- `infer`: only if model weights exist
- `walk_model`: only if model weights exist
- `trace`: only if model weights exist
- MLX engines: only if MLX deps import and platform/runtime allow it

### UX rule
The engine selector should only show valid choices for the current workspace, with disabled entries shown only if explanatory text is important.

---

## Top-Level Screens

## 1. Studio

### Purpose
Main recipe authoring and execution surface.

### Layout
Terminal-optimized stacked layout:

1. recipe header
2. input form
3. run controls
4. rendered preview
5. results

At wider terminal sizes, preview/results may become two panes. Default should still work vertically.

### Sections

#### Recipe header
- recipe name
- kind
- tags
- duplicate / save / delete
- unsaved changes indicator

#### Input form
- template textarea
- optional LQL template textarea
- variable fields
- notes
- advanced toggle

#### Run controls
- engine selector
- top-k
- band
- layers
- max tokens
- trace toggle
- compare toggle
- `Run`
- `Save recipe`

#### Rendered preview
Shows:
- rendered prompt
- rendered LQL if applicable
- validation warnings
- variable substitution summary

#### Results area
Tabbed, but degrades to stacked sections:
- `Result`
- `Structured`
- `Trace`
- `Raw`
- `Run JSON`

### Result display rules
- `Result`: short human-readable output
- `Structured`: edges/predictions tables or cards
- `Trace`: only shown when trace exists
- `Raw`: raw returned data
- `Run JSON`: exact persisted run record

### Primary workflows
- create recipe
- run recipe
- tweak parameters and rerun
- duplicate recipe
- compare subjects
- inspect output and save run

---

## 2. Explorer

### Purpose
Fast browse surface for loaded vindexes.

### Layout
- entity input
- quick action row
- results section
- detail section

### Quick actions
- `Describe`
- `Relations`
- `Stats`
- `Layer bands`
- `Feature by layer/id`

### Default mode
Entity browse with `Describe`.

### Result presentation
Use stacked relation cards by default:
- relation
- target
- score
- layer
- source
- also tokens

Expandable details:
- feature id
- confidence
- raw metadata

### Secondary views
- relations list
- workspace stats
- feature inspector
- tokenization helper

### Why this matters
This gives a no-ceremony browse mode without forcing recipe creation.

---

## 3. LQL

### Purpose
Power-user text console.

### Layout
- query editor textarea
- snippet picker
- run button
- output region
- optional history panel

### Features
- semicolon optional in UI
- recent queries
- save as recipe
- save as snippet
- pretty/plain result toggle

### Snippet library
Seed with:
- `STATS`
- `DESCRIBE "France"`
- `WALK "The capital of France is" TOP 5`
- `SHOW RELATIONS`
- others only if supported by current Python and session behavior

### Output
Prefer plain text or preformatted blocks first. Avoid forcing tables on all LQL output.

---

## 4. Trace

### Purpose
Dedicated analysis view for long-form inference introspection.

### Layout
- prompt field
- engine selector
- trace options
- summary section
- layer table
- per-layer drilldown

### Controls
- prompt
- positions: `last` / `all`
- answer token for trajectory
- top-k to display
- filter layer range

### Views
- summary
- answer trajectory
- per-layer top-k
- decomposition
- raw trace metadata

### UI presentation
Default to a layer table, not charts:
- layer
- rank
- probability
- attn contribution
- ffn contribution

Charts can be added later, but table-first is safer for Carbonyl.

---

## Supporting Screens

## Recipes
A searchable list of saved recipes.

Columns or cards:
- name
- kind
- default engine
- updated time
- tags

Actions:
- open
- duplicate
- delete
- export

## Runs
A searchable list of executed runs.

Fields:
- recipe name
- subject / variables summary
- engine
- status
- duration
- timestamp

Actions:
- open
- rerun
- compare
- export JSON

---

## Interaction Model

### Navigation
- explicit nav links at top
- no hamburger-only navigation
- breadcrumbs on deep views
- current page clearly marked

### Focus behavior
- keyboard focus always visible
- main heading focusable after route change
- post-run focus moves to result heading, not randomly into a nested control

### No hover dependency
All important information and actions must be visible or focusable without hover.

### Motion
- no decorative transitions
- only subtle expand/collapse if needed
- no animated dashboards

### Long-running actions
For `infer`, `trace`, and MLX:
- user submits job
- UI shows stable pending state
- page polls or HTMX-refreshes status region
- result replaces pending state when done

Use polling first; SSE can come later.

---

## Carbonyl-Specific UX Constraints

### Layout constraints
- target comfortable use at `~100x30`
- max content width around `80-120ch`
- stack panels before shrinking them too far
- avoid 3-column layouts
- avoid wide data tables as the primary presentation

### Typography
- monospace-friendly
- high contrast
- large line height
- clear section separation with spacing and borders, not subtle color shifts alone

### Controls
- label-above-input
- large click targets
- clear pressed/disabled state
- grouped action rows

### Tables
Allowed for:
- runs
- recipes
- relation lists
- trace summaries

But must:
- wrap gracefully
- allow stacked card fallback on narrow widths

### Avoid in v1
- drag/drop
- resizable panes
- canvas-dependent graph viewers
- complex charting libraries
- dense nested accordions

---

## Routing Spec

### Page routes
- `/`
- `/workspace`
- `/studio`
- `/explorer`
- `/lql`
- `/trace`
- `/recipes`
- `/recipes/{recipe_id}`
- `/runs`
- `/runs/{run_id}`

### Partial routes for HTMX
- `/partials/workspace-status`
- `/partials/recipe-editor`
- `/partials/run-controls`
- `/partials/result-panel`
- `/partials/trace-summary`
- `/partials/runs-table`
- `/partials/recipes-list`

### Internal API routes
- `POST /api/workspace/open`
- `GET /api/workspace/current`
- `POST /api/recipes`
- `PUT /api/recipes/{id}`
- `DELETE /api/recipes/{id}`
- `POST /api/runs`
- `GET /api/runs/{id}`
- `POST /api/runs/{id}/rerun`
- `POST /api/explorer/describe`
- `POST /api/explorer/relations`
- `POST /api/lql/query`
- `POST /api/trace/run`

### Why both page and API routes
- page routes support direct navigation and browser history
- API routes support partial updates, background jobs, and clean testing

---

## Backend Architecture

### Recommended stack
- FastAPI
- Jinja2 templates
- HTMX
- small Alpine.js only if necessary
- plain CSS with strong utility classes or a tiny custom design system

### App package layout
Suggested placement under `crates/larql-python/python/larql/ui/`:

- `app.py`
- `routes/`
  - `pages.py`
  - `workspace_api.py`
  - `recipes_api.py`
  - `runs_api.py`
  - `explorer_api.py`
  - `lql_api.py`
  - `trace_api.py`
- `services/`
  - `workspace_service.py`
  - `recipe_service.py`
  - `run_service.py`
  - `capability_service.py`
  - `execution_service.py`
  - `trace_service.py`
- `models/`
  - `workspace.py`
  - `recipe.py`
  - `run.py`
- `templates/`
- `static/`

### Service responsibilities

#### `workspace_service`
- open workspace
- cache `Vindex`, `Session`, and maybe `WalkModel`
- expose workspace summary

#### `capability_service`
- determine supported engines and warnings

#### `recipe_service`
- CRUD recipes
- validate templates
- render templates with variables

#### `execution_service`
- dispatch runs to appropriate engine
- normalize outputs into a common run artifact schema
- handle sync vs background execution

#### `trace_service`
- trace-specific transformation and summarization

#### `run_service`
- persist runs
- list/filter runs
- rerun from stored parameters

---

## Execution Flow

### Run submission flow
1. user edits or selects recipe
2. UI validates variables and engine availability
3. UI posts concrete rendered input to `/api/runs`
4. backend creates `Run` record with status `pending`
5. backend executes or enqueues job
6. UI polls run status
7. backend stores normalized results
8. UI shows result panels
9. run is available in history and rerunnable

### Sync vs async rules
- `describe`, `relations`, `stats`, simple LQL: sync
- `infer`, `trace`, MLX: async by default

### Caching rules
- cache workspace objects per path
- avoid reloading tokenizer/model on every request
- separate cached long-lived objects from per-run artifacts

---

## Run Artifact Schema

Every run should produce a normalized record independent of engine.

```json
{
  "id": "run_123",
  "recipe_id": "capital-probe",
  "workspace_id": "wksp_1",
  "status": "completed",
  "engine": "infer",
  "rendered_input": {
    "prompt": "The capital of France is",
    "lql": null,
    "variables": {
      "subject": "France"
    }
  },
  "options": {
    "top_k_predictions": 5,
    "trace": false
  },
  "timing": {
    "queued_ms": 0,
    "run_ms": 842
  },
  "result": {
    "summary": "Top token: Paris (80.5%)",
    "predictions": [
      ["Paris", 0.805]
    ],
    "edges": null,
    "trace": null,
    "raw": {}
  },
  "created_at": "2026-04-21T12:00:00Z"
}
```

### Output normalization
- `summary`: single human-readable line
- `predictions`: next-token style results
- `edges`: describe-style structured results
- `trace`: trace summary payload
- `raw`: exact backend-specific response material

This makes compare and rerun much simpler.

---

## Recipe-to-Engine Mapping

### Probe recipe mapping
Default choices:
- `describe` for entity lookup
- `lql` for query-like templates
- `infer` when prompt phrasing matters

### Generation recipe mapping
Default choices:
- `infer`
- `walk_model`
- MLX variants when available

### Explicit rule
Do not silently reinterpret a prompt as a residual walk. If the user wants `walk`, drive it through LQL or advanced tools, not direct `Vindex.walk(...)` text plumbing.

---

## Compare Mode

Compare mode is a major value add and should be first-class.

### Supported compare modes
- same recipe, different subjects
- same subject, different engines
- same prompt, different parameter sets

### Compare presentation
Use stacked comparison cards first, table second.

Default fields:
- rendered input
- top result
- latency
- engine
- warnings

### Compare constraints
- max 3-5 comparisons visible at once in v1
- explicit user-triggered compare only
- no auto-running huge comparison matrices

---

## Persistence

### Storage for v1
Local JSON or SQLite.

Recommended:
- SQLite for runs
- JSON files for recipes if simplicity is prioritized
- or SQLite for both if wanting better search/filter consistency

### Persisted entities
- workspaces (recent list only)
- recipes
- runs
- snippets
- user UI preferences

### Non-persisted
- large raw trace binaries unless explicitly exported
- cached live model objects

---

## Error Handling Spec

### Error classes to expose
- workspace load error
- missing weights
- missing MLX deps
- invalid template
- invalid LQL
- execution runtime error
- trace unavailable
- unsupported engine for workspace

### Error message format
Each error panel should show:
- short title
- what failed
- likely cause
- suggested next action

Example:
- `Inference unavailable`
- `This vindex does not include model weights.`
- `Extract the index with --level all or use browse/LQL modes.`

### Logging
- backend logs full traceback
- UI shows concise actionable message
- run record stores failure summary

---

## Keyboard Shortcut Spec

### Global
- `/` focus command/search
- `g s` go to Studio
- `g e` go to Explorer
- `g q` go to LQL
- `g t` go to Trace
- `g r` go to Runs

### Page-local
- `Ctrl+Enter` run current form
- `Esc` close modal/panel
- `[` and `]` switch result tabs
- `j` / `k` move selected result row where appropriate

### Important rule
All shortcuts must have clickable equivalents. Keyboard-first, not keyboard-only.

---

## Visual Spec

### Layout system
- single content column by default
- optional two-pane at large widths only
- section spacing consistent
- panels with solid borders
- active regions visibly highlighted

### Color
- minimal palette
- strong contrast
- status colors only for meaningful states:
  - green success
  - yellow warning
  - red error
  - blue active

### Result cards
Each result card should have:
- title row
- key fields
- details toggle
- actions row

### Code/text blocks
- preformatted output area
- horizontal wrapping preferred over forced overflow when possible
- copy/export button optional

---

## Validation and Testing

### Validation layers
1. form validation
2. template validation
3. engine availability validation
4. workspace capability validation
5. backend execution validation

### Acceptance checks for v1
- load valid `.vindex`
- reject invalid workspace cleanly
- save/load/edit/delete recipe
- run `describe`
- run `LQL`
- run `infer` when supported
- show disabled inference on browse-only workspace
- persist run history
- rerun prior run
- maintain usable layout in Carbonyl-sized viewport

### Suggested automated tests
- unit tests for recipe rendering
- unit tests for capability detection
- integration tests for route responses
- UI smoke tests for:
  - navigation
  - run creation
  - result rendering
  - error states

### Manual verification
Run inside Carbonyl and verify:
- navigation works by keyboard
- no critical controls require hover
- forms remain legible at narrow width
- no layout collapse under long prompts/results

---

## Rollout Plan

## Phase 1
- workspace load
- global shell
- Explorer
- LQL
- minimal Recipes
- describe/LQL runs
- run history

## Phase 2
- full Studio
- recipe CRUD
- infer and walk_model engines
- normalized run records
- compare mode

## Phase 3
- Trace screen
- MLX engine integration
- richer result presentation
- export/import recipes and runs

---

## Recommended First Implementation Slice

If you want the first build to be both useful and low-risk, implement this order:

1. workspace loader and capability inspector
2. global shell and navigation
3. Explorer `Describe`
4. LQL console
5. recipe model and persistence
6. Studio with `describe` + `infer`
7. run history
8. trace screen
9. MLX engines
10. compare mode polish

That gets a real product in users’ hands without betting early on the heaviest paths.

## Open Questions
- Should recipes be stored inside the repo/workspace or in a user-level app data directory?
- Do you want compare mode in the first milestone, or only after the basic run flow is stable?
- Should the first trace view use `WalkModel.trace(...)` only, or also expose `Vindex.infer_trace(...)` as a lighter-weight option?

## Implementation Plan
[ ] Add a local web app entrypoint under `crates/larql-python/python/larql/ui/` using FastAPI + Jinja + HTMX and keep routing server-rendered by default.  
[ ] Add workspace inspection and caching services that wrap `larql.load(...)`, `larql.session(...)`, and optionally `WalkModel(...)`, then expose capability badges and warnings.  
[ ] Add recipe models, validation, rendering, and persistence for probe/generation recipes with template variables and engine defaults.  
[ ] Add the `Studio`, `Explorer`, `LQL`, `Trace`, `Recipes`, and `Runs` page routes plus matching partials for HTMX updates.  
[ ] Add execution services that normalize `describe`, `LQL`, `infer`, `walk_model`, trace, and later MLX outputs into a single run artifact schema.  
[ ] Add background run handling for long-running engines with polling-based status updates and stable pending/error/result states.  
[ ] Add Carbonyl-first styling and interaction rules: stacked layouts, keyboard shortcuts, visible focus, no hover-only actions, and narrow-width-friendly result views.  
[ ] Add automated tests for recipe rendering, capability detection, API route behavior, and error states, then manually verify the UI inside Carbonyl at terminal-like dimensions.