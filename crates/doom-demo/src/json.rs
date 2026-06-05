//! Export LMP demo data to JSON format.

use crate::DemoPlayer;
use std::fmt::Write;

/// Exports a parsed `DemoPlayer` to a JSON string.
pub fn export_demo_to_json(player: &mut DemoPlayer) -> String {
    let mut out = String::from("{\n  \"tics\": [\n");

    let mut tic = 0;
    player.reset();
    let mut first = true;
    while let Some(cmds) = player.next_tic_cmds() {
        for (player_idx, cmd) in cmds.iter().enumerate() {
            if !first {
                out.push_str(",\n");
            }
            first = false;

            let _ = write!(&mut out,
                "    {{ \"tic\": {}, \"player\": {}, \"forward_move\": {}, \"side_move\": {}, \"angle_turn\": {}, \"buttons\": {} }}",
                tic, player_idx, cmd.forward_move, cmd.side_move, cmd.angle_turn, cmd.buttons
            );
        }
        tic += 1;
    }

    out.push_str("\n  ]\n}");
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
    fn test_export_demo_to_json() {
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

        let json = export_demo_to_json(&mut player);
        assert!(json.contains("\"tics\": ["));
        assert!(json.contains("\"forward_move\": 50"));
    }
}
