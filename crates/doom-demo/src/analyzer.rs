//! Demo analyzer module to extract player metrics and statistics.
//!
//! Exposes the [`DemoAnalyzer`] that processes an LMP format replay
//! to collect meaningful behavioral stats (SR40/50 movement, button
//! usage, and more).

use crate::player::DemoPlayer;
use doom_types::bt;

/// Player metrics extracted from a demo recording.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlayerMetrics {
    /// Total number of simulated tics for this player.
    pub total_tics: usize,
    /// Number of distinct attacks (button presses down).
    pub attack_presses: usize,
    /// Number of distinct uses (button presses down).
    pub use_presses: usize,
    /// Number of distinct weapon changes.
    pub weapon_changes: usize,
    /// Number of tics spent using SR40 movement.
    pub sr40_tics: usize,
    /// Number of tics spent using SR50 movement.
    pub sr50_tics: usize,
}

/// Analyzer for converting an LMP demo into player statistics.
#[derive(Debug, Default)]
pub struct DemoAnalyzer;

impl DemoAnalyzer {
    /// Analyze an LMP demo player to extract [`PlayerMetrics`] for each present player.
    #[must_use]
    pub fn analyze(player: &mut DemoPlayer) -> Vec<PlayerMetrics> {
        let num_players = player.header().num_players();
        let mut metrics = vec![PlayerMetrics::default(); num_players];
        let mut prev_buttons = vec![0u8; num_players];

        while let Some(tics) = player.next_tic_cmds() {
            for (i, tic) in tics.iter().enumerate() {
                let m = &mut metrics[i];
                let pb = prev_buttons[i];

                m.total_tics += 1;

                if (tic.buttons & bt::BT_ATTACK) != 0 && (pb & bt::BT_ATTACK) == 0 {
                    m.attack_presses += 1;
                }
                if (tic.buttons & bt::BT_USE) != 0 && (pb & bt::BT_USE) == 0 {
                    m.use_presses += 1;
                }
                if (tic.buttons & bt::BT_CHANGE) != 0 && (pb & bt::BT_CHANGE) == 0 {
                    m.weapon_changes += 1;
                }

                let fm = tic.forward_move.unsigned_abs();
                let sm = tic.side_move.unsigned_abs();

                if fm == 50 && sm == 40 {
                    m.sr40_tics += 1;
                } else if fm == 50 && sm == 50 {
                    m.sr50_tics += 1;
                }

                prev_buttons[i] = tic.buttons;
            }
        }

        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::LmpHeader;
    use crate::recorder::DemoRecorder;
    use crate::ticcmd::DemoTicCmd;

    #[test]
    fn test_analyzer_counts_presses_and_sr_movement() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);

        // Tic 1: move forward, press attack
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 50,
            side_move: 0,
            angle_turn: 0,
            buttons: bt::BT_ATTACK,
        }]);

        // Tic 2: keep holding attack, move sr40
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 50,
            side_move: 40,
            angle_turn: 0,
            buttons: bt::BT_ATTACK,
        }]);

        // Tic 3: release attack, move sr50
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 50,
            side_move: 50,
            angle_turn: 0,
            buttons: 0,
        }]);

        // Tic 4: press attack again, press use
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 0,
            side_move: 0,
            angle_turn: 0,
            buttons: bt::BT_ATTACK | bt::BT_USE,
        }]);

        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).unwrap();

        let metrics = DemoAnalyzer::analyze(&mut player);
        assert_eq!(metrics.len(), 1);

        let m = &metrics[0];
        assert_eq!(m.total_tics, 4);
        assert_eq!(m.attack_presses, 2); // pressed on tic 1 and tic 4
        assert_eq!(m.use_presses, 1); // pressed on tic 4
        assert_eq!(m.sr40_tics, 1);
        assert_eq!(m.sr50_tics, 1);
        assert_eq!(m.weapon_changes, 0);
    }
}
