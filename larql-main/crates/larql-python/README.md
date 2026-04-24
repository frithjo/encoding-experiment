# larql — Python Bindings

## Crate Role

- Role: User-facing Python SDK/bindings and workbench UI integration
- Zone: interface
- Release impact: critical
- Stability target: managed

## Public Contract

- Python SDK APIs and workbench behavior are release-facing interfaces.
- Breaking API/UI behavior changes require migration notes and release callouts.
- Rust-backed engine logic should remain in `larql._native` where practical;
  Python-side code should stay thin and integration-focused.

Python interface to the LARQL knowledge graph engine and vindex model format. Rust-powered via PyO3, with numpy array interop and MLX integration.

## Install

### Installation Patterns

The `larql` package provides two installation modes:

**SDK only (data scientists, integrators):**
```bash
pip install larql
```
Installs only the core Python bindings (numpy dependency). Minimal footprint for programmatic access.

**SDK + UI (interactive users):**
```bash
pip install "larql[ui]"
```
Installs SDK plus the workbench UI (starlette, jinja2, uvicorn, python-multipart). Required for `larql-workbench` CLI.

**Development (both + dev tools):**
```bash
cd crates/larql-python
uv sync --group dev --extra ui   # dev tools + UI stack
uv run --no-sync maturin develop --release
uv run --no-sync pytest tests/
```

Apple Silicon (MLX): add `--extra mlx` to the `uv sync` line. The `mlx` extra pulls `mlx-lm` and its Hugging Face / `transformers` stack — separate from the Rust tokenizer used inside `larql._native`, but required for `larql.mlx` and MLX generation.

### Installation

**SDK only (data scientists, integrators):**
```bash
pip install larql
```
Installs only the core Python bindings (numpy dependency). Minimal footprint for programmatic access.

**SDK + UI (interactive users):**
```bash
pip install "larql[ui]"
```
Installs SDK plus the workbench UI (starlette, jinja2, uvicorn, python-multipart). Required for `larql-workbench` CLI.

## Workbench UI

The workbench under `larql_ui.ui` is **only** specified for a **browser running in the terminal** via the first-party client **`larql-terminal-browser`** (`crates/larql-terminal-browser`)—**resizable** display port (compact to generous), high-resolution rendering (**GPU acceleration** when available; **CPU fallback** otherwise), monospace-friendly typography, keyboard-first. Images, video, and streaming are supported at the client (see `docs/ui/terminal-browser-spec.md`). It is **not** a desktop dashboard with a narrow mode; the terminal-embedded viewport is the sole normative target. Third-party terminal browsers are optional for development only.

Engine-adjacent workbench logic (workspace inspection, execution) should live in **Rust** via PyO3 (`larql._native`); the Python package keeps HTTP, persistence, and templates thin. See `AGENTS.md` (Rust-first workbench logic).

```bash
cd crates/larql-python
uv sync --group dev --extra ui
uv run --no-sync maturin develop --release
uv run --no-sync larql-ui --host 127.0.0.1 --port 8000
```

Or use the launcher script with graceful termination and port cleanup:

```bash
cd crates/larql-python
./scripts/start-ui.sh
# Optional overrides:
# HOST=0.0.0.0 PORT=8010 ./scripts/start-ui.sh
```

Terminal runtime integration (repo component):

```bash
cd larql-main
./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh
cd crates/larql-python
./scripts/start-ui.sh
```

When present, `apps/terminal-runtime/bin/carbonyl` is used automatically for terminal rendering.
The launcher always uses the terminal runtime, and supports display options:

```bash
# disable fullscreen
TERMINAL_FULLSCREEN=0 ./scripts/start-ui.sh

# show browser chrome
TERMINAL_HIDE_UI=0 ./scripts/start-ui.sh

# pass extra runtime flags (example)
TERMINAL_CARBONYL_ARGS="--zoom 1.2" ./scripts/start-ui.sh
```

Open `http://127.0.0.1:8000` in your terminal browser, then:
- load local `.vindex`
- browse with `Explorer`
- run raw queries in `LQL`
- save simple `describe` / `lql` recipes
- inspect run history in `Runs`

**Environment**

- **`LARQL_UI_DEBUG`**: Set to `1`, `true`, or `yes` to enable Jinja2 template auto-reload (the server re-checks template files on each request). When set, skipped invalid rows while loading `runs.json` are also logged to stderr (`UiStore.list_runs`). Leave unset for normal use so the UI does not stat templates on every request.
- **`LARQL_UI_RUNTIME_TTL`**: Optional seconds (default **300**) for the process-local **vindex / session** cache (`LarqlRuntimeCache`). Executor paths reuse `larql.load` per thread until TTL expires or the workspace changes; tune for fewer reloads on large vindexes.

**Deployment / scaling**

