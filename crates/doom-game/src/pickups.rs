//! Item pickup system — `p_check_pickups`.
//!
//! Called each tic to check whether the player has walked over any
//! `MF_SPECIAL` actor.  If so, the item's effect is applied and the
//! actor is removed from the slab (consumed).

use crate::mobj::{MobjHandle, MobjKind};
use crate::player::{AmmoType, WeaponType};
use crate::state::GameState;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check every MF_SPECIAL actor in the slab.
///
/// If the player's AABB overlaps it, pick it up (apply effect, free actor).
/// Dead players cannot pick up items.
pub fn p_check_pickups(gs: &mut GameState) {
    // Dead players skip pickups (caller should guard, but double-check).
    if gs.player.is_dead() {
        return;
    }

    // Get player position and radius.
    let (px, py, pradius) = match gs.mobjslab.get(gs.player.handle) {
        Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.radius.to_int()),
        None => return,
    };

    // Collect all MF_SPECIAL actor handles (to avoid borrow conflicts).
    let specials: Vec<MobjHandle> = gs
        .mobjslab
        .iter_handles()
        .filter(|&h| {
            gs.mobjslab
                .get(h)
                .map(|mo| mo.flags & crate::mobj::flags::MF_SPECIAL != 0)
                .unwrap_or(false)
        })
        .collect();

    for handle in specials {
        // Skip the player's own mobj if it somehow has MF_SPECIAL.
        if handle == gs.player.handle {
            continue;
        }

        // Extract item position and radius.
        let (ix, iy, item_radius, kind) = match gs.mobjslab.get(handle) {
            Some(mo) => (mo.x.to_int(), mo.y.to_int(), mo.radius.to_int(), mo.kind),
            None => continue,
        };

        // AABB overlap: combined-radius square check.
        let combined_radius = pradius + item_radius;
        let dx = (px - ix).abs();
        let dy = (py - iy).abs();

        if dx < combined_radius && dy < combined_radius {
            // Apply the item's effect.
            apply_pickup(gs, kind);
            // Free (consume) the actor.
            gs.mobjslab.free(handle);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal: apply item effect
// ---------------------------------------------------------------------------

fn apply_pickup(gs: &mut GameState, kind: MobjKind) {
    match kind {
        // ---- Ammo ----
        MobjKind::Clip => {
            gs.player.give_ammo(AmmoType::Bullets as usize, 10);
        }
        MobjKind::ClipBox => {
            gs.player.give_ammo(AmmoType::Bullets as usize, 50);
        }
        MobjKind::Shell => {
            gs.player.give_ammo(AmmoType::Shells as usize, 4);
        }
        MobjKind::ShellBox => {
            gs.player.give_ammo(AmmoType::Shells as usize, 20);
        }
        MobjKind::RocketAmmo => {
            gs.player.give_ammo(AmmoType::Rockets as usize, 1);
        }
        MobjKind::RocketBox => {
            gs.player.give_ammo(AmmoType::Rockets as usize, 5);
        }
        MobjKind::Cell => {
            gs.player.give_ammo(AmmoType::Cells as usize, 20);
        }
        MobjKind::CellPack => {
            gs.player.give_ammo(AmmoType::Cells as usize, 100);
        }

        // ---- Health ----
        MobjKind::HealthBonus => {
            gs.player.heal(1);
        }
        MobjKind::Stimpack => {
            gs.player.heal(10);
        }
        MobjKind::Medikit => {
            gs.player.heal(25);
        }
        MobjKind::Soulsphere => {
            // Soulsphere: overheal up to 200 hp, gives 100 hp.
            gs.player.heal_overheal(100, 200);
        }
        MobjKind::Megasphere => {
            // Megasphere: set health to 200, give 200 blue armor.
            gs.player.set_health_capped(200, 200);
            gs.player.give_armor(200, 2);
        }

        // ---- Armor ----
        MobjKind::ArmorBonus => {
            gs.player.give_armor(gs.player.armor() + 1, gs.player.armor_type.max(1));
        }
        MobjKind::GreenArmor => {
            gs.player.give_armor(100, 1);
        }
        MobjKind::BlueArmor => {
            gs.player.give_armor(200, 2);
        }

        // ---- Weapons ----
        MobjKind::Shotgun => {
            gs.player.weapons[WeaponType::Shotgun as usize] = true;
            gs.player.give_ammo(AmmoType::Shells as usize, 8);
        }
        MobjKind::SuperShotgun => {
            gs.player.weapons[WeaponType::SuperShotgun as usize] = true;
            gs.player.give_ammo(AmmoType::Shells as usize, 8);
        }
        MobjKind::Chaingun => {
            gs.player.weapons[WeaponType::Chaingun as usize] = true;
            gs.player.give_ammo(AmmoType::Bullets as usize, 20);
        }
        MobjKind::RocketLauncher => {
            gs.player.weapons[WeaponType::RocketLauncher as usize] = true;
            gs.player.give_ammo(AmmoType::Rockets as usize, 2);
        }
        MobjKind::PlasmaRifle => {
            gs.player.weapons[WeaponType::PlasmaRifle as usize] = true;
            gs.player.give_ammo(AmmoType::Cells as usize, 40);
        }
        MobjKind::BfgPickup => {
            gs.player.weapons[WeaponType::Bfg as usize] = true;
            gs.player.give_ammo(AmmoType::Cells as usize, 40);
        }
        MobjKind::Chainsaw => {
            gs.player.weapons[WeaponType::Chainsaw as usize] = true;
        }
        MobjKind::Berserk => {
            gs.player.weapons[WeaponType::Fist as usize] = true;
            gs.player.heal(100);
        }

        // ---- Keys ----
        MobjKind::BlueCard => {
            gs.player.give_key(crate::player::KEY_BLUE_CARD);
        }
        MobjKind::YellowCard => {
            gs.player.give_key(crate::player::KEY_YELLOW_CARD);
        }
        MobjKind::RedCard => {
            gs.player.give_key(crate::player::KEY_RED_CARD);
        }
        MobjKind::BlueSkull => {
            gs.player.give_key(crate::player::KEY_BLUE_SKULL);
        }
        MobjKind::YellowSkull => {
            gs.player.give_key(crate::player::KEY_YELLOW_SKULL);
        }
        MobjKind::RedSkull => {
            gs.player.give_key(crate::player::KEY_RED_SKULL);
        }

        // Everything else: decorations, power-ups not yet implemented.
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, MobjKind, flags};
    use crate::player::PlayerState;
    use doom_types::{Bam, Fixed16_16};

    /// Construct a game state with a live player Mobj at the origin.
    fn make_game_state() -> GameState {
        let mut gs = GameState::new("test");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = crate::mobj::flags::MF_SOLID | crate::mobj::flags::MF_SHOOTABLE;
        mo.radius = Fixed16_16::from_int(16);
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs
    }

    /// Spawn a MF_SPECIAL item at (x, y) with radius 20.
    fn spawn_item(gs: &mut GameState, kind: MobjKind, x: i32, y: i32) -> MobjHandle {
        let mut mo = Mobj::new(
            kind,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.flags = flags::MF_SPECIAL;
        mo.radius = Fixed16_16::from_int(20);
        gs.mobjslab.alloc(mo)
    }

    #[test]
    fn pickup_clip_gives_10_bullets() {
        let mut gs = make_game_state();
        let bullets_before = gs.player.ammo(AmmoType::Bullets as usize);
        spawn_item(&mut gs, MobjKind::Clip, 0, 0);

        p_check_pickups(&mut gs);

        let bullets_after = gs.player.ammo(AmmoType::Bullets as usize);
        assert_eq!(
            bullets_after,
            bullets_before + 10,
            "Clip should give 10 bullets"
        );
    }

    #[test]
    fn pickup_medikit_heals_25hp() {
        let mut gs = make_game_state();
        // Damage player to 50 hp first.
        gs.player.apply_damage(50);
        assert_eq!(gs.player.health(), 50);

        spawn_item(&mut gs, MobjKind::Medikit, 0, 0);
        p_check_pickups(&mut gs);

        assert_eq!(gs.player.health(), 75, "Medikit should heal 25 hp");
    }

    #[test]
    fn pickup_shotgun_grants_weapon_and_shells() {
        let mut gs = make_game_state();
        assert!(!gs.player.weapons[WeaponType::Shotgun as usize]);
        let shells_before = gs.player.ammo(AmmoType::Shells as usize);

        spawn_item(&mut gs, MobjKind::Shotgun, 0, 0);
        p_check_pickups(&mut gs);

        assert!(
            gs.player.weapons[WeaponType::Shotgun as usize],
            "Should have shotgun after pickup"
        );
        assert!(
            gs.player.ammo(AmmoType::Shells as usize) > shells_before,
            "Should have shells after pickup"
        );
    }

    #[test]
    fn item_consumed_after_pickup() {
        let mut gs = make_game_state();
        let handle = spawn_item(&mut gs, MobjKind::Clip, 0, 0);

        p_check_pickups(&mut gs);

        assert!(
            gs.mobjslab.get(handle).is_none(),
            "Item handle should be freed after pickup"
        );
    }

    #[test]
    fn out_of_range_item_not_consumed() {
        let mut gs = make_game_state();
        // Player is at origin with radius 16; item at 200,200 is far out of range.
        let handle = spawn_item(&mut gs, MobjKind::Clip, 200, 200);

        p_check_pickups(&mut gs);

        assert!(
            gs.mobjslab.get(handle).is_some(),
            "Out-of-range item should not be consumed"
        );
    }

    #[test]
    fn dead_player_does_not_pick_up() {
        let mut gs = make_game_state();
        // Kill the player.
        gs.player.apply_damage(200);
        assert!(gs.player.is_dead());

        let handle = spawn_item(&mut gs, MobjKind::Clip, 0, 0);
        p_check_pickups(&mut gs);

        assert!(
            gs.mobjslab.get(handle).is_some(),
            "Dead player should not consume items"
        );
    }

    #[test]
    fn pickup_shellbox_gives_20_shells() {
        let mut gs = make_game_state();
        let shells_before = gs.player.ammo(AmmoType::Shells as usize);
        spawn_item(&mut gs, MobjKind::ShellBox, 0, 0);

        p_check_pickups(&mut gs);

        assert_eq!(
            gs.player.ammo(AmmoType::Shells as usize),
            shells_before + 20,
            "ShellBox should give 20 shells"
        );
    }

    #[test]
    fn pickup_soulsphere_overheals_to_200() {
        let mut gs = make_game_state();
        // Player starts at 100 hp (MAX_HEALTH).
        assert_eq!(gs.player.health(), 100);

        spawn_item(&mut gs, MobjKind::Soulsphere, 0, 0);
        p_check_pickups(&mut gs);

        assert_eq!(gs.player.health(), 200, "Soulsphere should overheal to 200");
    }

    #[test]
    fn pickup_green_armor() {
        let mut gs = make_game_state();
        assert_eq!(gs.player.armor(), 0);

        spawn_item(&mut gs, MobjKind::GreenArmor, 0, 0);
        p_check_pickups(&mut gs);

        assert_eq!(gs.player.armor(), 100);
        assert_eq!(gs.player.armor_type, 1);
    }

    #[test]
    fn pickup_cell_gives_20_cells() {
        let mut gs = make_game_state();
        let cells_before = gs.player.ammo(AmmoType::Cells as usize);
        spawn_item(&mut gs, MobjKind::Cell, 0, 0);

        p_check_pickups(&mut gs);

        assert_eq!(
            gs.player.ammo(AmmoType::Cells as usize),
            cells_before + 20,
            "Cell should give 20 cells"
        );
    }

    #[test]
    fn pickup_rocket_ammo_gives_1_rocket() {
        let mut gs = make_game_state();
        let rockets_before = gs.player.ammo(AmmoType::Rockets as usize);
        spawn_item(&mut gs, MobjKind::RocketAmmo, 0, 0);

        p_check_pickups(&mut gs);

        assert_eq!(
            gs.player.ammo(AmmoType::Rockets as usize),
            rockets_before + 1,
            "RocketAmmo should give 1 rocket"
        );
    }

    #[test]
    fn pickup_blue_card_gives_key() {
        let mut gs = make_game_state();
        assert!(!gs.player.has_key(crate::player::KEY_BLUE_CARD));

        spawn_item(&mut gs, MobjKind::BlueCard, 0, 0);
        p_check_pickups(&mut gs);

        assert!(
            gs.player.has_key(crate::player::KEY_BLUE_CARD),
            "Player should have blue card after pickup"
        );
    }

    #[test]
    fn pickup_red_skull_gives_key() {
        let mut gs = make_game_state();
        assert!(!gs.player.has_key(crate::player::KEY_RED_SKULL));

        spawn_item(&mut gs, MobjKind::RedSkull, 0, 0);
        p_check_pickups(&mut gs);

        assert!(
            gs.player.has_key(crate::player::KEY_RED_SKULL),
            "Player should have red skull after pickup"
        );
    }
}
