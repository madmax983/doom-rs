//! Time-travel debugging and snapshot timeline management.
//!
//! The `Chronos` module tracks game state snapshots over time, allowing
//! for rewind and replay functionality without duplicating state into
//! the core `GameState` itself.

use crate::snapshot::Snapshot;
use std::collections::VecDeque;

/// Manages a sliding window timeline of game state snapshots.
pub struct Chronos {
    timeline: VecDeque<Snapshot>,
    max_capacity: usize,
}

impl Chronos {
    /// Creates a new Chronos instance with a maximum timeline capacity.
    pub fn new(max_capacity: usize) -> Self {
        Self {
            timeline: VecDeque::with_capacity(max_capacity),
            max_capacity,
        }
    }

    /// Captures and stores a new snapshot in the timeline.
    pub fn capture(&mut self, snapshot: Snapshot) {
        if self.timeline.len() >= self.max_capacity {
            self.timeline.pop_front();
        }
        self.timeline.push_back(snapshot);
    }

    /// Rewinds the timeline by one snapshot, returning it if available.
    pub fn rewind(&mut self) -> Option<Snapshot> {
        self.timeline.pop_back()
    }

    /// Returns the number of snapshots currently stored.
    pub fn len(&self) -> usize {
        self.timeline.len()
    }

    /// Returns true if the timeline is empty.
    pub fn is_empty(&self) -> bool {
        self.timeline.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;

    #[test]
    fn test_chronos_capture_and_rewind() {
        let mut chronos = Chronos::new(2);
        let mut gs = GameState::new("E1M1");

        gs.tic_num = 1;
        chronos.capture(gs.save_snapshot());

        gs.tic_num = 2;
        chronos.capture(gs.save_snapshot());

        gs.tic_num = 3;
        chronos.capture(gs.save_snapshot()); // Should evict tic 1

        assert_eq!(chronos.len(), 2);

        let snap2 = chronos.rewind().unwrap();
        assert_eq!(snap2.tic_num(), 3);

        let snap1 = chronos.rewind().unwrap();
        assert_eq!(snap1.tic_num(), 2);

        assert!(chronos.rewind().is_none());
    }
}
