//! Demo recorder: collects tic commands and serializes them to the LMP format.
//!
//! Each tic stores one [`DemoTicCmd`] per present player. The final LMP file
//! consists of a 13-byte header, followed by 4 bytes per player per tic,
//! terminated by a `0x80` sentinel byte.

use doom_types::TicCmd;

use crate::header::{LMP_TERMINATOR, LmpHeader};
use crate::ticcmd::DemoTicCmd;

// ---------------------------------------------------------------------------
// DemoRecorder
// ---------------------------------------------------------------------------

/// Records player input as an LMP-format byte stream.
///
/// Call [`DemoRecorder::record_tic`] once per simulation tic (with one
/// [`DemoTicCmd`] per present player), then [`DemoRecorder::to_lmp`] or
/// [`DemoRecorder::finish`] to produce a byte-accurate LMP file.
///
/// ## Examples
///
/// ```
/// use doom_demo::{DemoRecorder, LmpHeader};
/// use doom_types::TicCmd;
///
/// // Create a header for a single-player game on Hurt Me Plenty (skill 3), E1M1
/// let header = LmpHeader::new_singleplayer(3, 1, 1);
/// let mut recorder = DemoRecorder::new(header);
///
/// // Record some player movement
/// let mut cmd = TicCmd::default();
/// cmd.forward_move = 50;
/// recorder.record_tic(&cmd);
///
/// cmd.forward_move = 0;
/// cmd.side_move = -25;
/// recorder.record_tic(&cmd);
///
/// // Finalize the demo into a byte array
/// let lmp_bytes = recorder.to_lmp();
/// assert_eq!(recorder.tic_count(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct DemoRecorder {
    header: LmpHeader,
    tics: Vec<Vec<DemoTicCmd>>,
}

impl DemoRecorder {
    /// Create a new recorder with the given [`LmpHeader`].
    #[must_use]
    pub const fn new(header: LmpHeader) -> Self {
        Self {
            header,
            tics: Vec::new(),
        }
    }

    /// Append one tic from a single [`TicCmd`] (single-player convenience).
    ///
    /// This is the primary recording entry point for doom-app, which records
    /// one `TicCmd` per tic for the console player.
    pub fn record_tic(&mut self, cmd: &TicCmd) {
        self.tics.push(vec![DemoTicCmd::from_ticcmd(cmd)]);
    }

    /// Append one tic of commands (one [`DemoTicCmd`] per present player).
    ///
    /// For multi-player demos, pass one [`DemoTicCmd`] per present player
    /// slot.
    pub fn record_tic_cmds(&mut self, cmds: &[DemoTicCmd]) {
        self.tics.push(cmds.to_vec());
    }

    /// Return the number of tics recorded so far.
    #[must_use]
    pub const fn tic_count(&self) -> usize {
        self.tics.len()
    }

    /// Serialize the complete LMP file: header + tic data + terminator.
    #[must_use]
    pub fn to_lmp(&self) -> Vec<u8> {
        let mut buf = self.header.to_bytes();

        for tic_cmds in &self.tics {
            for cmd in tic_cmds {
                buf.extend_from_slice(&cmd.to_bytes());
            }
        }

        buf.push(LMP_TERMINATOR);
        buf
    }

