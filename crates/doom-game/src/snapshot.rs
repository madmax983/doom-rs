//! Rollback snapshot — save and restore `GameState` for netcode.
//!
//! `GameState` derives `Clone` (deep copy, no shared state), so a snapshot
//! is simply a full copy.  The `Snapshot` newtype makes intent explicit at
//! call sites and prevents accidentally passing a `GameState` where a
//! snapshot is expected.
//!
//! # Rollback flow (see Phase 7 for the full implementation)
//! ```text
//! // Save before uncertain tick:
//! let snap = gs.save_snapshot();
//!
//! // Later, if rollback needed:
//! gs.restore_snapshot(snap);
//! ```

use crate::state::GameState;

/// An opaque rollback snapshot of a complete `GameState`.
///
/// Created by [`GameState::save_snapshot`], consumed by
/// [`GameState::restore_snapshot`].  Contains a full deep copy.
#[derive(Clone, Debug)]
pub struct Snapshot(GameState);

impl Snapshot {
    /// Inspect the tic number this snapshot was taken at.
    pub fn tic_num(&self) -> u32 {
        self.0.tic_num
    }
}

impl GameState {
    /// Save the current state as a rollback snapshot.
    ///
    /// This is `O(n)` in the number of live actors.  For netcode,
    /// `SnapshotRing` (Phase 7) manages a circular buffer of these.
    pub fn save_snapshot(&self) -> Snapshot {
        Snapshot(self.clone())
    }

    /// Restore a previously saved snapshot, discarding current state.
    ///
    /// After restoration, `self` is identical to the state at the time
    /// `save_snapshot` was called.
    pub fn restore_snapshot(&mut self, snapshot: Snapshot) {
        *self = snapshot.0;
    }
}

