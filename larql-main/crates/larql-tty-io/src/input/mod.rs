//! TTY input decoding for CSI/SS3/kitty keyboard protocol, mouse SGR, bracketed
//! paste, focus events.
//!
//! The decoder is a streaming byte-level state machine. The caller owns the fd
//! and pushes raw bytes; decoded [`Event`]s are yielded lazily.
//!
//! Recognised sequences:
//! - Standard xterm CSI: arrows `CSI A/B/C/D`, `CSI H/F` (home/end),
//!   `CSI P..S` (F1-F4), `CSI <n>~` (F1-F12, ins/del/pgup/pgdn), all with the
//!   `CSI 1;<mods><final>` modifier form.
//! - SS3 application cursor: `ESC O A/B/C/D/P/Q/R/S/H/F`.
//! - Kitty keyboard protocol (CSI-u): `CSI <code>[;<mods>[;<text>]]u` — gated
//!   by the `kitty` feature flag (default on).
//! - Mouse SGR: `CSI < <b>;<x>;<y> M|m` (press/release).
//! - Bracketed paste: `CSI 200~` / `CSI 201~`.
//! - Focus events: `CSI I` / `CSI O`.
//! - Control chars: `C-a..C-z`, Enter, Tab, Backspace, Esc.
//! - Alt-prefixed bytes: `ESC <byte>` produces the event for `<byte>` with
//!   `alt=true`.

use std::fmt;

/// A decoded input event from the TTY.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    PasteStart,
    PasteEnd,
    FocusIn,
    FocusOut,
    /// Terminal resize (new width, height in cells). Not emitted by the byte
    /// decoder; injected by the mode layer's SIGWINCH handler.
    Resize(u16, u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub mods: KeyMods,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Delete,
    Esc,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    F(u8),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyMods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    pub super_: bool,
    pub hyper: bool,
    pub meta: bool,
}

impl KeyMods {
    pub const NONE: Self = Self {
        shift: false,
        alt: false,
        ctrl: false,
        super_: false,
        hyper: false,
        meta: false,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    pub button: MouseButton,
    pub mods: KeyMods,
    pub x: u16,
    pub y: u16,
    /// True for press (`M`), false for release (`m`).
    pub press: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    /// Mouse motion or wheel (encoded via high bits of the SGR code).
    None,
    WheelUp,
    WheelDown,
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Char(c) => write!(f, "{}", c),
            Key::Enter => write!(f, "Enter"),
            Key::Tab => write!(f, "Tab"),
            Key::Backspace => write!(f, "Backspace"),
            Key::Delete => write!(f, "Delete"),
            Key::Esc => write!(f, "Esc"),
            Key::Up => write!(f, "Up"),
            Key::Down => write!(f, "Down"),
            Key::Left => write!(f, "Left"),
            Key::Right => write!(f, "Right"),
            Key::Home => write!(f, "Home"),
            Key::End => write!(f, "End"),
            Key::PageUp => write!(f, "PageUp"),
            Key::PageDown => write!(f, "PageDown"),
            Key::Insert => write!(f, "Insert"),
            Key::F(n) => write!(f, "F{}", n),
            Key::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Streaming decoder for TTY input.
pub struct Decoder {
    state: State,
    /// CSI / OSC parameter accumulator (bytes between CSI introducer and final byte).
    params: Vec<u8>,
    /// CSI private-mode prefix byte if present (e.g. `<` for SGR mouse, `?` for DEC).
    csi_prefix: Option<u8>,
    #[cfg(feature = "kitty")]
    kitty_enabled: bool,
    /// Pending event queue for cases where one push produces multiple events
    /// (rare, but the API supports it).
    pending: std::collections::VecDeque<Event>,
    /// Non-emitting carry buffer used by the driver loop.
    carry: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Ground,
    Escape,
    Csi,
    Ss3,
    Osc,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            params: Vec::new(),
            csi_prefix: None,
            #[cfg(feature = "kitty")]
            kitty_enabled: true,
            pending: std::collections::VecDeque::new(),
            carry: Vec::new(),
        }
    }

