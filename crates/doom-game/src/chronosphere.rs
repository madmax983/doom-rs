use crate::snapshot::Snapshot;
use crate::state::GameState;
use std::collections::VecDeque;

/// Allows rewinding time by storing state snapshots.
pub struct Chronosphere {
    snapshots: VecDeque<Snapshot>,
    max_capacity: usize,
}

impl Chronosphere {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            snapshots: VecDeque::with_capacity(max_capacity),
            max_capacity,
        }
    }

    pub fn record(&mut self, state: &GameState) {
        if self.snapshots.len() == self.max_capacity {
            self.snapshots.pop_front();
        }
        self.snapshots.push_back(state.save_snapshot());
    }

    pub fn rewind(&mut self, state: &mut GameState, frames: usize) {
        for _ in 0..frames {
            if let Some(snap) = self.snapshots.pop_back() {
                state.restore_snapshot(snap);
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::GameState;

    #[test]
    fn test_rewind() {
        let mut chrono = Chronosphere::new(10);
        let mut gs = GameState::new("E1M1");
        gs.tic_num = 1;
        chrono.record(&gs);
        gs.tic_num = 2;
        chrono.record(&gs);
        gs.tic_num = 3;
        chrono.rewind(&mut gs, 1);
        assert_eq!(gs.tic_num, 2);
        chrono.rewind(&mut gs, 1);
        assert_eq!(gs.tic_num, 1);
    }
}
