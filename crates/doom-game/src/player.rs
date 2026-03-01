//! Player state: health, ammo, armor, weapons, and power-ups.
//!
//! `PlayerState` is separate from the player's `Mobj` (which lives in the slab).
//! The `Mobj` handles physics/position; `PlayerState` handles inventory/status.
//!
//! # Invariants (Verus-verifiable)
//! - `health ≤ MAX_HEALTH (100)` at all times
//! - `ammo[i] ≤ MAX_AMMO[i]` for all i

use doom_types::limits::{MAX_AMMO, MAX_ARMOR, MAX_HEALTH, NUM_AMMO, NUM_WEAPONS};

use crate::mobj::MobjHandle;

// ---------------------------------------------------------------------------
// Power-up constants
// ---------------------------------------------------------------------------

/// Number of distinct power-up types.
pub const NUM_POWERS: usize = 6;

/// Power-up slot indices.
pub mod powers {
    /// Invulnerability sphere.
    pub const PW_INVULNERABILITY: usize = 0;
    /// Berserk (strength — fist does 10× damage, full health restore).
    pub const PW_STRENGTH: usize = 1;
    /// Blur sphere / partial invisibility.
    pub const PW_INVISIBILITY: usize = 2;
    /// Radiation suit (iron feet).
    pub const PW_IRONFEET: usize = 3;
    /// Computer area map (all map visible).
    pub const PW_ALLMAP: usize = 4;
    /// Infrared (light amplification visor).
    pub const PW_INFRARED: usize = 5;
}

// ---------------------------------------------------------------------------
// Weapon types
// ---------------------------------------------------------------------------

/// Weapon slots (index = selection key − 1 for keys 1-7; chainsaw = key 1 alt).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum WeaponType {
    Fist           = 0,
    #[default]
    Pistol         = 1,
    Shotgun        = 2,
    Chaingun       = 3,
    RocketLauncher = 4,
    PlasmaRifle    = 5,
    Bfg            = 6,
    Chainsaw       = 7,
    SuperShotgun   = 8,
}

// ---------------------------------------------------------------------------
// Ammo types
// ---------------------------------------------------------------------------

/// Ammo pool indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AmmoType {
    Bullets = 0,
    Shells  = 1,
    Cells   = 2,
    Rockets = 3,
    /// Melee weapons (Fist, Chainsaw) — no ammo consumed.
    None    = 255,
}

/// Which ammo pool each weapon draws from.
pub const WEAPON_AMMO: [AmmoType; NUM_WEAPONS] = [
    AmmoType::None,    // Fist
    AmmoType::Bullets, // Pistol
    AmmoType::Shells,  // Shotgun
    AmmoType::Bullets, // Chaingun
    AmmoType::Rockets, // RocketLauncher
    AmmoType::Cells,   // PlasmaRifle
    AmmoType::Cells,   // BFG
    AmmoType::None,    // Chainsaw
    AmmoType::Shells,  // SuperShotgun
];

// ---------------------------------------------------------------------------
// PlayerState
// ---------------------------------------------------------------------------

/// Per-player game state (inventory, health, weapons, power-ups).
///
/// The player's `Mobj` (position, velocity, flags) lives in `MobjSlab`;
/// this struct holds everything the game logic needs beyond the physics object.
#[derive(Clone, Debug)]
pub struct PlayerState {
    /// Handle to this player's `Mobj` in the arena.
    pub handle: MobjHandle,

    // --- Health (invariant: ≤ MAX_HEALTH) ---
    health: i32,

    // --- Armor (invariant: ≤ MAX_ARMOR) ---
    armor: i32,
    /// 0 = none, 1 = green security armor, 2 = blue combat armor.
    pub armor_type: u8,

    // --- Ammo (invariant: ammo[i] ≤ MAX_AMMO[i]) ---
    ammo: [u32; NUM_AMMO],

    // --- Weapons ---
    /// `true` for each weapon slot the player currently owns.
    pub weapons: [bool; NUM_WEAPONS],
    /// Active weapon.
    pub weapon: WeaponType,
    /// Weapon to switch to next tic (if `Some`).
    pub pending_weapon: Option<WeaponType>,

    // --- Input debounce ---
    /// Was attack held last tic (for auto-fire).
    pub attack_down: bool,
    /// Was use held last tic (prevents continuous use on key hold).
    pub use_down: bool,

    // --- Power-ups: remaining tics (0 = not active) ---
    pub powers: [u32; NUM_POWERS],

