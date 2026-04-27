//! Item pickup system -- `p_check_pickups` and `p_touch_special_thing`.
//!
//! Called each tic to check whether the player has walked over any
//! `MF_SPECIAL` actor.  If so, the item's effect is applied and the
//! actor is removed from the slab (consumed).
//!
//! Items that cannot be picked up (e.g. health when already full) are
//! left in the world -- `p_touch_special_thing` returns `false`.

use crate::mobj::{MobjHandle, flags};
use crate::player::{self, powers};
use crate::state::GameState;
use doom_types::mobj_kind::MobjKind;
use doom_types::weapons::AmmoType;
use doom_types::weapons::WeaponType;

// ---------------------------------------------------------------------------
// Power-up duration constants (in tics, 35 tics = 1 second)
// ---------------------------------------------------------------------------

/// Duration for Invulnerability (30 seconds).
const INVULN_TICS: u32 = 30 * 35;

/// Duration for Partial Invisibility (60 seconds).
const INVIS_TICS: u32 = 60 * 35;

/// Duration for Radiation Suit (60 seconds).
const IRONFEET_TICS: u32 = 60 * 35;

/// Duration for Infrared / Light Amplification Visor (120 seconds).
const INFRARED_TICS: u32 = 120 * 35;

/// Berserk strength power lasts until the end of the level (huge value).
const STRENGTH_TICS: u32 = u32::MAX;

// ---------------------------------------------------------------------------
// DoomEd type -> MobjKind mapping
// ---------------------------------------------------------------------------

/// Map a DoomEd thing type number to a `MobjKind`.
///
/// Returns `None` for unknown or unimplemented thing types.  This covers
/// all standard Doom pickups, monsters, decorations, and player starts
/// that have corresponding `MobjKind` variants.
pub fn doomed_type_to_kind(doomed_type: u16) -> Option<MobjKind> {
    match doomed_type {
        // Player starts (only player 1 has a MobjKind variant)
        1 => Some(MobjKind::Player),

        // Monsters
        3004 => Some(MobjKind::Trooper),
        9 => Some(MobjKind::Sergeant),
        3001 => Some(MobjKind::Imp),
        3002 => Some(MobjKind::Demon),
        58 => Some(MobjKind::Spectre),
        3006 => Some(MobjKind::LostSoul),
        3005 => Some(MobjKind::Cacodemon),
        3003 => Some(MobjKind::BaronOfHell),
        69 => Some(MobjKind::HellKnight),
        68 => Some(MobjKind::Arachnotron),
        71 => Some(MobjKind::PainElemental),
        66 => Some(MobjKind::Revenant),
        67 => Some(MobjKind::Mancubus),
        64 => Some(MobjKind::ArchVile),
        7 => Some(MobjKind::SpiderMastermind),
        16 => Some(MobjKind::Cyberdemon),
        84 => Some(MobjKind::WolfSS),

        // Weapons
        2001 => Some(MobjKind::Shotgun),
        82 => Some(MobjKind::SuperShotgun),
        2002 => Some(MobjKind::Chaingun),
        2003 => Some(MobjKind::RocketLauncher),
        2004 => Some(MobjKind::PlasmaRifle),
        2006 => Some(MobjKind::BfgPickup),
        2005 => Some(MobjKind::Chainsaw),

        // Ammo
        2007 => Some(MobjKind::Clip),
        2048 => Some(MobjKind::ClipBox),
        2008 => Some(MobjKind::Shell),
        2049 => Some(MobjKind::ShellBox),
        2010 => Some(MobjKind::RocketAmmo),
        2046 => Some(MobjKind::RocketBox),
        2047 => Some(MobjKind::Cell),
        17 => Some(MobjKind::CellPack),

        // Health & Armor
        2014 => Some(MobjKind::HealthBonus),
        2015 => Some(MobjKind::ArmorBonus),
        2018 => Some(MobjKind::GreenArmor),
        2019 => Some(MobjKind::BlueArmor),
        2011 => Some(MobjKind::Stimpack),
        2012 => Some(MobjKind::Medikit),
        83 => Some(MobjKind::Megasphere),
        2013 => Some(MobjKind::Soulsphere),

        // Keys
        5 => Some(MobjKind::BlueCard),
        13 => Some(MobjKind::RedCard),
        6 => Some(MobjKind::YellowCard),
        40 => Some(MobjKind::BlueSkull),
        38 => Some(MobjKind::RedSkull),
        39 => Some(MobjKind::YellowSkull),

        // Power-ups
        2023 => Some(MobjKind::Berserk),
        2024 => Some(MobjKind::BlurSphere),
        2025 => Some(MobjKind::RadSuit),
        2026 => Some(MobjKind::Allmap),
        2045 => Some(MobjKind::Infrared),
        2022 => Some(MobjKind::InvulnerabilitySphere),
        8 => Some(MobjKind::Backpack),

        // Decorations
        2035 => Some(MobjKind::Barrel),
        85 => Some(MobjKind::TechLamp),
        86 => Some(MobjKind::TechLamp2),

        // Boss specials
        88 => Some(MobjKind::BossBrain),
        72 => Some(MobjKind::CommanderKeen),

        _ => None,
    }
}

