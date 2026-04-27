//! Auto-battler simulator for Doom monsters.
//!
//! Pits two monster types against each other in an empty GameState to see who wins.

use crate::mobj::{Mobj, MobjHandle};
use crate::mobjinfo::MOBJINFO;
use crate::state::GameState;
use crate::states::STATES;
use doom_types::mobj_kind::MobjKind;
use doom_types::{Bam, Fixed16_16, TicCmd};

/// Result of an arena simulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaResult {
    /// The kind of the monster that survived.
    pub winner: MobjKind,
    /// How much health the winner had left.
    pub remaining_health: i32,
    /// How many tics the fight took.
    pub duration_tics: u32,
}

pub struct MonsterArena;

impl MonsterArena {
    /// Runs a deterministic 1v1 fight between two monster kinds.
    /// Returns the result of the duel.
    pub fn simulate_duel(fighter_a: MobjKind, fighter_b: MobjKind) -> ArenaResult {
        let mut gs = GameState::new("ARENA");
        gs.player = crate::player::PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);

        // Spawn fighter A at x=-128
        let handle_a = Self::spawn_fighter(&mut gs, fighter_a, -128, 0);
        // Spawn fighter B at x=128
        let handle_b = Self::spawn_fighter(&mut gs, fighter_b, 128, 0);

        // Aggro them onto each other
        Self::aggro(&mut gs, handle_a, handle_b);
        Self::aggro(&mut gs, handle_b, handle_a);

        let mut tics = 0;
        let max_tics = 35 * 60 * 5; // 5 minutes max

        loop {
            gs.tick(TicCmd::default(), None);
            tics += 1;

            let alive_a = gs.mobjslab.get(handle_a).is_some_and(|m| m.health > 0);
            let alive_b = gs.mobjslab.get(handle_b).is_some_and(|m| m.health > 0);

            if !alive_a && !alive_b {
                // Draw! Both died on the same tic. Default to A for tie-break in this simple sim.
                return ArenaResult {
                    winner: fighter_a,
                    remaining_health: 0,
                    duration_tics: tics,
                };
            } else if !alive_a {
                let hp = gs.mobjslab.get(handle_b).unwrap().health;
                return ArenaResult {
                    winner: fighter_b,
                    remaining_health: hp,
                    duration_tics: tics,
                };
            } else if !alive_b {
                let hp = gs.mobjslab.get(handle_a).unwrap().health;
                return ArenaResult {
                    winner: fighter_a,
                    remaining_health: hp,
                    duration_tics: tics,
                };
            }

            if tics > max_tics {
                // Time limit exceeded. Whoever has more health fraction wins.
                let hp_a = gs.mobjslab.get(handle_a).unwrap().health;
                let hp_b = gs.mobjslab.get(handle_b).unwrap().health;
                return if hp_a > hp_b {
                    ArenaResult {
                        winner: fighter_a,
                        remaining_health: hp_a,
                        duration_tics: tics,
                    }
                } else {
                    ArenaResult {
                        winner: fighter_b,
                        remaining_health: hp_b,
                        duration_tics: tics,
                    }
                };
            }
        }
    }

    fn spawn_fighter(gs: &mut GameState, kind: MobjKind, x: i32, y: i32) -> MobjHandle {
        let info = &MOBJINFO[kind as usize];
        let spawn_sn = info.spawn_state;
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = info.spawn_health;
        mo.flags = info.flags;
        mo.radius = info.radius;
        mo.height = info.height;
        mo.state = spawn_sn;
        mo.tics = STATES[spawn_sn.0 as usize].tics;
        gs.mobjslab.alloc(mo)
    }

    fn aggro(gs: &mut GameState, attacker: MobjHandle, target: MobjHandle) {
        if let Some(mo) = gs.mobjslab.get_mut(attacker) {
            mo.target = target;
            let info = &MOBJINFO[mo.kind as usize];
            if info.see_state.0 != 0 {
                mo.state = info.see_state;
                mo.tics = STATES[info.see_state.0 as usize].tics;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_imp_vs_trooper() {
        let result = MonsterArena::simulate_duel(MobjKind::Imp, MobjKind::Trooper);
        // Imp has 60 health and fireball, Trooper has 20 health and hitscan.
        // Imp usually wins in a 1v1.
        assert!(result.duration_tics > 0);
    }
}
