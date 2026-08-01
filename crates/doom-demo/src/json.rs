//! Export LMP demo data to JSON format.
//!
//! This module provides the `export_demo_to_json` function.

use crate::DemoPlayer;

use std::fmt::Write;

/// Exports a parsed `DemoPlayer` to a JSON string.
pub fn export_demo_to_json(player: &mut DemoPlayer) -> String {
    let mut out = String::new();
    out.push_str("[\n");

    let mut tic = 0;
    let mut first = true;
    player.reset();
    while let Some(cmds) = player.next_tic_cmds() {
        for (player_idx, cmd) in cmds.iter().enumerate() {
            if !first {
                out.push_str(",\n");
            }
            first = false;
            let _ = write!(
                out,
                "  {{\n    \"tic\": {},\n    \"player\": {},\n    \"forward_move\": {},\n    \"side_move\": {},\n    \"angle_turn\": {},\n    \"buttons\": {}\n  }}",
                tic, player_idx, cmd.forward_move, cmd.side_move, cmd.angle_turn, cmd.buttons
            );
        }
        tic += 1;
    }
    out.push_str("\n]");

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
        let mut player = crate::DemoPlayer::from_lmp(&lmp).expect("value must exist in test");

        let json = export_demo_to_json(&mut player);
        assert!(json.starts_with("[\n"));
        assert!(json.contains(r#""tic": 0"#));
        assert!(json.contains(r#""player": 0"#));
        assert!(json.contains(r#""forward_move": 50"#));
        assert!(json.contains(r#""side_move": -10"#));
        assert!(json.contains(r#""angle_turn": 3"#));
        assert!(json.contains(r#""buttons": 31"#));
        assert!(json.ends_with("\n]"));
    }
}
