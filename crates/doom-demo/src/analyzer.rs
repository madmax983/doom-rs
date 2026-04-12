//! Demo analyzer module.
use crate::player::DemoPlayer;
use doom_types::bt;
use serde::{Deserialize, Serialize};

/// Demo stats
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DemoStats {
    /// Total tics
    pub total_tics: usize,
    /// Total distance
    pub total_distance: usize,
    /// Total attacks
    pub total_attacks: usize,
    /// Total uses
    pub total_uses: usize,
    /// Total weapon changes
    pub total_weapon_changes: usize,
}

/// Demo analyzer
pub struct DemoAnalyzer;

impl DemoAnalyzer {
    /// analyze the player
    #[must_use]
    pub fn analyze(mut player: DemoPlayer) -> DemoStats {
        let mut stats = DemoStats::default();

        while let Some(cmds) = player.next_tic_cmds() {
            stats.total_tics += 1;
            for cmd in cmds {
                stats.total_distance += cmd.forward_move.unsigned_abs() as usize
                    + cmd.side_move.unsigned_abs() as usize;

                if (cmd.buttons & bt::BT_ATTACK) != 0 {
                    stats.total_attacks += 1;
                }
                if (cmd.buttons & bt::BT_USE) != 0 {
                    stats.total_uses += 1;
                }
                if (cmd.buttons & bt::BT_CHANGE) != 0 {
                    stats.total_weapon_changes += 1;
                }
            }
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::LmpHeader;
    use crate::recorder::DemoRecorder;
    use crate::ticcmd::DemoTicCmd;

    #[test]
    fn test_demo_analyzer_stats() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);

        let cmds: Vec<DemoTicCmd> = vec![
            DemoTicCmd {
                forward_move: 50,
                side_move: -10,
                angle_turn: 3,
                buttons: bt::BT_ATTACK | bt::BT_USE,
            },
            DemoTicCmd {
                forward_move: -20,
                side_move: 30,
                angle_turn: -5,
                buttons: bt::BT_CHANGE,
            },
        ];

        for cmd in &cmds {
            rec.record_tic_cmds(&[*cmd]);
        }

        let lmp = rec.to_lmp();
        let player = DemoPlayer::from_lmp(&lmp).expect("should parse");

        let stats = DemoAnalyzer::analyze(player);

        assert_eq!(stats.total_tics, 2);
        assert_eq!(stats.total_attacks, 1);
        assert_eq!(stats.total_uses, 1);
        assert_eq!(stats.total_weapon_changes, 1);
        assert_eq!(stats.total_distance, 50 + 10 + 20 + 30);
    }
}
