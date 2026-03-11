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

use crossterm::event::{KeyCode, ModifierKeyCode};
use std::collections::HashSet;

/// Movement speed per tic when a walk key is held.
pub const MOVE_SPEED: i8 = 50;
/// Turn amount per tic when a turn key is held.
pub const TURN_SPEED: i16 = 640; // roughly 1/50 of a full circle in BAM >> 16

/// Button flags.
pub mod buttons {
    pub const BT_ATTACK: u8 = 0x01;
    pub const BT_USE: u8 = 0x02;
    pub const BT_CHANGE: u8 = 0x04;
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
    /// Escape was pressed this tic — edge-triggered (open/close menu, etc.).
    pub escape_pressed: bool,
    /// Up arrow/W pressed this tic — edge-triggered for menu navigation.
    pub menu_up: bool,
    /// Down arrow/S pressed this tic — edge-triggered for menu navigation.
    pub menu_down: bool,
    /// Enter pressed this tic — edge-triggered for menu selection.
    pub menu_select: bool,
}

/// Tracks which keys are currently held and produces `TicInput` on demand.
#[derive(Default)]
pub struct InputState {
    held: HashSet<KeyCode>,
    shift_held: bool,
    control_held: bool,
    /// Pending raw character press to forward to the console/cheat system.
    /// Set by `push_console_char`; consumed (and cleared) by `to_tic_input`.
    pending_console_char: Option<char>,
    /// F5 was pressed since the last tic — quick save trigger.
    pending_f5: bool,
    /// F9 was pressed since the last tic — quick load trigger.
    pending_f9: bool,
    /// Tab was pressed since the last tic — automap toggle trigger.
    pending_tab: bool,
    /// Escape was pressed since the last tic — menu/console edge trigger.
    pending_escape: bool,
    /// Up arrow pressed this tic — edge-triggered menu navigation.
    pending_menu_up: bool,
    /// Down arrow pressed this tic — edge-triggered menu navigation.
    pending_menu_down: bool,
    /// Enter pressed this tic — edge-triggered menu select.
    pending_menu_select: bool,
}

impl InputState {
    fn normalize_key(key: KeyCode) -> KeyCode {
        match key {
            KeyCode::Char(ch) => KeyCode::Char(ch.to_ascii_lowercase()),
            other => other,
        }
    }

    /// Create empty input state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a key-down event.
    pub fn key_down(&mut self, key: KeyCode) {
        match key {
            KeyCode::Modifier(ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift) => {
                self.shift_held = true;
                return;
            }
            KeyCode::Modifier(ModifierKeyCode::LeftControl | ModifierKeyCode::RightControl) => {
                self.control_held = true;
                return;
            }
            KeyCode::Modifier(_) => {
                return;
            }
            _ => {}
        }
        self.held.insert(Self::normalize_key(key));
    }

    /// Record a key-up event.
    pub fn key_up(&mut self, key: KeyCode) {
        match key {
            KeyCode::Modifier(ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift) => {
                self.shift_held = false;
            }
            KeyCode::Modifier(ModifierKeyCode::LeftControl | ModifierKeyCode::RightControl) => {
                self.control_held = false;
            }
            KeyCode::Modifier(_) => {}
            _ => {
                self.held.remove(&Self::normalize_key(key));
            }
        }
    }

    /// Signal shift state (crossterm delivers this separately).
    pub fn set_shift(&mut self, down: bool) {
        self.shift_held = down;
    }

    /// Signal control state (crossterm also exposes this via modifiers).
    pub fn set_control(&mut self, down: bool) {
        self.control_held = down;
    }

