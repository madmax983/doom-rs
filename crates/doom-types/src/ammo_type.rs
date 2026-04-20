/// Ammo pool indices.
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
