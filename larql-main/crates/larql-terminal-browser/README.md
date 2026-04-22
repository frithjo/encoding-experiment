# `larql-terminal-browser`

## Crate Role

- Role: User-facing terminal UX interface for the workbench
- Zone: interface
- Release impact: medium
- Stability target: iterative

**First-party terminal browser** for the LARQL workbench: a Rust-first binary that will render the workbench UI (`larql-ui`, server-rendered HTML) **inside the terminal**.

## Display port and rendering goals

- **Resizable, generous viewport:** The embedding surface (terminal window, etc.) must be **user-resizable**. Defaults should be **generous** when the environment allows; users shrink or expand to match task and screen real estate—there is no single fixed “tiny grid only” target.
- **Resolution:** **High logical resolution** is a priority (sharp text, crisp images/video).
- **GPU acceleration:** **Allowed and expected** when hardware and drivers support it (faster compositing, hardware video decode, smoother high-DPI). Use it by default where Chromium (or the chosen engine) would.
- **CPU / software fallback:** Remains important for environments **without** a usable GPU—software rendering and decode should still work.
- **Lightweight:** Minimize idle cost, memory, and dependency sprawl while meeting fidelity—see `plan.md` for the full UX contract.

## Media: images, video, streaming

The client must support **images**, **video**, and **streaming** playback in the terminal (whatever the embedded engine exposes: HTML5 `<video>`, **MSE**, progressive and adaptive streams where available). The workbench HTML may stay sparse in v1, but the **browser client** is not a text-only or no-media shell.

## Why this crate exists

The workbench is a normal local web app. The **normative** way to use it is **not** a full-size desktop window, but a **browser running in the TTY** (or equivalent embedding). That client should live **in this repo** and be **mostly Rust** (process glue, TTY I/O, keyboard routing, and embedding or driving a real engine for HTML/CSS/JS).

The current implementation uses Carbonyl as the terminal Chromium runtime and launches it from this Rust binary.

## Implementation status

- `larql-terminal-browser` is now a Rust launcher that delegates to Carbonyl for rendering.
- URL invocation is: `carbonyl [display flags] <url>`.
- Defaults:
  - fullscreen enabled
  - navigation UI hidden
  - `CARBONYL_ENV_FULLSCREEN=1` exported when fullscreen is enabled
- User display options:
  - `--no-fullscreen`
  - `--show-ui`
  - repeated `--carbonyl-arg ...` passthrough for runtime-specific flags (e.g. zoom)
- Binary resolution order:
  1. `--carbonyl-bin /path/to/carbonyl`
  2. `CARBONYL_BIN=/path/to/carbonyl`
  3. `apps/terminal-runtime/bin/carbonyl` (repo component runtime)
  4. `carbonyl` on `PATH`

## Usage

```bash
cargo run -p larql-terminal-browser -- http://127.0.0.1:8000
```

Explicit binary path:

```bash
cargo run -p larql-terminal-browser -- --carbonyl-bin /home/arty/.opencode/bin/carbonyl http://127.0.0.1:8000
```

Install repo component runtime:

```bash
./apps/terminal-runtime/scripts/install-carbonyl-runtime.sh
```

## Related

- Workbench server: `crates/larql-python` (`larql-ui` entrypoint).
- Spec: `plan.md` (terminal viewport UX, media, routing, minimal JS).

## Public vs Internal Surface

- Public: terminal-browser user experience contract (rendering, input, media,
  and viewport behavior) for release-facing workbench usage.
- Internal: embedding strategy and engine integration details may evolve while
  preserving that contract.
