//! Per-player input history for rollback netcode.
//!
//! [`InputLog`] stores the last N tics worth of `[TicCmd; MAX_PLAYERS]`
//! arrays in a ring buffer indexed by `tic % capacity`.  Each entry can
//! be marked as *authoritative* (server-confirmed) or *predicted* (local
//! extrapolation).

use crate::packet::{MAX_PLAYERS, MAX_ROLLBACK_TICS};
use doom_types::TicCmd;

// ---------------------------------------------------------------------------
// InputEntry
// ---------------------------------------------------------------------------

/// One tic's worth of inputs for all players, with an authoritative flag.
#[derive(Debug, Clone)]
struct InputEntry {
    /// The tic number this entry was recorded for.
    tic: u32,
    /// Input commands for each player slot.
    cmds: [TicCmd; MAX_PLAYERS],
    /// `true` if these inputs came from the server (authoritative).
    authoritative: bool,
}

// ---------------------------------------------------------------------------
// InputLog
// ---------------------------------------------------------------------------

/// Ring-buffered input history for the rollback window.
///
/// Capacity defaults to [`MAX_ROLLBACK_TICS`] but can be customized.
#[derive(Debug, Clone)]
pub struct InputLog {
    /// Ring of optional entries, indexed by `tic % capacity`.
    log: Vec<Option<InputEntry>>,
    /// Ring capacity.
    capacity: usize,
    /// The oldest tic we have recorded (informational).
    oldest_tic: u32,
}