    // --- HUD flash counters ---
    /// Tics to flash the screen yellow (pickup).
    pub bonus_count: u32,
    /// Tics to flash the screen red (damage).
    pub damage_count: u32,
}

impl PlayerState {
    /// Create a pistol-start player state (standard Doom start conditions).
    ///
    /// - Health: 100
    /// - Armor: 0
    /// - Weapons: fist + pistol
    /// - Ammo: 50 bullets
    pub fn pistol_start(handle: MobjHandle) -> Self {
        let mut weapons = [false; NUM_WEAPONS];
        weapons[WeaponType::Fist as usize] = true;
        weapons[WeaponType::Pistol as usize] = true;
        let mut ammo = [0u32; NUM_AMMO];
        ammo[AmmoType::Bullets as usize] = 50;

        Self {
            handle,
            health: MAX_HEALTH,
            armor: 0,
            armor_type: 0,
            ammo,
            weapons,
            weapon: WeaponType::Pistol,
            pending_weapon: None,
            attack_down: false,
            use_down: false,
            powers: [0; NUM_POWERS],
            bonus_count: 0,
            damage_count: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Health (invariant: health ≤ MAX_HEALTH)
    // -----------------------------------------------------------------------

    /// Current health.
    #[inline]
    pub fn health(&self) -> i32 {
        self.health
    }

    /// Apply `amount` points of damage (positive = hurt).
    ///
    /// Health is clamped to `[-32768, MAX_HEALTH]` after the operation.
    /// Note: negative `amount` heals, but `heal()` is the preferred API.
    pub fn apply_damage(&mut self, amount: i32) {
        self.health = (self.health - amount).clamp(-32768, MAX_HEALTH);
    }

    /// Heal the player by `amount`, capped at `MAX_HEALTH`.
    ///
    /// Has no effect if the player is already at or above `MAX_HEALTH`.
    pub fn heal(&mut self, amount: i32) {
        if self.health < MAX_HEALTH {
            self.health = (self.health + amount).min(MAX_HEALTH);
        }
    }

    /// Heal the player by `amount`, allowing health to exceed `MAX_HEALTH`.
    ///
    /// Used for power-up items (Soulsphere, Megasphere) that can overheal.
    /// Health is capped at `cap` (e.g. 200).
    pub fn heal_overheal(&mut self, amount: i32, cap: i32) {
        self.health = (self.health + amount).min(cap);
    }

    /// Set health directly to `value`, clamped to `[0, cap]`.
    ///
    /// Used for items that set health to a fixed value (e.g. Megasphere).
    pub fn set_health_capped(&mut self, value: i32, cap: i32) {
        self.health = value.clamp(0, cap);
    }

    /// Returns `true` if the player is dead (health ≤ 0).
    #[inline]
    pub fn is_dead(&self) -> bool {
        self.health <= 0
    }

    // -----------------------------------------------------------------------
    // Armor (invariant: armor ≤ MAX_ARMOR)
    // -----------------------------------------------------------------------

    /// Current armor value.
    #[inline]
    pub fn armor(&self) -> i32 {
        self.armor
    }

    /// Give armor of `armor_type` if `points` exceeds current armor.
    ///
    /// Armor is capped at `MAX_ARMOR`.
    pub fn give_armor(&mut self, points: i32, armor_type: u8) {
        if points > self.armor {
            self.armor = points.min(MAX_ARMOR);
            self.armor_type = armor_type;
        }
    }

    // -----------------------------------------------------------------------
    // Ammo (invariant: ammo[i] ≤ MAX_AMMO[i])
    // -----------------------------------------------------------------------

    /// Current ammo for `ammo_type` (0 if index out of range).
    #[inline]
    pub fn ammo(&self, ammo_type: usize) -> u32 {
        self.ammo.get(ammo_type).copied().unwrap_or(0)
    }

    /// Give `amount` units of ammo; clamped to `MAX_AMMO[ammo_type]`.
    pub fn give_ammo(&mut self, ammo_type: usize, amount: u32) {
        if let (Some(cur), Some(&max)) = (self.ammo.get_mut(ammo_type), MAX_AMMO.get(ammo_type)) {
            *cur = (*cur + amount).min(max);
        }
    }

    /// Consume `amount` units of ammo; returns `false` (no-op) if insufficient.
    pub fn use_ammo(&mut self, ammo_type: usize, amount: u32) -> bool {
        if let Some(cur) = self.ammo.get_mut(ammo_type) {
            if *cur >= amount {
                *cur -= amount;
                return true;
            }
        }
        false
    }

    /// Returns `true` if the player has at least 1 unit of ammo for `weapon`.
    pub fn has_ammo_for(&self, weapon: WeaponType) -> bool {
        let atype = WEAPON_AMMO[weapon as usize];
        match atype {
            AmmoType::None => true,
            _ => self.ammo.get(atype as usize).copied().unwrap_or(0) > 0,
        }
    }
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::pistol_start(MobjHandle::NULL)
    }
}

impl WeaponType {
    /// Convert a weapon number (0–8) from `BT_WEAPONMASK` to a `WeaponType`.
    ///
    /// Returns `None` for any out-of-range value.
    pub fn from_num(n: usize) -> Option<Self> {
        match n {
            0 => Some(WeaponType::Fist),
            1 => Some(WeaponType::Pistol),
            2 => Some(WeaponType::Shotgun),
            3 => Some(WeaponType::Chaingun),
            4 => Some(WeaponType::RocketLauncher),
            5 => Some(WeaponType::PlasmaRifle),
            6 => Some(WeaponType::Bfg),
            7 => Some(WeaponType::Chainsaw),
            8 => Some(WeaponType::SuperShotgun),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pistol_start_invariants() {
        let p = PlayerState::pistol_start(MobjHandle::NULL);
        assert_eq!(p.health(), MAX_HEALTH);
        assert!(!p.is_dead());
        assert!(p.weapons[WeaponType::Pistol as usize]);
        assert!(p.weapons[WeaponType::Fist as usize]);
        assert!(!p.weapons[WeaponType::Shotgun as usize]);
        assert_eq!(p.ammo(AmmoType::Bullets as usize), 50);
    }

    #[test]
    fn damage_drives_health_below_zero() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.apply_damage(200);
        assert!(p.health() <= 0);
        assert!(p.health() >= -32768, "health must not underflow past -32768");
        assert!(p.is_dead());
    }

    #[test]
    fn damage_never_exceeds_max_health() {
        // Negative damage (healing) must not push past cap.
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.apply_damage(-1000); // "negative damage" = massive heal
        assert_eq!(p.health(), MAX_HEALTH);
    }

    #[test]
    fn heal_caps_at_max_health() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.apply_damage(50);
        assert_eq!(p.health(), 50);
        p.heal(200);
        assert_eq!(p.health(), MAX_HEALTH);
    }

