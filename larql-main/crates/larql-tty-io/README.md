# larql-tty-io

TTY input decoding and terminal mode management for terminal-embedded UIs.

## Purpose

This crate provides engine-independent TTY primitives for terminal-embedded UIs:

- **Input decoding**: CSI/SS3 sequences, kitty keyboard protocol, mouse SGR, bracketed paste, focus events
- **Mode management**: TTY guard (alt-screen, cursor hide, termios raw mode, SIGWINCH)

Designed to be reusable across different rendering engines (Carbonyl, Servo, custom renderers, Leptos-in-TTY).

## Modules

### `input`

Decodes TTY input sequences into structured events:

- `Decoder`: Streaming decoder for CSI/SS3/kitty/mouse/paste/focus events
- `Event`: Decoded input events (key, mouse, paste, focus, resize)
- `Key`: Keyboard key representation
- `KeyEvent`: Key event with modifiers
- `KeyMods`: Key modifier flags (shift, alt, ctrl, super, hyper, meta)
- `MouseEvent`: Mouse button event with coordinates

Supported protocols:
- Standard xterm CSI sequences (arrows, F-keys, home/end, page up/down)
- Kitty keyboard protocol (CSI `u` form) - enabled by default via the `kitty` feature flag
- Mouse SGR protocol (CSI `<` / `M` / `m`)
- Bracketed paste (OSC 200;200p / 201;201p)
- Focus events (CSI `I` / `O`)

### `mode`

Terminal mode management:

- `is_tty()`: Non-destructive TTY check
- `TtyGuard`: RAII guard that enables terminal modes and restores them on drop

`TtyGuard` enables:
- Alternate screen (DECSET 1049)
- Cursor hide (DECTCEM)
- Mouse SGR reporting (DECSET 1006, 1003)
- Bracketed paste (DECSET 2004)
- Focus reporting (DECSET 1004)
- Raw mode (via termios)

## Usage

### Input Decoding

```rust
use larql_tty_io::input::Decoder;

let mut decoder = Decoder::new().with_kitty(true);
let events: Vec<_> = decoder.push(b"\x1b[A").collect();
assert_eq!(events[0], Event::Key(KeyEvent { key: Key::Up, mods: KeyMods::default() }));
```

### TTY Mode

```rust
use larql_tty_io::mode::{is_tty, TtyGuard};

if is_tty() {
    let guard = TtyGuard::new().unwrap();
    // Terminal modes are now active
    // Drop guard to restore terminal
}
```

## Caveats

- **TTY Guard is invasive**: `TtyGuard::new()` changes terminal modes (alt-screen, cursor hide, raw mode). Do not use when another process (e.g., Carbonyl) owns the TTY.
- **Linux-only**: SIGWINCH handling is currently Linux-only. macOS support requires additional work.
- **Paste mode incomplete**: Bracketed paste decoding is a placeholder in v1.
- **Kitty protocol variance**: Terminals may differ in kitty keyboard protocol edge cases. Feature-gated for safety.

## Platform Support

- Linux x86_64: Fully supported
- Linux aarch64: Supported (follow-up needed for artifacts)
- macOS: Partially supported (SIGWINCH needs work)
- Windows: Not supported

## Related Crates

- `larql-terminal-renderer`: Terminal graphics rendering (Sixel, Kitty, iTerm2, ANSI)
- `larql-terminal-browser`: Terminal browser launcher for LARQL workbench

## Future Work

Follow-up tickets filed in `docs/` for:
- aarch64 and macOS artifact support
- Damage tracker / dirty-rect scheduler
- DPR/zoom geometry helpers
- Servo evaluation for non-Carbonyl path