// ---------------------------------------------------------------------------
// Proptest property tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    /// Property: save → restore roundtrip preserves `tic_num`.
    ///
    /// `Snapshot` does not implement `PartialEq`, so we compare individual
    /// observable fields rather than the struct itself.
    proptest! {
        #[test]
        fn snapshot_roundtrip_preserves_tic_num(tic in 0u32..=u32::MAX) {
            let mut gs = GameState::new("E1M1");
            gs.tic_num = tic;
            let snap = gs.save_snapshot();
            // Mutate the live state before restoring.
            gs.tic_num = gs.tic_num.wrapping_add(1);
            gs.restore_snapshot(snap);
            prop_assert_eq!(gs.tic_num, tic, "tic_num must be restored exactly");
        }

        /// Property: save → restore roundtrip preserves the RNG index.
        #[test]
        fn snapshot_roundtrip_preserves_rng_index(advances in 0u32..=255u32) {
            let mut gs = GameState::new("E1M1");
            for _ in 0..advances {
                gs.rng.next();
            }
            let saved_idx = gs.rng.index();
            let snap = gs.save_snapshot();
            // Advance RNG further after snapshot.
            for _ in 0..10 {
                gs.rng.next();
            }
            gs.restore_snapshot(snap);
            prop_assert_eq!(
                gs.rng.index(), saved_idx,
                "RNG index must be restored to {}", saved_idx
            );
        }

        /// Property: save → restore roundtrip preserves kill/item/secret counts.
        #[test]
        fn snapshot_roundtrip_preserves_counters(
            kills in 0u32..=1000u32,
            items in 0u32..=1000u32,
            secrets in 0u32..=100u32,
        ) {
            let mut gs = GameState::new("E1M1");
            gs.kill_count   = kills;
            gs.item_count   = items;
            gs.secret_count = secrets;
            let snap = gs.save_snapshot();
            // Corrupt live state.
            gs.kill_count   = 0;
            gs.item_count   = 0;
            gs.secret_count = 0;
            gs.restore_snapshot(snap);
            prop_assert_eq!(gs.kill_count,   kills,   "kill_count mismatch");
            prop_assert_eq!(gs.item_count,   items,   "item_count mismatch");
            prop_assert_eq!(gs.secret_count, secrets, "secret_count mismatch");
        }

        /// Property: snapshot tic_num accessor matches the tic at save time.
        #[test]
        fn snapshot_tic_accessor_matches_save_time(tic in 0u32..=u32::MAX) {
            let mut gs = GameState::new("E1M1");
            gs.tic_num = tic;
            let snap = gs.save_snapshot();
            prop_assert_eq!(snap.tic_num(), tic);
        }

        /// Property: two independent snapshots are truly independent — restoring
        /// the first must not be affected by what snap2 captured.
        #[test]
        fn two_snapshots_are_independent(tic1 in 0u32..=500u32, tic2 in 501u32..=1000u32) {
            let mut gs = GameState::new("E1M1");
            gs.tic_num = tic1;
            let snap1 = gs.save_snapshot();
            gs.tic_num = tic2;
            let snap2 = gs.save_snapshot();

            // Restore snap1 — tic must revert to tic1.
            gs.restore_snapshot(snap1);
            prop_assert_eq!(gs.tic_num, tic1);

            // Restore snap2 — tic must revert to tic2.
            gs.restore_snapshot(snap2);
            prop_assert_eq!(gs.tic_num, tic2);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind};
    use doom_types::{Bam, Fixed16_16};

    #[test]
    fn snapshot_preserves_tic_num() {
        let mut gs = GameState::new("E1M1");
        gs.tic_num = 77;
        let snap = gs.save_snapshot();
        gs.tic_num = 999;
        gs.restore_snapshot(snap);
        assert_eq!(gs.tic_num, 77);
    }

    #[test]
    fn snapshot_is_deep_copy_of_actors() {
        let mut gs = GameState::new("E1M1");
        let mo = Mobj::new(MobjKind::Player, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
        let handle = gs.mobjslab.alloc(mo);

        let snap = gs.save_snapshot();

        // Mutate the live state after snapshot.
        gs.mobjslab.get_mut(handle).unwrap().health = 50;
        gs.restore_snapshot(snap);

        // Should be restored to 0 (initial health in Mobj::new).
        assert_eq!(
            gs.mobjslab.get(handle).unwrap().health,
            0,
            "health must revert to snapshot value"
        );
    }

    #[test]
    fn snapshot_captures_rng_index() {
        let mut gs = GameState::new("E1M1");
        for _ in 0..42 {
            gs.rng.next();
        }
        let snap = gs.save_snapshot();
        // Advance rng further.
        for _ in 0..10 {
            gs.rng.next();
        }
        gs.restore_snapshot(snap);
        assert_eq!(gs.rng.index(), 42);
    }

    #[test]
    fn restore_brings_back_freed_actor() {
        let mut gs = GameState::new("E1M1");
        let mo = Mobj::new(MobjKind::Imp, Fixed16_16::from_int(100), Fixed16_16::ZERO, Bam::ZERO);
        let handle = gs.mobjslab.alloc(mo);

        let snap = gs.save_snapshot();

        // Free the actor after snapshot.
        gs.mobjslab.free(handle);
        assert!(gs.mobjslab.get(handle).is_none());

        gs.restore_snapshot(snap);

        // Actor must be alive again after restore.
        assert!(
            gs.mobjslab.get(handle).is_some(),
            "actor must survive snapshot/restore"
        );
    }

    #[test]
    fn snapshot_tic_num_accessor() {
        let mut gs = GameState::new("E1M1");
        gs.tic_num = 55;
        let snap = gs.save_snapshot();
        assert_eq!(snap.tic_num(), 55);
    }

    #[test]
    fn multiple_snapshots_are_independent() {
        let mut gs = GameState::new("E1M1");
        gs.tic_num = 10;
        let snap1 = gs.save_snapshot();
        gs.tic_num = 20;
        let snap2 = gs.save_snapshot();

        gs.restore_snapshot(snap1);
        assert_eq!(gs.tic_num, 10);

        gs.tic_num = 99;
        // snap2 was cloned when saved, not referencing live state.
        gs.restore_snapshot(snap2);
        assert_eq!(gs.tic_num, 20);
    }
}
