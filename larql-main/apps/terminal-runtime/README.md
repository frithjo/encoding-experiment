# Terminal Runtime Component

This component owns the terminal browser runtime used by the LARQL workbench.

## Layout

- `bin/carbonyl` — repo-local wrapper used by `larql-terminal-browser` when present.
- `bin/carbonyl.real` — copied Carbonyl executable.
- `bin/libcarbonyl.so` + selected runtime assets — minimal shared runtime needed for terminal rendering.
- `scripts/install-carbonyl-runtime.sh` — installs/updates only the required runtime subset from an existing Carbonyl install.

## Install runtime into this repo

From `larql-main/`:

```bash
# Auto-download pinned version (recommended)
./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh
```

This downloads the pinned Carbonyl release (see `CARBONYL_VERSION` for version/SHA256) and installs the minimal runtime subset.

Or with explicit source path:

```bash
./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh /absolute/path/to/carbonyl
```

Or point to a Carbonyl runtime directory directly (recommended for deterministic cherry-pick):

```bash
./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh /home/arty/Documents/projects/carbonyl/opt/carbonyl-0.0.3
```

## Runtime resolution order

`larql-terminal-browser` resolves the terminal browser binary in this order:

1. `--carbonyl-bin`
2. `CARBONYL_BIN` environment variable
3. repo component runtime: `apps/terminal-runtime/bin/carbonyl`
4. `carbonyl` on `PATH`
