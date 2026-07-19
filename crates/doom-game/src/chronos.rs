use crate::state::GameState;
use crate::snapshot::Snapshot;
use std::collections::VecDeque;

pub struct ChronosSystem {
    history: VecDeque<Snapshot>,
    max_frames: usize,
}

impl ChronosSystem {
    pub fn new(max_frames: usize) -> Self {
        Self {
            history: VecDeque::with_capacity(max_frames),
            max_frames,
        }
    }

    pub fn record(&mut self, state: &GameState) {
        if self.history.len() >= self.max_frames {
            self.history.pop_front();
        }
        self.history.push_back(state.save_snapshot());
    }

    pub fn rewind(&mut self) -> Option<Snapshot> {
        self.history.pop_back()
    }
}

impl Default for ChronosSystem {
    fn default() -> Self {
        Self::new(35 * 5) // 5 seconds of history at 35 tic/s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;

    #[test]
    fn test_chronos_rewind() {
        let mut chronos = ChronosSystem::new(10);
        let mut state = GameState::new("E1M1");
        state.tic_num = 1;
        chronos.record(&state);

        let mut state2 = GameState::new("E1M1");
        state2.tic_num = 2;
        chronos.record(&state2);

        let rewind_state = chronos.rewind().expect("Should be able to rewind");
        assert_eq!(rewind_state.tic_num(), 2);
    }
}
