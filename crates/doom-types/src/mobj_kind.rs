//! Map object kinds.
//!
//! The `mobj_kind` module defines the enumeration of all possible dynamic map objects (`MobjKind`) that can exist in the Doom game world. These are the actors that populate the map, ranging from players and monsters to projectiles and powerups.
//!
//! ## Serialization
//! Note that `MobjKind` uses a stable `repr(u16)` definition for demo compatibility and network serialization. Changing these values will break demo playback and savefiles.

/// Species / type of a map object.
///
/// Represents the distinct class of an actor within the map.
/// Every dynamic entity in the game loop has an associated `MobjKind`, which determines its behavior, sprites, health, and interactions.
///
/// Values are stable (`repr u16`) for wire serialization and demo compatibility.
///
/// ## Examples
/// ```
/// use doom_types::mobj_kind::MobjKind;
///
/// let monster = MobjKind::Imp;
/// assert_eq!(monster as u16, 3);
/// ```
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::FromRepr, strum_macros::EnumIter,
)]
#[repr(u16)]
pub enum MobjKind {
    // Players
    /// The human-controlled space marine.
    ///
    /// The player is the primary agent in the world, capable of picking up items, firing weapons, and interacting with the environment.
    Player = 0,

    // Monsters
    /// Zombie man.
    ///
    /// The weakest enemy in the game. A former human soldier armed with a basic rifle.
    Trooper = 1,
    /// Shotgun guy.
    ///
    /// A former human sergeant armed with a deadly shotgun. Highly dangerous at close range.
    Sergeant = 2,
    /// Imp.
    ///
    /// A common, brown humanoid demon that throws fireballs and slashes at close range.
    Imp = 3,
    /// Demon (Pinky).
    ///
    /// A fast, pink demonic beast that relies entirely on a biting melee attack.
    Demon = 4,
    /// Spectre.
    ///
    /// A partially invisible variant of the Demon (Pinky), making it harder to spot in low-light conditions.
    Spectre = 5,
    /// Lost Soul.
    ///
    /// A flying, flaming skull that charges rapidly at its target. Often spat out by Pain Elementals.
    LostSoul = 6,
    /// Cacodemon.
    ///
    /// A large, floating red spherical demon that spits lightning balls. Iconic and resilient.
    Cacodemon = 7,
    /// Baron of Hell.
    ///
    /// A tough, muscular minotaur-like demon that hurls green plasma fireballs. Found at the end of Knee-Deep in the Dead.
    BaronOfHell = 8,
    /// Hell Knight.
    ///
    /// A slightly weaker, pale-skinned variant of the Baron of Hell introduced in Doom II.
    HellKnight = 9,
    /// Arachnotron.
    ///
    /// A cybernetic spider-like demon armed with a rapid-fire plasma cannon.
    Arachnotron = 10,
    /// Pain Elemental.
    ///
    /// A brownish, floating flesh-orb that continuously spawns Lost Souls instead of attacking directly.
    PainElemental = 11,
    /// Revenant.
    ///
    /// A tall, screaming skeleton equipped with shoulder-mounted rocket launchers that fire homing missiles.
    Revenant = 12,
    /// Mancubus.
    ///
    /// A massive, sluggish demon with dual flamethrower arms that shoot wide-spread fireballs.
    Mancubus = 13,
    /// Arch-Vile.
    ///
    /// A fast, highly dangerous spellcaster that summons explosive pillars of fire and resurrects dead monsters.
    ArchVile = 14,
    /// Spider Mastermind.
    ///
    /// The colossal boss of the original Doom, riding a mechanical chassis equipped with a devastating chaingun.
    SpiderMastermind = 15,
    /// Cyberdemon.
    ///
    /// A towering, cybernetic minotaur with a rocket launcher replacing its right arm. The ultimate brute-force boss.
    Cyberdemon = 16,
    /// Wolfenstein SS.
    ///
    /// A blue-clad Nazi soldier from Wolfenstein 3D, appearing only in secret levels.
    WolfSS = 17,

    // Visual effects
    /// Bullet puff.
    ///
    /// The visual spark and smoke effect spawned when a hitscan weapon strikes a wall.
    BulletPuff = 18,
    /// Blood splatter.
    ///
    /// The visual effect spawned when a hitscan weapon strikes a bleadable target (like a monster).
    Blood = 19,
    /// Smoke trail.
    ///
    /// The trailing smoke left behind by flying rockets.
    SmokeTrail = 20,
    /// Spawn fire.
    ///
    /// The green flame and fog effect created when a monster teleports into the level.
    SpawnFire = 21,