    /// Synchronize modifier state from a platform-level snapshot.
    ///
    /// `None` leaves the existing state unchanged so callers can refresh only
    /// the modifiers they can observe on the current platform.
    pub fn sync_modifiers(&mut self, shift: Option<bool>, control: Option<bool>) {
        if let Some(down) = shift {
            self.shift_held = down;
        }
        if let Some(down) = control {
            self.control_held = down;
        }
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

    /// Signal that Escape was pressed (menu toggle, console close, etc.).
    pub fn push_escape(&mut self) {
        self.pending_escape = true;
    }

    /// Signal that the Up arrow was pressed (edge-triggered menu navigation).
    pub fn push_menu_up(&mut self) {
        self.pending_menu_up = true;
    }

    /// Signal that the Down arrow was pressed (edge-triggered menu navigation).
    pub fn push_menu_down(&mut self) {
        self.pending_menu_down = true;
    }

    /// Signal that Enter was pressed (edge-triggered menu select).
    pub fn push_menu_select(&mut self) {
        self.pending_menu_select = true;
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
            if self.is_held(KeyCode::Char('a')) {
                t.angle_turn = t.angle_turn.saturating_add(TURN_SPEED);
            }
            if self.is_held(KeyCode::Char('d')) {
                t.angle_turn = t.angle_turn.saturating_sub(TURN_SPEED);
            }
        }

        // Arrow keys always turn, even while Shift is held for A/D strafing.
        if self.is_held(KeyCode::Left) {
            t.angle_turn = t.angle_turn.saturating_add(TURN_SPEED);
        }
        if self.is_held(KeyCode::Right) {
            t.angle_turn = t.angle_turn.saturating_sub(TURN_SPEED);
        }

        // Attack
        if self.control_held {
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
        if let Some(ch) = t.console_char
            && ch.is_ascii()
        {
            t.chatchar = ch as u8;
        }

        // Consume pending F-key presses (cleared each tic).
        t.f5_save = std::mem::take(&mut self.pending_f5);
        t.f9_load = std::mem::take(&mut self.pending_f9);

        // Consume pending Tab press (cleared each tic).
        t.tab_pressed = std::mem::take(&mut self.pending_tab);

        // Consume pending Escape press.
        t.escape_pressed = std::mem::take(&mut self.pending_escape);

        // Consume pending edge-triggered menu navigation.
        t.menu_up = std::mem::take(&mut self.pending_menu_up);
        t.menu_down = std::mem::take(&mut self.pending_menu_down);
        t.menu_select = std::mem::take(&mut self.pending_menu_select);

        t
    }

    /// Clear all held keys (e.g. when the window loses focus).
    pub fn clear(&mut self) {
        self.held.clear();
        self.shift_held = false;
        self.control_held = false;
        self.pending_console_char = None;
        self.pending_f5 = false;
        self.pending_f9 = false;
        self.pending_tab = false;
        self.pending_escape = false;
        self.pending_menu_up = false;
        self.pending_menu_down = false;
        self.pending_menu_select = false;
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
    fn uppercase_forward_key_is_normalized() {
        let mut s = InputState::new();
        s.key_down(KeyCode::Char('W'));
        assert_eq!(s.to_tic_input().forward_move, MOVE_SPEED);
    }

    #[test]
    fn uppercase_a_strafes_with_shift() {
        let mut s = InputState::new();
        s.set_shift(true);
        s.key_down(KeyCode::Char('A'));
        let t = s.to_tic_input();
        assert_eq!(t.angle_turn, 0);
        assert!(t.side_move < 0);
    }

    #[test]
    fn control_modifier_sets_attack_button() {
        let mut s = InputState::new();
        s.key_down(KeyCode::Modifier(ModifierKeyCode::LeftControl));
        assert_ne!(s.to_tic_input().buttons & buttons::BT_ATTACK, 0);
    }

    #[test]
    fn sampled_control_state_sets_and_clears_attack_button() {
        let mut s = InputState::new();
        s.sync_modifiers(None, Some(true));
        assert_ne!(s.to_tic_input().buttons & buttons::BT_ATTACK, 0);

        s.sync_modifiers(None, Some(false));
        assert_eq!(s.to_tic_input().buttons & buttons::BT_ATTACK, 0);
    }

    #[test]
    fn f_key_does_not_set_attack_button() {
        let mut s = InputState::new();
        s.key_down(KeyCode::Char('f'));
        assert_eq!(s.to_tic_input().buttons & buttons::BT_ATTACK, 0);
    }

    #[test]
    fn arrows_still_turn_while_shift_is_held() {
        let mut s = InputState::new();
        s.set_shift(true);
        s.key_down(KeyCode::Left);
        let t = s.to_tic_input();
        assert!(t.angle_turn > 0);
        assert_eq!(t.side_move, 0);
    }

    #[test]
    fn console_char_also_populates_chatchar() {
        let mut s = InputState::new();
        s.push_console_char('i');
        let t = s.to_tic_input();
        assert_eq!(t.console_char, Some('i'));
        assert_eq!(t.chatchar, b'i');
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
        assert!(
            s.to_tic_input().tab_pressed,
            "tab_pressed must be true after push_tab"
        );
        // Second call must return false — it is consumed (one-shot).
        assert!(
            !s.to_tic_input().tab_pressed,
            "tab_pressed must be false after being consumed"
        );
    }
}
