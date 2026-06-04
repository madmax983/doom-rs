//! Snapshot ring buffer for rollback netcode.
//!
//! [`SnapshotRing`] stores the last N game states in a fixed-size ring,
//! indexed by `tic % capacity`.  Writing tic N automatically evicts the
//! snapshot from tic `N - capacity`.
//!
//! The type parameter `S` is generic so that doom-net does not depend on
//! `doom-game::GameState` directly.

use crate::packet::MAX_ROLLBACK_TICS;

// ---------------------------------------------------------------------------
// SnapshotRing
// ---------------------------------------------------------------------------

/// Fixed-size ring buffer of `(tic, state)` pairs for rollback.
///
/// Capacity defaults to [`MAX_ROLLBACK_TICS`] but can be set at construction.
#[derive(Debug, Clone)]
pub struct SnapshotRing<S: Clone> {
    /// Each slot holds an optional `(tic_num, state)` pair.
    ring: Vec<Option<(u32, S)>>,
    /// Maximum number of snapshots stored simultaneously.
    capacity: usize,
}

impl<S: Clone> SnapshotRing<S> {
    /// Create an empty ring with the given `capacity`.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "SnapshotRing capacity must be > 0");
        let cap = capacity.min(1024);
        let mut ring = Vec::with_capacity(cap);
        ring.resize_with(cap, || None);
        Self {
            ring,
            capacity: cap,
        }
    }

    /// Create a ring with the default capacity ([`MAX_ROLLBACK_TICS`]).
    #[must_use]
    pub fn with_default_capacity() -> Self {
        Self::new(MAX_ROLLBACK_TICS)
    }

    /// Save `state` at `tic`, evicting any older occupant of the same slot.
    pub fn save(&mut self, tic: u32, state: S) {
        let slot = (tic as usize) % self.capacity;
        self.ring[slot] = Some((tic, state));
    }

    /// Retrieve the state for `tic`, or `None` if not in the ring.
    ///
    /// Returns `None` if the slot has been overwritten by a later tic or
    /// was never written.
    #[must_use]
    pub fn get(&self, tic: u32) -> Option<&S> {
        let slot = (tic as usize) % self.capacity;
        match &self.ring[slot] {
            Some((stored_tic, state)) if *stored_tic == tic => Some(state),
            _ => None,
        }
    }

    /// The most recent tic number stored in the ring, or `None` if empty.
    #[must_use]
    pub fn latest_tic(&self) -> Option<u32> {
        self.ring
            .iter()
            .filter_map(|slot| slot.as_ref().map(|(tic, _)| *tic))
            .max()
    }

    /// Clear all slots, resetting the ring to empty.
    pub fn clear(&mut self) {
        for slot in &mut self.ring {
            *slot = None;
        }
    }

    /// The capacity of this ring.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_default_capacity_works() {
        let snap: SnapshotRing<u32> = SnapshotRing::with_default_capacity();
        assert_eq!(snap.capacity, crate::packet::MAX_ROLLBACK_TICS);
    }

    #[test]
    fn new_creates_empty_ring() {
        let ring: SnapshotRing<u64> = SnapshotRing::new(8);
        assert!(ring.get(0).is_none(), "fresh ring must return None");
        assert!(ring.get(7).is_none(), "fresh ring must return None");
        assert!(ring.latest_tic().is_none(), "fresh ring has no latest tic");
    }

    #[test]
    fn save_then_get_returns_state() {
        let mut ring: SnapshotRing<String> = SnapshotRing::new(8);
        ring.save(5, "hello".to_string());
        let got = ring.get(5);
        assert!(got.is_some());
        assert_eq!(got.expect("value must exist in test"), "hello");
    }

    #[test]
    fn get_wrong_tic_returns_none() {
        let mut ring: SnapshotRing<u32> = SnapshotRing::new(8);
        ring.save(5, 100);
        assert!(ring.get(6).is_none(), "get with wrong tic must return None");
        assert!(
            ring.get(13).is_none(),
            "tic 13 maps to same slot as 5, but stored tic differs"
        );
    }

    #[test]
    fn save_overwrites_old_entry_at_same_slot() {
        let mut ring: SnapshotRing<u32> = SnapshotRing::new(8);
        ring.save(0, 100);
        assert_eq!(ring.get(0), Some(&100));

        // tic 8 maps to slot 0 (8 % 8 == 0), evicts tic 0.
        ring.save(8, 200);
        assert!(ring.get(0).is_none(), "tic 0 must be evicted");
        assert_eq!(ring.get(8), Some(&200));
    }

    #[test]
    fn latest_tic_tracks_most_recent() {
        let mut ring: SnapshotRing<u32> = SnapshotRing::new(8);
        ring.save(3, 10);
        assert_eq!(ring.latest_tic(), Some(3));

        ring.save(7, 20);
        assert_eq!(ring.latest_tic(), Some(7));

        ring.save(1, 30);
        assert_eq!(
            ring.latest_tic(),
            Some(7),
            "latest_tic must be 7 (highest stored)"
        );
    }

    #[test]
    fn clear_empties_all_slots() {
        let mut ring: SnapshotRing<u32> = SnapshotRing::new(8);
        for tic in 0..8u32 {
            ring.save(tic, tic * 10);
        }
        assert!(ring.latest_tic().is_some());

        ring.clear();

        for tic in 0..8u32 {
            assert!(
                ring.get(tic).is_none(),
                "tic {tic} must be None after clear"
            );
        }
        assert!(
            ring.latest_tic().is_none(),
            "latest_tic must be None after clear"
        );
    }

    #[test]
    fn capacity_is_correct() {
        let ring: SnapshotRing<u32> = SnapshotRing::new(16);
        assert_eq!(ring.capacity(), 16);
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn zero_capacity_panics() {
        let _ring: SnapshotRing<u32> = SnapshotRing::new(0);
    }
}
