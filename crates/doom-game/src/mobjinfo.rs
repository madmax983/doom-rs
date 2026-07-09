//! Actor properties table — one `MobjInfo` entry per `MobjKind`.
//!
//! Indexed by `MobjKind as usize`.  Values are drawn from Doom's original
//! `mobjinfo` table in `info.c`, trimmed to the fields needed for Batch 2
//! (health, speed, radius, height, mass, flags, pain chance, state refs).
//!
//! Non-monster entries (pickups, decorations, projectiles) use conservative
//! defaults — their AI fields are all `S_NULL`.

use crate::mobj::{StateNum, flags};
use crate::states::ids;
use doom_types::Fixed16_16;
use doom_types::mobj_kind::MobjKind;

// ---------------------------------------------------------------------------
// MobjInfo struct
// ---------------------------------------------------------------------------

/// Per-actor-type properties loaded from the original Doom info table.
#[derive(Clone, Copy, Debug)]
pub struct MobjInfo {
    /// Starting health when the actor is spawned.
    pub spawn_health: i32,
    /// Movement speed (map units per tic, fixed-point).
    pub speed: Fixed16_16,
    /// Collision radius (fixed-point map units).
    pub radius: Fixed16_16,
    /// Collision height (fixed-point map units).
    pub height: Fixed16_16,
    /// Mass (affects knockback; `i32::MAX` = immovable).
    pub mass: i32,
    /// Behavior flags (`flags::MF_*` bitmask).
    pub flags: u32,
    /// Probability of flinching when hit (0..=255, 255 = always).
    pub pain_chance: u8,
    /// Idle spawn state (A_Look runs here).
    pub spawn_state: StateNum,
    /// First chase state (A_Chase runs here), entered when the monster sees
    /// the player.
    pub see_state: StateNum,
    /// Pain reaction state (entered when damaged, Batch 3).
    pub pain_state: StateNum,
    /// Death animation start state (Batch 3).
    pub death_state: StateNum,
    /// State entered when monster decides to do a melee attack (A_Chase
    /// triggers this when within `MELEERANGE`). `S_NULL` = no melee attack.
    pub melee_state: StateNum,
    /// State entered for ranged / missile attacks. `S_NULL` = no ranged
    /// attack.
    pub missile_state: StateNum,
    /// State to resurrect into when an Arch-Vile raises a corpse.
    /// `S_NULL` = cannot be raised.
    pub raise_state: StateNum,
}

// ---------------------------------------------------------------------------
// Const helpers
// ---------------------------------------------------------------------------

const fn fixed(n: i32) -> Fixed16_16 {
    Fixed16_16(n << 16)
}

const fn sn(n: u16) -> StateNum {
    StateNum(n)
}

/// Vanilla `info->reactiontime` for a spawned actor (`info.c`).
///
/// Every monster spawns with `reactiontime = 8`, which delays its first attack
/// (A_Chase decrements it, and P_CheckMeleeRange/P_CheckMissileRange bail while
/// it is nonzero). Non-monster actors do not use it. This must be applied at
/// spawn for demo sync, since it gates the first monster-attack RNG draws.
pub const fn reactiontime(kind: MobjKind) -> i32 {
    match kind {
        MobjKind::Trooper
        | MobjKind::Sergeant
        | MobjKind::Imp
        | MobjKind::Demon
        | MobjKind::Spectre
        | MobjKind::LostSoul
        | MobjKind::Cacodemon
        | MobjKind::BaronOfHell
        | MobjKind::HellKnight
        | MobjKind::Arachnotron
        | MobjKind::PainElemental
        | MobjKind::Revenant
        | MobjKind::Mancubus
        | MobjKind::ArchVile
        | MobjKind::SpiderMastermind
        | MobjKind::Cyberdemon
        | MobjKind::WolfSS => 8,
        _ => 0,
    }
}

/// Combined flags for standard ground monsters.
const MF_MONSTER: u32 = flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;

/// Combined flags for floating monsters (cacodemons, lost souls, etc.).
const MF_FLOAT_MONSTER: u32 = MF_MONSTER | flags::MF_FLOAT | flags::MF_NOGRAVITY;

/// Combined flags for the player.
const MF_PLAYER: u32 = flags::MF_SOLID
    | flags::MF_SHOOTABLE
    | flags::MF_DROPOFF
    | flags::MF_PICKUP
    | flags::MF_NOTDMATCH;

