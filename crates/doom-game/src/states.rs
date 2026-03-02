//! Mobj state machine table — sparse subset covering core E1 monsters.
//!
//! Full Doom has ~900 state entries.  This table covers IDLE (A_Look),
//! CHASE (A_Chase), ATTACK, pain, and death states for the 8 standard
//! monsters.
//!
//! # Layout
//! Entry 0 is always `S_NULL` (terminal / hold-forever state).
//! Within each monster group: STND (idle/look loop) → RUN1 ↔ RUN2 (2-frame
//! chase loop, each 4 tics).  Attack states: ATK1 (windup) → ATK2 (fire)
//! → ATK3 (recovery) → RUN1.

use crate::mobj::{MobjStateEntry, StateNum};

// ---------------------------------------------------------------------------
// Action index constants (must match `actions::ACTION_*`)
// ---------------------------------------------------------------------------
const NONE: u8 = 0;
const LOOK: u8 = 1;
const CHASE: u8 = 2;
const POS_ATTACK: u8 = 3;
const SPOS_ATTACK: u8 = 4;
const TROO_ATTACK: u8 = 5;
const SARG_ATTACK: u8 = 6;

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

    // -----------------------------------------------------------------------
    // Death and pain states (Batch 3)
    // Death: 2 frames — DIE1 (active, 8 tics) → DIE2 (holds at S_NULL forever)
    // Pain:  1 frame  — PAIN (6 tics) → returns to spawn_state (STND)
    // -----------------------------------------------------------------------

    // --- Trooper ---
    pub const S_POSS_DIE1: u16 = 25;
    pub const S_POSS_DIE2: u16 = 26;
    pub const S_POSS_PAIN: u16 = 27;

    // --- Sergeant ---
    pub const S_SPOS_DIE1: u16 = 28;
    pub const S_SPOS_DIE2: u16 = 29;
    pub const S_SPOS_PAIN: u16 = 30;

    // --- Imp ---
    pub const S_TROO_DIE1: u16 = 31;
    pub const S_TROO_DIE2: u16 = 32;
    pub const S_TROO_PAIN: u16 = 33;

    // --- Demon ---
    pub const S_SARG_DIE1: u16 = 34;
    pub const S_SARG_DIE2: u16 = 35;
    pub const S_SARG_PAIN: u16 = 36;

    // --- Cacodemon ---
    pub const S_HEAD_DIE1: u16 = 37;
    pub const S_HEAD_DIE2: u16 = 38;
    pub const S_HEAD_PAIN: u16 = 39;

    // --- Baron of Hell ---
    pub const S_BOSS_DIE1: u16 = 40;
    pub const S_BOSS_DIE2: u16 = 41;
    pub const S_BOSS_PAIN: u16 = 42;

    // --- Cyberdemon ---
    pub const S_CYBER_DIE1: u16 = 43;
    pub const S_CYBER_DIE2: u16 = 44;
    pub const S_CYBER_PAIN: u16 = 45;

    // --- Spider Mastermind ---
    pub const S_SPID_DIE1: u16 = 46;
    pub const S_SPID_DIE2: u16 = 47;
    pub const S_SPID_PAIN: u16 = 48;

    // -----------------------------------------------------------------------
    // Attack states (Batch 5)
    // ATK1 = windup (4 tics, no action) → ATK2
    // ATK2 = fire   (4 tics, attack action) → ATK3
    // ATK3 = recover (4 tics, no action) → RUN1
    // -----------------------------------------------------------------------

    // --- Trooper attack ---
    pub const S_POSS_ATK1: u16 = 49;
    pub const S_POSS_ATK2: u16 = 50;
    pub const S_POSS_ATK3: u16 = 51;

    // --- Sergeant attack ---
    pub const S_SPOS_ATK1: u16 = 52;
    pub const S_SPOS_ATK2: u16 = 53;
    pub const S_SPOS_ATK3: u16 = 54;

    // --- Imp attack ---
    pub const S_TROO_ATK1: u16 = 55;
    pub const S_TROO_ATK2: u16 = 56;
    pub const S_TROO_ATK3: u16 = 57;

    // --- Demon attack ---
    pub const S_SARG_ATK1: u16 = 58;
    pub const S_SARG_ATK2: u16 = 59;
    pub const S_SARG_ATK3: u16 = 60;

    /// Total number of entries in the `STATES` table.
    pub const STATES_COUNT: usize = 61;
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
    MobjStateEntry {
        tics: -1,
        next_state: StateNum(ids::S_NULL),
        action: NONE,
    },
    // --- Trooper ---
    // 1: S_POSS_STND — idle, try to see player every 10 tics
    MobjStateEntry {
        tics: 10,
        next_state: StateNum(ids::S_POSS_STND),
        action: LOOK,
    },
    // 2: S_POSS_RUN1 — chase frame 1
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_POSS_RUN2),
        action: CHASE,
    },
    // 3: S_POSS_RUN2 — chase frame 2 (loops back)
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_POSS_RUN1),
        action: CHASE,
    },
    // --- Sergeant ---
    MobjStateEntry {
        tics: 10,
        next_state: StateNum(ids::S_SPOS_STND),
        action: LOOK,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SPOS_RUN2),
        action: CHASE,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SPOS_RUN1),
        action: CHASE,
    },
    // --- Imp ---
    MobjStateEntry {
        tics: 10,
        next_state: StateNum(ids::S_TROO_STND),
        action: LOOK,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_TROO_RUN2),
        action: CHASE,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_TROO_RUN1),
        action: CHASE,
    },
    // --- Demon ---
    MobjStateEntry {
        tics: 10,
        next_state: StateNum(ids::S_SARG_STND),
        action: LOOK,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SARG_RUN2),
        action: CHASE,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SARG_RUN1),
        action: CHASE,
    },
    // --- Cacodemon ---
    MobjStateEntry {
        tics: 10,
        next_state: StateNum(ids::S_HEAD_STND),
        action: LOOK,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_HEAD_RUN2),
        action: CHASE,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_HEAD_RUN1),
        action: CHASE,
    },
    // --- Baron of Hell ---
    MobjStateEntry {
        tics: 10,
        next_state: StateNum(ids::S_BOSS_STND),
        action: LOOK,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_BOSS_RUN2),
        action: CHASE,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_BOSS_RUN1),
        action: CHASE,
    },
    // --- Cyberdemon ---
    MobjStateEntry {
        tics: 10,
        next_state: StateNum(ids::S_CYBER_STND),
        action: LOOK,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_CYBER_RUN2),
        action: CHASE,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_CYBER_RUN1),
        action: CHASE,
    },
    // --- Spider Mastermind ---
    MobjStateEntry {
        tics: 10,
        next_state: StateNum(ids::S_SPID_STND),
        action: LOOK,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SPID_RUN2),
        action: CHASE,
    },
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SPID_RUN1),
        action: CHASE,
    },
    // -----------------------------------------------------------------------
    // Death and pain states
    // -----------------------------------------------------------------------

    // --- Trooper death / pain ---
    // 25: S_POSS_DIE1
    MobjStateEntry {
        tics: 8,
        next_state: StateNum(ids::S_POSS_DIE2),
        action: NONE,
    },
    // 26: S_POSS_DIE2 — holds forever (next_state = S_NULL)
    MobjStateEntry {
        tics: -1,
        next_state: StateNum(ids::S_NULL),
        action: NONE,
    },
    // 27: S_POSS_PAIN — returns to idle
    MobjStateEntry {
        tics: 6,
        next_state: StateNum(ids::S_POSS_STND),
        action: NONE,
    },
    // --- Sergeant death / pain ---
    // 28: S_SPOS_DIE1
    MobjStateEntry {
        tics: 8,
        next_state: StateNum(ids::S_SPOS_DIE2),
        action: NONE,
    },
    // 29: S_SPOS_DIE2
    MobjStateEntry {
        tics: -1,
        next_state: StateNum(ids::S_NULL),
        action: NONE,
    },
    // 30: S_SPOS_PAIN
    MobjStateEntry {
        tics: 6,
        next_state: StateNum(ids::S_SPOS_STND),
        action: NONE,
    },
    // --- Imp death / pain ---
    // 31: S_TROO_DIE1
    MobjStateEntry {
        tics: 8,
        next_state: StateNum(ids::S_TROO_DIE2),
        action: NONE,
    },
    // 32: S_TROO_DIE2
    MobjStateEntry {
        tics: -1,
        next_state: StateNum(ids::S_NULL),
        action: NONE,
    },
    // 33: S_TROO_PAIN
    MobjStateEntry {
        tics: 6,
        next_state: StateNum(ids::S_TROO_STND),
        action: NONE,
    },
    // --- Demon death / pain ---
    // 34: S_SARG_DIE1
    MobjStateEntry {
        tics: 8,
        next_state: StateNum(ids::S_SARG_DIE2),
        action: NONE,
    },
    // 35: S_SARG_DIE2
    MobjStateEntry {
        tics: -1,
        next_state: StateNum(ids::S_NULL),
        action: NONE,
    },
    // 36: S_SARG_PAIN
    MobjStateEntry {
        tics: 6,
        next_state: StateNum(ids::S_SARG_STND),
        action: NONE,
    },
    // --- Cacodemon death / pain ---
    // 37: S_HEAD_DIE1
    MobjStateEntry {
        tics: 8,
        next_state: StateNum(ids::S_HEAD_DIE2),
        action: NONE,
    },
    // 38: S_HEAD_DIE2
    MobjStateEntry {
        tics: -1,
        next_state: StateNum(ids::S_NULL),
        action: NONE,
    },
    // 39: S_HEAD_PAIN
    MobjStateEntry {
        tics: 6,
        next_state: StateNum(ids::S_HEAD_STND),
        action: NONE,
    },
    // --- Baron of Hell death / pain ---
    // 40: S_BOSS_DIE1
    MobjStateEntry {
        tics: 8,
        next_state: StateNum(ids::S_BOSS_DIE2),
        action: NONE,
    },
    // 41: S_BOSS_DIE2
    MobjStateEntry {
        tics: -1,
        next_state: StateNum(ids::S_NULL),
        action: NONE,
    },
    // 42: S_BOSS_PAIN
    MobjStateEntry {
        tics: 6,
        next_state: StateNum(ids::S_BOSS_STND),
        action: NONE,
    },
    // --- Cyberdemon death / pain ---
    // 43: S_CYBER_DIE1
    MobjStateEntry {
        tics: 8,
        next_state: StateNum(ids::S_CYBER_DIE2),
        action: NONE,
    },
    // 44: S_CYBER_DIE2
    MobjStateEntry {
        tics: -1,
        next_state: StateNum(ids::S_NULL),
        action: NONE,
    },
    // 45: S_CYBER_PAIN
    MobjStateEntry {
        tics: 6,
        next_state: StateNum(ids::S_CYBER_STND),
        action: NONE,
    },
    // --- Spider Mastermind death / pain ---
    // 46: S_SPID_DIE1
    MobjStateEntry {
        tics: 8,
        next_state: StateNum(ids::S_SPID_DIE2),
        action: NONE,
    },
    // 47: S_SPID_DIE2
    MobjStateEntry {
        tics: -1,
        next_state: StateNum(ids::S_NULL),
        action: NONE,
    },
    // 48: S_SPID_PAIN
    MobjStateEntry {
        tics: 6,
        next_state: StateNum(ids::S_SPID_STND),
        action: NONE,
    },
    // -----------------------------------------------------------------------
    // Attack states (Batch 5)
    // -----------------------------------------------------------------------

    // --- Trooper (Zombie Man) attack ---
    // 49: S_POSS_ATK1 — windup (no action)
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_POSS_ATK2),
        action: NONE,
    },
    // 50: S_POSS_ATK2 — fire hitscan
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_POSS_ATK3),
        action: POS_ATTACK,
    },
    // 51: S_POSS_ATK3 — recovery → resume chasing
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_POSS_RUN1),
        action: NONE,
    },
    // --- Sergeant (Shotgun Guy) attack ---
    // 52: S_SPOS_ATK1 — windup
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SPOS_ATK2),
        action: NONE,
    },
    // 53: S_SPOS_ATK2 — fire 3-pellet burst
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SPOS_ATK3),
        action: SPOS_ATTACK,
    },
    // 54: S_SPOS_ATK3 — recovery → resume chasing
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SPOS_RUN1),
        action: NONE,
    },
    // --- Imp attack ---
    // 55: S_TROO_ATK1 — windup
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_TROO_ATK2),
        action: NONE,
    },
    // 56: S_TROO_ATK2 — melee if close, else hitscan
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_TROO_ATK3),
        action: TROO_ATTACK,
    },
    // 57: S_TROO_ATK3 — recovery → resume chasing
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_TROO_RUN1),
        action: NONE,
    },
    // --- Demon attack ---
    // 58: S_SARG_ATK1 — windup
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SARG_ATK2),
        action: NONE,
    },
    // 59: S_SARG_ATK2 — melee only
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SARG_ATK3),
        action: SARG_ATTACK,
    },
    // 60: S_SARG_ATK3 — recovery → resume chasing
    MobjStateEntry {
        tics: 4,
        next_state: StateNum(ids::S_SARG_RUN1),
        action: NONE,
    },
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

    #[test]
    fn trooper_death_state_is_two_frames() {
        let die1 = &STATES[ids::S_POSS_DIE1 as usize];
        let die2 = &STATES[ids::S_POSS_DIE2 as usize];
        // DIE1 must transition to DIE2.
        assert_eq!(die1.next_state, StateNum(ids::S_POSS_DIE2));
        // DIE2 must hold forever (tics = -1) and chain to S_NULL.
        assert_eq!(die2.tics, -1);
        assert_eq!(die2.next_state, StateNum(ids::S_NULL));
    }

    #[test]
    fn pain_state_returns_to_idle() {
        let pain = &STATES[ids::S_POSS_PAIN as usize];
        // Pain state must transition back to the trooper's idle state.
        assert_eq!(pain.next_state, StateNum(ids::S_POSS_STND));
        assert_eq!(pain.tics, 6);
    }

    #[test]
    fn attack_states_return_to_run() {
        // Trooper: ATK3 must return to RUN1.
        let atk3 = &STATES[ids::S_POSS_ATK3 as usize];
        assert_eq!(atk3.next_state, StateNum(ids::S_POSS_RUN1));
        assert_eq!(atk3.action, actions::ACTION_NONE);

        // Sergeant: ATK3 must return to RUN1.
        let spos_atk3 = &STATES[ids::S_SPOS_ATK3 as usize];
        assert_eq!(spos_atk3.next_state, StateNum(ids::S_SPOS_RUN1));

        // Imp: ATK3 must return to RUN1.
        let troo_atk3 = &STATES[ids::S_TROO_ATK3 as usize];
        assert_eq!(troo_atk3.next_state, StateNum(ids::S_TROO_RUN1));

        // Demon: ATK3 must return to RUN1.
        let sarg_atk3 = &STATES[ids::S_SARG_ATK3 as usize];
        assert_eq!(sarg_atk3.next_state, StateNum(ids::S_SARG_RUN1));
    }

    #[test]
    fn attack_atk2_fires_correct_action() {
        assert_eq!(
            STATES[ids::S_POSS_ATK2 as usize].action,
            actions::ACTION_POS_ATTACK
        );
        assert_eq!(
            STATES[ids::S_SPOS_ATK2 as usize].action,
            actions::ACTION_SPOS_ATTACK
        );
        assert_eq!(
            STATES[ids::S_TROO_ATK2 as usize].action,
            actions::ACTION_TROO_ATTACK
        );
        assert_eq!(
            STATES[ids::S_SARG_ATK2 as usize].action,
            actions::ACTION_SARG_ATTACK
        );
    }
}
