//! LMP demo recording and byte-accurate playback.
//!
//! Implements the vanilla Doom 1.9 LMP format:
//! - 13-byte header (version, skill, episode, map, flags, player presence)
//! - N x 4-byte tic entries per present player (forward, side, turn byte, buttons)
//! - 1-byte `0x80` terminator
//!
//! # Modules
//! - [`header`] — [`LmpHeader`], constants (`LMP_VERSION_1_9`, `LMP_TERMINATOR`)
//! - [`ticcmd`] — [`DemoTicCmd`]: 4-byte wire-format tic command
//! - [`recorder`] — [`DemoRecorder`]: accumulates tics and writes an LMP file
//! - [`player`] — [`DemoPlayer`]: parses an LMP file and replays tics

mod csv;
mod header;
mod player;
mod recorder;
mod ticcmd;

// Re-export primary types at crate root for convenience.
pub use csv::export_demo_to_csv;
pub use header::{LMP_HEADER_SIZE, LMP_TERMINATOR, LMP_VERSION_1_9, LmpHeader};
pub use player::{DemoError, DemoPlayer};
pub use recorder::DemoRecorder;
pub use ticcmd::{DEMO_TIC_SIZE, DemoTicCmd};

// ---------------------------------------------------------------------------
// Roundtrip tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_single_player() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);

        let cmds: Vec<DemoTicCmd> = vec![
            DemoTicCmd {
                forward_move: 50,
                side_move: -10,
                angle_turn: 3,
                buttons: 0x11,
            },
            DemoTicCmd {
                forward_move: -20,
                side_move: 30,
                angle_turn: -5,
                buttons: 0x22,
            },
            DemoTicCmd {
                forward_move: 0,
                side_move: 0,
                angle_turn: 0,
                buttons: 0,
            },
            DemoTicCmd {
                forward_move: i8::MAX,
                side_move: i8::MIN,
                angle_turn: i8::MAX,
                buttons: u8::MAX,
            },
        ];

        for cmd in &cmds {
            rec.record_tic_cmds(&[*cmd]);
        }

        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).expect("should parse");

        assert_eq!(player.total_tics(), cmds.len());

        for expected in &cmds {
            let tic = player.next_tic_cmds().expect("should have tic");
            assert_eq!(tic.len(), 1);
            assert_eq!(tic[0], *expected);
        }

        assert!(player.is_finished());
        assert!(player.next_tic_cmds().is_none());
    }

    #[test]
    fn roundtrip_two_players() {
        let header = LmpHeader {
            version: LMP_VERSION_1_9,
            skill: 2,
            episode: 1,
            map: 3,
            deathmatch: 1,
            respawn: false,
            fast: false,
            nomonsters: false,
            consoleplayer: 0,
            players_present: [true, true, false, false],
        };

        let mut rec = DemoRecorder::new(header);

        let p1_cmd = DemoTicCmd {
            forward_move: 10,
            side_move: 5,
            angle_turn: 1,
            buttons: 0x01,
        };
        let p2_cmd = DemoTicCmd {
            forward_move: 20,
            side_move: -5,
            angle_turn: -2,
            buttons: 0x02,
        };

        rec.record_tic_cmds(&[p1_cmd, p2_cmd]);
        rec.record_tic_cmds(&[
            DemoTicCmd {
                forward_move: 30,
                side_move: 0,
                angle_turn: 0,
                buttons: 0x03,
            },
            DemoTicCmd {
                forward_move: 40,
                side_move: 0,
                angle_turn: 0,
                buttons: 0x04,
            },
        ]);

        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).expect("should parse");

        assert_eq!(player.total_tics(), 2);
        assert_eq!(player.header().num_players(), 2);

        let tic0 = player.next_tic_cmds().unwrap();
        assert_eq!(tic0.len(), 2);
        assert_eq!(tic0[0], p1_cmd);
        assert_eq!(tic0[1], p2_cmd);

        let tic1 = player.next_tic_cmds().unwrap();
        assert_eq!(tic1.len(), 2);
        assert_eq!(tic1[0].forward_move, 30);
        assert_eq!(tic1[1].forward_move, 40);

        assert!(player.is_finished());
    }

    #[test]
    fn roundtrip_empty_demo() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let rec = DemoRecorder::new(header);
        let lmp = rec.to_lmp();

        let player = DemoPlayer::from_lmp(&lmp).expect("should parse empty demo");
        assert_eq!(player.total_tics(), 0);
        assert!(player.is_finished());
    }

    #[test]
    fn finish_and_to_lmp_are_identical() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec1 = DemoRecorder::new(header.clone());
        let mut rec2 = DemoRecorder::new(header);

        let cmd = DemoTicCmd {
            forward_move: 42,
            side_move: -7,
            angle_turn: 12,
            buttons: 0x3f,
        };
        rec1.record_tic_cmds(&[cmd]);
        rec2.record_tic_cmds(&[cmd]);

        let lmp = rec1.to_lmp();
        let finished = rec2.finish();
        assert_eq!(lmp, finished);
    }

    #[test]
    fn roundtrip_four_players() {
        let header = LmpHeader {
            version: LMP_VERSION_1_9,
            skill: 4,
            episode: 1,
            map: 1,
            deathmatch: 0,
            respawn: false,
            fast: false,
            nomonsters: false,
            consoleplayer: 0,
            players_present: [true, true, true, true],
        };
        let mut rec = DemoRecorder::new(header);
        let cmds: Vec<DemoTicCmd> = (0..4)
            .map(|i| DemoTicCmd {
                forward_move: (i * 10) as i8,
                side_move: 0,
                angle_turn: 0,
                buttons: i as u8,
            })
            .collect();
        rec.record_tic_cmds(&cmds);

        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).unwrap();
        let tic = player.next_tic_cmds().unwrap();
        assert_eq!(tic.len(), 4);
        for (i, cmd) in tic.iter().enumerate() {
            assert_eq!(cmd.forward_move, (i * 10) as i8);
        }
    }

    #[test]
    fn lmp_byte_layout_correctness() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 50, // 0x32
            side_move: -10,   // 0xF6 as u8
            angle_turn: 0x12, // one-byte turn value
            buttons: 0x1f,
        }]);
        let lmp = rec.to_lmp();

        // Header: 13 bytes, tic: 4 bytes, terminator: 1 byte = 18 total
        assert_eq!(lmp.len(), 18);

        // Check tic bytes at offset 13..17
        assert_eq!(lmp[13], 50u8); // forward_move
        assert_eq!(lmp[14], 0xF6u8); // side_move as u8
        assert_eq!(lmp[15], 0x12u8); // angle_turn byte
        assert_eq!(lmp[16], 0x1fu8); // buttons

        // Terminator
        assert_eq!(lmp[17], LMP_TERMINATOR);
    }
}
