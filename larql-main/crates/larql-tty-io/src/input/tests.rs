use super::*;

fn decode(bytes: &[u8]) -> Vec<Event> {
    let mut d = Decoder::new();
    d.push(bytes).collect()
}

fn decode_no_kitty(bytes: &[u8]) -> Vec<Event> {
    let mut d = Decoder::new().with_kitty(false);
    d.push(bytes).collect()
}

fn key(k: Key) -> Event {
    Event::Key(KeyEvent {
        key: k,
        mods: KeyMods::NONE,
    })
}

fn key_mods(k: Key, mods: KeyMods) -> Event {
    Event::Key(KeyEvent { key: k, mods })
}

// --- Printable + control ---------------------------------------------------

#[test]
fn printable_ascii_stream() {
    let evs = decode(b"abc");
    assert_eq!(evs.len(), 3);
    assert_eq!(evs[0], key(Key::Char('a')));
    assert_eq!(evs[2], key(Key::Char('c')));
}

#[test]
fn enter_cr() {
    assert_eq!(decode(b"\r"), vec![key(Key::Enter)]);
}

#[test]
fn enter_lf() {
    assert_eq!(decode(b"\n"), vec![key(Key::Enter)]);
}

#[test]
fn tab() {
    assert_eq!(decode(b"\t"), vec![key(Key::Tab)]);
}

#[test]
fn backspace_del() {
    assert_eq!(decode(&[0x7f]), vec![key(Key::Backspace)]);
}

#[test]
fn ctrl_a() {
    assert_eq!(
        decode(&[0x01]),
        vec![key_mods(
            Key::Char('a'),
            KeyMods {
                ctrl: true,
                ..KeyMods::NONE
            }
        )]
    );
}

#[test]
fn ctrl_z() {
    assert_eq!(
        decode(&[0x1a]),
        vec![key_mods(
            Key::Char('z'),
            KeyMods {
                ctrl: true,
                ..KeyMods::NONE
            }
        )]
    );
}

// --- CSI arrows ------------------------------------------------------------

#[test]
fn csi_up() {
    assert_eq!(decode(b"\x1b[A"), vec![key(Key::Up)]);
}

#[test]
fn csi_down() {
    assert_eq!(decode(b"\x1b[B"), vec![key(Key::Down)]);
}

#[test]
fn csi_right() {
    assert_eq!(decode(b"\x1b[C"), vec![key(Key::Right)]);
}

#[test]
fn csi_left() {
    assert_eq!(decode(b"\x1b[D"), vec![key(Key::Left)]);
}

#[test]
fn csi_home() {
    assert_eq!(decode(b"\x1b[H"), vec![key(Key::Home)]);
}

#[test]
fn csi_end() {
    assert_eq!(decode(b"\x1b[F"), vec![key(Key::End)]);
}

#[test]
fn csi_shift_up() {
    // CSI 1;2 A = Shift+Up
    assert_eq!(
        decode(b"\x1b[1;2A"),
        vec![key_mods(
            Key::Up,
            KeyMods {
                shift: true,
                ..KeyMods::NONE
            }
        )]
    );
}

#[test]
fn csi_ctrl_right() {
    // CSI 1;5 C = Ctrl+Right
    assert_eq!(
        decode(b"\x1b[1;5C"),
        vec![key_mods(
            Key::Right,
            KeyMods {
                ctrl: true,
                ..KeyMods::NONE
            }
        )]
    );
}

#[test]
fn csi_alt_ctrl_left() {
    // CSI 1;7 D = Alt+Ctrl+Left (bits = 6 -> alt|ctrl)
    assert_eq!(
        decode(b"\x1b[1;7D"),
        vec![key_mods(
            Key::Left,
            KeyMods {
                alt: true,
                ctrl: true,
                ..KeyMods::NONE
            }
        )]
    );
}

// --- CSI tilde-form keys ---------------------------------------------------

#[test]
fn csi_insert() {
    assert_eq!(decode(b"\x1b[2~"), vec![key(Key::Insert)]);
}

#[test]
fn csi_delete() {
    assert_eq!(decode(b"\x1b[3~"), vec![key(Key::Delete)]);
}

#[test]
fn csi_pageup() {
    assert_eq!(decode(b"\x1b[5~"), vec![key(Key::PageUp)]);
}

#[test]
fn csi_pagedown() {
    assert_eq!(decode(b"\x1b[6~"), vec![key(Key::PageDown)]);
}

#[test]
fn csi_f5() {
    assert_eq!(decode(b"\x1b[15~"), vec![key(Key::F(5))]);
}

#[test]
fn csi_f12() {
    assert_eq!(decode(b"\x1b[24~"), vec![key(Key::F(12))]);
}

#[test]
fn csi_f6_with_shift() {
    // CSI 17;2 ~ = Shift+F6
    assert_eq!(
        decode(b"\x1b[17;2~"),
        vec![key_mods(
            Key::F(6),
            KeyMods {
                shift: true,
                ..KeyMods::NONE
            }
        )]
    );
}

// --- SS3 / F1-F4 -----------------------------------------------------------

#[test]
fn ss3_f1() {
    assert_eq!(decode(b"\x1bOP"), vec![key(Key::F(1))]);
}

#[test]
fn ss3_f4() {
    assert_eq!(decode(b"\x1bOS"), vec![key(Key::F(4))]);
}

#[test]
fn ss3_up_application_mode() {
    assert_eq!(decode(b"\x1bOA"), vec![key(Key::Up)]);
}

