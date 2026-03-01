//! Mobj state machine table — sparse subset covering core E1 monsters.
//!
//! Full Doom has ~900 state entries.  This table covers IDLE (A_Look) and
//! CHASE (A_Chase) states for the 8 standard monsters, sufficient to drive
//! the Batch 2 AI.  Pain and death states are wired in Batch 3.
//!
//! # Layout
//! Entry 0 is always `S_NULL` (terminal / hold-forever state).
//! Within each monster group: STND (idle/look loop) → RUN1 ↔ RUN2 (2-frame
//! chase loop, each 4 tics).

use crate::mobj::{MobjStateEntry, StateNum};

// ---------------------------------------------------------------------------
// Action index constants (must match `actions::ACTION_*`)
// ---------------------------------------------------------------------------
const NONE: u8 = 0;
const LOOK: u8 = 1;
const CHASE: u8 = 2;

// ---------------------------------------------------------------------------
// State ID constants
// ---------------------------------------------------------------------------

/// Named indices into the `STATES` table.
pub mod ids {
    // --- Sentinel ---
    pub const S_NULL: u16 = 0;

    // --- Trooper (Zombie Man) ---
    pub const S_POSS_STND: u16 = 1; // idle: tics=10, A_Look, loops to self
    pub const S_POSS_RUN1: u16 = 2; // chase frame 1: tics=4, A_Chase, →RUN2
    pub const S_POSS_RUN2: u16 = 3; // chase frame 2: tics=4, A_Chase, →RUN1

    // --- Sergeant (Shotgun Guy) ---
    pub const S_SPOS_STND: u16 = 4;
    pub const S_SPOS_RUN1: u16 = 5;
    pub const S_SPOS_RUN2: u16 = 6;

    // --- Imp ---
    pub const S_TROO_STND: u16 = 7;
    pub const S_TROO_RUN1: u16 = 8;
    pub const S_TROO_RUN2: u16 = 9;

    // --- Demon (Pink Demon) ---
    pub const S_SARG_STND: u16 = 10;
    pub const S_SARG_RUN1: u16 = 11;
    pub const S_SARG_RUN2: u16 = 12;

    // --- Cacodemon ---
    pub const S_HEAD_STND: u16 = 13;
    pub const S_HEAD_RUN1: u16 = 14;
    pub const S_HEAD_RUN2: u16 = 15;

    // --- Baron of Hell ---
    pub const S_BOSS_STND: u16 = 16;
    pub const S_BOSS_RUN1: u16 = 17;
    pub const S_BOSS_RUN2: u16 = 18;

    // --- Cyberdemon ---
    pub const S_CYBER_STND: u16 = 19;
    pub const S_CYBER_RUN1: u16 = 20;
    pub const S_CYBER_RUN2: u16 = 21;

    // --- Spider Mastermind ---
    pub const S_SPID_STND: u16 = 22;
    pub const S_SPID_RUN1: u16 = 23;
    pub const S_SPID_RUN2: u16 = 24;

    /// Total number of entries in the `STATES` table.
    pub const STATES_COUNT: usize = 25;
}

// ---------------------------------------------------------------------------
// Global state table
// ---------------------------------------------------------------------------

/// The global Mobj state machine table.
///
/// Index with `StateNum(n)`.  Entry 0 (`S_NULL`) is the terminal/dead state.
/// Action fires on state *entry* (when `tics` reaches 0 and the state
/// transitions to `next_state`).
pub static STATES: &[MobjStateEntry] = &[
    // 0: S_NULL — hold forever, no action
    MobjStateEntry { tics: -1, next_state: StateNum(ids::S_NULL),      action: NONE  },

    // --- Trooper ---
    // 1: S_POSS_STND — idle, try to see player every 10 tics
    MobjStateEntry { tics: 10, next_state: StateNum(ids::S_POSS_STND), action: LOOK  },
    // 2: S_POSS_RUN1 — chase frame 1
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_POSS_RUN2), action: CHASE },
    // 3: S_POSS_RUN2 — chase frame 2 (loops back)
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_POSS_RUN1), action: CHASE },

    // --- Sergeant ---
    MobjStateEntry { tics: 10, next_state: StateNum(ids::S_SPOS_STND), action: LOOK  },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_SPOS_RUN2), action: CHASE },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_SPOS_RUN1), action: CHASE },

    // --- Imp ---
    MobjStateEntry { tics: 10, next_state: StateNum(ids::S_TROO_STND), action: LOOK  },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_TROO_RUN2), action: CHASE },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_TROO_RUN1), action: CHASE },

    // --- Demon ---
    MobjStateEntry { tics: 10, next_state: StateNum(ids::S_SARG_STND), action: LOOK  },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_SARG_RUN2), action: CHASE },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_SARG_RUN1), action: CHASE },

    // --- Cacodemon ---
    MobjStateEntry { tics: 10, next_state: StateNum(ids::S_HEAD_STND), action: LOOK  },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_HEAD_RUN2), action: CHASE },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_HEAD_RUN1), action: CHASE },

    // --- Baron of Hell ---
    MobjStateEntry { tics: 10, next_state: StateNum(ids::S_BOSS_STND), action: LOOK  },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_BOSS_RUN2), action: CHASE },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_BOSS_RUN1), action: CHASE },

    // --- Cyberdemon ---
    MobjStateEntry { tics: 10, next_state: StateNum(ids::S_CYBER_STND), action: LOOK  },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_CYBER_RUN2), action: CHASE },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_CYBER_RUN1), action: CHASE },

    // --- Spider Mastermind ---
    MobjStateEntry { tics: 10, next_state: StateNum(ids::S_SPID_STND), action: LOOK  },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_SPID_RUN2), action: CHASE },
    MobjStateEntry { tics:  4, next_state: StateNum(ids::S_SPID_RUN1), action: CHASE },
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions;

    #[test]
    fn s_null_holds_forever() {
        let e = &STATES[ids::S_NULL as usize];
        assert_eq!(e.tics, -1);
        assert_eq!(e.next_state, StateNum(ids::S_NULL));
        assert_eq!(e.action, actions::ACTION_NONE);
    }

    #[test]
    fn poss_stnd_loops_to_self_with_look() {
        let e = &STATES[ids::S_POSS_STND as usize];
        assert_eq!(e.tics, 10);
        assert_eq!(e.next_state, StateNum(ids::S_POSS_STND));
        assert_eq!(e.action, actions::ACTION_LOOK);
    }

    #[test]
    fn poss_run_cycles_with_chase() {
        let run1 = &STATES[ids::S_POSS_RUN1 as usize];
        let run2 = &STATES[ids::S_POSS_RUN2 as usize];
        assert_eq!(run1.next_state, StateNum(ids::S_POSS_RUN2));
        assert_eq!(run2.next_state, StateNum(ids::S_POSS_RUN1));
        assert_eq!(run1.action, actions::ACTION_CHASE);
        assert_eq!(run2.action, actions::ACTION_CHASE);
    }

    #[test]
    fn table_length_matches_count() {
        assert_eq!(STATES.len(), ids::STATES_COUNT);
    }

    #[test]
    fn all_next_state_indices_in_bounds() {
        for (i, e) in STATES.iter().enumerate() {
            let next = e.next_state.0 as usize;
            assert!(
                next < STATES.len(),
                "state {i} has next_state {next} out of bounds"
            );
        }
    }
}
