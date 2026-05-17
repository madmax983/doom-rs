//! Chronos time-manipulation system.
//!
//! Provides a gameplay mechanic to rewind time using the engine's snapshot system.
//! This allows the player to survive an otherwise fatal blow, returning to a
//! safe state from a few seconds ago, similar to Prince of Persia or Braid.

use crate::snapshot::Snapshot;
use crate::state::GameState;
use std::collections::VecDeque;

/// The Chronos system maintains a sliding window of recent game states.
#[derive(Clone, Debug)]
pub struct Chronos {
    history: VecDeque<Snapshot>,
    /// Number of tics between snapshots (e.g., 35 = 1 second)
    interval: u32,
    /// Maximum number of snapshots to keep
    max_snapshots: usize,
    /// Tic of the last snapshot
    last_snapshot_tic: u32,
}

impl Default for Chronos {
    fn default() -> Self {
        Self::new(35, 5) // 1 snapshot per second, max 5 seconds of history
    }
}

impl Chronos {
    /// Creates a new Chronos instance.
    pub fn new(interval: u32, max_snapshots: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_snapshots),
            interval,
            max_snapshots,
            last_snapshot_tic: 0,
        }
    }

    /// Called every tic to potentially record a snapshot.
    pub fn tick(&mut self, gs: &GameState) {
        if gs.tic_num >= self.last_snapshot_tic.saturating_add(self.interval) {
            if self.history.len() >= self.max_snapshots {
                self.history.pop_front();
            }
            self.history.push_back(gs.save_snapshot());
            self.last_snapshot_tic = gs.tic_num;
        }
    }

    /// Rewinds the game state to the oldest available snapshot.
    /// Returns true if a rewind occurred.
    pub fn rewind(&mut self, gs: &mut GameState) -> bool {
        if let Some(old_snap) = self.history.pop_front() {
            let target_tic = old_snap.tic_num();
            gs.restore_snapshot(old_snap);
            // After restoring, gs.tic_num will be the old tic!
            // We must clear history so we don't rewind into the future
            self.history.clear();
            self.last_snapshot_tic = target_tic;
            true
        } else {
            false
        }
    }

    /// Checks if the player is dead, and if so, automatically rewinds time.
    /// Returns true if a fatal blow was prevented.
    pub fn auto_rewind_on_death(&mut self, gs: &mut GameState) -> bool {
        if gs.player.health() <= 0 {
            return self.rewind(gs);
        }
        false
    }

    /// Returns the number of snapshots currently stored.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chronos_records_history() {
        let mut gs = GameState::new("E1M1");
        let mut chronos = Chronos::new(10, 3);

        gs.tic_num = 10;
        chronos.tick(&gs);
        assert_eq!(chronos.history_len(), 1);

        gs.tic_num = 20;
        chronos.tick(&gs);
        assert_eq!(chronos.history_len(), 2);

        gs.tic_num = 30;
        chronos.tick(&gs);
        assert_eq!(chronos.history_len(), 3);

        gs.tic_num = 40;
        chronos.tick(&gs);
        // Max is 3, so oldest should be evicted
        assert_eq!(chronos.history_len(), 3);
    }

    #[test]
    fn test_chronos_rewind() {
        let mut gs = GameState::new("E1M1");
        let mut chronos = Chronos::new(10, 3);

        gs.tic_num = 10;
        chronos.tick(&gs);

        gs.tic_num = 20;
        gs.damage_player(50); // player takes 50 damage
        let current_health = gs.player.health();

        // Rewind to tic 10!
        let success = chronos.rewind(&mut gs);
        assert!(success);
        assert_eq!(gs.tic_num, 10);
        // Player should be back to full health
        assert!(gs.player.health() > current_health);
        assert_eq!(chronos.history_len(), 0); // History cleared after rewind
    }

    #[test]
    fn test_chronos_auto_rewind_on_death() {
        let mut gs = GameState::new("E1M1");
        let mut chronos = Chronos::new(10, 3);

        gs.tic_num = 10;
        chronos.tick(&gs);

        gs.tic_num = 20;
        gs.damage_player(1000); // Fatal damage!
        assert!(gs.player.health() <= 0);

        let saved = chronos.auto_rewind_on_death(&mut gs);
        assert!(saved);
        // Player is alive again!
        assert!(gs.player.health() > 0);
        assert_eq!(gs.tic_num, 10);
    }
}
