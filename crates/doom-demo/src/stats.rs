//! Statistics analysis for LMP demos.

use crate::DemoPlayer;
use doom_types::bt;

/// Aggregated statistics for a single player in a demo.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlayerStats {
    /// Total number of tics the player was present for.
    pub total_tics: usize,
    /// Number of tics the player was moving forward or backward.
    pub moving_tics: usize,
    /// Number of tics the player was strafing laterally.
    pub strafing_tics: usize,
    /// Number of tics the player was turning.
    pub turning_tics: usize,
    /// Number of tics the player had the attack button held.
    pub attack_tics: usize,
    /// Number of tics the player had the use button held.
    pub use_tics: usize,
}

/// Analyzes a parsed demo and computes statistics for each present player.
pub fn analyze_demo(player: &mut DemoPlayer) -> Vec<PlayerStats> {
    let mut stats = vec![PlayerStats::default(); player.header().num_players()];

    player.reset();
    while let Some(cmds) = player.next_tic_cmds() {
        for (i, cmd) in cmds.iter().enumerate() {
            if i < stats.len() {
                stats[i].total_tics += 1;
                if cmd.forward_move != 0 {
                    stats[i].moving_tics += 1;
                }
                if cmd.side_move != 0 {
                    stats[i].strafing_tics += 1;
                }
                if cmd.angle_turn != 0 {
                    stats[i].turning_tics += 1;
                }
                if cmd.buttons & bt::BT_ATTACK != 0 {
                    stats[i].attack_tics += 1;
                }
                if cmd.buttons & bt::BT_USE != 0 {
                    stats[i].use_tics += 1;
                }
            }
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
    fn test_analyze_demo_basic() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);

        // Tic 1: moving forward + attack
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 50,
            side_move: 0,
            angle_turn: 0,
            buttons: bt::BT_ATTACK,
        }]);

        // Tic 2: strafing + use
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 0,
            side_move: 20,
            angle_turn: 0,
            buttons: bt::BT_USE,
        }]);

        // Tic 3: turning + attack
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 0,
            side_move: 0,
            angle_turn: 15,
            buttons: bt::BT_ATTACK,
        }]);

        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).expect("should parse");

        let stats = analyze_demo(&mut player);
        assert_eq!(stats.len(), 1);

        let p1 = &stats[0];
        assert_eq!(p1.total_tics, 3);
        assert_eq!(p1.moving_tics, 1);
        assert_eq!(p1.strafing_tics, 1);
        assert_eq!(p1.turning_tics, 1);
        assert_eq!(p1.attack_tics, 2);
        assert_eq!(p1.use_tics, 1);
    }
}
