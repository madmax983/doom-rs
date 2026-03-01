//! Snapshot ring buffer for rollback netcode.
//!
//! Stores the last [`MAX_ROLLBACK_TICS`] `GameState` snapshots in a fixed-size
//! ring. Indexing is `tic % MAX_ROLLBACK_TICS`, so writing tic N automatically
//! evicts the snapshot from tic `N - MAX_ROLLBACK_TICS`.
//!
//! # Invariants
//! - A slot is valid only if `tic_nums[slot] == tic`.
//! - `u32::MAX` is the sentinel meaning "slot not yet written".

use doom_game::GameState;

/// Number of tics the ring can hold simultaneously.
///
/// Eight tics at 35 Hz ≈ 229 ms of rollback window — sufficient for
/// typical LAN play and early internet connections.
pub const MAX_ROLLBACK_TICS: usize = 8;

// ---------------------------------------------------------------------------
// SnapshotRing
// ---------------------------------------------------------------------------

/// Fixed-size ring buffer holding the last [`MAX_ROLLBACK_TICS`] `GameState` snapshots.
///
/// Ring index: `tic % MAX_ROLLBACK_TICS`.
pub struct SnapshotRing {
    /// Stored states. `None` = slot not yet written.
    states: [Option<GameState>; MAX_ROLLBACK_TICS],
    /// Tic number stored in each slot. Used to verify slot validity.
    /// Sentinel value `u32::MAX` means the slot has never been written.
    tic_nums: [u32; MAX_ROLLBACK_TICS],
}

impl SnapshotRing {
    /// Create an empty ring (all slots vacant).
    #[must_use]
    pub fn new() -> Self {
        Self {
            states:   std::array::from_fn(|_| None),
            tic_nums: [u32::MAX; MAX_ROLLBACK_TICS],
        }
    }

    /// Save a snapshot of `state` at `tic`, evicting any older occupant of
    /// the same ring slot.
    pub fn save(&mut self, tic: u32, state: &GameState) {
        let slot = (tic as usize) % MAX_ROLLBACK_TICS;
        self.states[slot]   = Some(state.clone());
        self.tic_nums[slot] = tic;
    }

    /// Restore the `GameState` for `tic`, or `None` if not in the ring.
    ///
    /// Returns `None` if the slot has been overwritten by a later tic or
    /// has never been written.
    #[must_use]
    pub fn restore(&self, tic: u32) -> Option<&GameState> {
        let slot = (tic as usize) % MAX_ROLLBACK_TICS;
        if self.tic_nums[slot] == tic {
            self.states[slot].as_ref()
        } else {
            None
        }
    }

    /// The oldest tic currently stored (for determining the rollback window).
    ///
    /// Returns `None` if the ring is empty.
    #[must_use]
    pub fn oldest_valid_tic(&self) -> Option<u32> {
        self.tic_nums
            .iter()
            .filter(|&&t| t != u32::MAX)
            .copied()
            .min()
    }
}

impl Default for SnapshotRing {
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

    fn make_state(tic: u32) -> GameState {
        let mut gs = GameState::new("test");
        gs.tic_num = tic;
        gs
    }

    #[test]
    fn snapshot_ring_new_is_empty() {
        let ring = SnapshotRing::new();
        assert!(ring.restore(0).is_none(), "fresh ring must return None for any tic");
    }

    #[test]
    fn snapshot_ring_save_and_restore() {
        let mut ring = SnapshotRing::new();
        let state = make_state(5);
        ring.save(5, &state);

        let restored = ring.restore(5);
        assert!(restored.is_some(), "restore(5) must return Some after save(5)");
        assert_eq!(restored.unwrap().tic_num, 5);
        assert!(ring.restore(6).is_none(), "restore(6) must return None — never saved");
    }

    #[test]
    fn snapshot_ring_overwrites_old_slot() {
        let mut ring = SnapshotRing::new();
        // tic 0 and tic 8 map to the same slot (0 % 8 == 8 % 8 == 0).
        ring.save(0, &make_state(0));
        assert!(ring.restore(0).is_some());

        ring.save(8, &make_state(8));
        assert!(ring.restore(0).is_none(), "tic 0 must be evicted after tic 8 overwrites slot");
        let r = ring.restore(8);
        assert!(r.is_some(), "tic 8 must be present");
        assert_eq!(r.unwrap().tic_num, 8);
    }

    #[test]
    fn snapshot_ring_oldest_valid() {
        let mut ring = SnapshotRing::new();
        ring.save(3, &make_state(3));
        ring.save(5, &make_state(5));

        assert_eq!(
            ring.oldest_valid_tic(),
            Some(3),
            "oldest valid tic must be 3 when tics 3 and 5 are stored"
        );
    }

    #[test]
    fn snapshot_ring_covers_window() {
        let mut ring = SnapshotRing::new();
        for tic in 0..MAX_ROLLBACK_TICS as u32 {
            ring.save(tic, &make_state(tic));
        }
        for tic in 0..MAX_ROLLBACK_TICS as u32 {
            assert!(
                ring.restore(tic).is_some(),
                "tic {tic} must be restorable after filling all slots"
            );
        }
    }
}
