# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Python SDK/UI split: UI dependencies now optional via `pip install "larql[ui]"`
- HuggingFace resolution in Vindexfile: `FROM hf://user/repo` directives now download from HuggingFace
- Installation patterns documentation in README
- Migration guide for `larql.ui` namespace deprecation

### Changed
- Python package now supports SDK-only installation (`pip install larql`) without UI dependencies
- `larql.ui` namespace removed; use `larql_ui.ui` for UI imports
- `larql-workbench` CLI entrypoint (replaces deprecated `larql-ui`)
- Updated AGENTS.md with Python SDK/UI split guidance

### Migration Guide

#### Python Installation
```bash
# SDK only (minimal footprint)
pip install larql

# SDK + UI (interactive workbench)
pip install "larql[ui]"
```

#### Namespace Changes
```python
# Old (no longer works)
from larql.ui.app import create_app

# New (required)
from larql_ui.ui.app import create_app
```

#### CLI Changes
```bash
# Old (no longer works)
larql-ui --host 127.0.0.1 --port 8000

# New (required)
larql-workbench --host 127.0.0.1 --port 8000
```

See [crates/larql-python/README.md](crates/larql-python/README.md) for full installation patterns.
