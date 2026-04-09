/// Species / type of a map object.
///
/// Values are stable (`repr u16`) for wire serialization and demo compatibility.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::FromRepr, strum_macros::EnumIter,
)]
#[repr(u16)]
pub enum MobjKind {
    // Players
    Player = 0,

    // Monsters
    Trooper = 1,  // Zombie man
    Sergeant = 2, // Shotgun guy
    Imp = 3,
    Demon = 4,
    Spectre = 5,
    LostSoul = 6,
    Cacodemon = 7,
    BaronOfHell = 8,
    HellKnight = 9,
    Arachnotron = 10,
    PainElemental = 11,
    Revenant = 12,
    Mancubus = 13,
    ArchVile = 14,
    SpiderMastermind = 15,
    Cyberdemon = 16,
    WolfSS = 17,

    // Visual effects
    BulletPuff = 18,
    Blood = 19,
    SmokeTrail = 20,
    SpawnFire = 21,

    // Projectiles
    Rocket = 22,
    PlasmaBall = 23,
    BfgBall = 24,
    ArachPlaz = 25,
    Tracer = 26,

    // Pickups — weapons
    BfgPickup = 27,
    Chaingun = 28,
    Chainsaw = 29,
    RocketLauncher = 30,
    PlasmaRifle = 31,
    Shotgun = 32,
    SuperShotgun = 33,

    // Pickups — ammo
    Clip = 34,
    ClipBox = 35,
    RocketAmmo = 36,
    RocketBox = 37,
    Cell = 38,
    CellPack = 39,
    Shell = 40,
    ShellBox = 41,

    // Pickups — health & armor
    HealthBonus = 42,
    ArmorBonus = 43,
    GreenArmor = 44,
    BlueArmor = 45,
    Stimpack = 46,
    Medikit = 47,
    Megasphere = 48,
    Soulsphere = 49,

    // Pickups — keys
    BlueCard = 50,
    RedCard = 51,
    YellowCard = 52,
    BlueSkull = 53,
    RedSkull = 54,
    YellowSkull = 55,

    // Pickups — power-ups
    Berserk = 56,
    BlurSphere = 57,
    RadSuit = 58,
    Allmap = 59,
    Infrared = 60,

    // Misc
    Column = 61,
    TechLamp = 62,
    TechLamp2 = 63,
    Barrel = 64,
    BossBrain = 65,
    CommanderKeen = 66,

    // Additional projectiles (Batch 21)
    BfgExtra = 67,     // BFG tracers (secondary damage)
    ImpFireball = 68,  // Imp ranged attack
    CacoFireball = 69, // Cacodemon ranged attack
    BaronBall = 70,    // Baron/Hell Knight plasma ball
    FatShot = 71,      // Mancubus fireball

    // Pickups — missing items (Batch: item pickups)
    InvulnerabilitySphere = 72, // DoomEd 2022 — invulnerability power-up
    Backpack = 73,              // DoomEd 8 — doubles max ammo + gives ammo

    // Arch-Vile fire column (visual effect that tracks the target)
    VileFire = 74,
    // Boss Brain cube projectile (flies to spawn spots, morphs into monster)
    BossCube = 75,
}