    // Projectiles
    /// Rocket projectile.
    ///
    /// A deadly explosive fired by the player's Rocket Launcher or the Cyberdemon.
    Rocket = 22,
    /// Plasma ball projectile.
    ///
    /// A fast-moving blue energy sphere fired by the Plasma Rifle or Arachnotrons.
    PlasmaBall = 23,
    /// BFG ball projectile.
    ///
    /// The massive, slow-moving green orb of annihilation fired by the BFG 9000.
    BfgBall = 24,
    /// Arachnotron plasma projectile.
    ///
    /// The distinct yellow/green plasma volley fired by the Arachnotron.
    ArachPlaz = 25,
    /// Revenant tracer missile.
    ///
    /// The screaming, smoke-trailing rocket fired by the Revenant. Often homes in on its target.
    Tracer = 26,

    // Pickups — weapons
    /// BFG 9000 pickup.
    ///
    /// The ultimate weapon. "Big Fucking Gun." Vaporizes entire rooms of enemies.
    BfgPickup = 27,
    /// Chaingun pickup.
    ///
    /// A rapid-fire hitscan weapon. Excellent for stunning enemies.
    Chaingun = 28,
    /// Chainsaw pickup.
    ///
    /// "The Great Communicator." A melee weapon that deals rapid damage and locks demons in pain states.
    Chainsaw = 29,
    /// Rocket launcher pickup.
    ///
    /// A heavy weapon firing explosive ordnance. Use with caution in tight spaces.
    RocketLauncher = 30,
    /// Plasma rifle pickup.
    ///
    /// A high-tech energy weapon that unleashes a torrent of blue plasma.
    PlasmaRifle = 31,
    /// Shotgun pickup.
    ///
    /// The reliable pump-action workhorse of the Doom marine's arsenal.
    Shotgun = 32,
    /// Super shotgun pickup.
    ///
    /// A sawed-off, double-barreled harbinger of point-blank destruction.
    SuperShotgun = 33,

    // Pickups — ammo
    /// Clip ammo pickup.
    ///
    /// Provides 10 bullets for the Pistol and Chaingun.
    Clip = 34,
    /// Box of bullets ammo pickup.
    ///
    /// A large ammunition container providing 50 bullets.
    ClipBox = 35,
    /// Rocket ammo pickup.
    ///
    /// Provides 1 explosive rocket.
    RocketAmmo = 36,
    /// Box of rockets ammo pickup.
    ///
    /// A large crate providing 5 rockets.
    RocketBox = 37,
    /// Cell ammo pickup.
    ///
    /// Provides 20 energy cells for the Plasma Rifle and BFG.
    Cell = 38,
    /// Cell pack ammo pickup.
    ///
    /// A bulk battery providing 100 energy cells.
    CellPack = 39,
    /// Shell ammo pickup.
    ///
    /// Provides 4 shotgun shells.
    Shell = 40,
    /// Box of shells ammo pickup.
    ///
    /// A large container providing 20 shotgun shells.
    ShellBox = 41,

    // Pickups — health & armor
    /// Health bonus.
    ///
    /// A glowing blue vial that grants 1% health, even pushing the player beyond the normal 100% cap.
    HealthBonus = 42,
    /// Armor bonus.
    ///
    /// A glowing green helmet that grants 1% armor, even pushing the player beyond the normal 100% cap.
    ArmorBonus = 43,
    /// Green armor.
    ///
    /// A standard security vest that provides 100% armor and absorbs 1/3 of incoming damage.
    GreenArmor = 44,
    /// Blue armor.
    ///
    /// Heavy combat armor that provides 200% armor and absorbs 1/2 of incoming damage.
    BlueArmor = 45,
    /// Stimpack.
    ///
    /// A small medical kit that restores 10% health (up to 100%).
    Stimpack = 46,
    /// Medikit.
    ///
    /// A large medical kit that restores 25% health (up to 100%).
    Medikit = 47,
    /// Megasphere.
    ///
    /// A powerful brown artifact that maximizes the player's resilience, granting 200% health and 200% armor.
    Megasphere = 48,
    /// Soulsphere.
    ///
    /// A swirling blue orb, sometimes called a Supercharge, that grants 100% health (up to a max of 200%).
    Soulsphere = 49,

