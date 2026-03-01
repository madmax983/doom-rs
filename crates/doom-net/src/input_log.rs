//! Ring-buffered player input log for rollback netcode.
//!
//! Stores the last [`crate::rollback::MAX_ROLLBACK_TICS`] `WireTicCmd`s for
//! each of up to [`MAX_PLAYERS`] players.  Indexing is
//! `tic % MAX_ROLLBACK_TICS`, matching the `SnapshotRing` layout so that
//! snapshot and input log slots stay in sync.

use crate::packet::WireTicCmd;
use crate::rollback::MAX_ROLLBACK_TICS;

/// Maximum number of players in a multiplayer session.
pub use crate::packet::MAX_PLAYERS;

// ---------------------------------------------------------------------------
// InputLog
// ---------------------------------------------------------------------------

/// Ring buffer of player inputs for the rollback window.
///
/// Stores the last [`MAX_ROLLBACK_TICS`] inputs for each of up to
/// [`MAX_PLAYERS`] players.  Ring index: `tic % MAX_ROLLBACK_TICS`.
pub struct InputLog {
    /// `cmds[player][slot]` — `None` = slot not yet written.
    cmds: [[Option<WireTicCmd>; MAX_ROLLBACK_TICS]; MAX_PLAYERS],
    /// `tic_nums[player][slot]` — sentinel `u32::MAX` = not written.
    tic_nums: [[u32; MAX_ROLLBACK_TICS]; MAX_PLAYERS],
}

impl InputLog {
    /// Create an empty log (all slots vacant).
    #[must_use]
    pub fn new() -> Self {
        Self {
            cmds:     [[None; MAX_ROLLBACK_TICS]; MAX_PLAYERS],
            tic_nums: [[u32::MAX; MAX_ROLLBACK_TICS]; MAX_PLAYERS],
        }
    }

    /// Store a command for `player` at `tic`, evicting any older occupant.
    ///
    /// Silently ignores out-of-bounds player indices.
    pub fn store(&mut self, player: usize, tic: u32, cmd: WireTicCmd) {
        if player >= MAX_PLAYERS {
            return;
        }
        let slot = (tic as usize) % MAX_ROLLBACK_TICS;
        self.cmds[player][slot]     = Some(cmd);
        self.tic_nums[player][slot] = tic;
    }

    /// Retrieve the command for `player` at `tic`, if available.
    ///
    /// Returns `None` if the slot has been evicted, never written, or
    /// `player` is out of bounds.
    #[must_use]
    pub fn get(&self, player: usize, tic: u32) -> Option<WireTicCmd> {
        if player >= MAX_PLAYERS {
            return None;
        }
        let slot = (tic as usize) % MAX_ROLLBACK_TICS;
        if self.tic_nums[player][slot] == tic {
            self.cmds[player][slot]
        } else {
            None
        }
    }

    /// Returns `true` if inputs are available for all `num_players` players at `tic`.
    ///
    /// `num_players` is clamped to [`MAX_PLAYERS`].
    #[must_use]
    pub fn is_tic_complete(&self, tic: u32, num_players: usize) -> bool {
        let count = num_players.min(MAX_PLAYERS);
        (0..count).all(|p| self.get(p, tic).is_some())
    }

    /// Returns `true` if player `player` has a recorded input for every tic
    /// in the inclusive range `lo..=hi`.
    ///
    /// Returns `false` if `player` is out of bounds or any tic in the range
    /// has been evicted or never written.
    #[must_use]
    pub fn is_contiguous(&self, player: usize, lo: u32, hi: u32) -> bool {
        if player >= MAX_PLAYERS {
            return false;
        }
        (lo..=hi).all(|t| self.get(player, t).is_some())
    }
}

impl Default for InputLog {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(forward: i8) -> WireTicCmd {
        WireTicCmd {
            forward_move: forward,
            ..WireTicCmd::default()
        }
    }

    #[test]
    fn input_log_new_empty() {
        let log = InputLog::new();
        assert!(log.get(0, 0).is_none(), "fresh log must return None for any (player, tic)");
    }

    #[test]
    fn input_log_store_and_get() {
        let mut log = InputLog::new();
        log.store(0, 10, cmd(50));
        let result = log.get(0, 10);
        assert!(result.is_some(), "get(0, 10) must return Some after store");
        assert_eq!(result.unwrap().forward_move, 50);
    }

    #[test]
    fn input_log_wrong_player_returns_none() {
        let mut log = InputLog::new();
        log.store(0, 10, cmd(50));
        assert!(
            log.get(1, 10).is_none(),
            "get(1, 10) must return None when only player 0 was stored"
        );
    }

    #[test]
    fn input_log_out_of_bounds_player() {
        let log = InputLog::new();
        // Must not panic for player indices beyond MAX_PLAYERS.
        assert!(log.get(MAX_PLAYERS + 5, 0).is_none());
    }

    #[test]
    fn input_log_is_tic_complete_true() {
        let mut log = InputLog::new();
        log.store(0, 3, cmd(1));
        log.store(1, 3, cmd(2));
        assert!(
            log.is_tic_complete(3, 2),
            "tic 3 must be complete when both players have inputs"
        );
    }

    #[test]
    fn input_log_is_tic_complete_false() {
        let mut log = InputLog::new();
        log.store(0, 3, cmd(1));
        // Player 1 missing at tic 3.
        assert!(
            !log.is_tic_complete(3, 2),
            "tic 3 must not be complete when player 1 is missing"
        );
    }

    #[test]
    fn input_log_contiguous_true() {
        let mut log = InputLog::new();
        log.store(0, 5, cmd(1));
        log.store(0, 6, cmd(2));
        log.store(0, 7, cmd(3));
        assert!(
            log.is_contiguous(0, 5, 7),
            "tics 5..=7 must be contiguous when all three are stored"
        );
    }

    #[test]
    fn input_log_contiguous_false() {
        let mut log = InputLog::new();
        log.store(0, 5, cmd(1));
        // tic 6 is missing.
        log.store(0, 7, cmd(3));
        assert!(
            !log.is_contiguous(0, 5, 7),
            "tics 5..=7 must not be contiguous when tic 6 is missing"
        );
    }
}