/// Default entry for non-monster actors (pickups, decorations, projectiles).
const ITEM: MobjInfo = MobjInfo {
    spawn_health: 1,
    speed: Fixed16_16(0),
    radius: fixed(20),
    height: fixed(16),
    mass: 100,
    flags: flags::MF_SPECIAL,
    pain_chance: 0,
    spawn_state: sn(ids::S_NULL),
    see_state: sn(ids::S_NULL),
    pain_state: sn(ids::S_NULL),
    death_state: sn(ids::S_NULL),
    melee_state: sn(ids::S_NULL),
    missile_state: sn(ids::S_NULL),
    raise_state: sn(ids::S_NULL),
};

// ---------------------------------------------------------------------------
// Global MOBJINFO table  (indexed by MobjKind as usize, 0..=75)
// ---------------------------------------------------------------------------

/// Actor property table.  Index with `MobjKind as usize`.
///
/// Based on Doom's `mobjinfo[]` in `info.c`.
pub static MOBJINFO: [MobjInfo; 76] = [
    // -----------------------------------------------------------------------
    // 0: Player
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 100,
        speed: Fixed16_16(0),
        radius: fixed(16),
        height: fixed(56),
        mass: 100,
        flags: MF_PLAYER,
        pain_chance: 255,
        spawn_state: sn(ids::S_PLAY),
        see_state: sn(ids::S_NULL),
        pain_state: sn(ids::S_NULL),
        death_state: sn(ids::S_NULL),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_NULL),
        raise_state: sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 1: Trooper (Zombie Man) — MT_POSSESSED
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 20,
        speed: fixed(8),
        radius: fixed(20),
        height: fixed(56),
        mass: 100,
        flags: MF_MONSTER,
        pain_chance: 200,
        spawn_state: sn(ids::S_POSS_STND),
        see_state: sn(ids::S_POSS_RUN1),
        pain_state: sn(ids::S_POSS_PAIN),
        death_state: sn(ids::S_POSS_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_POSS_ATK1),
        raise_state: sn(ids::S_POSS_STND),
    },
    // -----------------------------------------------------------------------
    // 2: Sergeant (Shotgun Guy) — MT_SHOTGUY
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 30,
        speed: fixed(8),
        radius: fixed(20),
        height: fixed(56),
        mass: 100,
        flags: MF_MONSTER,
        pain_chance: 170,
        spawn_state: sn(ids::S_SPOS_STND),
        see_state: sn(ids::S_SPOS_RUN1),
        pain_state: sn(ids::S_SPOS_PAIN),
        death_state: sn(ids::S_SPOS_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_SPOS_ATK1),
        raise_state: sn(ids::S_SPOS_STND),
    },
    // -----------------------------------------------------------------------
    // 3: Imp — MT_TROOP
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 60,
        speed: fixed(8),
        radius: fixed(20),
        height: fixed(56),
        mass: 100,
        flags: MF_MONSTER,
        pain_chance: 200,
        spawn_state: sn(ids::S_TROO_STND),
        see_state: sn(ids::S_TROO_RUN1),
        pain_state: sn(ids::S_TROO_PAIN),
        death_state: sn(ids::S_TROO_DIE1),
        melee_state: sn(ids::S_TROO_ATK1),
        missile_state: sn(ids::S_TROO_ATK1),
        raise_state: sn(ids::S_TROO_STND),
    },
    // -----------------------------------------------------------------------
    // 4: Demon (Pink Demon) — MT_SERGEANT
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 150,
        speed: fixed(10),
        radius: fixed(30),
        height: fixed(56),
        mass: 400,
        flags: MF_MONSTER,
        pain_chance: 180,
        spawn_state: sn(ids::S_SARG_STND),
        see_state: sn(ids::S_SARG_RUN1),
        pain_state: sn(ids::S_SARG_PAIN),
        death_state: sn(ids::S_SARG_DIE1),
        melee_state: sn(ids::S_SARG_ATK1),
        missile_state: sn(ids::S_NULL),
        raise_state: sn(ids::S_SARG_STND),
    },
    // -----------------------------------------------------------------------
    // 5: Spectre — same as Demon + MF_SHADOW
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 150,
        speed: fixed(10),
        radius: fixed(30),
        height: fixed(56),
        mass: 400,
        flags: MF_MONSTER | flags::MF_SHADOW,
        pain_chance: 180,
        spawn_state: sn(ids::S_SARG_STND),
        see_state: sn(ids::S_SARG_RUN1),
        pain_state: sn(ids::S_SARG_PAIN),
        death_state: sn(ids::S_SARG_DIE1),
        melee_state: sn(ids::S_SARG_ATK1),
        missile_state: sn(ids::S_NULL),
        raise_state: sn(ids::S_SARG_STND),
    },
    // -----------------------------------------------------------------------
    // 6: Lost Soul — MT_SKULL
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 100,
        speed: fixed(8),
        radius: fixed(16),
        height: fixed(56),
        mass: 50,
        flags: MF_FLOAT_MONSTER | flags::MF_SKULLFLY,
        pain_chance: 0,
        spawn_state: sn(ids::S_SKULL_STND),
        see_state: sn(ids::S_SKULL_RUN1),
        pain_state: sn(ids::S_SKULL_PAIN),
        death_state: sn(ids::S_SKULL_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_SKULL_ATK1),
        raise_state: sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 7: Cacodemon — MT_HEAD
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 400,
        speed: fixed(8),
        radius: fixed(31),
        height: fixed(56),
        mass: 400,
        flags: MF_FLOAT_MONSTER,
        pain_chance: 128,
        spawn_state: sn(ids::S_HEAD_STND),
        see_state: sn(ids::S_HEAD_RUN1),
        pain_state: sn(ids::S_HEAD_PAIN),
        death_state: sn(ids::S_HEAD_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_NULL),
        raise_state: sn(ids::S_HEAD_STND),
    },
    // -----------------------------------------------------------------------
    // 8: Baron of Hell — MT_BRUISER
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 1000,
        speed: fixed(8),
        radius: fixed(24),
        height: fixed(64),
        mass: 1000,
        flags: MF_MONSTER,
        pain_chance: 50,
        spawn_state: sn(ids::S_BOSS_STND),
        see_state: sn(ids::S_BOSS_RUN1),
        pain_state: sn(ids::S_BOSS_PAIN),
        death_state: sn(ids::S_BOSS_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_NULL),
        raise_state: sn(ids::S_BOSS_STND),
    },
    // -----------------------------------------------------------------------
    // 9: Hell Knight — own BOS2 states
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 500,
        speed: fixed(8),
        radius: fixed(24),
        height: fixed(64),
        mass: 1000,
        flags: MF_MONSTER,
        pain_chance: 50,
        spawn_state: sn(ids::S_BOS2_STND),
        see_state: sn(ids::S_BOS2_RUN1),
        pain_state: sn(ids::S_BOS2_PAIN),
        death_state: sn(ids::S_BOS2_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_BOS2_ATK1),
        raise_state: sn(ids::S_BOS2_STND),
    },
    // -----------------------------------------------------------------------
    // 10: Arachnotron — MT_BABY
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 500,
        speed: fixed(12),
        radius: fixed(64),
        height: fixed(64),
        mass: 600,
        flags: MF_MONSTER,
        pain_chance: 128,
        spawn_state: sn(ids::S_BSPI_STND),
        see_state: sn(ids::S_BSPI_RUN1),
        pain_state: sn(ids::S_BSPI_PAIN),
        death_state: sn(ids::S_BSPI_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_BSPI_ATK1),
        raise_state: sn(ids::S_NULL),
    },
    // 11: Pain Elemental
    MobjInfo {
        spawn_health: 400,
        speed: fixed(8),
        radius: fixed(31),
        height: fixed(56),
        mass: 400,
        flags: MF_FLOAT_MONSTER,
        pain_chance: 128,
        spawn_state: sn(ids::S_PAIN_STND),
        see_state: sn(ids::S_PAIN_RUN1),
        pain_state: sn(ids::S_PAIN_PAIN1),
        death_state: sn(ids::S_PAIN_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_PAIN_ATK1),
        raise_state: sn(ids::S_NULL),
    },
    // 12: Revenant
    MobjInfo {
        spawn_health: 300,
        speed: fixed(10),
        radius: fixed(20),
        height: fixed(56),
        mass: 500,
        flags: MF_MONSTER,
        pain_chance: 100,
        spawn_state: sn(ids::S_SKEL_STND),
        see_state: sn(ids::S_SKEL_RUN1),
        pain_state: sn(ids::S_SKEL_PAIN),
        death_state: sn(ids::S_SKEL_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_SKEL_ATK1),
        raise_state: sn(ids::S_NULL),
    },
    // 13: Mancubus
    MobjInfo {
        spawn_health: 600,
        speed: fixed(8),
        radius: fixed(48),
        height: fixed(64),
        mass: 1000,
        flags: MF_MONSTER,
        pain_chance: 80,
        spawn_state: sn(ids::S_FATT_STND),
        see_state: sn(ids::S_FATT_RUN1),
        pain_state: sn(ids::S_FATT_PAIN),
        death_state: sn(ids::S_FATT_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_FATT_ATK1),
        raise_state: sn(ids::S_FATT_STND),
    },
    // 14: ArchVile
    MobjInfo {
        spawn_health: 700,
        speed: fixed(15),
        radius: fixed(20),
        height: fixed(56),
        mass: 500,
        flags: MF_MONSTER,
        pain_chance: 10,
        spawn_state: sn(ids::S_VILE_STND),
        see_state: sn(ids::S_VILE_RUN1),
        pain_state: sn(ids::S_VILE_PAIN),
        death_state: sn(ids::S_VILE_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_VILE_ATK1),
        raise_state: sn(ids::S_NULL),
    },
    // 15: Spider Mastermind — MT_SPIDER
    MobjInfo {
        spawn_health: 3000,
        speed: fixed(12),
        radius: fixed(128),
        height: fixed(100),
        mass: i32::MAX,
        flags: MF_MONSTER,
        pain_chance: 40,
        spawn_state: sn(ids::S_SPID_STND),
        see_state: sn(ids::S_SPID_RUN1),
        pain_state: sn(ids::S_SPID_PAIN),
        death_state: sn(ids::S_SPID_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_NULL),
        raise_state: sn(ids::S_NULL),
    },
    // 16: Cyberdemon — MT_CYBORG
    MobjInfo {
        spawn_health: 4000,
        speed: fixed(16),
        radius: fixed(40),
        height: fixed(110),
        mass: i32::MAX,
        flags: MF_MONSTER,
        pain_chance: 20,
        spawn_state: sn(ids::S_CYBER_STND),
        see_state: sn(ids::S_CYBER_RUN1),
        pain_state: sn(ids::S_CYBER_PAIN),
        death_state: sn(ids::S_CYBER_DIE1),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_NULL),
        raise_state: sn(ids::S_NULL),
    },
    // 17: Wolf SS — reuses Trooper states
    MobjInfo {
        spawn_health: 50,
        speed: fixed(8),
        radius: fixed(20),
        height: fixed(56),
        mass: 100,
        flags: MF_MONSTER,
        pain_chance: 170,
        spawn_state: sn(ids::S_POSS_STND),
        see_state: sn(ids::S_POSS_RUN1),
        pain_state: sn(ids::S_NULL),
        death_state: sn(ids::S_NULL),
        melee_state: sn(ids::S_NULL),
        missile_state: sn(ids::S_NULL),
        raise_state: sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 18..=26: Projectiles and visual effects
    // -----------------------------------------------------------------------
    // 18: BulletPuff
    MobjInfo {
        flags: flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_PUFF1),
        ..ITEM
    },
    // 19: Blood
    MobjInfo {
        flags: flags::MF_NOBLOCKMAP,
        spawn_state: sn(ids::S_BLOOD1),
        ..ITEM
    },
    // 20: SmokeTrail
    MobjInfo {
        flags: flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY,
        ..ITEM
    },
    // 21: SpawnFire
    MobjInfo {
        flags: flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY,
        ..ITEM
    },
    // 22: Rocket
    MobjInfo {
        speed: fixed(20),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_ROCKET),
        death_state: sn(ids::S_EXPLODE1),
        ..ITEM
    },
    // 23: PlasmaBall
    MobjInfo {
        speed: fixed(25),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_PLASBALL1),
        death_state: sn(ids::S_PLASEXP1),
        ..ITEM
    },
    // 24: BfgBall
    MobjInfo {
        speed: fixed(25),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_BFGSHOT1),
        death_state: sn(ids::S_BFGLAND1),
        ..ITEM
    },
    // 25: ArachPlaz
    MobjInfo {
        speed: fixed(25),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_ARACH_PLAZ1),
        death_state: sn(ids::S_ARACH_PLEX1),
        ..ITEM
    },
    // 26: Tracer (Revenant missile)
    MobjInfo {
        speed: fixed(10),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_TRACER1),
        death_state: sn(ids::S_TRACEEXP1),
        ..ITEM
    },
    // -----------------------------------------------------------------------
    // 27..=33: Weapon pickups
    // -----------------------------------------------------------------------
    ITEM, // 27: BfgPickup
    ITEM, // 28: Chaingun
    ITEM, // 29: Chainsaw
    ITEM, // 30: RocketLauncher
    ITEM, // 31: PlasmaRifle
    ITEM, // 32: Shotgun
    ITEM, // 33: SuperShotgun
    // -----------------------------------------------------------------------
    // 34..=41: Ammo pickups
    // -----------------------------------------------------------------------
    ITEM, // 34: Clip
    ITEM, // 35: ClipBox
    ITEM, // 36: RocketAmmo
    ITEM, // 37: RocketBox
    ITEM, // 38: Cell
    ITEM, // 39: CellPack
    ITEM, // 40: Shell
    ITEM, // 41: ShellBox
    // -----------------------------------------------------------------------
    // 42..=49: Health and armor pickups
    // -----------------------------------------------------------------------
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_COUNTITEM,
        ..ITEM
    }, // 42: HealthBonus
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_COUNTITEM,
        ..ITEM
    }, // 43: ArmorBonus
    ITEM, // 44: GreenArmor
    ITEM, // 45: BlueArmor
    ITEM, // 46: Stimpack
    ITEM, // 47: Medikit
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_COUNTITEM,
        ..ITEM
    }, // 48: Megasphere
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_COUNTITEM,
        ..ITEM
    }, // 49: Soulsphere
    // -----------------------------------------------------------------------
    // 50..=55: Key pickups (no deathmatch)
    // -----------------------------------------------------------------------
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH,
        ..ITEM
    }, // 50: BlueCard
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH,
        ..ITEM
    }, // 51: RedCard
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH,
        ..ITEM
    }, // 52: YellowCard
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH,
        ..ITEM
    }, // 53: BlueSkull
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH,
        ..ITEM
    }, // 54: RedSkull
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH,
        ..ITEM
    }, // 55: YellowSkull
    // -----------------------------------------------------------------------
    // 56..=60: Power-up pickups
    // -----------------------------------------------------------------------
    ITEM, // 56: Berserk
    ITEM, // 57: BlurSphere (Partial Invisibility)
    ITEM, // 58: RadSuit
    ITEM, // 59: Allmap (Computer map)
    ITEM, // 60: Infrared (Lite Amp goggles)
    // -----------------------------------------------------------------------
    // 61..=64: Decorations
    // -----------------------------------------------------------------------
    MobjInfo {
        flags: flags::MF_SOLID,
        ..ITEM
    }, // 61: Column
    MobjInfo {
        flags: flags::MF_SOLID,
        ..ITEM
    }, // 62: TechLamp
    MobjInfo {
        flags: flags::MF_SOLID,
        ..ITEM
    }, // 63: TechLamp2
    MobjInfo {
        flags: flags::MF_SOLID | flags::MF_SHOOTABLE,
        spawn_health: 20,
        spawn_state: sn(ids::S_BAR1),
        radius: fixed(10),
        height: fixed(42),
        ..ITEM
    }, // 64: Barrel
    // -----------------------------------------------------------------------
    // 65..=66: Specials
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 250,
        flags: flags::MF_SOLID | flags::MF_SHOOTABLE,
        spawn_state: sn(ids::S_BRAIN_STND),
        see_state: sn(ids::S_BRAIN_SEE),
        death_state: sn(ids::S_BRAIN_DIE1),
        ..ITEM
    }, // 65: BossBrain
    MobjInfo {
        spawn_health: 100,
        flags: flags::MF_SOLID | flags::MF_SHOOTABLE,
        ..ITEM
    }, // 66: CommanderKeen
    // -----------------------------------------------------------------------
    // 67..=71: Additional projectiles (Batch 21)
    // -----------------------------------------------------------------------
    // 67: BfgExtra (BFG tracers — secondary damage)
    MobjInfo {
        speed: fixed(25),
        radius: fixed(6),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_BFGLAND1),
        death_state: sn(ids::S_BFGLAND1),
        ..ITEM
    },
    // 68: ImpFireball
    MobjInfo {
        speed: fixed(10),
        radius: fixed(6),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_TBALL1),
        death_state: sn(ids::S_TBALLX1),
        ..ITEM
    },
    // 69: CacoFireball
    MobjInfo {
        speed: fixed(10),
        radius: fixed(6),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_TBALL1),
        death_state: sn(ids::S_TBALLX1),
        ..ITEM
    },
    // 70: BaronBall
    MobjInfo {
        speed: fixed(15),
        radius: fixed(6),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_BRBALL1),
        death_state: sn(ids::S_BRBALLX1),
        ..ITEM
    },
    // 71: FatShot (Mancubus fireball)
    MobjInfo {
        speed: fixed(20),
        radius: fixed(6),
        height: fixed(8),
        mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        spawn_state: sn(ids::S_FATSHOT1),
        death_state: sn(ids::S_FATSHOTX1),
        ..ITEM
    },
    // -----------------------------------------------------------------------
    // 72..=73: Additional pickups (item pickup batch)
    // -----------------------------------------------------------------------
    // 72: InvulnerabilitySphere
    MobjInfo {
        flags: flags::MF_SPECIAL | flags::MF_COUNTITEM,
        ..ITEM
    },
    // 73: Backpack
    ITEM,
    // -----------------------------------------------------------------------
    // 74..=75: Arch-Vile fire and Boss Brain cube
    // -----------------------------------------------------------------------
    // 74: VileFire — visual effect that tracks the Arch-Vile's target
    MobjInfo {
        flags: flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY,
        ..ITEM
    },
    // 75: BossCube — cube projectile spawned by the Boss Brain
    MobjInfo {
        speed: fixed(15),
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        ..ITEM
    },
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_types::mobj_kind::MobjKind;

    #[test]
    fn table_covers_all_kinds() {
        // MobjKind::BossCube = 75 → table must have 76 entries.
        assert_eq!(MOBJINFO.len(), 76);
        assert_eq!(MobjKind::BossCube as usize, 75);
    }

    #[test]
    fn player_stats() {
        let info = &MOBJINFO[MobjKind::Player as usize];
        assert_eq!(info.spawn_health, 100);
        assert_eq!(info.radius, fixed(16));
        assert_eq!(info.height, fixed(56));
    }

    #[test]
    fn trooper_stats() {
        let info = &MOBJINFO[MobjKind::Trooper as usize];
        assert_eq!(info.spawn_health, 20);
        assert_eq!(info.speed, fixed(8));
        assert_eq!(info.see_state, StateNum(ids::S_POSS_RUN1));
        assert_eq!(info.spawn_state, StateNum(ids::S_POSS_STND));
    }

    #[test]
    fn cyberdemon_is_immovable() {
        let info = &MOBJINFO[MobjKind::Cyberdemon as usize];
        assert_eq!(info.mass, i32::MAX);
        assert_eq!(info.spawn_health, 4000);
    }

    #[test]
    fn cacodemon_is_floating() {
        let info = &MOBJINFO[MobjKind::Cacodemon as usize];
        assert_ne!(info.flags & flags::MF_FLOAT, 0);
        assert_ne!(info.flags & flags::MF_NOGRAVITY, 0);
    }

    #[test]
    fn all_monster_see_states_in_bounds() {
        // For monsters with see_state != S_NULL, it must be within STATES table.
        use crate::states::STATES;
        for (kind_idx, info) in MOBJINFO.iter().enumerate().take(18) {
            let see = info.see_state.0 as usize;
            assert!(
                see < STATES.len(),
                "MobjKind {kind_idx} see_state {see} out of STATES bounds"
            );
        }
    }

    #[test]
    fn trooper_has_death_state() {
        let info = &MOBJINFO[MobjKind::Trooper as usize];
        assert_eq!(info.death_state, StateNum(ids::S_POSS_DIE1));
    }

    #[test]
    fn trooper_has_pain_state() {
        let info = &MOBJINFO[MobjKind::Trooper as usize];
        assert_eq!(info.pain_state, StateNum(ids::S_POSS_PAIN));
    }

    #[test]
    fn cyberdemon_pain_chance_is_low() {
        let info = &MOBJINFO[MobjKind::Cyberdemon as usize];
        assert_eq!(info.pain_chance, 20);
    }

    #[test]
    fn trooper_has_missile_attack_state() {
        use crate::states::ids;
        let info = &MOBJINFO[MobjKind::Trooper as usize];
        assert_eq!(info.missile_state, StateNum(ids::S_POSS_ATK1));
        assert_eq!(info.melee_state, StateNum(ids::S_NULL));
    }

    #[test]
    fn demon_has_melee_attack_state() {
        use crate::states::ids;
        let info = &MOBJINFO[MobjKind::Demon as usize];
        assert_eq!(info.melee_state, StateNum(ids::S_SARG_ATK1));
        assert_eq!(info.missile_state, StateNum(ids::S_NULL));
    }

    #[test]
    fn imp_has_both_attack_states_same() {
        use crate::states::ids;
        let info = &MOBJINFO[MobjKind::Imp as usize];
        assert_eq!(info.melee_state, StateNum(ids::S_TROO_ATK1));
        assert_eq!(info.missile_state, StateNum(ids::S_TROO_ATK1));
    }
}