    // Pickups — keys
    /// Blue keycard.
    ///
    /// Grants access to tech-base doors with blue security panels.
    BlueCard = 50,
    /// Red keycard.
    ///
    /// Grants access to tech-base doors with red security panels.
    RedCard = 51,
    /// Yellow keycard.
    ///
    /// Grants access to tech-base doors with yellow security panels.
    YellowCard = 52,
    /// Blue skull key.
    ///
    /// Grants access to demonic or hellish doors requiring a blue insignia.
    BlueSkull = 53,
    /// Red skull key.
    ///
    /// Grants access to demonic or hellish doors requiring a red insignia.
    RedSkull = 54,
    /// Yellow skull key.
    ///
    /// Grants access to demonic or hellish doors requiring a yellow insignia.
    YellowSkull = 55,

    // Pickups — power-ups
    /// Berserk pack.
    ///
    /// A black medical box that fully heals the player and imbues their fists with tremendous, bone-crushing strength.
    Berserk = 56,
    /// Blur sphere.
    ///
    /// A red orb that grants partial invisibility, causing enemy projectiles to fire wildly off-target.
    BlurSphere = 57,
    /// Radiation shielding suit.
    ///
    /// A green hazmat suit that temporarily protects the player from damaging floor hazards like slime and lava.
    RadSuit = 58,
    /// Computer area map.
    ///
    /// A device that reveals the entire level layout on the automap, including hidden areas.
    Allmap = 59,
    /// Light amplification visor.
    ///
    /// High-tech goggles that temporarily illuminate the entire map to maximum brightness.
    Infrared = 60,

    // Misc
    /// Decorative column.
    ///
    /// A tall, solid obstacle often used for aesthetic detailing or cover.
    Column = 61,
    /// Tech lamp.
    ///
    /// A tall, glowing technological light source that blocks movement.
    TechLamp = 62,
    /// Short tech lamp.
    ///
    /// A shorter, wider variant of the standard tech lamp.
    TechLamp2 = 63,
    /// Exploding barrel.
    ///
    /// A volatile container of toxic waste that detonates violently when shot, damaging everything nearby.
    Barrel = 64,
    /// Boss brain.
    ///
    /// The true final boss of Doom II—John Romero's head on a stick, hidden behind the Icon of Sin's wall texture.
    BossBrain = 65,
    /// Commander Keen.
    ///
    /// An easter egg character hanging from a noose. Shooting him usually opens a secret exit.
    CommanderKeen = 66,

    // Additional projectiles (Batch 21)
    /// BFG invisible tracers.
    ///
    /// Invisible "rays" spawned from the player shortly after the main BFG ball detonates, dealing massive secondary damage.
    BfgExtra = 67,
    /// Imp fireball.
    ///
    /// The standard, slow-moving projectile thrown by Imps.
    ImpFireball = 68,
    /// Cacodemon lightning ball.
    ///
    /// The crackling, highly damaging spherical projectile spat by Cacodemons.
    CacoFireball = 69,
    /// Baron fireball.
    ///
    /// The green, comet-like plasma projectile hurled by both Barons of Hell and Hell Knights.
    BaronBall = 70,
    /// Mancubus fireball.
    ///
    /// Large, wide-spreading fireballs fired in volleys by the Mancubus.
    FatShot = 71,

    // Pickups — missing items (Batch: item pickups)
    /// Invulnerability sphere.
    ///
    /// A green demonic artifact that grants total immunity to all damage (except telefragging) and inverts the screen colors.
    InvulnerabilitySphere = 72,
    /// Backpack.
    ///
    /// A tactical supply pack that permanently doubles the maximum carrying capacity for all ammo types and provides a small amount of each.
    Backpack = 73,

    // Arch-Vile fire column (visual effect that tracks the target)
    /// Arch-Vile fire.
    ///
    /// The devastating, tracking pillar of flame summoned by the Arch-Vile before its blast attack lands.
    VileFire = 74,
    // Boss Brain cube projectile (flies to spawn spots, morphs into monster)
    /// Boss spawn cube.
    ///
    /// The cubic projectile launched by the Icon of Sin, which travels to a target location and materializes into a random demon.
    BossCube = 75,
}
