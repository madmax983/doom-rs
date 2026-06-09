//! The Arsenal of the Marine: Weapon and Ammo primitives.
//!
//! This module defines the core abstractions for the player's arsenal. In the Doom engine,
//! physical weapons (`WeaponType`) are completely decoupled from the logical ammunition
//! pools (`AmmoType`) that fuel them. This allows multiple weapons (like the Pistol and
//! Chaingun) to draw from the same shared pool of bullets, ensuring that resource management
//! is centralized.
//!
//! It also defines the lookup table `WEAPON_AMMO` to map each weapon to its corresponding
//! ammunition type, enabling generic weapon logic during gameplay.

use crate::limits::NUM_WEAPONS;

/// Represents the specific physical weapon currently equipped or available.
///
/// The indices directly correlate with the keyboard selection keys (1-7), with the
/// exception of the chainsaw, which shares the '1' key with the fist but sits at the
/// end of the enumeration.
///
/// ## Examples
/// ```
/// use doom_types::weapons::WeaponType;
///
/// let w = WeaponType::Shotgun;
/// assert_eq!(w as u8, 2);
/// ```
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum WeaponType {
    /// Bare fists.
    Fist = 0,
    /// Standard starting pistol.
    #[default]
    Pistol = 1,
    /// Pump-action shotgun.
    Shotgun = 2,
    /// Rapid-fire chaingun.
    Chaingun = 3,
    /// Explosive rocket launcher.
    RocketLauncher = 4,
    /// Rapid-fire plasma rifle.
    PlasmaRifle = 5,
    /// Big Fucking Gun 9000.
    Bfg = 6,
    /// Melee chainsaw.
    Chainsaw = 7,
    /// Double-barreled super shotgun (Doom II).
    SuperShotgun = 8,
}

/// Represents the shared logical ammunition pools.
///
/// Instead of weapons tracking their own ammo, the player's inventory tracks these
/// specific pools. Melee weapons explicitly use `AmmoType::None` to bypass ammo consumption
/// checks entirely.
///
/// ## Examples
/// ```
/// use doom_types::weapons::AmmoType;
///
/// let a = AmmoType::Shells;
/// assert_eq!(a as u8, 1);
/// ```
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AmmoType {
    /// Ammo for Pistol and Chaingun.
    Bullets = 0,
    /// Ammo for Shotgun and Super Shotgun.
    Shells = 1,
    /// Ammo for Plasma Rifle and BFG.
    Cells = 2,
    /// Ammo for Rocket Launcher.
    Rockets = 3,
    /// Melee weapons (Fist, Chainsaw) — no ammo consumed.
    None = 255,
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

impl WeaponType {
    /// Parses an integer weapon number (typically masked from the network `TicCmd`)
    /// into a valid `WeaponType`.
    ///
    /// This is crucial for safely converting unvalidated network input into safe enums.
    ///
    /// ## Examples
    /// ```
    /// use doom_types::weapons::WeaponType;
    ///
    /// // Network sends us a '3'
    /// let weapon = WeaponType::from_num(3);
    /// assert_eq!(weapon, Some(WeaponType::Chaingun));
    ///
    /// // Invalid input gracefully fails
    /// assert_eq!(WeaponType::from_num(99), None);
    /// ```
    pub fn from_num(n: usize) -> Option<Self> {
        u8::try_from(n).ok().and_then(Self::from_repr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_type_from_num_valid() {
        assert_eq!(WeaponType::from_num(0), Some(WeaponType::Fist));
        assert_eq!(WeaponType::from_num(1), Some(WeaponType::Pistol));
        assert_eq!(WeaponType::from_num(2), Some(WeaponType::Shotgun));
        assert_eq!(WeaponType::from_num(3), Some(WeaponType::Chaingun));
        assert_eq!(WeaponType::from_num(4), Some(WeaponType::RocketLauncher));
        assert_eq!(WeaponType::from_num(5), Some(WeaponType::PlasmaRifle));
        assert_eq!(WeaponType::from_num(6), Some(WeaponType::Bfg));
        assert_eq!(WeaponType::from_num(7), Some(WeaponType::Chainsaw));
        assert_eq!(WeaponType::from_num(8), Some(WeaponType::SuperShotgun));
    }

    #[test]
    fn weapon_type_from_num_invalid() {
        assert_eq!(WeaponType::from_num(9), None);
        assert_eq!(WeaponType::from_num(100), None);
        assert_eq!(WeaponType::from_num(usize::MAX), None);
    }
}
