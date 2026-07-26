//! Auto-Pilot Bot for Doom.
//!
//! This module provides a simple bot that can generate random inputs.

use crate::state::GameState;
use doom_types::TicCmd;

pub struct SimpleBot;

impl SimpleBot {
    pub fn new() -> Self {
        Self
    }

    pub fn generate_input(&mut self, gs: &mut GameState) -> TicCmd {
        let forward = (gs.p_random() as i16 - 128) as i8;
        let side = (gs.p_random() as i16 - 128) as i8;
        let turn = (gs.p_random() as i16 - 128) * 100;

        TicCmd {
            forward_move: forward,
            side_move: side,
            angle_turn: turn,
            ..Default::default()
        }
    }
}

impl Default for SimpleBot {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;

    #[test]
    fn test_bot_generates_random_input() {
        let mut gs = GameState::new("E1M1");
        let mut bot = SimpleBot::new();
        let mut found_non_zero = false;

        for _ in 0..100 {
            let cmd = bot.generate_input(&mut gs);
            if cmd.forward_move != 0 || cmd.side_move != 0 || cmd.angle_turn != 0 {
                found_non_zero = true;
                break;
            }
        }

        assert!(
            found_non_zero,
            "Bot should generate non-zero inputs over time"
        );
    }
}
