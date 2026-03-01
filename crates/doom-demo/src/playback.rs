//! Demo playback: parse a raw LMP byte slice and replay [`TicCmd`]s.

use doom_game::TicCmd;

use crate::lmp::{DemoError, LmpHeader, LmpTicEntry, LMP_VERSION, DEMO_SENTINEL};

// ---------------------------------------------------------------------------
// DemoPlayer
// ---------------------------------------------------------------------------

/// Plays back a pre-recorded LMP demo by replaying [`TicCmd`]s in order.
///
/// Construct with [`DemoPlayer::parse`], then call [`DemoPlayer::next_tic`]
/// once per simulation tic until [`DemoPlayer::is_finished`] returns `true`.
#[derive(Debug)]
pub struct DemoPlayer {
    /// Parsed header from the LMP file.
    pub header: LmpHeader,
    tics: Vec<LmpTicEntry>,
    pos: usize,
}

impl DemoPlayer {
    /// Parse a complete LMP byte slice into a [`DemoPlayer`].
    ///
    /// # Errors
    /// - [`DemoError::TooShort`]  — fewer than 13 bytes, or tic stream truncated.
    /// - [`DemoError::BadVersion`] — version byte is not [`LMP_VERSION`].
    /// - [`DemoError::UnexpectedEof`] — tic stream ends without a `0x80` sentinel.
    pub fn parse(data: &[u8]) -> Result<Self, DemoError> {
        let header = LmpHeader::parse(data)?;
        if header.version != LMP_VERSION {
            return Err(DemoError::BadVersion(header.version));
        }

        let mut tics = Vec::new();
        let mut i = 13usize; // start immediately after the 13-byte header

        loop {
            if i >= data.len() {
                return Err(DemoError::UnexpectedEof);
            }
            if data[i] == DEMO_SENTINEL {
                break;
            }
            if i + 4 > data.len() {
                return Err(DemoError::TooShort);
            }
            tics.push(LmpTicEntry::from_bytes(&data[i..i + 4])?);
            i += 4;
        }

        Ok(Self { header, tics, pos: 0 })
    }

    /// Return the next [`TicCmd`] from the demo and advance the playback cursor.
    ///
    /// Returns `None` once all recorded tics have been consumed.
    pub fn next_tic(&mut self) -> Option<TicCmd> {
        let entry = self.tics.get(self.pos)?;
        self.pos += 1;
        Some(entry.to_ticcmd())
    }

    /// Return `true` when the demo has been fully consumed.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.pos >= self.tics.len()
    }

    /// Total number of tics in the demo.
    #[must_use]
    pub fn tic_count(&self) -> usize {
        self.tics.len()
    }

    /// Index of the tic that will be returned by the next call to
    /// [`DemoPlayer::next_tic`].
    #[must_use]
    pub fn current_tic(&self) -> usize {
        self.pos
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lmp::LmpHeader;
    use crate::record::DemoRecorder;
    use doom_game::TicCmd;

    fn record_n_tics(n: usize) -> Vec<u8> {
        let mut rec = DemoRecorder::new(LmpHeader::new_singleplayer(3, 1, 1));
        for _ in 0..n {
            rec.record_tic(&TicCmd::default());
        }
        rec.to_bytes().expect("recording should not fail")
    }

    #[test]
    fn playback_roundtrip_empty() {
        let bytes = record_n_tics(0);
        let player = DemoPlayer::parse(&bytes).expect("parse should succeed");
        assert!(player.is_finished(), "empty demo should be finished immediately");
    }

    #[test]
    fn playback_roundtrip_5_tics() {
        let bytes = record_n_tics(5);
        let mut player = DemoPlayer::parse(&bytes).expect("parse should succeed");
        for i in 0..5 {
            assert!(
                player.next_tic().is_some(),
                "tic {i} should return Some"
            );
        }
        assert!(
            player.next_tic().is_none(),
            "6th next_tic should return None"
        );
    }

    #[test]
    fn playback_wrong_version_errors() {
        let mut bytes = record_n_tics(0);
        bytes[0] = 0x01; // corrupt the version byte
        let result = DemoPlayer::parse(&bytes);
        assert!(
            matches!(result, Err(DemoError::BadVersion(0x01))),
            "expected BadVersion(0x01), got {result:?}"
        );
    }

    #[test]
    fn playback_tic_count_matches_recorded() {
        let bytes = record_n_tics(10);
        let player = DemoPlayer::parse(&bytes).expect("parse should succeed");
        assert_eq!(player.tic_count(), 10);
    }

    #[test]
    fn playback_is_finished_mid_demo_false() {
        let bytes = record_n_tics(5);
        let mut player = DemoPlayer::parse(&bytes).expect("parse should succeed");
        for _ in 0..3 {
            player.next_tic();
        }
        assert!(
            !player.is_finished(),
            "should not be finished after 3 of 5 tics"
        );
    }
}
