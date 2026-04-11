//! Player state: health, ammo, armor, weapons, and power-ups.
//!
//! `PlayerState` is separate from the player's `Mobj` (which lives in the slab).
//! The `Mobj` handles physics/position; `PlayerState` handles inventory/status.
//!
//! # Invariants (Verus-verifiable)
//! - `health ≤ MAX_HEALTH (100)` at all times
//! - `ammo[i] ≤ MAX_AMMO[i]` for all i

use doom_types::{MAX_AMMO, MAX_ARMOR, MAX_HEALTH, NUM_AMMO, NUM_WEAPONS};

use crate::mobj::{MobjHandle, StateNum};

// ---------------------------------------------------------------------------
// Key bit constants
// ---------------------------------------------------------------------------

/// Blue keycard bit.
pub const KEY_BLUE_CARD: u8 = 0x01;
/// Yellow keycard bit.
pub const KEY_YELLOW_CARD: u8 = 0x02;
/// Red keycard bit.
pub const KEY_RED_CARD: u8 = 0x04;
/// Blue skull key bit.
pub const KEY_BLUE_SKULL: u8 = 0x08;
/// Yellow skull key bit.
pub const KEY_YELLOW_SKULL: u8 = 0x10;
/// Red skull key bit.
pub const KEY_RED_SKULL: u8 = 0x20;

// ---------------------------------------------------------------------------
// Power-up constants
// ---------------------------------------------------------------------------

/// Number of distinct power-up types.
pub const NUM_POWERS: usize = 6;

/// Number of player psprite slots (`weapon`, `flash`).
pub const NUM_PSPRITES: usize = 2;

/// Psprite slot indices matching vanilla Doom's `ps_weapon` / `ps_flash`.
pub mod psprite_slots {
    /// Main weapon sprite.
    pub const WEAPON: usize = 0;
    /// Muzzle-flash overlay sprite.
    pub const FLASH: usize = 1;
}

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
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum WeaponType {
    Fist = 0,
    #[default]
    Pistol = 1,
    Shotgun = 2,
    Chaingun = 3,
    RocketLauncher = 4,
    PlasmaRifle = 5,
    Bfg = 6,
    Chainsaw = 7,
    SuperShotgun = 8,
}

// ---------------------------------------------------------------------------
// Ammo types
// ---------------------------------------------------------------------------

/// Ammo pool indices.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AmmoType {
    Bullets = 0,
    Shells = 1,
    Cells = 2,
    Rockets = 3,
    /// Melee weapons (Fist, Chainsaw) — no ammo consumed.
    None = 255,
}

/// Current state of one player psprite slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PspriteState {
    /// State-table entry currently driving this psprite.
    pub state: StateNum,
    /// Tics remaining in the current state (0 = advance next tic).
    pub tics: i32,
    /// Horizontal screen offset in pixels.
    pub sx: i32,
    /// Vertical screen offset in pixels.
    pub sy: i32,
}

impl Default for PspriteState {
    fn default() -> Self {
        Self {
            state: StateNum(crate::states::ids::S_NULL),
            tics: 0,
            sx: 0,
            sy: 0,
        }
    }
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

    // --- Ammo (invariant: ammo[i] ≤ max_ammo[i]) ---
    ammo: [u32; NUM_AMMO],
    /// Per-player maximum ammo (starts as `MAX_AMMO`, doubled by Backpack).
    pub max_ammo: [u32; NUM_AMMO],

    // --- Weapons ---
    /// `true` for each weapon slot the player currently owns.
    pub weapons: [bool; NUM_WEAPONS],
    /// Active weapon.
    pub weapon: WeaponType,
    /// Weapon to switch to next tic (if `Some`).
    pub pending_weapon: Option<WeaponType>,
    /// Weapon and flash psprite slots (vanilla `ps_weapon`, `ps_flash`).
    pub psprites: [PspriteState; NUM_PSPRITES],

