# Python Bindings vs UI Split Design

## Objective

Define a clear boundary between the Python SDK (bindings) and the workbench UI, enabling independent versioning and release cadence while preserving backward compatibility.

## Current State

The `larql` Python package currently combines:
- **Core bindings**: PyO3 native extension (`larql._native`) exposing vindex, graph, and LQL APIs
- **UI workbench**: Starlette web server (`larql.ui`) with Jinja templates, served via `larql-ui` CLI entrypoint

Both are shipped in a single `pyproject.toml` with:
- Core dependencies: `numpy` (required)
- UI dependencies: `starlette`, `jinja2`, `uvicorn`, `python-multipart` (optional via `[ui]` extra)

## Target Ownership Model

### Core SDK Package (`larql`)

**Owner**: Release-facing contract team
**Stability**: Stable (avoid breaking changes without migration notes)
**Purpose**: Provide programmatic access to LARQL engine for data scientists, researchers, and integrators

**Public API surface**:
```python
import larql

# Vindex API
vindex = larql.load("path/to.vindex")
embed = vindex.embed("France")
edges = vindex.describe("France")

# LQL API
session = larql.session("path/to.vindex")
session.query("DESCRIBE 'France'")

# Graph API
from larql import load_graph, save_graph, pagerank, bfs_traversal
```

**Dependencies**: Minimal - only `numpy` for array interoperability
**Release cadence**: Tied to Rust workspace releases

### UI Workbench Package (`larql-ui`)

**Owner**: Workbench/UX team
**Stability**: Iterative (rapid changes acceptable)
**Purpose**: Browser-in-terminal workbench for interactive exploration

**Public API surface**:
```bash
larql-ui --host 127.0.0.1 --port 8000
```

**Dependencies**: Web stack (`starlette`, `jinja2`, `uvicorn`, `python-multipart`)
**Release cadence**: Independent from core SDK - can ship UI features faster

## Split Mechanics

### Option A: Single Crate, Two Packages (Recommended for P2)

Keep the current Rust crate structure (`crates/larql-python/`) but split Python packaging:

**Structure**:
```
crates/larql-python/
  pyproject.toml          # Defines both packages
  python/
    larql/                # Core SDK package
      __init__.py
      _native/            # PyO3 extension
      mlx/                # Optional MLX integration
      streaming/          # Optional streaming
      walk_ffn/           # Optional Walk FFN
    larql_ui/             # UI package (separate namespace)
      __init__.py
      ui/
        __main__.py
        app.py
        templates/
        static/
  tests/
    test_bindings.py      # SDK tests
    test_ui.py            # UI tests
```

**pyproject.toml changes**:
```toml
[project]
name = "larql"  # Core SDK only

[project.optional-dependencies]
ui = ["larql-ui>=0.1.0"]  # UI as separate package dependency

# Add second project for UI
[project.urls-larql-ui]
name = "larql-ui"
version = "0.1.0"
dependencies = [
    "larql>=0.1.0",
    "starlette>=0.37.0",
    "jinja2>=3.1.6",
    "python-multipart>=0.0.26",
    "uvicorn>=0.44.0",
]

[project.scripts]
larql-ui = "larql_ui.ui.__main__:main"
```

**Pros**:
- Minimal Rust changes
- Shared PyO3 extension reduces build complexity
- Clear dependency direction (UI depends on SDK)

**Cons**:
- Still coupled in Rust build
- Requires coordinated releases if PyO3 API changes

### Option B: Separate Crates (Future P3)

Create two distinct Rust crates:
```
crates/
  larql-python/          # Core SDK only
    python/larql/
  larql-python-ui/       # UI workbench only
    python/larql_ui/
```

**Pros**:
- Complete decoupling of release cycles
- Independent dependency management

**Cons**:
- Duplicate PyO3 build infrastructure
- Higher maintenance burden
- Risk of API divergence

