//! Power-up constants.

/// Power-up slot indices.
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


/// Psprite slot indices matching vanilla Doom's `ps_weapon` / `ps_flash`.
pub mod psprite_slots {
/// Main weapon sprite.
pub const WEAPON: usize = 0;
/// Muzzle-flash overlay sprite.
pub const FLASH: usize = 1;
}
