//! Vampirism feature for healing the player on melee damage.
//!
//! This module allows the player to steal health from enemies when dealing damage
//! using melee weapons, encouraging high-risk close-quarters combat.

use crate::mobj::MobjHandle;
use crate::state::GameState;
use doom_types::weapons::WeaponType;

/// Process vampiric healing if the player dealt melee damage.
pub fn process_vampirism(gs: &mut GameState, inflictor: MobjHandle, damage: i32) {
    if damage <= 0 {
        return;
    }

    // Check if the source of the damage is the player
    if inflictor == gs.player.handle {
        // Only heal on melee attacks
        let is_melee = matches!(gs.player.weapon, WeaponType::Fist | WeaponType::Chainsaw);

        if is_melee {
            // Heal 50% of the damage dealt, minimum 1
            let heal_amount = (damage / 2).max(1);
            gs.heal_player(heal_amount);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::Mobj;
    use crate::player::PlayerState;
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    #[test]
    fn test_vampirism_heals_player() {
        let mut gs = GameState::new("E1M1");

        // Setup player
        let mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs.player.set_health_capped(50, 100);
        gs.player.weapon = WeaponType::Fist;
        gs.sync_player_mobj_health();

        // Process vampirism
        process_vampirism(&mut gs, handle, 20);

        // Should heal 10
        assert_eq!(gs.player.health(), 60);

        // Process vampirism with non-melee
        gs.player.weapon = WeaponType::Pistol;
        process_vampirism(&mut gs, handle, 20);

        // Should not heal
        assert_eq!(gs.player.health(), 60);
    }
}
