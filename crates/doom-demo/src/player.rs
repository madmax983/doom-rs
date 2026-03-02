//! Demo playback: parse a raw LMP byte slice and replay tic commands.
//!
//! The [`DemoPlayer`] parses an LMP file into its header and tic data, then
//! provides an iterator-like interface to step through tics one at a time.

use doom_game::TicCmd;

use crate::header::{LMP_HEADER_SIZE, LMP_TERMINATOR, LmpHeader};
use crate::ticcmd::DemoTicCmd;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur while parsing an LMP demo file.
#[derive(Debug, thiserror::Error)]
pub enum DemoError {
    /// The byte slice was shorter than expected.
    #[error("data too short")]
    TooShort,
    /// The tic stream ended without a sentinel byte.
    #[error("demo ended unexpectedly (missing 0x80 terminator)")]
    UnexpectedEof,
    /// The tic data is not evenly divisible by the per-tic stride.
    #[error("tic data length {0} not divisible by tic size {1}")]
    MalformedTics(usize, usize),
}

// ---------------------------------------------------------------------------
// DemoPlayer
// ---------------------------------------------------------------------------

/// Plays back a pre-recorded LMP demo by replaying tic commands in order.
///
/// Construct with [`DemoPlayer::from_lmp`], then call [`DemoPlayer::next_tic`]
/// once per simulation tic until [`DemoPlayer::is_finished`] returns `true`.
#[derive(Debug, Clone)]
pub struct DemoPlayer {
    header: LmpHeader,
    tics: Vec<Vec<DemoTicCmd>>,
    current_tic: usize,
}

impl DemoPlayer {
    /// Parse a complete LMP byte slice into a [`DemoPlayer`].
    ///
    /// Returns `None` if the data is too short, missing the terminator, or has
    /// malformed tic data.
    ///
    /// The `0x80` terminator is only checked at tic boundaries (at the byte
    /// position where the first player's `forward_move` would be). This
    /// matches vanilla Doom's parsing behavior and avoids false positives
    /// from `i8::MIN` (-128 = 0x80) appearing inside tic data fields.
    #[must_use]
    pub fn from_lmp(data: &[u8]) -> Option<Self> {
        let header = LmpHeader::from_bytes(data)?;

        let num_players = header.num_players();
        let tic_stride = header.tic_size(); // 4 * num_players

        let mut tics = Vec::new();
        let mut offset = LMP_HEADER_SIZE;

        if num_players == 0 {
            // Degenerate: no players present. Expect terminator immediately.
            if offset >= data.len() || data[offset] != LMP_TERMINATOR {
                return None;
            }
            return Some(Self {
                header,
                tics,
                current_tic: 0,
            });
        }

        // Read tics until we hit the terminator or run out of data.
        // The terminator byte is only valid at the start of a tic (the byte
        // that would be player 0's forward_move).
        loop {
            if offset >= data.len() {
                // Ran out of data without finding a terminator.
                return None;
            }

            // Check for the terminator at a tic boundary.
            if data[offset] == LMP_TERMINATOR {
                break;
            }

            // Need at least tic_stride bytes for this tic.
            if offset + tic_stride > data.len() {
                return None;
            }

            let mut cmds = Vec::with_capacity(num_players);
            for _ in 0..num_players {
                let cmd = DemoTicCmd::from_bytes(&data[offset..])?;
                cmds.push(cmd);
                offset += 4;
            }
            tics.push(cmds);
        }

        Some(Self {
            header,
            tics,
            current_tic: 0,
        })
    }

    /// Parse a complete LMP byte slice, returning a `Result` for backward
    /// compatibility with code that calls `DemoPlayer::parse()`.
    ///
    /// # Errors
    ///
    /// Returns [`DemoError::TooShort`] if the data is shorter than the
    /// 13-byte header, or [`DemoError::UnexpectedEof`] if parsing fails.
    pub fn parse(data: &[u8]) -> Result<Self, DemoError> {
        if data.len() < LMP_HEADER_SIZE {
            return Err(DemoError::TooShort);
        }
        Self::from_lmp(data).ok_or(DemoError::UnexpectedEof)
    }

    /// Access the parsed header.
    #[must_use]
    pub const fn header(&self) -> &LmpHeader {
        &self.header
    }

    /// Index of the tic that will be returned by the next call to [`next_tic`](Self::next_tic).
    #[must_use]
    pub const fn current_tic(&self) -> usize {
        self.current_tic
    }

    /// Total number of tics in the demo.
    #[must_use]
    pub const fn total_tics(&self) -> usize {
        self.tics.len()
    }

    /// Backward-compatible alias for [`total_tics`](Self::total_tics).
    #[must_use]
    pub const fn tic_count(&self) -> usize {
        self.tics.len()
    }

