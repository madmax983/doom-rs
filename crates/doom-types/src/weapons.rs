//! Weapon and ammo primitives.
use crate::limits::NUM_WEAPONS;

/// Weapon slots (index = selection key − 1 for keys 1-7; chainsaw = key 1 alt).
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

/// Ammo pool indices.
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
///
/// ## Examples
/// ```
/// use doom_types::weapons::{WEAPON_AMMO, WeaponType, AmmoType};
///
/// let shotgun_ammo = WEAPON_AMMO[WeaponType::Shotgun as usize];
/// assert_eq!(shotgun_ammo, AmmoType::Shells);
/// ```
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
    /// Convert a weapon number (0–8) from `BT_WEAPONMASK` to a `WeaponType`.
    ///
    /// Returns `None` for any out-of-range value.
    ///
    /// ## Examples
    /// ```
    /// use doom_types::weapons::WeaponType;
    ///
    /// let w = WeaponType::from_num(2);
    /// assert_eq!(w, Some(WeaponType::Shotgun));
    ///
    /// let invalid = WeaponType::from_num(99);
    /// assert_eq!(invalid, None);
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