**Recommendation**: Start with Option A for P2, evaluate Option B for P3 if release cadence needs diverge significantly.

## Staged Migration Path

### Stage 1: Packaging Isolation (P2 - Current Wave)

**Goal**: Make UI an installable extra without breaking existing imports

**Changes**:
1. Update `pyproject.toml` to declare UI as optional dependency
2. Keep current `larql.ui` namespace for backward compatibility
3. Add deprecation notice for direct `larql.ui` imports
4. Document recommended installation patterns

**Installation patterns**:
```bash
# SDK only (data scientists, integrators)
pip install larql

# SDK + UI (interactive users)
pip install "larql[ui]"

# Development (both + dev tools)
uv sync --extra ui --group dev
```

**Backward compatibility**: Preserve all existing imports:
```python
# Still works (deprecated but supported)
from larql.ui import app
import larql.ui

# Recommended going forward
from larql_ui.ui import app
import larql_ui
```

### Stage 2: Namespace Migration (Future Wave)

**Goal**: Move UI to separate `larql-ui` package namespace

**Changes**:
1. Create separate `larql-ui` PyPI package
2. Update `larql` package to depend on `larql-ui` for `[ui]` extra
3. Add migration guide for existing users
4. Keep `larql.ui` as compatibility shim for 1-2 releases

### Stage 3: Full Decoupling (Future Wave)

**Goal**: Option B - separate Rust crates if needed

**Trigger**: If UI release cadence needs to diverge significantly from SDK (e.g., weekly UI releases vs monthly SDK releases)

## Contract Boundaries

### SDK Contract (Stable)

**What users can depend on**:
- `larql.load()`, `larql.session()` entrypoints
- Vindex API: `.embed()`, `.describe()`, `.walk()`
- LQL session API: `.query()`, `.vindex` access
- Graph API: `load_graph`, `save_graph`, algorithms
- PyO3 native types: `Vindex`, `Session`, `Edge`, `Node`, etc.

**Stability policy**:
- Breaking changes require major version bump
- Migration notes required for any API change
- Deprecated APIs supported for at least 1 minor version

### UI Contract (Iterative)

**What users can depend on**:
- `larql-ui` CLI entrypoint (`--host`, `--port` flags)
- HTTP API surface (documented in server docs)
- Template structure for customizations

**Stability policy**:
- UI layout and UX can change rapidly
- HTTP API versioned separately
- Breaking changes documented in changelog

## Compatibility Strategy

### During Transition (Stage 1)

**For SDK-only users**:
```bash
pip install larql
# No change - UI not installed
```

**For UI users**:
```bash
pip install "larql[ui]"
# UI installed as optional extra
# larql.ui namespace still works
```

**For developers**:
```bash
uv sync --extra ui --group dev
# Both SDK and UI available
```

### After Namespace Migration (Stage 2)

**For SDK-only users**:
```bash
pip install larql
# No change
```

**For UI users**:
```bash
pip install "larql[ui]"  # Installs larql-ui as dependency
# or
pip install larql-ui    # Direct UI installation
```

**Migration guide**:
```python
# Old (deprecated, still works)
from larql.ui import app

# New (recommended)
from larql_ui.ui import app
```

## Implementation Checklist (P2)

- [ ] Update `pyproject.toml` to isolate UI dependencies
- [ ] Add deprecation notices for `larql.ui` namespace
- [ ] Document installation patterns in README
- [ ] Update AGENTS.md with split guidance
- [ ] Add migration notes to CHANGELOG
- [ ] Verify `pip install larql` works without UI deps
- [ ] Verify `pip install "larql[ui]"` includes UI deps
- [ ] Run existing tests to ensure backward compatibility

## Future Considerations (P3+)

- Evaluate if separate Rust crates needed (Option B)
- Consider separate PyPI packages (`larql` vs `larql-ui`)
- Define independent versioning policy for UI
- Establish separate CI/CD lanes for UI vs SDK
