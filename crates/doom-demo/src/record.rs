//! Demo recorder: collects [`TicCmd`]s and serialises them to the LMP format.

use std::io::Write;

use doom_game::TicCmd;

use crate::lmp::{DemoError, LmpHeader, LmpTicEntry, DEMO_SENTINEL};

// ---------------------------------------------------------------------------
// DemoRecorder
// ---------------------------------------------------------------------------

/// Records player input as an LMP-format byte stream.
///
/// Call [`DemoRecorder::record_tic`] once per simulation tic for the console
/// player, then [`DemoRecorder::to_bytes`] (or [`DemoRecorder::write_to`]) to
/// produce a byte-accurate LMP file.
pub struct DemoRecorder {
    header: LmpHeader,
    tics: Vec<LmpTicEntry>,
    /// Number of active players (derived from `header.player_present`).
    #[allow(dead_code)]
    num_players: usize,
}

impl DemoRecorder {
    /// Create a new recorder with the given [`LmpHeader`].
    ///
    /// `num_players` is computed from `header.player_present`.
    #[must_use]
    pub fn new(header: LmpHeader) -> Self {
        let num_players = header.player_present.iter().filter(|&&p| p).count();
        Self {
            header,
            tics: Vec::new(),
            num_players,
        }
    }

    /// Append one [`TicCmd`] for the console player.
    pub fn record_tic(&mut self, cmd: &TicCmd) {
        self.tics.push(LmpTicEntry::from_ticcmd(cmd));
    }

    /// Write the complete LMP demo (header + tic stream + sentinel) to `w`.
    ///
    /// # Errors
    /// Propagates any [`std::io::Error`] from the underlying writer.
    pub fn write_to<W: Write>(&self, mut w: W) -> Result<(), DemoError> {
        w.write_all(&self.header.to_bytes())?;
        for entry in &self.tics {
            w.write_all(&entry.to_bytes())?;
        }
        w.write_all(&[DEMO_SENTINEL])?;
        Ok(())
    }

    /// Serialise the complete demo to a [`Vec<u8>`].
    ///
    /// Convenience wrapper around [`DemoRecorder::write_to`].
    ///
    /// # Errors
    /// Returns [`DemoError::Io`] if the in-memory write fails (unlikely in
    /// practice).
    pub fn to_bytes(&self) -> Result<Vec<u8>, DemoError> {
        let mut buf = Vec::new();
        self.write_to(&mut buf)?;
        Ok(buf)
    }

    /// Return the number of tics recorded so far.
    #[must_use]
    pub fn tic_count(&self) -> usize {
        self.tics.len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lmp::{LmpHeader, LMP_VERSION};
    use doom_game::TicCmd;

    fn singleplayer_recorder() -> DemoRecorder {
        DemoRecorder::new(LmpHeader::new_singleplayer(3, 1, 1))
    }

    #[test]
    fn recorder_empty_demo_has_header_and_sentinel() {
        let rec = singleplayer_recorder();
        let bytes = rec.to_bytes().expect("to_bytes should succeed");
        // 13 header bytes + 1 sentinel = 14
        assert_eq!(bytes.len(), 14);
    }

    #[test]
    fn recorder_single_tic() {
        let mut rec = singleplayer_recorder();
        rec.record_tic(&TicCmd::default());
        let bytes = rec.to_bytes().expect("to_bytes should succeed");
        // 13 header + 4 tic + 1 sentinel = 18
        assert_eq!(bytes.len(), 18);
    }

    #[test]
    fn recorder_tic_count() {
        let mut rec = singleplayer_recorder();
        for _ in 0..5 {
            rec.record_tic(&TicCmd::default());
        }
        assert_eq!(rec.tic_count(), 5);
    }

    #[test]
    fn recorder_bytes_start_with_header() {
        let rec = singleplayer_recorder();
        let bytes = rec.to_bytes().expect("to_bytes should succeed");
        assert_eq!(bytes[0], LMP_VERSION);
    }
}