    /// Return `true` when the demo has been fully consumed.
    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.current_tic >= self.tics.len()
    }

    /// Advance the playback cursor and return the first player's command as a
    /// [`TicCmd`].
    ///
    /// This is the primary playback entry point for doom-app (single-player
    /// demos). For multi-player demos, use [`next_tic_cmds`](Self::next_tic_cmds).
    ///
    /// Returns `None` once all recorded tics have been consumed.
    pub fn next_tic(&mut self) -> Option<TicCmd> {
        let cmds = self.next_tic_cmds()?;
        Some(
            cmds.first()
                .map_or_else(TicCmd::default, DemoTicCmd::to_ticcmd),
        )
    }

    /// Advance the playback cursor and return all player commands for this tic.
    ///
    /// Returns one [`DemoTicCmd`] per present player. Returns `None` once all
    /// recorded tics have been consumed.
    pub fn next_tic_cmds(&mut self) -> Option<Vec<DemoTicCmd>> {
        if self.current_tic >= self.tics.len() {
            return None;
        }
        let cmds = self.tics[self.current_tic].clone();
        self.current_tic += 1;
        Some(cmds)
    }

    /// Look at the current tic's commands without advancing the cursor.
    #[must_use]
    pub fn peek_tic(&self) -> Option<&[DemoTicCmd]> {
        self.tics.get(self.current_tic).map(Vec::as_slice)
    }

    /// Rewind playback to the beginning.
    pub const fn reset(&mut self) {
        self.current_tic = 0;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::LmpHeader;
    use crate::recorder::DemoRecorder;
    use crate::ticcmd::DemoTicCmd;

    /// Build a minimal LMP from a recorder with N tics (single player, all
    /// default commands).
    fn record_n_tics(n: usize) -> Vec<u8> {
        let mut rec = DemoRecorder::new(LmpHeader::new_singleplayer(3, 1, 1));
        for _ in 0..n {
            rec.record_tic_cmds(&[DemoTicCmd::default()]);
        }
        rec.to_lmp()
    }

    // Test 17: from_lmp parses valid file
    #[test]
    fn from_lmp_parses_valid_file() {
        let lmp = record_n_tics(5);
        let player = DemoPlayer::from_lmp(&lmp);
        assert!(player.is_some());
        let player = player.unwrap();
        assert_eq!(player.total_tics(), 5);
    }

    // Test 18: from_lmp returns None for empty data
    #[test]
    fn from_lmp_returns_none_for_empty_data() {
        assert!(DemoPlayer::from_lmp(&[]).is_none());
        assert!(DemoPlayer::from_lmp(&[0u8; 5]).is_none());
    }

    // Test 19: next_tic_cmds returns cmds in order
    #[test]
    fn next_tic_cmds_returns_in_order() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);
        for i in 0..3i8 {
            rec.record_tic_cmds(&[DemoTicCmd {
                forward_move: i * 10,
                side_move: 0,
                angle_turn: 0,
            }]);
        }
        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).unwrap();

        let t0 = player.next_tic_cmds().unwrap();
        assert_eq!(t0[0].forward_move, 0);

        let t1 = player.next_tic_cmds().unwrap();
        assert_eq!(t1[0].forward_move, 10);

        let t2 = player.next_tic_cmds().unwrap();
        assert_eq!(t2[0].forward_move, 20);

        assert!(player.next_tic_cmds().is_none());
    }

    // Test 20: is_finished returns true after all tics consumed
    #[test]
    fn is_finished_after_all_tics_consumed() {
        let lmp = record_n_tics(3);
        let mut player = DemoPlayer::from_lmp(&lmp).unwrap();
        assert!(!player.is_finished());
        for _ in 0..3 {
            player.next_tic();
        }
        assert!(player.is_finished());
    }

    // Test 21: reset rewinds to start
    #[test]
    fn reset_rewinds_to_start() {
        let lmp = record_n_tics(5);
        let mut player = DemoPlayer::from_lmp(&lmp).unwrap();

        // Consume 3 tics.
        for _ in 0..3 {
            player.next_tic();
        }
        assert_eq!(player.current_tic(), 3);

        // Reset.
        player.reset();
        assert_eq!(player.current_tic(), 0);
        assert!(!player.is_finished());

        // Can consume all 5 again.
        for _ in 0..5 {
            assert!(player.next_tic().is_some());
        }
        assert!(player.is_finished());
    }

    // Test 22: peek_tic doesn't advance
    #[test]
    fn peek_tic_does_not_advance() {
        let lmp = record_n_tics(3);
        let player = DemoPlayer::from_lmp(&lmp).unwrap();

        assert_eq!(player.current_tic(), 0);
        let peeked = player.peek_tic();
        assert!(peeked.is_some());
        assert_eq!(player.current_tic(), 0, "peek must not advance the cursor");
    }

    #[test]
    fn from_lmp_returns_none_for_missing_terminator() {
        // Build a valid header but omit the terminator.
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let bytes = header.to_bytes();
        // No terminator appended.
        assert!(DemoPlayer::from_lmp(&bytes).is_none());
    }

    #[test]
    fn parse_backward_compat() {
        let lmp = record_n_tics(2);
        let player = DemoPlayer::parse(&lmp).expect("parse should succeed");
        assert_eq!(player.tic_count(), 2);
    }

    #[test]
    fn next_tic_returns_ticcmd() {
        let header = LmpHeader::new_singleplayer(3, 1, 1);
        let mut rec = DemoRecorder::new(header);
        rec.record_tic_cmds(&[DemoTicCmd {
            forward_move: 42,
            side_move: -7,
            angle_turn: 1234,
        }]);
        let lmp = rec.to_lmp();
        let mut player = DemoPlayer::from_lmp(&lmp).unwrap();

        let cmd: TicCmd = player.next_tic().unwrap();
        assert_eq!(cmd.forward_move, 42);
        assert_eq!(cmd.side_move, -7);
        assert_eq!(cmd.angle_turn, 1234);
        assert_eq!(cmd.buttons, 0);
    }

    #[test]
    fn peek_after_finish_returns_none() {
        let lmp = record_n_tics(1);
        let mut player = DemoPlayer::from_lmp(&lmp).unwrap();
        player.next_tic();
        assert!(player.peek_tic().is_none());
    }
}
