//! The Chronos time-manipulation subsystem.
//!
//! It acts as a buffer of GameState snapshots, allowing the
//! simulation to rewind time, similar to games like Braid.

use crate::snapshot::Snapshot;
use crate::state::GameState;
use std::collections::VecDeque;

/// The current direction of time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFlow {
    /// Normal simulation progression.
    Forward,
    /// Popping snapshots from history to reverse time.
    Rewind,
}

/// A buffer for storing GameState snapshots to enable time rewinding.
#[derive(Debug, Clone)]
pub struct Chronos {
    /// The stored history of snapshots.
    pub history: VecDeque<Snapshot>,
    /// The maximum number of snapshots to retain.
    pub max_history: usize,
    /// The current direction of time.
    pub flow: TimeFlow,
}

impl Chronos {
    /// Initializes a new Chronos tracker.
    pub fn new(max_history: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
            flow: TimeFlow::Forward,
        }
    }

    /// Switch to rewind mode.
    pub fn start_rewind(&mut self) {
        self.flow = TimeFlow::Rewind;
    }

    /// Switch to forward mode.
    pub fn stop_rewind(&mut self) {
        self.flow = TimeFlow::Forward;
    }

    /// Process a game frame.
    /// If moving forward, saves the current state.
    /// If rewinding, pops the last state and restores it,
    /// unless history is empty, in which case it switches to forward.
    pub fn process(&mut self, state: &mut GameState) {
        match self.flow {
            TimeFlow::Forward => {
                if self.history.len() >= self.max_history {
                    self.history.pop_front();
                }
                self.history.push_back(state.save_snapshot());
            }
            TimeFlow::Rewind => {
                if let Some(snap) = self.history.pop_back() {
                    state.restore_snapshot(snap);
                } else {
                    self.flow = TimeFlow::Forward;
                }
            }
        }
    }

    /// Clear the snapshot history.
    pub fn clear(&mut self) {
        self.history.clear();
        self.flow = TimeFlow::Forward;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chronos_forward_caps_history() {
        let mut chronos = Chronos::new(2);
        let mut state = GameState::new("E1M1");

        state.tic_num = 1;
        chronos.process(&mut state);
        state.tic_num = 2;
        chronos.process(&mut state);
        state.tic_num = 3;
        chronos.process(&mut state);

        assert_eq!(chronos.history.len(), 2);
        assert_eq!(chronos.history.front().unwrap().tic_num(), 2);
        assert_eq!(chronos.history.back().unwrap().tic_num(), 3);
    }

    #[test]
    fn test_chronos_rewind_restores_state() {
        let mut chronos = Chronos::new(10);
        let mut state = GameState::new("E1M1");

        state.tic_num = 1;
        chronos.process(&mut state);
        state.tic_num = 2;
        chronos.process(&mut state);

        chronos.start_rewind();
        chronos.process(&mut state);

        // Popped the latest state (tic 2)
        assert_eq!(state.tic_num, 2);

        chronos.process(&mut state);
        // Popped the previous state (tic 1)
        assert_eq!(state.tic_num, 1);

        chronos.process(&mut state);
        // History empty, switches to forward
        assert_eq!(chronos.flow, TimeFlow::Forward);
    }
}
