//! Keyboard input state tracker and TicCmd synthesizer.
//!
//! Maintains a set of currently-held keys and, at the end of each tic,
//! produces a `TicInput` struct with movement deltas and button flags.
//!
//! # Input mapping (from the plan)
//! | Key          | Effect                          |
//! |--------------|----------------------------------|
//! | W / ↑        | forward_move += MOVE_SPEED       |
//! | S / ↓        | forward_move -= MOVE_SPEED       |
//! | A            | turn_left (or strafe with Shift) |
//! | D            | turn_right (or strafe with Shift)|
//! | ← / →       | angle_turn                       |
//! | Ctrl         | attack                           |
//! | Space / E    | use                              |
//! | 1-7          | weapon change                    |

use crossterm::event::KeyCode;
use std::collections::HashSet;

/// Movement speed per tic when a walk key is held.
pub const MOVE_SPEED: i8 = 50;
/// Turn amount per tic when a turn key is held.
pub const TURN_SPEED: i16 = 640; // roughly 1/50 of a full circle in BAM >> 16

/// Button flags.
pub mod buttons {
    pub const BT_ATTACK:    u8 = 0x01;
    pub const BT_USE:       u8 = 0x02;
    pub const BT_CHANGE:    u8 = 0x04;
    pub const BT_WEAPONMASK: u8 = 0x38; // bits 3-5 = weapon number
}

/// A single tic's worth of player input — wire-compatible with the plan's `TicCmd`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TicInput {
    /// Forward/backward movement (-128..127, positive = forward).
    pub forward_move: i8,
    /// Lateral movement (-128..127, positive = right).
    pub side_move: i8,
    /// Angle delta per tic (16-bit BAM units).
    pub angle_turn: i16,
    /// Button bitfield.
    pub buttons: u8,
    /// ASCII chatchar (0 = none).
    pub chatchar: u8,
    /// Raw character keypress for console/cheat input (`None` if no printable
    /// key was pressed this tic, or if multiple keys arrived — first wins).
    pub console_char: Option<char>,
    /// F5 was pressed this tic — quick save.
    pub f5_save: bool,
    /// F9 was pressed this tic — quick load.
    pub f9_load: bool,
    /// Tab was pressed this tic — toggle automap.
    pub tab_pressed: bool,
}

/// Tracks which keys are currently held and produces `TicInput` on demand.
#[derive(Default)]
pub struct InputState {
    held: HashSet<KeyCode>,
    shift_held: bool,
    /// Pending raw character press to forward to the console/cheat system.
    /// Set by `push_console_char`; consumed (and cleared) by `to_tic_input`.
    pending_console_char: Option<char>,
    /// F5 was pressed since the last tic — quick save trigger.
    pending_f5: bool,
    /// F9 was pressed since the last tic — quick load trigger.
    pending_f9: bool,
    /// Tab was pressed since the last tic — automap toggle trigger.
    pending_tab: bool,
}

impl InputState {
    /// Create empty input state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a key-down event.
    pub fn key_down(&mut self, key: KeyCode) {
        if matches!(key, KeyCode::Modifier(_)) {
            return; // modifiers tracked via `shift_held`
        }
        if key == KeyCode::Char('s') {
            // crossterm reports Shift+char as uppercase
        }
        self.held.insert(key);
    }

    /// Record a key-up event.
    pub fn key_up(&mut self, key: KeyCode) {
        self.held.remove(&key);
    }

    /// Signal shift state (crossterm delivers this separately).
    pub fn set_shift(&mut self, down: bool) {
        self.shift_held = down;
    }

    /// Returns `true` if the key is currently held.
    pub fn is_held(&self, key: KeyCode) -> bool {
        self.held.contains(&key)
    }

    /// Queue a raw character for console/cheat delivery.
    ///
    /// Only the first character queued between two consecutive tics is
    /// delivered (first-wins). This keeps the console responsive without
    /// requiring a full event queue.
    pub fn push_console_char(&mut self, ch: char) {
        if self.pending_console_char.is_none() {
            self.pending_console_char = Some(ch);
        }
    }

    /// Signal that F5 (quick save) was pressed.
    pub fn push_f5(&mut self) {
        self.pending_f5 = true;
    }

    /// Signal that F9 (quick load) was pressed.
    pub fn push_f9(&mut self) {
        self.pending_f9 = true;
    }

    /// Signal that Tab (automap toggle) was pressed.
    pub fn push_tab(&mut self) {
        self.pending_tab = true;
    }

