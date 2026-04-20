/// Weapon slots (index = selection key − 1 for keys 1-7; chainsaw = key 1 alt).
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

impl WeaponType {
    /// Returns an iterator over all `WeaponType` variants.
    pub fn weapons() -> impl Iterator<Item = WeaponType> {
        [
            WeaponType::Fist,
            WeaponType::Pistol,
            WeaponType::Shotgun,
            WeaponType::Chaingun,
            WeaponType::RocketLauncher,
            WeaponType::PlasmaRifle,
            WeaponType::Bfg,
            WeaponType::Chainsaw,
            WeaponType::SuperShotgun,
        ]
        .into_iter()
    }
}