    /// Consume the recorder and produce the final LMP bytes.
    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.to_lmp()
    }

    /// Access the header.
    #[must_use]
    pub const fn header(&self) -> &LmpHeader {
        &self.header
    }

    /// Serialize the demo to bytes, wrapped in a `Result` for backward
    /// compatibility with code that expects `Result<Vec<u8>, _>`.
    ///
    /// # Errors
    ///
    /// This never fails; the `Result` wrapper exists only for backward
    /// compatibility.
    pub fn to_bytes(&self) -> Result<Vec<u8>, std::io::Error> {
        Ok(self.to_lmp())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{LMP_TERMINATOR, LMP_VERSION_1_9, LmpHeader};
    use crate::ticcmd::DemoTicCmd;

    fn singleplayer_recorder() -> DemoRecorder {
        DemoRecorder::new(LmpHeader::new_singleplayer(3, 1, 1))
    }

    // Test 12: new creates empty recorder
    #[test]
    fn new_creates_empty_recorder() {
        let rec = singleplayer_recorder();
        assert_eq!(rec.tic_count(), 0);
    }

    // Test 13: record_tic increases tic_count
    #[test]
    fn record_tic_increases_tic_count() {
        let mut rec = singleplayer_recorder();
        let cmd = DemoTicCmd::default();
        rec.record_tic_cmds(&[cmd]);
        assert_eq!(rec.tic_count(), 1);
        rec.record_tic_cmds(&[cmd]);
        assert_eq!(rec.tic_count(), 2);
    }

    // Test 14: to_lmp starts with header bytes
    #[test]
    fn to_lmp_starts_with_header() {
        let rec = singleplayer_recorder();
        let lmp = rec.to_lmp();
        assert_eq!(lmp[0], LMP_VERSION_1_9);
        assert_eq!(lmp[1], 3); // skill
        assert_eq!(lmp[2], 1); // episode
        assert_eq!(lmp[3], 1); // map
    }

    // Test 15: to_lmp ends with 0x80 terminator
    #[test]
    fn to_lmp_ends_with_terminator() {
        let mut rec = singleplayer_recorder();
        rec.record_tic_cmds(&[DemoTicCmd::default()]);
        let lmp = rec.to_lmp();
        assert_eq!(
            *lmp.last().expect("LMP file must have at least one byte"),
            LMP_TERMINATOR
        );
    }

    // Test 16: finish produces valid LMP
    #[test]
    fn finish_produces_valid_lmp() {
        let mut rec = singleplayer_recorder();
        let cmd = DemoTicCmd {
            forward_move: 50,
            side_move: -10,
            angle_turn: 3,
            buttons: 0x1f,
        };
        rec.record_tic_cmds(&[cmd]);
        let lmp = rec.finish();

        // 13-byte header + 4-byte tic + 1-byte terminator = 18
        assert_eq!(lmp.len(), 18);
        assert_eq!(lmp[0], LMP_VERSION_1_9);
        assert_eq!(
            *lmp.last().expect("LMP file must have at least one byte"),
            LMP_TERMINATOR
        );
    }

    #[test]
    fn empty_demo_has_header_and_terminator() {
        let rec = singleplayer_recorder();
        let lmp = rec.to_lmp();
        // 13 header + 1 terminator = 14
        assert_eq!(lmp.len(), 14);
    }

    #[test]
    fn record_tic_single_ticcmd() {
        let mut rec = singleplayer_recorder();
        let cmd = TicCmd::default();
        rec.record_tic(&cmd);
        assert_eq!(rec.tic_count(), 1);
    }

    #[test]
    fn to_bytes_backward_compat() {
        let rec = singleplayer_recorder();
        let result = rec.to_bytes();
        assert!(result.is_ok());
        assert_eq!(result.expect("to_bytes should always succeed").len(), 14);
    }

    #[test]
    fn multi_player_tic_size() {
        let header = LmpHeader {
            players_present: [true, true, false, false],
            ..LmpHeader::new_singleplayer(3, 1, 1)
        };
        let mut rec = DemoRecorder::new(header);
        let p1 = DemoTicCmd {
            forward_move: 10,
            side_move: 0,
            angle_turn: 0,
            buttons: 1,
        };
        let p2 = DemoTicCmd {
            forward_move: 20,
            side_move: 0,
            angle_turn: 0,
            buttons: 2,
        };
        rec.record_tic_cmds(&[p1, p2]);
        let lmp = rec.to_lmp();
        // 13 header + 8 (2 players * 4 bytes) + 1 terminator = 22
        assert_eq!(lmp.len(), 22);
    }
}