- Run the ASGI app with **one worker process** if you rely on **background async runs** (`async: true` on `/api/*`, or HTML “Background run” on Studio / Explorer / LQL / Trace). Tasks are scheduled with `asyncio.create_task` in that process only; extra Uvicorn/Gunicorn workers will not share those jobs, so polling from another worker can look “stuck.”
- The workbench keeps only the **latest N runs** on disk (`DEFAULT_RUN_HISTORY_LIMIT` in `larql_ui/ui/store.py`, default **250**). The Runs page states this so rotations are not mistaken for data loss.
- **Workspace summaries** are cached in-process per resolved path (`WorkspaceManager`); switching workspaces or refreshing invalidates as needed. This is separate from the vindex/session TTL above.

**Errors — HTML vs JSON**

- Page flows use redirects and query parameters such as `?error=…` and `?notice=…` (and inline `page_error` in templates). JSON `/api/*` routes return a JSON object with an `error` string and an HTTP status (400, 404, 415, etc.). Clients should not expect identical shapes between the two.

Configure Python’s `logging` to capture the `larql_ui.ui` logger if you need **structured visibility** into background task failures (replacing ad-hoc stderr prints).

## Quickstart

```python
import larql

# Load a vindex
vindex = larql.load("output/gemma3-4b-v2.vindex")

# Knowledge queries — instant, no inference
edges = vindex.describe("France")
for e in edges[:5]:
    print(f"  {e.relation} → {e.target}  score={e.gate_score:.0f}")

# Full inference — Rust attention + walk FFN
result = vindex.infer("The capital of France is")
# [("Paris", 0.805), ...]

# Insert knowledge — no training
vindex.insert("Colchester", "country", "England")

# Bulk gate vectors for research (SVD, PCA)
gates = vindex.gate_vectors(layer=26)     # numpy (10240, 2560)
```

## Inference — Three Paths

### 1. Pure Rust (`vindex.infer`)

Full forward pass in Rust. No MLX, no GPU, no dependencies.

```python
vindex = larql.load("model.vindex")
result = vindex.infer("The capital of France is")
# [("Paris", 0.805), ...]
```

### 2. MLX Generation (`larql.mlx.load`)

MLX handles generation (KV cache, sampling, chat). Weights loaded from vindex.

```python
import larql, mlx_lm

model, tokenizer = larql.mlx.load("model.vindex")
response = mlx_lm.generate(model, tokenizer, prompt="...", max_tokens=20)
```

### 3. Walk FFN (`larql.walk_ffn.load`)

MLX attention + Rust sparse FFN. FFN weights mmap'd — only touched pages loaded.
For models that don't fit in memory.

```python
from larql.walk_ffn import load
import mlx_lm

model, tokenizer = load("model.vindex", top_k=4096)
response = mlx_lm.generate(model, tokenizer, prompt="...", max_tokens=20)
# Walk FFN: 7.1 GB FFN weights handled by Rust (not in MLX memory)
```

### 4. WalkModel (zero-copy mmap)

Rust inference with mmap'd weights. Load RSS: ~450 MB for a 4B model (vs 18 GB heap).
For 120B models: ~1 GB RSS instead of 220 GB.

```python
wm = larql.WalkModel("model.vindex", top_k=4096)
result = wm.predict("The capital of France is")
# [("Paris", 0.498), ...]
```

### Memory & Performance (Gemma 3 4B, f32)

| Path | Load RSS | Inference | How |
|---|---|---|---|
| `WalkModel` / `vindex.infer()` | **+0 MB** (mmap) | 19s→13s (warms up) | Zero-copy mmap, OS pages on demand |
| `larql.mlx.load()` | +22 GB | 0.9s (GPU) | All weights in MLX/GPU memory |
| Native MLX | +8.6 GB | 0.9s (GPU) | Safetensors in GPU memory |

`vindex.infer()` uses mmap'd weights (lazy-loaded on first call, reused after).
The OS page cache warms up across calls — second call is faster, third faster still.

For 120B models: `WalkModel` ~1 GB load RSS vs native 220 GB.
With madvise prefetching, steady-state ~200-500ms/token after cache warms.

## LQL Session

```python
session = larql.session("model.vindex")
session.query("DESCRIBE 'France'")
session.query("WALK 'The capital of France is' TOP 10")
session.vindex.gate_vectors(layer=26)  # numpy access on same session
```

## API Reference

### Loading