    /// Synthesize a `TicInput` from the current held state.
    ///
    /// This method takes `&mut self` so it can consume the pending
    /// `console_char` (it is cleared after being placed in the returned
    /// `TicInput`).
    pub fn to_tic_input(&mut self) -> TicInput {
        let mut t = TicInput::default();

        // Forward / backward
        if self.is_held(KeyCode::Char('w')) || self.is_held(KeyCode::Up) {
            t.forward_move = t.forward_move.saturating_add(MOVE_SPEED);
        }
        if self.is_held(KeyCode::Char('s')) || self.is_held(KeyCode::Down) {
            t.forward_move = t.forward_move.saturating_sub(MOVE_SPEED);
        }

        if self.shift_held {
            // Shift held → A/D strafe
            if self.is_held(KeyCode::Char('a')) {
                t.side_move = t.side_move.saturating_sub(MOVE_SPEED);
            }
            if self.is_held(KeyCode::Char('d')) {
                t.side_move = t.side_move.saturating_add(MOVE_SPEED);
            }
        } else {
            // No shift → A/D turn
            if self.is_held(KeyCode::Char('a')) || self.is_held(KeyCode::Left) {
                t.angle_turn = t.angle_turn.saturating_add(TURN_SPEED);
            }
            if self.is_held(KeyCode::Char('d')) || self.is_held(KeyCode::Right) {
                t.angle_turn = t.angle_turn.saturating_sub(TURN_SPEED);
            }
        }

        // Attack
        if self.is_held(KeyCode::Char('\n'))    // Ctrl is tricky in crossterm
            || self.is_held(KeyCode::Char('f')) {
            t.buttons |= buttons::BT_ATTACK;
        }

        // Use
        if self.is_held(KeyCode::Char(' ')) || self.is_held(KeyCode::Char('e')) {
            t.buttons |= buttons::BT_USE;
        }

        // Weapon change: keys 1-7
        for (key, weapon) in [
            (KeyCode::Char('1'), 0u8),
            (KeyCode::Char('2'), 1),
            (KeyCode::Char('3'), 2),
            (KeyCode::Char('4'), 3),
            (KeyCode::Char('5'), 4),
            (KeyCode::Char('6'), 5),
            (KeyCode::Char('7'), 6),
        ] {
            if self.is_held(key) {
                t.buttons |= buttons::BT_CHANGE;
                t.buttons |= (weapon << 3) & buttons::BT_WEAPONMASK;
            }
        }

        // Consume pending console char (first-wins, cleared each tic).
        t.console_char = self.pending_console_char.take();

        // Consume pending F-key presses (cleared each tic).
        t.f5_save = std::mem::take(&mut self.pending_f5);
        t.f9_load = std::mem::take(&mut self.pending_f9);

        // Consume pending Tab press (cleared each tic).
        t.tab_pressed = std::mem::take(&mut self.pending_tab);

        t
    }

    /// Clear all held keys (e.g. when the window loses focus).
    pub fn clear(&mut self) {
        self.held.clear();
        self.shift_held = false;
        self.pending_console_char = None;
        self.pending_f5 = false;
        self.pending_f9 = false;
        self.pending_tab = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forward_key_sets_forward_move() {
        let mut s = InputState::new();
        s.key_down(KeyCode::Char('w'));
        assert_eq!(s.to_tic_input().forward_move, MOVE_SPEED);
    }

    #[test]
    fn opposite_keys_cancel() {
        let mut s = InputState::new();
        s.key_down(KeyCode::Char('w'));
        s.key_down(KeyCode::Char('s'));
        assert_eq!(s.to_tic_input().forward_move, 0);
    }

    #[test]
    fn turn_left_without_shift() {
        let mut s = InputState::new();
        s.key_down(KeyCode::Char('a'));
        let t = s.to_tic_input();
        assert!(t.angle_turn > 0);
        assert_eq!(t.side_move, 0);
    }

    #[test]
    fn strafe_with_shift() {
        let mut s = InputState::new();
        s.set_shift(true);
        s.key_down(KeyCode::Char('a'));
        let t = s.to_tic_input();
        assert_eq!(t.angle_turn, 0);
        assert!(t.side_move < 0);
    }

    #[test]
    fn use_button_on_space() {
        let mut s = InputState::new();
        s.key_down(KeyCode::Char(' '));
        assert_ne!(s.to_tic_input().buttons & buttons::BT_USE, 0);
    }

    #[test]
    fn key_up_clears_key() {
        let mut s = InputState::new();
        s.key_down(KeyCode::Char('w'));
        s.key_up(KeyCode::Char('w'));
        assert_eq!(s.to_tic_input().forward_move, 0);
    }

    #[test]
    fn no_keys_produces_zero_input() {
        let mut s = InputState::new();
        assert_eq!(s.to_tic_input(), TicInput::default());
    }

    #[test]
    fn tab_pressed_in_tic_input() {
        let mut s = InputState::new();
        s.push_tab();
        // First to_tic_input() must deliver tab_pressed = true.
        assert!(s.to_tic_input().tab_pressed, "tab_pressed must be true after push_tab");
        // Second call must return false — it is consumed (one-shot).
        assert!(!s.to_tic_input().tab_pressed, "tab_pressed must be false after being consumed");
    }
}
