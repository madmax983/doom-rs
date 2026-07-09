//! winit → crossterm keyboard adapter.
//!
//! doom-tui's [`InputState`](doom_tui::InputState) is typed on crossterm's
//! `KeyCode`/`KeyModifiers`. The windowed host receives `winit` keyboard events
//! instead, so this module translates them, **mirroring exactly** what the
//! terminal event loop (`DoomEventLoop::poll_events`) feeds `InputState`:
//!
//! * Arrows + WASD movement, `Ctrl` = fire (via modifier sync), `Space`/`E` =
//!   use, `Shift` = run/strafe (via modifier sync), `Esc` = menu, weapon number
//!   keys `1`-`7`.
//! * Edge-triggered menu navigation: `Up`/`Down`/`Enter`.
//! * `Tab` automap, `F5`/`F9` quick save/load, `.` wait, and console characters.
//!
//! The core [`winit_key_to_crossterm`] mapping is a pure function so it can be
//! unit-tested without opening a window.

use crossterm::event::KeyCode;
use winit::keyboard::{Key, NamedKey};

/// Translate a winit **logical** key into the crossterm [`KeyCode`] that
/// [`InputState`](doom_tui::InputState) understands.
///
/// Returns `None` for keys with no doom-rs meaning (including bare modifier
/// keys, which are handled separately via `ModifiersChanged`).
#[must_use]
pub fn winit_key_to_crossterm(key: &Key) -> Option<KeyCode> {
    match key {
        Key::Named(named) => named_key_to_crossterm(*named),
        // Character keys (letters, digits, punctuation) pass through as-is; case
        // is preserved to match crossterm (InputState lowercases held keys
        // internally, and console/cheat input wants the raw character).
        Key::Character(s) => s.chars().next().map(KeyCode::Char),
        _ => None,
    }
}

/// Map winit's [`NamedKey`] variants doom-rs cares about to crossterm codes.
#[must_use]
fn named_key_to_crossterm(named: NamedKey) -> Option<KeyCode> {
    Some(match named {
        NamedKey::ArrowUp => KeyCode::Up,
        NamedKey::ArrowDown => KeyCode::Down,
        NamedKey::ArrowLeft => KeyCode::Left,
        NamedKey::ArrowRight => KeyCode::Right,
        NamedKey::Space => KeyCode::Char(' '),
        NamedKey::Enter => KeyCode::Enter,
        NamedKey::Escape => KeyCode::Esc,
        NamedKey::Tab => KeyCode::Tab,
        NamedKey::Backspace => KeyCode::Backspace,
        NamedKey::F1 => KeyCode::F(1),
        NamedKey::F2 => KeyCode::F(2),
        NamedKey::F3 => KeyCode::F(3),
        NamedKey::F4 => KeyCode::F(4),
        NamedKey::F5 => KeyCode::F(5),
        NamedKey::F6 => KeyCode::F(6),
        NamedKey::F7 => KeyCode::F(7),
        NamedKey::F8 => KeyCode::F(8),
        NamedKey::F9 => KeyCode::F(9),
        NamedKey::F10 => KeyCode::F(10),
        NamedKey::F11 => KeyCode::F(11),
        NamedKey::F12 => KeyCode::F(12),
        _ => return None,
    })
}

/// Apply a key-**press** to `input`, mirroring `DoomEventLoop::poll_events`'
/// `KeyEventKind::Press` arm: edge-triggered signals first, then the held-key
/// insertion via `key_down`.
///
/// (`F2` renderer cycling and `q` quit are host-level terminal concerns and are
/// intentionally handled by the caller, not here.)
pub fn apply_press(input: &mut doom_tui::InputState, code: KeyCode) {
    if code == KeyCode::Esc {
        input.push_escape();
    }
    if code == KeyCode::F(5) {
        input.push_f5();
    } else if code == KeyCode::F(9) {
        input.push_f9();
    } else if code == KeyCode::Tab {
        input.push_tab();
    }
    if code == KeyCode::Up {
        input.push_menu_up();
    } else if code == KeyCode::Down {
        input.push_menu_down();
    } else if code == KeyCode::Enter {
        input.push_menu_select();
    }
    if let KeyCode::Char(ch) = code {
        input.push_console_char(ch);
        if ch == '.' {
            input.push_wait();
        }
    } else if code == KeyCode::Enter {
        input.push_console_char('\n');
    } else if code == KeyCode::Backspace {
        input.push_console_char('\x08');
    }
    input.key_down(code);
}

/// Apply a key-**release** to `input` (mirrors the `KeyEventKind::Release` arm).
pub fn apply_release(input: &mut doom_tui::InputState, code: KeyCode) {
    input.key_up(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::SmolStr;

    fn ch(c: char) -> Key {
        Key::Character(SmolStr::new(c.to_string()))
    }

    #[test]
    fn arrows_map_to_crossterm_arrows() {
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::ArrowUp)),
            Some(KeyCode::Up)
        );
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::ArrowDown)),
            Some(KeyCode::Down)
        );
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::ArrowLeft)),
            Some(KeyCode::Left)
        );
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::ArrowRight)),
            Some(KeyCode::Right)
        );
    }

    #[test]
    fn space_and_control_keys_map() {
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::Space)),
            Some(KeyCode::Char(' '))
        );
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::Enter)),
            Some(KeyCode::Enter)
        );
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::Escape)),
            Some(KeyCode::Esc)
        );
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::Tab)),
            Some(KeyCode::Tab)
        );
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::Backspace)),
            Some(KeyCode::Backspace)
        );
    }

    #[test]
    fn wasd_and_weapon_digits_map_to_chars() {
        for c in ['w', 'a', 's', 'd', 'e'] {
            assert_eq!(winit_key_to_crossterm(&ch(c)), Some(KeyCode::Char(c)));
        }
        for c in ['1', '2', '3', '4', '5', '6', '7'] {
            assert_eq!(winit_key_to_crossterm(&ch(c)), Some(KeyCode::Char(c)));
        }
    }

    #[test]
    fn function_keys_map() {
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::F5)),
            Some(KeyCode::F(5))
        );
        assert_eq!(
            winit_key_to_crossterm(&Key::Named(NamedKey::F9)),
            Some(KeyCode::F(9))
        );
    }

    #[test]
    fn unmapped_named_keys_return_none() {
        // Bare modifiers have no KeyCode mapping (handled via ModifiersChanged).
        assert_eq!(winit_key_to_crossterm(&Key::Named(NamedKey::Control)), None);
        assert_eq!(winit_key_to_crossterm(&Key::Named(NamedKey::Shift)), None);
        assert_eq!(winit_key_to_crossterm(&Key::Named(NamedKey::Alt)), None);
    }

    #[test]
    fn press_forward_sets_forward_move() {
        // End-to-end through InputState: W press → forward movement.
        let mut input = doom_tui::InputState::new();
        apply_press(&mut input, winit_key_to_crossterm(&ch('w')).unwrap());
        assert!(input.to_tic_input().forward_move > 0);
    }

    #[test]
    fn release_clears_held_key() {
        let mut input = doom_tui::InputState::new();
        let code = winit_key_to_crossterm(&ch('w')).unwrap();
        apply_press(&mut input, code);
        apply_release(&mut input, code);
        assert_eq!(input.to_tic_input().forward_move, 0);
    }

    #[test]
    fn escape_is_edge_triggered() {
        let mut input = doom_tui::InputState::new();
        apply_press(
            &mut input,
            winit_key_to_crossterm(&Key::Named(NamedKey::Escape)).unwrap(),
        );
        assert!(input.to_tic_input().escape_pressed);
    }
}