| Function | Description |
|---|---|
| `larql.load(path)` | Load vindex, returns `Vindex` |
| `larql.session(path)` | LQL session with `.query()` and `.vindex` |
| `larql.mlx.load(path)` | MLX model from vindex (all weights in MLX) |
| `larql.walk_ffn.load(path, top_k)` | MLX attention + Rust FFN (mmap'd) |
| `larql.WalkModel(path, top_k)` | Rust inference with mmap'd weights |

### Vindex — Inference

| Method | Description |
|---|---|
| `infer(prompt, top_k_predictions=5, top_k_features=8192)` | Full Rust forward pass, returns `[(token, prob)]` |

### Vindex — Knowledge Queries

| Method | Description |
|---|---|
| `describe(entity, band="knowledge", verbose=False)` | Find all knowledge edges |
| `has_edge(entity, relation=None)` | Check if entity has edges |
| `get_target(entity, relation)` | Get target token for entity+relation |
| `relations()` | Cluster-derived relation types (`Relation`: `name`, `cluster_id`, `count`, `top_tokens`), sorted by decreasing count. Empty if there is no cluster catalogue (e.g. missing `relation_clusters.json` or zero clusters); **probe-only** vindexes return `[]` while `describe()` can still attach probe labels. Garbage-like cluster labels are skipped in Rust. Use `stats()` for `num_clusters` / `num_probe_labels`. |
| `probe_relations()` | Probe-only relation names from `feature_labels.json`, aggregated per name (`ProbeRelation`: `name`, `count`). Sorted by decreasing count. Empty if there are no probe entries. Complements `relations()` (clusters vs probes). |
| `cluster_centre(relation)` | Relation direction vector as numpy |
| `typical_layer(relation)` | Most common layer for a relation |
| `stats()` | Model metadata as dict |

### Vindex — Feature Access

| Method | Returns |
|---|---|
| `embed(text)` | `numpy (hidden_size,)` — scaled, multi-token averaged |
| `gate_vector(layer, feature)` | `numpy (hidden_size,)` |
| `gate_vectors(layer)` | `numpy (num_features, hidden_size)` |
| `embedding(token_id)` | `numpy (hidden_size,)` — unscaled |
| `embedding_matrix()` | `numpy (vocab_size, hidden_size)` |
| `feature_meta(layer, feature)` | `FeatureMeta` or `None` |
| `feature(layer, feature)` | `dict` or `None` |
| `feature_label(layer, feature)` | `str` or `None` |
| `tokenize(text)` / `decode(ids)` | Tokenizer access |

### Vindex — KNN & Walk

| Method | Description |
|---|---|
| `gate_knn(layer, query_vector, top_k=10)` | Raw KNN with vector |
| `entity_knn(entity, layer, top_k=10)` | Embed entity then KNN |
| `walk(residual, layers=None, top_k=5)` | Walk with raw vector |
| `entity_walk(entity, layers=None, top_k=5)` | Walk with entity string |

### Vindex — Mutation

| Method | Description |
|---|---|
| `insert(entity, relation, target, layer=None, confidence=0.8)` | Insert knowledge edge |
| `delete(entity, relation=None, layer=None)` | Delete matching edges |

### WalkModel

| Method | Description |
|---|---|
| `WalkModel(path, top_k=8192)` | Load with mmap'd weights (zero-copy) |
| `predict(prompt, top_k_predictions=5)` | Full forward pass, returns `[(token, prob)]` |
| `ffn_layer(layer, x_bytes, seq_len)` | Per-layer sparse FFN (bytes in/out) |
| `num_layers`, `hidden_size`, `top_k` | Properties |

### Session

| Method | Description |
|---|---|
| `query(lql)` | Execute LQL, returns `list[str]` |
| `query_text(lql)` | Execute LQL, returns joined string |
| `vindex` | Access underlying `Vindex` |

### Types

| Type | Key Fields |
|---|---|
| `DescribeEdge` | `relation`, `target`, `gate_score`, `layer`, `feature`, `source`, `confidence`, `also` |
| `WalkHit` | `layer`, `feature`, `gate_score`, `top_token`, `target`, `meta` |
| `FeatureMeta` | `top_token`, `top_token_id`, `c_score`, `top_k` |
| `Relation` | `name`, `cluster_id`, `count`, `top_tokens` (one row per cluster catalogue entry, not probe-only features) |
| `ProbeRelation` | `name`, `count` (aggregated features per probe relation name from `feature_labels.json`) |

## Project Structure

```
crates/larql-python/
  src/
    lib.rs              # Module registration, graph bindings
    vindex.rs           # PyVindex: describe, insert, relations, infer
    session.rs          # PySession (LQL queries)
    walk.rs             # WalkModel: mmap'd weights, Rust walk FFN
  python/larql/
    __init__.py         # Clean Python API
    mlx.py              # MLX model loading from vindex (mmap)
    walk_ffn.py         # MLX attention + Rust walk FFN
  tests/
    test_bindings.py    # Synthetic vindex tests + real vindex integration
  examples/
    knowledge.py        # Describe, relations, steering
    insert.py           # Insert knowledge, no training
    session.py          # LQL session + numpy access
    infer.py            # Rust inference (vindex.infer / WalkModel)
    mlx_vindex.py       # MLX generation from vindex weights
  bench/
    bench_bindings.py   # Speed + memory benchmarks
```

### Running Tests

```bash
# Synthetic tests (run anywhere, no model files)
pytest crates/larql-python/tests/ -v

# With real vindex (integration tests for infer, WalkModel, MLX)
REAL_VINDEX_PATH=output/gemma3-4b-v2.vindex pytest crates/larql-python/tests/ -v
```

### Extracting a Vindex

```bash
# Browse level (knowledge queries only)
larql extract-index "google/gemma-3-4b-it" -o model.vindex

# All weights (for inference + MLX)
larql extract-index "google/gemma-3-4b-it" -o model.vindex --level all

# Half precision (recommended for MLX)
larql extract-index "google/gemma-3-4b-it" -o model.vindex --level all --f16
```