/// Reverse of [`doomed_type_to_kind`]: map a `MobjKind` back to its DoomEd
/// type number so live mobjs can be passed to the sprite renderer.
///
/// Returns `None` for internal kinds that have no WAD thing type (projectiles,
/// visual effects, etc.).
pub fn kind_to_doomed_type(kind: MobjKind) -> Option<u16> {
    Some(match kind {
        MobjKind::Player => 1,
        // Monsters
        MobjKind::Trooper => 3004,
        MobjKind::Sergeant => 9,
        MobjKind::Imp => 3001,
        MobjKind::Demon => 3002,
        MobjKind::Spectre => 58,
        MobjKind::LostSoul => 3006,
        MobjKind::Cacodemon => 3005,
        MobjKind::BaronOfHell => 3003,
        MobjKind::HellKnight => 69,
        MobjKind::Arachnotron => 68,
        MobjKind::PainElemental => 71,
        MobjKind::Revenant => 66,
        MobjKind::Mancubus => 67,
        MobjKind::ArchVile => 64,
        MobjKind::SpiderMastermind => 7,
        MobjKind::Cyberdemon => 16,
        MobjKind::WolfSS => 84,
        // Weapons
        MobjKind::Shotgun => 2001,
        MobjKind::SuperShotgun => 82,
        MobjKind::Chaingun => 2002,
        MobjKind::RocketLauncher => 2003,
        MobjKind::PlasmaRifle => 2004,
        MobjKind::BfgPickup => 2006,
        MobjKind::Chainsaw => 2005,
        // Ammo
        MobjKind::Clip => 2007,
        MobjKind::ClipBox => 2048,
        MobjKind::Shell => 2008,
        MobjKind::ShellBox => 2049,
        MobjKind::RocketAmmo => 2010,
        MobjKind::RocketBox => 2046,
        MobjKind::Cell => 2047,
        MobjKind::CellPack => 17,
        // Health & armor
        MobjKind::HealthBonus => 2014,
        MobjKind::ArmorBonus => 2015,
        MobjKind::GreenArmor => 2018,
        MobjKind::BlueArmor => 2019,
        MobjKind::Stimpack => 2011,
        MobjKind::Medikit => 2012,
        MobjKind::Megasphere => 83,
        MobjKind::Soulsphere => 2013,
        // Keys
        MobjKind::BlueCard => 5,
        MobjKind::RedCard => 13,
        MobjKind::YellowCard => 6,
        MobjKind::BlueSkull => 40,
        MobjKind::RedSkull => 38,
        MobjKind::YellowSkull => 39,
        // Power-ups
        MobjKind::Berserk => 2023,
        MobjKind::BlurSphere => 2024,
        MobjKind::RadSuit => 2025,
        MobjKind::Allmap => 2026,
        MobjKind::Infrared => 2045,
        MobjKind::InvulnerabilitySphere => 2022,
        MobjKind::Backpack => 8,
        // Decorations
        MobjKind::Barrel => 2035,
        MobjKind::TechLamp => 85,
        MobjKind::TechLamp2 => 86,
        // Boss specials
        MobjKind::BossBrain => 88,
        MobjKind::CommanderKeen => 72,
        // Projectiles and visual effects have no DoomEd number.
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check every MF_SPECIAL actor in the slab.
///
/// If the player's AABB overlaps it, attempt to pick it up.
/// Items that are successfully picked up are freed (consumed).
/// Items that cannot be picked up (e.g. health at max) are left in place.
/// Dead players cannot pick up items.
///
/// Avoids intermediate `.collect::<Vec<_>>()` by processing generations directly.
pub fn p_check_pickups(gs: &mut GameState) {
    // Dead players skip pickups (caller should guard, but double-check).
    if gs.player.is_dead() {
        return;
    }

    // Get player position and radius.
    let Some(mo) = gs.mobjslab.get(gs.player.handle) else {
        return;
    };
    let (px, py, pradius) = (mo.x.to_int(), mo.y.to_int(), mo.radius.to_int());

    // Collect all MF_SPECIAL actor handles (to avoid borrow conflicts).
    // Avoid intermediate `Vec` allocation by iterating directly over generations.
    // This prevents heap allocations on every tick.
    let initial_slot_count = gs.mobjslab.slot_count();
    let initial_generation = gs.mobjslab.next_generation();

    for index in 0..initial_slot_count {
        let Some(handle) = gs.mobjslab.handle_at(index) else {
            continue;
        };

        if handle.generation >= initial_generation {
            continue;
        }

        if gs
            .mobjslab
            .get(handle)
            .is_none_or(|mo| mo.flags & flags::MF_SPECIAL == 0)
        {
            continue;
        }
        // Skip the player's own mobj if it somehow has MF_SPECIAL.
        if handle == gs.player.handle {
            continue;
        }

        // Extract item position and radius.
        let Some(mo) = gs.mobjslab.get(handle) else {
            continue;
        };
        let (ix, iy, item_radius) = (mo.x.to_int(), mo.y.to_int(), mo.radius.to_int());

        // AABB overlap: combined-radius square check.
        let combined_radius = pradius + item_radius;
        let dx = (px - ix).abs();
        let dy = (py - iy).abs();

        if dx < combined_radius && dy < combined_radius && p_touch_special_thing(gs, handle) {
            // Item was picked up: free (consume) the actor.
            gs.mobjslab.free(handle);
        }
    }
}

/// Called when the player overlaps an item (`MF_SPECIAL` flag set).
///
/// Returns `true` if the item was picked up (caller should remove it).
/// Returns `false` if the item cannot be picked up right now (e.g. health
/// when already at max) -- the item stays in the world.
pub fn p_touch_special_thing(gs: &mut GameState, item_handle: MobjHandle) -> bool {
    let Some(mo) = gs.mobjslab.get(item_handle) else {
        return false;
    };
    let kind = mo.kind;

    let picked_up = match kind {
        // ---- Health ----
        MobjKind::HealthBonus => {
            // Always picked up. +1 health, cap at 200 (overheal).
            gs.heal_player_overheal(1, 200);
            true
        }
        MobjKind::Stimpack => {
            // Skip if health >= 100.
            if gs.player.health() >= 100 {
                return false;
            }
            gs.heal_player(10);
            true
        }
        MobjKind::Medikit => {
            // Skip if health >= 100.
            if gs.player.health() >= 100 {
                return false;
            }
            gs.heal_player(25);
            true
        }
        MobjKind::Soulsphere => {
            // Always picked up. Overheal up to 200 hp, gives 100 hp.
            gs.heal_player_overheal(100, 200);
            true
        }
        MobjKind::Megasphere => {
            // Always picked up. Set health to 200, give 200 blue armor.
            gs.set_player_health_capped(200, 200);
            gs.player.give_armor(200, 2);
            true
        }

        // ---- Armor ----
        MobjKind::ArmorBonus => {
            // Always picked up. +1 armor (cap at 200). If no armor type, set to green.
            let new_armor = (gs.player.armor() + 1).min(200);
            let new_type = gs.player.armor_type.max(1);
            // give_armor only upgrades if new > current, so we call it directly.
            gs.player.give_armor(new_armor, new_type);
            true
        }
        MobjKind::GreenArmor => {
            // Skip if current armor >= 100 (green is not an upgrade).
            if gs.player.armor() >= 100 {
                return false;
            }
            gs.player.give_armor(100, 1);
            true
        }
        MobjKind::BlueArmor => {
            // Skip if current armor >= 200.
            if gs.player.armor() >= 200 {
                return false;
            }
            gs.player.give_armor(200, 2);
            true
        }

        // ---- Ammo ----
        MobjKind::Clip => gs.player.give_ammo(AmmoType::Bullets as usize, 10),
        MobjKind::ClipBox => gs.player.give_ammo(AmmoType::Bullets as usize, 50),
        MobjKind::Shell => gs.player.give_ammo(AmmoType::Shells as usize, 4),
        MobjKind::ShellBox => gs.player.give_ammo(AmmoType::Shells as usize, 20),
        MobjKind::RocketAmmo => gs.player.give_ammo(AmmoType::Rockets as usize, 1),
        MobjKind::RocketBox => gs.player.give_ammo(AmmoType::Rockets as usize, 5),
        MobjKind::Cell => gs.player.give_ammo(AmmoType::Cells as usize, 20),
        MobjKind::CellPack => gs.player.give_ammo(AmmoType::Cells as usize, 100),

        // ---- Weapons ----
        MobjKind::Shotgun => give_weapon(
            &mut gs.player,
            WeaponType::Shotgun,
            AmmoType::Shells as usize,
            8,
        ),
        MobjKind::SuperShotgun => give_weapon(
            &mut gs.player,
            WeaponType::SuperShotgun,
            AmmoType::Shells as usize,
            8,
        ),
        MobjKind::Chaingun => give_weapon(
            &mut gs.player,
            WeaponType::Chaingun,
            AmmoType::Bullets as usize,
            20,
        ),
        MobjKind::RocketLauncher => give_weapon(
            &mut gs.player,
            WeaponType::RocketLauncher,
            AmmoType::Rockets as usize,
            2,
        ),
        MobjKind::PlasmaRifle => give_weapon(
            &mut gs.player,
            WeaponType::PlasmaRifle,
            AmmoType::Cells as usize,
            40,
        ),
        MobjKind::BfgPickup => give_weapon(
            &mut gs.player,
            WeaponType::Bfg,
            AmmoType::Cells as usize,
            40,
        ),
        MobjKind::Chainsaw => {
            let had = gs.player.weapons[WeaponType::Chainsaw as usize];
            gs.player.weapons[WeaponType::Chainsaw as usize] = true;
            if !had {
                gs.player.pending_weapon = Some(WeaponType::Chainsaw);
            }
            true // Chainsaw is always picked up
        }

        // ---- Keys ----
        MobjKind::BlueCard => {
            gs.player.give_key(player::KEY_BLUE_CARD);
            true // Keys are always picked up
        }
        MobjKind::YellowCard => {
            gs.player.give_key(player::KEY_YELLOW_CARD);
            true
        }
        MobjKind::RedCard => {
            gs.player.give_key(player::KEY_RED_CARD);
            true
        }
        MobjKind::BlueSkull => {
            gs.player.give_key(player::KEY_BLUE_SKULL);
            true
        }
        MobjKind::YellowSkull => {
            gs.player.give_key(player::KEY_YELLOW_SKULL);
            true
        }
        MobjKind::RedSkull => {
            gs.player.give_key(player::KEY_RED_SKULL);
            true
        }

        // ---- Power-ups ----
        MobjKind::Berserk => {
            // Set health to max(100, current), give PW_STRENGTH, auto-switch to fist.
            if gs.player.health() < 100 {
                gs.set_player_health_capped(100, 200);
            }
            gs.player.powers[powers::PW_STRENGTH] = STRENGTH_TICS;
            gs.player.weapons[WeaponType::Fist as usize] = true;
            gs.player.pending_weapon = Some(WeaponType::Fist);
            true
        }
        MobjKind::InvulnerabilitySphere => {
            gs.player.powers[powers::PW_INVULNERABILITY] = INVULN_TICS;
            true
        }
        MobjKind::BlurSphere => {
            gs.player.powers[powers::PW_INVISIBILITY] = INVIS_TICS;
            true
        }
        MobjKind::RadSuit => {
            gs.player.powers[powers::PW_IRONFEET] = IRONFEET_TICS;
            true
        }
        MobjKind::Allmap => {
            gs.player.powers[powers::PW_ALLMAP] = 1; // Allmap is permanent (just nonzero)
            true
        }
        MobjKind::Infrared => {
            gs.player.powers[powers::PW_INFRARED] = INFRARED_TICS;
            true
        }
        MobjKind::Backpack => {
            // Double all max_ammo values (only once -- cap at 2x original).
            // The original Doom doubles max_ammo unconditionally but since
            // we track per-player max_ammo, we cap at 2x the base.
            use doom_types::limits::MAX_AMMO;
            for (i, max_ammo) in MAX_AMMO.iter().copied().enumerate() {
                let doubled = max_ammo * 2;
                if gs.player.max_ammo[i] < doubled {
                    gs.player.max_ammo[i] = doubled;
                }
            }
            // Give some ammo of each type.
            gs.player.give_ammo(AmmoType::Bullets as usize, 10);
            gs.player.give_ammo(AmmoType::Shells as usize, 4);
            gs.player.give_ammo(AmmoType::Cells as usize, 20);
            gs.player.give_ammo(AmmoType::Rockets as usize, 1);
            true // Backpack is always picked up
        }

        // Everything else: decorations, monsters, projectiles -- not pickups.
        _ => false,
    };

    // Increment item count for items with MF_COUNTITEM flag.
    if picked_up {
        if let Some(mo) = gs.mobjslab.get(item_handle) {
            if mo.flags & flags::MF_COUNTITEM != 0 {
                gs.stats.item_count += 1;
            }
            #[cfg(feature = "telemetry")]
            {
                let name = format!("{:?}", kind);
                gs.telemetry.record(
                    gs.tic_num,
                    mo.x.to_int(),
                    mo.y.to_int(),
                    crate::telemetry::TelemetryKind::ItemPickup(name),
                );
            }
        }
        gs.player.bonus_count = 6; // HUD flash for 6 tics
    }

    picked_up
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Give a weapon and bonus ammo.  Returns `true` if the pickup should be
/// consumed (always true -- weapons are always picked up for their ammo).
fn give_weapon(
    player: &mut crate::player::PlayerState,
    weapon: WeaponType,
    ammo_type: usize,
    ammo_amount: u32,
) -> bool {
    let had_weapon = player.weapons[weapon as usize];
    player.weapons[weapon as usize] = true;
    player.give_ammo(ammo_type, ammo_amount);
    if !had_weapon {
        player.pending_weapon = Some(weapon);
    }
    true // always consume weapon pickups (they at least give ammo)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::{Mobj, flags};
    use crate::player::PlayerState;
    use doom_types::mobj_kind::MobjKind;
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
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
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

    // =======================================================================
    // Health pickups
    // =======================================================================

    #[test]
    fn pickup_health_bonus_adds_one() {
        let mut gs = make_game_state();
        gs.player.apply_damage(50);
        assert_eq!(gs.player.health(), 50);
        let item = spawn_item(&mut gs, MobjKind::HealthBonus, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.health(), 51);
    }

    #[test]
    fn pickup_health_bonus_syncs_player_mobj_health() {
        let mut gs = make_game_state();
        gs.player.apply_damage(50);
        assert_eq!(gs.player.health(), 50);
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("value must exist in test")
                .health,
            100,
            "setup should start desynced so the pickup path has to repair it"
        );

        let item = spawn_item(&mut gs, MobjKind::HealthBonus, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));

        assert_eq!(gs.player.health(), 51);
        assert_eq!(
            gs.mobjslab
                .get(gs.player.handle)
                .expect("value must exist in test")
                .health,
            51,
            "pickup health must sync the live player mobj used by monster AI"
        );
    }

    #[test]
    fn pickup_health_bonus_caps_at_200() {
        let mut gs = make_game_state();
        gs.player.heal_overheal(100, 200); // health = 200
        assert_eq!(gs.player.health(), 200);
        let item = spawn_item(&mut gs, MobjKind::HealthBonus, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item)); // always picks up
        assert_eq!(gs.player.health(), 200); // capped at 200
    }

    #[test]
    fn pickup_stimpack_heals_10() {
        let mut gs = make_game_state();
        gs.player.apply_damage(50);
        assert_eq!(gs.player.health(), 50);
        let item = spawn_item(&mut gs, MobjKind::Stimpack, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.health(), 60);
    }

    #[test]
    fn pickup_stimpack_skipped_at_full_health() {
        let mut gs = make_game_state();
        assert_eq!(gs.player.health(), 100);
        let item = spawn_item(&mut gs, MobjKind::Stimpack, 0, 0);
        assert!(!p_touch_special_thing(&mut gs, item)); // no pickup
    }

    #[test]
    fn pickup_medikit_heals_25hp() {
        let mut gs = make_game_state();
        gs.player.apply_damage(50);
        assert_eq!(gs.player.health(), 50);
        let item = spawn_item(&mut gs, MobjKind::Medikit, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.health(), 75);
    }

    #[test]
    fn pickup_medikit_skipped_at_full_health() {
        let mut gs = make_game_state();
        assert_eq!(gs.player.health(), 100);
        let item = spawn_item(&mut gs, MobjKind::Medikit, 0, 0);
        assert!(!p_touch_special_thing(&mut gs, item));
    }

    #[test]
    fn pickup_soulsphere_overheals_to_200() {
        let mut gs = make_game_state();
        assert_eq!(gs.player.health(), 100);
        let item = spawn_item(&mut gs, MobjKind::Soulsphere, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.health(), 200);
    }

    #[test]
    fn pickup_megasphere_full_health_and_armor() {
        let mut gs = make_game_state();
        let item = spawn_item(&mut gs, MobjKind::Megasphere, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.health(), 200);
        assert_eq!(gs.player.armor(), 200);
        assert_eq!(gs.player.armor_type, 2);
    }

    // =======================================================================
    // Armor pickups
    // =======================================================================

    #[test]
    fn pickup_armor_bonus_adds_one() {
        let mut gs = make_game_state();
        assert_eq!(gs.player.armor(), 0);
        let item = spawn_item(&mut gs, MobjKind::ArmorBonus, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.armor(), 1);
        assert_eq!(gs.player.armor_type, 1); // auto-set to green
    }

    #[test]
    fn pickup_green_armor() {
        let mut gs = make_game_state();
        assert_eq!(gs.player.armor(), 0);
        let item = spawn_item(&mut gs, MobjKind::GreenArmor, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.armor(), 100);
        assert_eq!(gs.player.armor_type, 1);
    }

    #[test]
    fn pickup_green_armor_skipped_if_blue_armor() {
        let mut gs = make_game_state();
        gs.player.give_armor(200, 2);
        let item = spawn_item(&mut gs, MobjKind::GreenArmor, 0, 0);
        assert!(!p_touch_special_thing(&mut gs, item)); // blue armor is better
    }

    #[test]
    fn pickup_blue_armor() {
        let mut gs = make_game_state();
        let item = spawn_item(&mut gs, MobjKind::BlueArmor, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.armor(), 200);
        assert_eq!(gs.player.armor_type, 2);
    }

    #[test]
    fn pickup_blue_armor_skipped_at_200() {
        let mut gs = make_game_state();
        gs.player.give_armor(200, 2);
        let item = spawn_item(&mut gs, MobjKind::BlueArmor, 0, 0);
        assert!(!p_touch_special_thing(&mut gs, item));
    }

    // =======================================================================
    // Ammo pickups
    // =======================================================================

    #[test]
    fn pickup_ammo_clip_gives_10_bullets() {
        let mut gs = make_game_state();
        // Drain starting ammo.
        gs.player.use_ammo(AmmoType::Bullets as usize, 50);
        assert_eq!(gs.player.ammo(AmmoType::Bullets as usize), 0);
        let item = spawn_item(&mut gs, MobjKind::Clip, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.ammo(AmmoType::Bullets as usize), 10);
    }

    #[test]
    fn pickup_ammo_skipped_at_max() {
        let mut gs = make_game_state();
        gs.player.give_ammo(AmmoType::Bullets as usize, 10_000); // max out
        let item = spawn_item(&mut gs, MobjKind::Clip, 0, 0);
        assert!(!p_touch_special_thing(&mut gs, item));
    }

    #[test]
    fn pickup_clip_gives_10_bullets_via_p_check() {
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

    // =======================================================================
    // Weapon pickups
    // =======================================================================

    #[test]
    fn pickup_weapon_gives_weapon_and_ammo() {
        let mut gs = make_game_state();
        assert!(!gs.player.weapons[WeaponType::Shotgun as usize]);
        let shells_before = gs.player.ammo(AmmoType::Shells as usize);
        let item = spawn_item(&mut gs, MobjKind::Shotgun, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert!(gs.player.weapons[WeaponType::Shotgun as usize]);
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), shells_before + 8);
        assert_eq!(gs.player.weapon, WeaponType::Pistol);
        assert_eq!(gs.player.pending_weapon, Some(WeaponType::Shotgun));
    }

    #[test]
    fn pickup_weapon_already_owned_still_gives_ammo() {
        let mut gs = make_game_state();
        gs.player.weapons[WeaponType::Shotgun as usize] = true;
        gs.player.weapon = WeaponType::Pistol;
        let shells_before = gs.player.ammo(AmmoType::Shells as usize);
        let item = spawn_item(&mut gs, MobjKind::Shotgun, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), shells_before + 8);
        // Should NOT auto-switch since player already has it.
        assert_eq!(gs.player.weapon, WeaponType::Pistol);
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
    fn pickup_chainsaw() {
        let mut gs = make_game_state();
        let item = spawn_item(&mut gs, MobjKind::Chainsaw, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert!(gs.player.weapons[WeaponType::Chainsaw as usize]);
        assert_eq!(gs.player.weapon, WeaponType::Pistol);
        assert_eq!(gs.player.pending_weapon, Some(WeaponType::Chainsaw));
    }

    // =======================================================================
    // Key pickups
    // =======================================================================

    #[test]
    fn pickup_blue_card_sets_key_bit() {
        let mut gs = make_game_state();
        assert!(!gs.player.has_key(player::KEY_BLUE_CARD));
        let item = spawn_item(&mut gs, MobjKind::BlueCard, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert!(gs.player.has_key(player::KEY_BLUE_CARD));
    }

    #[test]
    fn pickup_all_keys() {
        let mut gs = make_game_state();
        let keys = [
            (MobjKind::BlueCard, player::KEY_BLUE_CARD),
            (MobjKind::YellowCard, player::KEY_YELLOW_CARD),
            (MobjKind::RedCard, player::KEY_RED_CARD),
            (MobjKind::BlueSkull, player::KEY_BLUE_SKULL),
            (MobjKind::YellowSkull, player::KEY_YELLOW_SKULL),
            (MobjKind::RedSkull, player::KEY_RED_SKULL),
        ];
        for (kind, key_bit) in keys {
            let item = spawn_item(&mut gs, kind, 0, 0);
            assert!(p_touch_special_thing(&mut gs, item));
            assert!(gs.player.has_key(key_bit));
        }
    }

    #[test]
    fn pickup_blue_card_gives_key_via_p_check() {
        let mut gs = make_game_state();
        assert!(!gs.player.has_key(player::KEY_BLUE_CARD));
        spawn_item(&mut gs, MobjKind::BlueCard, 0, 0);
        p_check_pickups(&mut gs);
        assert!(
            gs.player.has_key(player::KEY_BLUE_CARD),
            "Player should have blue card after pickup"
        );
    }

    #[test]
    fn pickup_red_skull_gives_key() {
        let mut gs = make_game_state();
        assert!(!gs.player.has_key(player::KEY_RED_SKULL));
        spawn_item(&mut gs, MobjKind::RedSkull, 0, 0);
        p_check_pickups(&mut gs);
        assert!(
            gs.player.has_key(player::KEY_RED_SKULL),
            "Player should have red skull after pickup"
        );
    }

    // =======================================================================
    // Power-up pickups
    // =======================================================================

    #[test]
    fn pickup_berserk_heals_and_gives_fist() {
        let mut gs = make_game_state();
        gs.player.apply_damage(60);
        assert_eq!(gs.player.health(), 40);
        let item = spawn_item(&mut gs, MobjKind::Berserk, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.health(), 100);
        assert!(gs.player.weapons[WeaponType::Fist as usize]);
        assert_eq!(gs.player.weapon, WeaponType::Pistol);
        assert_eq!(gs.player.pending_weapon, Some(WeaponType::Fist));
        assert!(gs.player.powers[powers::PW_STRENGTH] > 0);
    }

    #[test]
    fn pickup_invulnerability_sphere() {
        let mut gs = make_game_state();
        let item = spawn_item(&mut gs, MobjKind::InvulnerabilitySphere, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.powers[powers::PW_INVULNERABILITY], INVULN_TICS);
    }

    #[test]
    fn pickup_blur_sphere() {
        let mut gs = make_game_state();
        let item = spawn_item(&mut gs, MobjKind::BlurSphere, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.powers[powers::PW_INVISIBILITY], INVIS_TICS);
    }

    #[test]
    fn pickup_rad_suit() {
        let mut gs = make_game_state();
        let item = spawn_item(&mut gs, MobjKind::RadSuit, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.powers[powers::PW_IRONFEET], IRONFEET_TICS);
    }

    #[test]
    fn pickup_computer_map() {
        let mut gs = make_game_state();
        let item = spawn_item(&mut gs, MobjKind::Allmap, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert!(gs.player.powers[powers::PW_ALLMAP] > 0);
    }

    #[test]
    fn pickup_infrared() {
        let mut gs = make_game_state();
        let item = spawn_item(&mut gs, MobjKind::Infrared, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.powers[powers::PW_INFRARED], INFRARED_TICS);
    }

    // =======================================================================
    // Backpack
    // =======================================================================

    #[test]
    fn pickup_backpack_doubles_max_ammo() {
        let mut gs = make_game_state();
        use doom_types::limits::MAX_AMMO;
        let old_max = gs.player.max_ammo[0];
        assert_eq!(old_max, MAX_AMMO[0]);
        let item = spawn_item(&mut gs, MobjKind::Backpack, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.max_ammo[0], old_max * 2);
        assert_eq!(gs.player.max_ammo[1], MAX_AMMO[1] * 2);
        assert_eq!(gs.player.max_ammo[2], MAX_AMMO[2] * 2);
        assert_eq!(gs.player.max_ammo[3], MAX_AMMO[3] * 2);
    }

    #[test]
    fn pickup_backpack_gives_ammo() {
        let mut gs = make_game_state();
        gs.player.use_ammo(AmmoType::Bullets as usize, 50); // drain to 0
        let item = spawn_item(&mut gs, MobjKind::Backpack, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item));
        assert_eq!(gs.player.ammo(AmmoType::Bullets as usize), 10);
        assert_eq!(gs.player.ammo(AmmoType::Shells as usize), 4);
        assert_eq!(gs.player.ammo(AmmoType::Cells as usize), 20);
        assert_eq!(gs.player.ammo(AmmoType::Rockets as usize), 1);
    }

    #[test]
    fn pickup_backpack_does_not_triple_max() {
        let mut gs = make_game_state();
        use doom_types::limits::MAX_AMMO;
        // Pick up backpack twice.
        let item1 = spawn_item(&mut gs, MobjKind::Backpack, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item1));
        let item2 = spawn_item(&mut gs, MobjKind::Backpack, 0, 0);
        assert!(p_touch_special_thing(&mut gs, item2));
        // Max should still be 2x, not 4x.
        assert_eq!(gs.player.max_ammo[0], MAX_AMMO[0] * 2);
    }

    // =======================================================================
    // p_check_pickups integration
    // =======================================================================

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
    fn p_check_pickups_removes_collected_items() {
        let mut gs = make_game_state();
        gs.player.apply_damage(50);
        assert_eq!(gs.player.health(), 50);
        let item = spawn_item(&mut gs, MobjKind::HealthBonus, 0, 0);
        p_check_pickups(&mut gs);
        assert!(gs.mobjslab.get(item).is_none(), "item should be removed");
        assert_eq!(gs.player.health(), 51);
    }

    #[test]
    fn unpickable_item_stays_in_world() {
        let mut gs = make_game_state();
        // Player at full health -- stimpack should not be picked up.
        assert_eq!(gs.player.health(), 100);
        let item = spawn_item(&mut gs, MobjKind::Stimpack, 0, 0);
        p_check_pickups(&mut gs);
        assert!(
            gs.mobjslab.get(item).is_some(),
            "Stimpack at full health should stay in world"
        );
    }

    // =======================================================================
    // DoomEd type mapping
    // =======================================================================

    #[test]
    fn doomed_type_maps_health_bonus() {
        assert_eq!(doomed_type_to_kind(2014), Some(MobjKind::HealthBonus));
    }

    #[test]
    fn doomed_type_maps_shotgun() {
        assert_eq!(doomed_type_to_kind(2001), Some(MobjKind::Shotgun));
    }

    #[test]
    fn doomed_type_maps_backpack() {
        assert_eq!(doomed_type_to_kind(8), Some(MobjKind::Backpack));
    }

    #[test]
    fn doomed_type_maps_invulnerability() {
        assert_eq!(
            doomed_type_to_kind(2022),
            Some(MobjKind::InvulnerabilitySphere)
        );
    }

    #[test]
    fn doomed_type_unknown_returns_none() {
        assert_eq!(doomed_type_to_kind(9999), None);
    }

    #[test]
    fn doomed_type_maps_all_keys() {
        assert_eq!(doomed_type_to_kind(5), Some(MobjKind::BlueCard));
        assert_eq!(doomed_type_to_kind(6), Some(MobjKind::YellowCard));
        assert_eq!(doomed_type_to_kind(13), Some(MobjKind::RedCard));
        assert_eq!(doomed_type_to_kind(40), Some(MobjKind::BlueSkull));
        assert_eq!(doomed_type_to_kind(39), Some(MobjKind::YellowSkull));
        assert_eq!(doomed_type_to_kind(38), Some(MobjKind::RedSkull));
    }

    #[test]
    fn doomed_type_maps_all_ammo() {
        assert_eq!(doomed_type_to_kind(2007), Some(MobjKind::Clip));
        assert_eq!(doomed_type_to_kind(2048), Some(MobjKind::ClipBox));
        assert_eq!(doomed_type_to_kind(2008), Some(MobjKind::Shell));
        assert_eq!(doomed_type_to_kind(2049), Some(MobjKind::ShellBox));
        assert_eq!(doomed_type_to_kind(2010), Some(MobjKind::RocketAmmo));
        assert_eq!(doomed_type_to_kind(2046), Some(MobjKind::RocketBox));
        assert_eq!(doomed_type_to_kind(2047), Some(MobjKind::Cell));
        assert_eq!(doomed_type_to_kind(17), Some(MobjKind::CellPack));
    }

    #[test]
    fn doomed_type_maps_all_weapons() {
        assert_eq!(doomed_type_to_kind(2001), Some(MobjKind::Shotgun));
        assert_eq!(doomed_type_to_kind(82), Some(MobjKind::SuperShotgun));
        assert_eq!(doomed_type_to_kind(2002), Some(MobjKind::Chaingun));
        assert_eq!(doomed_type_to_kind(2003), Some(MobjKind::RocketLauncher));
        assert_eq!(doomed_type_to_kind(2004), Some(MobjKind::PlasmaRifle));
        assert_eq!(doomed_type_to_kind(2006), Some(MobjKind::BfgPickup));
        assert_eq!(doomed_type_to_kind(2005), Some(MobjKind::Chainsaw));
    }

    #[test]
    fn doomed_type_maps_all_powerups() {
        assert_eq!(doomed_type_to_kind(2023), Some(MobjKind::Berserk));
        assert_eq!(doomed_type_to_kind(2024), Some(MobjKind::BlurSphere));
        assert_eq!(doomed_type_to_kind(2025), Some(MobjKind::RadSuit));
        assert_eq!(doomed_type_to_kind(2026), Some(MobjKind::Allmap));
        assert_eq!(doomed_type_to_kind(2045), Some(MobjKind::Infrared));
        assert_eq!(
            doomed_type_to_kind(2022),
            Some(MobjKind::InvulnerabilitySphere)
        );
        assert_eq!(doomed_type_to_kind(8), Some(MobjKind::Backpack));
    }
}