#[test]
fn csi_f1_letter_form() {
    assert_eq!(decode(b"\x1b[P"), vec![key(Key::F(1))]);
}

// --- Focus / paste ---------------------------------------------------------

#[test]
fn focus_in() {
    assert_eq!(decode(b"\x1b[I"), vec![Event::FocusIn]);
}

#[test]
fn focus_out() {
    assert_eq!(decode(b"\x1b[O"), vec![Event::FocusOut]);
}

#[test]
fn paste_start() {
    assert_eq!(decode(b"\x1b[200~"), vec![Event::PasteStart]);
}

#[test]
fn paste_end() {
    assert_eq!(decode(b"\x1b[201~"), vec![Event::PasteEnd]);
}

// --- Mouse SGR -------------------------------------------------------------

#[test]
fn mouse_sgr_left_press() {
    let evs = decode(b"\x1b[<0;10;20M");
    assert_eq!(
        evs,
        vec![Event::Mouse(MouseEvent {
            button: MouseButton::Left,
            mods: KeyMods::NONE,
            x: 10,
            y: 20,
            press: true,
        })]
    );
}

#[test]
fn mouse_sgr_right_release() {
    let evs = decode(b"\x1b[<2;5;6m");
    assert_eq!(
        evs,
        vec![Event::Mouse(MouseEvent {
            button: MouseButton::Right,
            mods: KeyMods::NONE,
            x: 5,
            y: 6,
            press: false,
        })]
    );
}

#[test]
fn mouse_sgr_with_shift() {
    // code=4 => Left with shift bit (4)
    let evs = decode(b"\x1b[<4;1;1M");
    assert_eq!(
        evs,
        vec![Event::Mouse(MouseEvent {
            button: MouseButton::Left,
            mods: KeyMods {
                shift: true,
                ..KeyMods::NONE
            },
            x: 1,
            y: 1,
            press: true,
        })]
    );
}

#[test]
fn mouse_sgr_wheel_up() {
    // code 64 => wheel up
    let evs = decode(b"\x1b[<64;3;4M");
    assert_eq!(
        evs,
        vec![Event::Mouse(MouseEvent {
            button: MouseButton::WheelUp,
            mods: KeyMods::NONE,
            x: 3,
            y: 4,
            press: true,
        })]
    );
}

// --- Kitty CSI-u -----------------------------------------------------------

#[cfg(feature = "kitty")]
#[test]
fn kitty_ascii_a() {
    // CSI 97 u => 'a'
    assert_eq!(decode(b"\x1b[97u"), vec![key(Key::Char('a'))]);
}

#[cfg(feature = "kitty")]
#[test]
fn kitty_ctrl_a() {
    // CSI 97;5 u => Ctrl+a
    assert_eq!(
        decode(b"\x1b[97;5u"),
        vec![key_mods(
            Key::Char('a'),
            KeyMods {
                ctrl: true,
                ..KeyMods::NONE
            }
        )]
    );
}

#[cfg(feature = "kitty")]
#[test]
fn kitty_disabled_ignores_u() {
    // With kitty off, CSI 97 u should be ignored (no event).
    let evs = decode_no_kitty(b"\x1b[97u");
    assert!(evs.is_empty());
}

#[cfg(feature = "kitty")]
#[test]
fn kitty_named_f5() {
    // Kitty functional key F5 = 57364
    assert_eq!(decode(b"\x1b[57364u"), vec![key(Key::F(1))]);
}

// --- Alt-prefixed ----------------------------------------------------------

#[test]
fn alt_a() {
    assert_eq!(
        decode(b"\x1ba"),
        vec![key_mods(
            Key::Char('a'),
            KeyMods {
                alt: true,
                ..KeyMods::NONE
            }
        )]
    );
}

// --- Shift-Tab -------------------------------------------------------------

#[test]
fn shift_tab() {
    assert_eq!(
        decode(b"\x1b[Z"),
        vec![key_mods(
            Key::Tab,
            KeyMods {
                shift: true,
                ..KeyMods::NONE
            }
        )]
    );
}

// --- Chunked / split input -------------------------------------------------

#[test]
fn csi_split_across_pushes() {
    let mut d = Decoder::new();
    let first: Vec<_> = d.push(b"\x1b[").collect();
    assert!(first.is_empty());
    let second: Vec<_> = d.push(b"A").collect();
    assert_eq!(second, vec![key(Key::Up)]);
}

#[test]
fn mouse_split_across_pushes() {
    let mut d = Decoder::new();
    assert!(d.push(b"\x1b[<0;").next().is_none());
    assert!(d.push(b"10;20").next().is_none());
    let tail: Vec<_> = d.push(b"M").collect();
    assert_eq!(tail.len(), 1);
}

#[test]
fn multiple_events_one_push() {
    let evs = decode(b"a\x1b[Bb");
    assert_eq!(evs.len(), 3);
    assert_eq!(evs[0], key(Key::Char('a')));
    assert_eq!(evs[1], key(Key::Down));
    assert_eq!(evs[2], key(Key::Char('b')));
}

// --- Regression: OSC terminator must not leak ------------------------------

#[test]
fn osc_bel_terminated_consumed() {
    // OSC title set — terminated by BEL. Should produce no events and leave
    // subsequent input decodable.
    let evs = decode(b"\x1b]0;title\x07a");
    assert_eq!(evs, vec![key(Key::Char('a'))]);
}

#[test]
fn osc_st_terminated_consumed() {
    let evs = decode(b"\x1b]0;title\x1b\\a");
    assert_eq!(evs, vec![key(Key::Char('a'))]);
}