impl InputLog {
    /// Create an empty log with the given `capacity`.
    ///
    /// # Panics
    /// Panics if `capacity` is 0.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::InputLog;
    ///
    /// let log = InputLog::new(32);
    /// assert_eq!(log.get(0), None);
    /// ```
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "InputLog capacity must be > 0");
        let mut log = Vec::with_capacity(capacity);
        log.resize_with(capacity, || None);
        Self {
            log,
            capacity,
            oldest_tic: 0,
        }
    }

    /// Create a log with the default capacity ([`MAX_ROLLBACK_TICS`]).
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::InputLog;
    ///
    /// let log = InputLog::with_default_capacity();
    /// ```
    #[must_use]
    pub fn with_default_capacity() -> Self {
        Self::new(MAX_ROLLBACK_TICS)
    }

    /// Record inputs for `tic`.  Marks the entry as non-authoritative
    /// (predicted) by default.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::{InputLog, packet::MAX_PLAYERS};
    /// use doom_types::TicCmd;
    ///
    /// let mut log = InputLog::new(8);
    /// let mut cmds = [TicCmd::default(); MAX_PLAYERS];
    /// cmds[0].forward_move = 50;
    ///
    /// log.record(10, cmds);
    /// assert!(!log.has_authoritative(10)); // Just recorded, so it's predicted
    /// ```
    pub fn record(&mut self, tic: u32, cmds: [TicCmd; MAX_PLAYERS]) {
        let slot = (tic as usize) % self.capacity;
        self.log[slot] = Some(InputEntry {
            tic,
            cmds,
            authoritative: false,
        });
    }

    /// Retrieve the inputs for `tic`, if stored and the tic matches.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::{InputLog, packet::MAX_PLAYERS};
    /// use doom_types::TicCmd;
    ///
    /// let mut log = InputLog::new(8);
    /// log.record(5, [TicCmd::default(); MAX_PLAYERS]);
    ///
    /// assert!(log.get(5).is_some());
    /// assert!(log.get(6).is_none());
    /// ```
    #[must_use]
    pub fn get(&self, tic: u32) -> Option<&[TicCmd; MAX_PLAYERS]> {
        let slot = (tic as usize) % self.capacity;
        match &self.log[slot] {
            Some(entry) if entry.tic == tic => Some(&entry.cmds),
            _ => None,
        }
    }

    /// Returns `true` if we have authoritative (server-confirmed) inputs
    /// for `tic`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::{InputLog, packet::MAX_PLAYERS};
    /// use doom_types::TicCmd;
    ///
    /// let mut log = InputLog::new(8);
    /// log.record(1, [TicCmd::default(); MAX_PLAYERS]); // Predicted
    /// assert_eq!(log.has_authoritative(1), false);
    ///
    /// log.set_authoritative(1, [TicCmd::default(); MAX_PLAYERS]); // Server confirmed
    /// assert_eq!(log.has_authoritative(1), true);
    /// ```
    #[must_use]
    pub fn has_authoritative(&self, tic: u32) -> bool {
        let slot = (tic as usize) % self.capacity;
        matches!(&self.log[slot], Some(entry) if entry.tic == tic && entry.authoritative)
    }

    /// Overwrite the entry for `tic` with server-confirmed inputs.
    ///
    /// Marks the entry as authoritative.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_net::{InputLog, packet::MAX_PLAYERS};
    /// use doom_types::TicCmd;
    ///
    /// let mut log = InputLog::new(8);
    /// log.set_authoritative(42, [TicCmd::default(); MAX_PLAYERS]);
    ///
    /// assert!(log.has_authoritative(42));
    /// ```
    pub fn set_authoritative(&mut self, tic: u32, cmds: [TicCmd; MAX_PLAYERS]) {
        let slot = (tic as usize) % self.capacity;
        self.log[slot] = Some(InputEntry {
            tic,
            cmds,
            authoritative: true,
        });
    }

    /// The oldest tic number tracked (informational, updated on record).
    #[must_use]
    pub const fn oldest_tic(&self) -> u32 {
        self.oldest_tic
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cmds(forward: i8) -> [TicCmd; MAX_PLAYERS] {
        let mut cmds = [TicCmd::default(); MAX_PLAYERS];
        cmds[0].forward_move = forward;
        cmds
    }

    #[test]
    fn new_creates_empty_log() {
        let log = InputLog::new(8);
        assert!(log.get(0).is_none(), "fresh log must return None");
        assert!(log.get(7).is_none(), "fresh log must return None");
    }

    #[test]
    fn record_then_get_returns_cmds() {
        let mut log = InputLog::new(8);
        let cmds = make_cmds(42);
        log.record(5, cmds);
        let got = log.get(5);
        assert!(got.is_some());
        assert_eq!(got.unwrap()[0].forward_move, 42);
    }

    #[test]
    fn get_unrecorded_tic_returns_none() {
        let mut log = InputLog::new(8);
        log.record(5, make_cmds(42));
        assert!(log.get(6).is_none(), "unrecorded tic must return None");
    }

    #[test]
    fn set_authoritative_overwrites_predicted() {
        let mut log = InputLog::new(8);
        log.record(3, make_cmds(10));
        assert!(!log.has_authoritative(3), "initially not authoritative");

        let auth_cmds = make_cmds(99);
        log.set_authoritative(3, auth_cmds);
        assert!(log.has_authoritative(3), "must be authoritative after set");
        assert_eq!(
            log.get(3).unwrap()[0].forward_move,
            99,
            "authoritative cmds must overwrite predicted"
        );
    }

    #[test]
    fn has_authoritative_returns_false_for_unrecorded() {
        let log = InputLog::new(8);
        assert!(
            !log.has_authoritative(0),
            "has_authoritative must return false for unrecorded tic"
        );
    }

    #[test]
    fn has_authoritative_returns_false_for_predicted() {
        let mut log = InputLog::new(8);
        log.record(7, make_cmds(1));
        assert!(
            !log.has_authoritative(7),
            "predicted entry must not be authoritative"
        );
    }

    #[test]
    fn slot_collision_evicts_old_entry() {
        let mut log = InputLog::new(8);
        log.record(0, make_cmds(10));
        assert!(log.get(0).is_some());

        // tic 8 collides with slot 0.
        log.record(8, make_cmds(20));
        assert!(log.get(0).is_none(), "tic 0 evicted by tic 8");
        assert_eq!(log.get(8).unwrap()[0].forward_move, 20);
    }

    #[test]
    fn set_authoritative_on_empty_slot() {
        let mut log = InputLog::new(8);
        log.set_authoritative(4, make_cmds(77));
        assert!(log.has_authoritative(4));
        assert_eq!(log.get(4).unwrap()[0].forward_move, 77);
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn zero_capacity_panics() {
        let _log = InputLog::new(0);
    }
}
