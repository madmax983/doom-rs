use crate::snapshot::Snapshot;
use crate::state::GameState;
use std::collections::VecDeque;

/// A buffer that stores `GameState` snapshots to allow rewinding time.
pub struct RewindBuffer {
    /// History of snapshots, where the front is the oldest and the back is the newest.
    pub history: VecDeque<Snapshot>,
    /// Maximum number of snapshots to keep in memory.
    pub max_frames: usize,
}

impl RewindBuffer {
    /// Create a new `RewindBuffer` with a specified maximum capacity.
    #[must_use]
    pub fn new(max_frames: usize) -> Self {
        Self {
            history: VecDeque::new(),
            max_frames,
        }
    }

    /// Record the current state into the rewind buffer.
    /// If the buffer is at capacity, the oldest snapshot is dropped.
    pub fn record(&mut self, state: &GameState) {
        if self.max_frames == 0 {
            return;
        }
        if self.history.len() >= self.max_frames {
            self.history.pop_front();
        }
        self.history.push_back(state.save_snapshot());
    }

    /// Rewind time by one recorded step, modifying the provided `GameState`.
    /// Returns `true` if a step was rewound, `false` if the buffer was empty.
    pub fn rewind_one_step(&mut self, state: &mut GameState) -> bool {
        if let Some(snap) = self.history.pop_back() {
            state.restore_snapshot(snap);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;

    #[test]
    fn test_rewind_buffer() {
        let mut gs = GameState::new("E1M1");
        let mut buffer = RewindBuffer::new(2);

        gs.tic_num = 1;
        buffer.record(&gs);
        gs.tic_num = 2;
        buffer.record(&gs);
        gs.tic_num = 3;
        buffer.record(&gs);

        assert_eq!(buffer.history.len(), 2);

        let mut rewound_gs = GameState::new("E1M1");
        assert!(buffer.rewind_one_step(&mut rewound_gs));
        assert_eq!(rewound_gs.tic_num, 3);

        assert!(buffer.rewind_one_step(&mut rewound_gs));
        assert_eq!(rewound_gs.tic_num, 2);

        assert!(!buffer.rewind_one_step(&mut rewound_gs));
    }
}
