//! TTY input decoding and terminal mode management.
//!
//! This crate provides:
//! - [`input`] module: CSI/SS3/kitty keyboard protocol, mouse SGR, bracketed paste, focus events
//! - [`mode`] module: TTY guard (alt-screen, cursor hide, termios raw mode, SIGWINCH)
//!
//! Designed for terminal-embedded UIs (e.g., terminal browsers, TUI applications).

pub mod input;
pub mod mode;

// Re-export commonly-used items for convenience
pub use mode::{is_tty, query_graphics_capabilities, GraphicsCapabilities, TtyGuard};