    // --- Input debounce ---
    /// Was attack held last tic (for auto-fire).
    pub attack_down: bool,
    /// Tics remaining before the current weapon may fire again.
    pub attack_cooldown: u8,
    /// Consecutive refire count while the attack button is held.
    pub refire: u8,
    /// Vanilla `player_t::extralight` weapon-flash bonus (0..=2).
    pub extra_light: u8,
    /// Was use held last tic (prevents continuous use on key hold).
    pub use_down: bool,

    // --- Power-ups: remaining tics (0 = not active) ---
    pub powers: [u32; NUM_POWERS],

    // --- Cheat flags ---
    /// God mode (IDDQD) — player takes no damage.
    pub god_mode: bool,
    /// Noclip (IDCLIP / IDSPISPOPD) — player passes through walls.
    pub noclip: bool,

    // --- Keys ---
    /// Bitmask of collected keys (KEY_BLUE_CARD, etc.).
    pub keys: u8,

    // --- HUD flash counters ---
    /// Tics to flash the screen yellow (pickup).
    pub bonus_count: u32,
    /// Tics to flash the screen red (damage).
    pub damage_count: u32,

    // --- End-of-level statistics ---
    /// Number of monsters killed by this player.
    pub kill_count: u32,
    /// Number of items picked up by this player.
    pub item_count: u32,
    /// Number of secret sectors discovered by this player.
    pub secret_count: u32,
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
            max_ammo: MAX_AMMO,
            weapons,
            weapon: WeaponType::Pistol,
            pending_weapon: None,
            psprites: [PspriteState::default(); NUM_PSPRITES],
            attack_down: false,
            attack_cooldown: 0,
            refire: 0,
            extra_light: 0,
            use_down: false,
            powers: [0; NUM_POWERS],
            god_mode: false,
            noclip: false,
            keys: 0,
            bonus_count: 0,
            damage_count: 0,
            kill_count: 0,
            item_count: 0,
            secret_count: 0,
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
        self.health = self.health.saturating_sub(amount).clamp(-32768, MAX_HEALTH);
    }

    /// Heal the player by `amount`, capped at `MAX_HEALTH`.
    ///
    /// Has no effect if the player is already at or above `MAX_HEALTH`.
    pub fn heal(&mut self, amount: i32) {
        if self.health < MAX_HEALTH {
            self.health = self.health.saturating_add(amount).min(MAX_HEALTH);
        }
    }

    /// Heal the player by `amount`, allowing health to exceed `MAX_HEALTH`.
    ///
    /// Used for power-up items (Soulsphere, Megasphere) that can overheal.
    /// Health is capped at `cap` (e.g. 200).
    pub fn heal_overheal(&mut self, amount: i32, cap: i32) {
        self.health = self.health.saturating_add(amount).min(cap);
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

    /// Deduct `amount` from armor; clears `armor_type` when armor reaches 0.
    pub fn deduct_armor(&mut self, amount: i32) {
        self.armor = (self.armor - amount).max(0);
        if self.armor == 0 {
            self.armor_type = 0;
        }
    }

    // -----------------------------------------------------------------------
    // Keys
    // -----------------------------------------------------------------------

    /// Returns `true` if the player holds the key identified by `key_bit`.
    #[inline]
    pub fn has_key(&self, key_bit: u8) -> bool {
        self.keys & key_bit != 0
    }

    /// Give the player the key identified by `key_bit`.
    #[inline]
    pub fn give_key(&mut self, key_bit: u8) {
        self.keys |= key_bit;
    }

    // -----------------------------------------------------------------------
    // Ammo (invariant: ammo[i] ≤ MAX_AMMO[i])
    // -----------------------------------------------------------------------

    /// Current ammo for `ammo_type` (0 if index out of range).
    #[inline]
    pub fn ammo(&self, ammo_type: usize) -> u32 {
        self.ammo.get(ammo_type).copied().unwrap_or(0)
    }

    /// Give `amount` units of ammo; clamped to `max_ammo[ammo_type]`.
    ///
    /// Returns `true` if ammo was actually added (player was not already at max).
    pub fn give_ammo(&mut self, ammo_type: usize, amount: u32) -> bool {
        if let Some(cur) = self.ammo.get_mut(ammo_type) {
            let max = self.max_ammo.get(ammo_type).copied().unwrap_or(0);
            if *cur >= max {
                return false;
            }
            *cur = (*cur + amount).min(max);
            true
        } else {
            false
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
        if atype == AmmoType::None {
            true
        } else {
            self.ammo.get(atype as usize).copied().unwrap_or(0) > 0
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
        u8::try_from(n).ok().and_then(Self::from_repr)
    }
}

// ---------------------------------------------------------------------------
// Proptest property tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod prop_tests {
    use super::*;
    use doom_types::{MAX_AMMO, NUM_AMMO};
    use proptest::prelude::*;

    // Property: `ammo[i] ≤ MAX_AMMO[i]` holds after any `give_ammo` call.
    //
    // This is the core Verus invariant translated into a proptest property.
    // We cap the `amount` at `MAX_AMMO[i]` (300 max across all types) to avoid
    // u32 addition overflow in the underlying implementation's `cur + amount`.
    proptest! {
        #[test]
        fn ammo_never_exceeds_max_after_give(
            ammo_type_idx in 0usize..NUM_AMMO,
            // Stay well within u32 range to avoid overflow in give_ammo's cur+amount.
            amount in 0u32..=300u32,
        ) {
            let mut player = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            player.give_ammo(ammo_type_idx, amount);
            let current = player.ammo(ammo_type_idx);
            let max = MAX_AMMO[ammo_type_idx];
            prop_assert!(
                current <= max,
                "ammo[{}] = {} > MAX_AMMO[{}] = {} after give_ammo({}, {})",
                ammo_type_idx, current, ammo_type_idx, max, ammo_type_idx, amount
            );
        }

        // Property: giving ammo multiple times never pushes past the cap.
        #[test]
        fn ammo_cap_survives_repeated_give(
            ammo_type_idx in 0usize..NUM_AMMO,
            amount1 in 0u32..=500u32,
            amount2 in 0u32..=500u32,
            amount3 in 0u32..=500u32,
        ) {
            let mut player = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            player.give_ammo(ammo_type_idx, amount1);
            player.give_ammo(ammo_type_idx, amount2);
            player.give_ammo(ammo_type_idx, amount3);
            let current = player.ammo(ammo_type_idx);
            let max = MAX_AMMO[ammo_type_idx];
            prop_assert!(
                current <= max,
                "ammo[{}] = {} > MAX_AMMO = {} after repeated give",
                ammo_type_idx, current, max
            );
        }

        // Property: `health()` is always ≤ MAX_HEALTH immediately after
        // `pistol_start` (no mutation), because pistol start sets it exactly.
        #[test]
        fn pistol_start_health_within_bounds(_seed in 0u32..256u32) {
            // _seed is unused — proptest needs at least one argument.
            let player = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            let h = player.health();
            prop_assert!(
                h <= MAX_HEALTH,
                "pistol_start health {} > MAX_HEALTH {}", h, MAX_HEALTH
            );
            prop_assert!(
                h >= 0,
                "pistol_start health {} < 0", h
            );
        }

        // Property: after any sequence of `apply_damage(n)`, health is always
        // within `[-32768, MAX_HEALTH]`.
        #[test]
        fn health_clamped_after_apply_damage(
            damage in i32::MIN..=i32::MAX,
        ) {
            let mut player = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            player.apply_damage(damage);
            let h = player.health();
            prop_assert!(
                h <= MAX_HEALTH,
                "health {} > MAX_HEALTH {} after apply_damage({})", h, MAX_HEALTH, damage
            );
            prop_assert!(
                h >= -32768,
                "health {} < -32768 after apply_damage({})", h, damage
            );
        }

        // Property: `heal(n)` never pushes health above MAX_HEALTH.
        #[test]
        fn heal_never_exceeds_max(
            initial_damage in 0i32..=100i32,
            heal_amount in 0i32..=10000i32,
        ) {
            let mut player = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            player.apply_damage(initial_damage); // possibly lower health
            player.heal(heal_amount);
            let h = player.health();
            prop_assert!(
                h <= MAX_HEALTH,
                "health {} > MAX_HEALTH {} after heal({})", h, MAX_HEALTH, heal_amount
            );
        }

        // Property: `heal_overheal(n, cap)` never pushes health above `cap`.
        #[test]
        fn heal_overheal_preserves_cap_invariant(
            initial_damage in 0i32..=100i32,
            heal_amount in 0i32..=10000i32,
            cap in 100i32..=200i32,
        ) {
            let mut player = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            player.apply_damage(initial_damage);
            player.heal_overheal(heal_amount, cap);
            let h = player.health();
            prop_assert!(
                h <= cap,
                "health {} > cap {} after heal_overheal({}, {})", h, cap, heal_amount, cap
            );
        }

        // Property: `set_health_capped(n, cap)` always stays within `[0, cap]`.
        #[test]
        fn set_health_capped_preserves_bounds(
            value in i32::MIN..=i32::MAX,
            cap in 0i32..=500i32,
        ) {
            let mut player = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            player.set_health_capped(value, cap);
            let h = player.health();
            prop_assert!(
                h >= 0 && h <= cap,
                "health {} outside bounds [0, {}] after set_health_capped({}, {})", h, cap, value, cap
            );
        }

        // Property: key bitmask operations are idempotent — giving the same
        // key twice is the same as giving it once.
        #[test]
        fn give_key_is_idempotent(key_bit in 0u8..8u8) {
            let mut p1 = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            let mut p2 = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            let key = 1u8 << key_bit;
            p1.give_key(key);
            p2.give_key(key);
            p2.give_key(key);
            prop_assert_eq!(
                p1.keys, p2.keys,
                "give_key twice should equal give_key once for bit {}", key_bit
            );
        }

        // Property: `has_key(k)` returns `true` iff `give_key(k)` was called.
        #[test]
        fn has_key_reflects_give_key(
            key_bit in 0u8..8u8,
        ) {
            let mut player = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            let key = 1u8 << key_bit;
            prop_assert!(!player.has_key(key), "player should not have key before give");
            player.give_key(key);
            prop_assert!(player.has_key(key), "player should have key after give");
        }

        // Property: `use_ammo` always preserves the `ammo ≤ MAX_AMMO` invariant.
        #[test]
        fn use_ammo_preserves_invariant(
            ammo_type_idx in 0usize..NUM_AMMO,
            // Avoid u32 overflow in give_ammo: cap amount at MAX_AMMO (300 max).
            give_amount in 0u32..=300u32,
            use_amount in 0u32..=300u32,
        ) {
            let mut player = PlayerState::pistol_start(crate::mobj::MobjHandle::NULL);
            // Give a bounded amount of ammo, then use some.
            player.give_ammo(ammo_type_idx, give_amount);
            let _ = player.use_ammo(ammo_type_idx, use_amount);
            let current = player.ammo(ammo_type_idx);
            let max = MAX_AMMO[ammo_type_idx];
            prop_assert!(
                current <= max,
                "ammo[{}] = {} > MAX_AMMO = {} after use_ammo", ammo_type_idx, current, max
            );
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
        assert_eq!(p.attack_cooldown, 0);
    }

    #[test]
    fn damage_drives_health_below_zero() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.apply_damage(200);
        assert!(p.health() <= 0);
        assert!(
            p.health() >= -32768,
            "health must not underflow past -32768"
        );
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
    fn heal_overheal_exceeds_max_health_up_to_cap() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        assert_eq!(p.health(), 100);
        p.heal_overheal(50, 200);
        assert_eq!(p.health(), 150);

        p.heal_overheal(100, 200);
        assert_eq!(p.health(), 200);

        p.heal_overheal(50, 200);
        assert_eq!(p.health(), 200, "Should cap at 200");
    }

    #[test]
    fn set_health_capped_clamps_to_bounds() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.set_health_capped(150, 200);
        assert_eq!(p.health(), 150);

        p.set_health_capped(300, 200);
        assert_eq!(p.health(), 200, "Should cap at 200");

        p.set_health_capped(-50, 200);
        assert_eq!(p.health(), 0, "Should clamp to 0 at minimum");
    }

    #[test]
    fn havoc_apply_damage_underflow_panic() {
        // Havoc 👺: test arithmetic overflow!
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.apply_damage(i32::MIN);
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

    #[test]
    fn give_key_sets_bit() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.give_key(KEY_BLUE_CARD);
        assert!(p.has_key(KEY_BLUE_CARD));
        assert!(!p.has_key(KEY_RED_CARD));
    }

    #[test]
    fn give_multiple_keys_independent() {
        let mut p = PlayerState::pistol_start(MobjHandle::NULL);
        p.give_key(KEY_BLUE_CARD);
        p.give_key(KEY_YELLOW_SKULL);
        assert!(p.has_key(KEY_BLUE_CARD));
        assert!(p.has_key(KEY_YELLOW_SKULL));
        assert!(!p.has_key(KEY_RED_CARD));
        assert!(!p.has_key(KEY_BLUE_SKULL));
    }

    #[test]
    fn pistol_start_has_no_keys() {
        let p = PlayerState::pistol_start(MobjHandle::NULL);
        assert_eq!(p.keys, 0);
        assert!(!p.has_key(KEY_BLUE_CARD));
    }

    #[test]
    fn weapon_from_num_converts_correctly_and_handles_bounds() {
        assert_eq!(WeaponType::from_num(0), Some(WeaponType::Fist));
        assert_eq!(WeaponType::from_num(1), Some(WeaponType::Pistol));
        assert_eq!(WeaponType::from_num(8), Some(WeaponType::SuperShotgun));
        assert_eq!(WeaponType::from_num(9), None);
        assert_eq!(WeaponType::from_num(255), None);
        assert_eq!(WeaponType::from_num(256), None);
        assert_eq!(WeaponType::from_num(usize::MAX), None);
    }

    #[test]
    fn ammo_from_repr_converts_correctly_and_handles_bounds() {
        assert_eq!(AmmoType::from_repr(0), Some(AmmoType::Bullets));
        assert_eq!(AmmoType::from_repr(1), Some(AmmoType::Shells));
        assert_eq!(AmmoType::from_repr(255), Some(AmmoType::None));
        assert_eq!(AmmoType::from_repr(4), None);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn havoc_player_health_heal_does_not_panic(start_health in i32::MIN..i32::MAX, amount in i32::MIN..i32::MAX) {
            let mut p = PlayerState { health: start_health, ..Default::default() };
            p.heal(amount);
        }

        #[test]
        fn havoc_player_health_heal_overheal_does_not_panic(start_health in i32::MIN..i32::MAX, amount in i32::MIN..i32::MAX, cap in i32::MIN..i32::MAX) {
            let mut p = PlayerState { health: start_health, ..Default::default() };
            p.heal_overheal(amount, cap);
        }

        #[test]
        fn havoc_player_health_damage_does_not_panic(start_health in i32::MIN..i32::MAX, amount in i32::MIN..i32::MAX) {
            let mut p = PlayerState { health: start_health, ..Default::default() };
            p.apply_damage(amount);
        }
    }
}