    /// Enable or disable kitty keyboard protocol (CSI-u) decoding. Default: on.
    /// Only available when the `kitty` feature is enabled.
    #[cfg(feature = "kitty")]
    pub fn with_kitty(mut self, enabled: bool) -> Self {
        self.kitty_enabled = enabled;
        self
    }

    /// No-op when kitty feature is disabled (always disabled at compile time).
    #[cfg(not(feature = "kitty"))]
    pub fn with_kitty(self, _enabled: bool) -> Self {
        self
    }

    /// Push bytes and return any decoded events.
    ///
    /// The iterator borrows `self` mutably; drain it before the next `push`.
    pub fn push<'a>(&'a mut self, bytes: &[u8]) -> impl Iterator<Item = Event> + 'a {
        self.carry.extend_from_slice(bytes);
        // Drive the state machine over the carried input.
        let input: Vec<u8> = std::mem::take(&mut self.carry);
        for &b in &input {
            self.step(b);
        }
        std::iter::from_fn(move || self.pending.pop_front())
    }

    fn emit(&mut self, ev: Event) {
        self.pending.push_back(ev);
    }

    fn step(&mut self, byte: u8) {
        match self.state {
            State::Ground => self.step_ground(byte),
            State::Escape => self.step_escape(byte),
            State::Csi => self.step_csi(byte),
            State::Ss3 => self.step_ss3(byte),
            State::Osc => self.step_osc(byte),
        }
    }

    fn step_ground(&mut self, byte: u8) {
        match byte {
            0x1b => {
                self.state = State::Escape;
            }
            0x0d => self.emit(Event::Key(KeyEvent {
                key: Key::Enter,
                mods: KeyMods::NONE,
            })),
            0x0a => self.emit(Event::Key(KeyEvent {
                key: Key::Enter,
                mods: KeyMods::NONE,
            })),
            0x09 => self.emit(Event::Key(KeyEvent {
                key: Key::Tab,
                mods: KeyMods::NONE,
            })),
            0x7f => self.emit(Event::Key(KeyEvent {
                key: Key::Backspace,
                mods: KeyMods::NONE,
            })),
            0x08 => self.emit(Event::Key(KeyEvent {
                key: Key::Backspace,
                mods: KeyMods {
                    ctrl: true,
                    ..KeyMods::NONE
                },
            })),
            0x01..=0x07 | 0x0b..=0x0c | 0x0e..=0x1a => {
                // Ctrl-A..Ctrl-Z (minus Enter/Tab/Bs handled above).
                let ch = (byte + 0x60) as char;
                self.emit(Event::Key(KeyEvent {
                    key: Key::Char(ch),
                    mods: KeyMods {
                        ctrl: true,
                        ..KeyMods::NONE
                    },
                }));
            }
            0x20..=0x7e => self.emit(Event::Key(KeyEvent {
                key: Key::Char(byte as char),
                mods: KeyMods::NONE,
            })),
            _ => { /* ignore 0x1c..0x1f, 0x80+ for now */ }
        }
    }

    fn step_escape(&mut self, byte: u8) {
        match byte {
            b'[' => {
                self.state = State::Csi;
                self.params.clear();
                self.csi_prefix = None;
            }
            b'O' => {
                self.state = State::Ss3;
            }
            b']' => {
                self.state = State::Osc;
                self.params.clear();
            }
            0x1b => {
                // ESC ESC — lone Esc followed by another ESC: emit an Esc key
                // and stay in Escape state for the second introducer.
                self.emit(Event::Key(KeyEvent {
                    key: Key::Esc,
                    mods: KeyMods::NONE,
                }));
                // remain in Escape
            }
            _ => {
                // ESC + <byte> => Alt-modified event for that byte.
                self.state = State::Ground;
                self.alt_emit(byte);
            }
        }
    }

    fn alt_emit(&mut self, byte: u8) {
        // Reuse ground decoding but mark alt=true.
        let before = self.pending.len();
        self.step_ground(byte);
        if let Some(ev) = self.pending.get_mut(before) {
            if let Event::Key(ke) = ev {
                ke.mods.alt = true;
            }
        }
    }

    fn step_csi(&mut self, byte: u8) {
        // Parameter prefix byte (private mode introducer) appears as first byte
        // after CSI. Common cases: '<' (SGR mouse), '?' (DEC private),
        // '>' (secondary DA), '=' (tertiary).
        if self.params.is_empty() && self.csi_prefix.is_none() && matches!(byte, b'<' | b'?' | b'>' | b'=') {
            self.csi_prefix = Some(byte);
            return;
        }
        // Final byte range 0x40..=0x7e
        if (0x40..=0x7e).contains(&byte) {
            self.finalize_csi(byte);
            self.state = State::Ground;
            self.params.clear();
            self.csi_prefix = None;
            return;
        }
        // Parameter / intermediate bytes.
        self.params.push(byte);
    }

    fn finalize_csi(&mut self, final_byte: u8) {
        let seq = std::str::from_utf8(&self.params).unwrap_or("");
        let nums: Vec<u32> = if seq.is_empty() {
            Vec::new()
        } else {
            seq.split(';').map(|s| s.parse().unwrap_or(0)).collect()
        };

        // Mouse SGR (CSI < b;x;y M|m)
        if self.csi_prefix == Some(b'<') && matches!(final_byte, b'M' | b'm') {
            if nums.len() >= 3 {
                let code = nums[0];
                let x = nums[1] as u16;
                let y = nums[2] as u16;
                let button = match code & 0b0100_0011 {
                    0 => MouseButton::Left,
                    1 => MouseButton::Middle,
                    2 => MouseButton::Right,
                    _ if (code & 64) != 0 && (code & 3) == 0 => MouseButton::WheelUp,
                    _ if (code & 64) != 0 && (code & 3) == 1 => MouseButton::WheelDown,
                    _ => MouseButton::None,
                };
                let mods = decode_mod_bits_sgr(code);
                self.emit(Event::Mouse(MouseEvent {
                    button,
                    mods,
                    x,
                    y,
                    press: final_byte == b'M',
                }));
            }
            return;
        }

        // Bracketed paste: CSI 200~ / 201~
        if final_byte == b'~' {
            let code = nums.first().copied().unwrap_or(0);
            if code == 200 {
                self.emit(Event::PasteStart);
                return;
            }
            if code == 201 {
                self.emit(Event::PasteEnd);
                return;
            }
            // Plain "CSI n~" / "CSI n;mods~" key events.
            let mods = decode_mod_bits(nums.get(1).copied().unwrap_or(1));
            let key = match code {
                1 | 7 => Key::Home,
                2 => Key::Insert,
                3 => Key::Delete,
                4 | 8 => Key::End,
                5 => Key::PageUp,
                6 => Key::PageDown,
                11..=15 => Key::F((code - 10) as u8),
                // Skip 16 (xterm reserved).
                17..=21 => Key::F((code - 11) as u8),
                23..=24 => Key::F((code - 12) as u8),
                _ => Key::Unknown,
            };
            self.emit(Event::Key(KeyEvent { key, mods }));
            return;
        }

        // Kitty keyboard protocol: CSI <code>[;<mods>...]u
        #[cfg(feature = "kitty")]
        if final_byte == b'u' && self.kitty_enabled {
            if let Some(&code) = nums.first() {
                let mods = decode_mod_bits(nums.get(1).copied().unwrap_or(1));
                let key = kitty_code_to_key(code);
                self.emit(Event::Key(KeyEvent { key, mods }));
            }
            return;
        }

        #[cfg(not(feature = "kitty"))]
        if final_byte == b'u' {
            // Ignore CSI-u when kitty feature is disabled
            return;
        }

        // Modifier-form cursor/F-keys: CSI 1;<mods><final>
        let mods = if nums.len() >= 2 && nums[0] == 1 {
            decode_mod_bits(nums[1])
        } else {
            KeyMods::NONE
        };
        let key = match final_byte {
            b'A' => Key::Up,
            b'B' => Key::Down,
            b'C' => Key::Right,
            b'D' => Key::Left,
            b'H' => Key::Home,
            b'F' => Key::End,
            b'P' => Key::F(1),
            b'Q' => Key::F(2),
            b'R' => Key::F(3),
            b'S' => Key::F(4),
            b'Z' => {
                // Shift-Tab
                self.emit(Event::Key(KeyEvent {
                    key: Key::Tab,
                    mods: KeyMods {
                        shift: true,
                        ..KeyMods::NONE
                    },
                }));
                return;
            }
            b'I' => {
                self.emit(Event::FocusIn);
                return;
            }
            b'O' => {
                self.emit(Event::FocusOut);
                return;
            }
            _ => Key::Unknown,
        };
        if matches!(key, Key::Unknown) {
            return;
        }
        self.emit(Event::Key(KeyEvent { key, mods }));
    }

    fn step_ss3(&mut self, byte: u8) {
        self.state = State::Ground;
        let key = match byte {
            b'A' => Key::Up,
            b'B' => Key::Down,
            b'C' => Key::Right,
            b'D' => Key::Left,
            b'H' => Key::Home,
            b'F' => Key::End,
            b'P' => Key::F(1),
            b'Q' => Key::F(2),
            b'R' => Key::F(3),
            b'S' => Key::F(4),
            _ => return,
        };
        self.emit(Event::Key(KeyEvent {
            key,
            mods: KeyMods::NONE,
        }));
    }

    fn step_osc(&mut self, byte: u8) {
        // Terminated by BEL (0x07) or ST (ESC \). We accept both; the ESC in ST
        // is swallowed when the next byte is '\\'.
        if byte == 0x07 {
            self.state = State::Ground;
            self.params.clear();
            return;
        }
        if byte == b'\\' && self.params.last() == Some(&0x1b) {
            self.params.pop();
            self.state = State::Ground;
            self.params.clear();
            return;
        }
        self.params.push(byte);
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Decode an xterm-style numeric modifier parameter. The on-wire value is
/// `1 + bitfield`, where `shift=1, alt=2, ctrl=4, super=8, hyper=16, meta=32`.
fn decode_mod_bits(n: u32) -> KeyMods {
    if n == 0 {
        return KeyMods::NONE;
    }
    let bits = n.saturating_sub(1);
    KeyMods {
        shift: (bits & 1) != 0,
        alt: (bits & 2) != 0,
        ctrl: (bits & 4) != 0,
        super_: (bits & 8) != 0,
        hyper: (bits & 16) != 0,
        meta: (bits & 32) != 0,
    }
}

/// SGR mouse encodes modifiers in the button byte at bits 2/3/4.
fn decode_mod_bits_sgr(code: u32) -> KeyMods {
    KeyMods {
        shift: (code & 4) != 0,
        alt: (code & 8) != 0,
        ctrl: (code & 16) != 0,
        ..KeyMods::NONE
    }
}

#[cfg(feature = "kitty")]
fn kitty_code_to_key(code: u32) -> Key {
    // Kitty functional key codes (subset): 57344+ reserved for named keys.
    // We cover a practical subset for v1.
    match code {
        13 => Key::Enter,
        9 => Key::Tab,
        127 => Key::Backspace,
        27 => Key::Esc,
        57361 => Key::Delete,
        57359 => Key::Insert,
        57356 => Key::Home,
        57357 => Key::End,
        57358 => Key::PageUp,
        57360 => Key::PageDown,
        57352 => Key::Up,
        57353 => Key::Down,
        57354 => Key::Left,
        57355 => Key::Right,
        57364..=57375 => Key::F((code - 57363) as u8),
        // Printable unicode codepoint.
        c if (0x20..=0x10_FFFF).contains(&c) => {
            char::from_u32(c).map(Key::Char).unwrap_or(Key::Unknown)
        }
        _ => Key::Unknown,
    }
}

#[cfg(test)]
mod tests;
