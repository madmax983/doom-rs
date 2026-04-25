//! Export LMP demo data to CSV format.
//!
//! This module provides the `export_demo_to_csv` function, which translates
//! the parsed demo tics and player input commands into a human-readable CSV.

use crate::DemoPlayer;
use std::fmt::Write;

/// Exports a parsed `DemoPlayer` to a CSV string.
pub fn export_demo_to_csv(player: &mut DemoPlayer) -> String {
    let mut out = String::new();
    out.push_str("tic,player,forward_move,side_move,angle_turn,buttons\n");

    let mut tic = 0;
    player.reset();
    while let Some(cmds) = player.next_tic_cmds() {
        for (player_idx, cmd) in cmds.iter().enumerate() {
            let _ = writeln!(
                out,
                "{},{},{},{},{},{}",
                tic, player_idx, cmd.forward_move, cmd.side_move, cmd.angle_turn, cmd.buttons
            );
        }
        tic += 1;
    }

    player.reset();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::LmpHeader;
    use crate::recorder::DemoRecorder;
    use crate::ticcmd::DemoTicCmd;

    #[test]
    fn test_export_demo_to_csv() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 50,
            side_move: -10,
            angle_turn: 3,
            buttons: 0x1f,
        }]);
        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).expect("value must exist in test");

        let csv = export_demo_to_csv(&mut player);
        assert!(csv.contains("tic,player,forward_move,side_move,angle_turn,buttons"));
        assert!(csv.contains("0,0,50,-10,3,31"));
    }
}
