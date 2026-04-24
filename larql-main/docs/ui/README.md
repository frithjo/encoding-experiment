# UI Strategy

LARQL provides multiple interfaces to access the core analytic framework. The core value is the analytic capability—viewing transformer models as graph-like databases and providing deep profiling tools. The UIs are interfaces to access these capabilities, not the primary product focus.

## Core Principle

**UIs are interfaces to the analytic framework, not the framework itself.**

The LARQL framework (vindex + LQL) enables deep model analysis. Multiple UIs can coexist to serve different use cases and user preferences. All UIs access the same core analytic capabilities.

## Interface Approaches

### Terminal Browser (Primary Interface)

**Implementation:** `crates/larql-terminal-browser`
**Status:** Primary interface, full integration needed
**Technology:** Rust launcher delegating to Carbonyl (Chromium in terminal)

**Description:**
First-party terminal browser client that renders the workbench UI inside the terminal. Uses a real Chromium-class engine, supporting modern HTML/CSS/JS including images, video, and streaming media.

**Use Cases:**
- Interactive exploration in terminal environments
- Users who prefer terminal-first workflows
- Environments where desktop browsers are unavailable

**Installation:**
```bash
cargo run -p larql-terminal-browser -- http://127.0.0.1:8000
```

**Documentation:** `crates/larql-terminal-browser/README.md`, `docs/ui/terminal-browser-spec.md`

---

### Python Workbench

**Implementation:** `crates/larql-python` (larql-ui)
**Status:** Active, stable
**Technology:** Starlette web server with Jinja2 templates

**Description:**
Server-rendered HTML workbench for interactive exploration. Served via `larql-workbench` CLI entrypoint. Provides recipe-driven analysis workflows.

**Use Cases:**
- Interactive web-based exploration
- Recipe-driven analysis workflows
- Users comfortable with Python/web stack

**Installation:**
```bash
pip install "larql[ui]"
larql-workbench --host 127.0.0.1 --port 8000
```

**Documentation:** `crates/larql-python/README.md`, `docs/larql-python.md`

---

### Leptos Frontend

**Implementation:** `crates/larql-leptos`
**Status:** Active migration target from React
**Technology:** Leptos (Rust/WASM)

**Description:**
Rust/WASM frontend being developed as the migration target from the React-based chuk-kv-anatomist. Provides model-specific tools with larql-server backend.

**Use Cases:**
- Model-specific tool interfaces
- WASM-based web deployment
- Users preferring Rust-based frontend

**Migration Status:** Active. See `docs/leptos-migration-guide.md` for details.

**Documentation:** `crates/larql-leptos/README.md` (to be added)

---

### React UI (Deprecated)

**Implementation:** `apps/chuk-kv-anatomist`
**Status:** Deprecated, migrating to Leptos
**Technology:** Vite + React + TypeScript

**Description:**
Vite + React UI for inspecting KV/context maps against a Lazarus backend. Being migrated to Leptos with larql-server backend.

**Use Cases:** None (deprecated during migration)

**Migration Path:** See `docs/leptos-migration-guide.md` for migration status and timeline.

**Note:** This interface should not be used for new work. Use Leptos or other active interfaces.

---

## Interface Selection Guide

Choose the interface based on your use case:

| Use Case | Recommended Interface |
|----------|---------------------|
| Terminal-first workflow | Terminal Browser |
| Interactive web exploration | Python Workbench |
| Model-specific tools | Leptos (when migration complete) |
| Programmatic access | Python SDK (`pip install larql`) |
| Remote API access | CLI (`larql serve`) |

## Coexistence Strategy

Multiple UIs can coexist because they:
- Access the same core analytic framework (vindex + LQL)
- Use the same underlying Rust crates
- Provide different user experiences for different needs
- Can be developed independently

**No single UI is the "correct" one**—choose based on your workflow and environment.

## Canonical Flow for Scientific Analysis

All UIs must consume scientific analysis through the canonical flow:

```
larql-inference (structured analysis API)
    ↓
larql-lql (ANALYZE statements)
    ↓
larql-server (transport adapter)
    ↓
UI consumers (TUI, Workbench, etc.)
```

**UI Adapter Pattern:**
- UIs should not implement scientific analysis logic
- UIs should consume LQL ANALYZE statements or call server adapters
- Server tool routes are transport adapters, not analysis owners
- Scientific semantics live in core crates (larql-inference)
- TUI recipes compile to explicit LQL ANALYZE clauses

**Example:**
- `larql-terminal-batch-dla` should call LQL ANALYZE or server adapter
- `larql-python` workbench should execute LQL statements, not duplicate logic
- Server `batch_dla_scan` route is a thin adapter over larql-inference API

**Key Invariant:** One experiment engine, one language surface, one transport surface. No duplication of analysis logic across UIs.

## Development Priorities

1. **Terminal Browser:** Primary interface, needs full integration and polish
2. **Leptos:** Active migration target, complete migration from React
3. **Python Workbench:** Stable, maintain for web-based users
4. **React UI:** Deprecate after Leptos migration complete

## Related Documentation

- `VISION.md`: Core analytic framework vision
- `docs/analytic-capabilities.md`: Overview of available analyses
- `docs/ui/terminal-browser-spec.md`: Terminal browser implementation spec
- `docs/leptos-migration-guide.md`: Leptos migration details
- `docs/architecture/release-surface.md`: Release-facing surfaces
