//! Demo Analytics and Statistics
//!
//! This module parses an LMP demo and extracts high-level analytics,
//! such as total movement, idle time, and action occurrences (attacking, using).
//! It is useful for comparing speedrun efficiencies or tracking player behaviors.

use crate::DemoPlayer;
use doom_types::bt;

/// High-level analytics extracted from a demo recording.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DemoStats {
    /// Total number of tics in the demo.
    pub total_tics: usize,
    /// Number of tics where the player did absolutely nothing (no movement, no turning, no buttons).
    pub idle_tics: usize,
    /// Number of tics where the attack button was pressed.
    pub attack_tics: usize,
    /// Number of tics where the use button was pressed.
    pub use_tics: usize,
    /// Total absolute forward/backward movement units applied.
    pub total_forward_movement: u32,
    /// Total absolute lateral movement units applied.
    pub total_side_movement: u32,
    /// Total absolute turn angle units applied.
    pub total_turning: u32,
}

/// Analyzes a parsed demo for player 0 and computes movement/action statistics.
///
/// # Examples
///
/// ```
/// use doom_demo::{DemoPlayer, DemoRecorder, LmpHeader, ticcmd::DemoTicCmd};
/// use doom_demo::analyzer::analyze_demo;
/// use doom_types::bt;
///
/// let header = LmpHeader::new_singleplayer(3, 1, 1);
/// let mut rec = DemoRecorder::new(header);
///
/// rec.record_tic_cmds(&[DemoTicCmd {
///     forward_move: 50,
///     side_move: -10,
///     angle_turn: 3,
///     buttons: bt::BT_ATTACK,
/// }]);
///
/// let lmp = rec.to_lmp();
/// let mut player = DemoPlayer::from_lmp(&lmp).unwrap();
///
/// let stats = analyze_demo(&mut player);
/// assert_eq!(stats.total_tics, 1);
/// assert_eq!(stats.attack_tics, 1);
/// assert_eq!(stats.total_forward_movement, 50);
/// ```
pub fn analyze_demo(player: &mut DemoPlayer) -> DemoStats {
    let mut stats = DemoStats::default();

    player.reset();
    while let Some(cmds) = player.next_tic_cmds() {
        if let Some(cmd) = cmds.first() {
            stats.total_tics += 1;

            if cmd.forward_move == 0
                && cmd.side_move == 0
                && cmd.angle_turn == 0
                && cmd.buttons == 0
            {
                stats.idle_tics += 1;
            }

            if (cmd.buttons & bt::BT_ATTACK) != 0 {
                stats.attack_tics += 1;
            }

            if (cmd.buttons & bt::BT_USE) != 0 {
                stats.use_tics += 1;
            }

            stats.total_forward_movement += u32::from(cmd.forward_move.unsigned_abs());
            stats.total_side_movement += u32::from(cmd.side_move.unsigned_abs());
            stats.total_turning += u32::from(cmd.angle_turn.unsigned_abs());
        }
    }

    player.reset();
    stats
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::LmpHeader;
    use crate::recorder::DemoRecorder;
    use crate::ticcmd::DemoTicCmd;

    #[test]
    fn test_analyze_demo_empty() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let rec = DemoRecorder::new(header);
        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).expect("valid demo");

        let stats = analyze_demo(&mut player);
        assert_eq!(stats, DemoStats::default());
    }

    #[test]
    fn test_analyze_demo_mixed() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);

        // Tic 1: Move forward, attack
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 50,
            side_move: 0,
            angle_turn: 0,
            buttons: bt::BT_ATTACK,
        }]);

        // Tic 2: Idle
        rec.record_tic_cmds(&[DemoTicCmd::default()]);

        // Tic 3: Move back, strafe left, turn, use
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: -20,
            side_move: -30,
            angle_turn: 5,
            buttons: bt::BT_USE,
        }]);

        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).expect("valid demo");

        let stats = analyze_demo(&mut player);

        assert_eq!(stats.total_tics, 3);
        assert_eq!(stats.idle_tics, 1);
        assert_eq!(stats.attack_tics, 1);
        assert_eq!(stats.use_tics, 1);
        assert_eq!(stats.total_forward_movement, 70); // 50 + |-20|
        assert_eq!(stats.total_side_movement, 30); // 0 + |-30|
        assert_eq!(stats.total_turning, 5); // 0 + |5|
    }
}
