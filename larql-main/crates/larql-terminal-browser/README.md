# `larql-terminal-browser`

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

**You do not need to install Carbonyl** (or any other third-party terminal browser) as a dependency of LARQL. External projects may be used only as **optional references** while this binary is built out.

## Implementation direction (honest scope)

- **Full** HTML/CSS/JS and **media** in the terminal implies a **real browser engine** (typically Chromium-class) with output adapted to the TTY—similar in *shape* to projects like Carbonyl, but **owned and shipped here**.
- A from-scratch HTML engine in Rust is **not** the default plan; embedding or driving an engine via Rust **is** consistent with “mostly Rust where we can.”
- Until that lands, `larql-terminal-browser` is a **stub** that documents the contract and fails fast; the workbench remains testable with `larql-ui` + any browser during development.

## Usage (when implemented)

```bash
cargo run -p larql-terminal-browser -- http://127.0.0.1:8000
```

## Related

- Workbench server: `crates/larql-python` (`larql-ui` entrypoint).
- Spec: `plan.md` (terminal viewport UX, media, routing, minimal JS).