    #[test]
    fn heal_no_effect_when_full() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.heal(50);
        assert_eq!(p.health(), MAX_HEALTH);
    }

    #[test]
    fn ammo_capped_at_max() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.give_ammo(AmmoType::Bullets as usize, 100_000);
        assert_eq!(p.ammo(AmmoType::Bullets as usize), MAX_AMMO[0]);
    }

    #[test]
    fn use_ammo_success_and_deduct() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        assert!(p.use_ammo(AmmoType::Bullets as usize, 10));
        assert_eq!(p.ammo(AmmoType::Bullets as usize), 40);
    }

    #[test]
    fn use_ammo_fails_when_insufficient() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        // Drain all bullets first.
        p.ammo[AmmoType::Bullets as usize] = 0;
        assert!(!p.use_ammo(AmmoType::Bullets as usize, 1));
    }

    #[test]
    fn has_ammo_for_pistol_on_start() {
        let p = PlayerState::pistol_start(MobjHandle::NULL);
        assert!(p.has_ammo_for(WeaponType::Pistol));
    }

    #[test]
    fn fist_never_needs_ammo() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.ammo[AmmoType::Bullets as usize] = 0;
        assert!(p.has_ammo_for(WeaponType::Fist));
        assert!(p.has_ammo_for(WeaponType::Chainsaw));
    }

    #[test]
    fn rocket_launcher_needs_rockets() {
        let p = PlayerState::pistol_start(MobjHandle::NULL);
        assert!(!p.has_ammo_for(WeaponType::RocketLauncher));
    }

    #[test]
    fn give_armor_upgrades_type() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.give_armor(100, 1); // green armor
        assert_eq!(p.armor(), 100);
        assert_eq!(p.armor_type, 1);
        p.give_armor(200, 2); // blue armor replaces
        assert_eq!(p.armor(), 200);
        assert_eq!(p.armor_type, 2);
    }

    #[test]
    fn armor_capped_at_max() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.give_armor(MAX_ARMOR + 100, 2);
        assert_eq!(p.armor(), MAX_ARMOR);
    }
}
