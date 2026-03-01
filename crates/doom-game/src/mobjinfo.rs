//! Actor properties table — one `MobjInfo` entry per `MobjKind`.
//!
//! Indexed by `MobjKind as usize`.  Values are drawn from Doom's original
//! `mobjinfo` table in `info.c`, trimmed to the fields needed for Batch 2
//! (health, speed, radius, height, mass, flags, pain chance, state refs).
//!
//! Non-monster entries (pickups, decorations, projectiles) use conservative
//! defaults — their AI fields are all `S_NULL`.

use doom_types::Fixed16_16;
use crate::mobj::{StateNum, flags};
use crate::states::ids;

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

/// Combined flags for standard ground monsters.
const MF_MONSTER: u32 =
    flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_COUNTKILL;

/// Combined flags for floating monsters (cacodemons, lost souls, etc.).
const MF_FLOAT_MONSTER: u32 =
    MF_MONSTER | flags::MF_FLOAT | flags::MF_NOGRAVITY;

/// Combined flags for the player.
const MF_PLAYER: u32 =
    flags::MF_SOLID | flags::MF_SHOOTABLE | flags::MF_DROPOFF
    | flags::MF_PICKUP | flags::MF_NOTDMATCH;

/// Default entry for non-monster actors (pickups, decorations, projectiles).
const ITEM: MobjInfo = MobjInfo {
    spawn_health: 1,
    speed:        Fixed16_16(0),
    radius:       fixed(20),
    height:       fixed(16),
    mass:         100,
    flags:        flags::MF_SPECIAL,
    pain_chance:  0,
    spawn_state:  sn(ids::S_NULL),
    see_state:    sn(ids::S_NULL),
    pain_state:   sn(ids::S_NULL),
    death_state:  sn(ids::S_NULL),
};

// ---------------------------------------------------------------------------
// Global MOBJINFO table  (indexed by MobjKind as usize, 0..=66)
// ---------------------------------------------------------------------------

/// Actor property table.  Index with `MobjKind as usize`.
///
/// Based on Doom's `mobjinfo[]` in `info.c`.
pub static MOBJINFO: [MobjInfo; 67] = [
    // -----------------------------------------------------------------------
    // 0: Player
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 100,
        speed:        Fixed16_16(0),
        radius:       fixed(16),
        height:       fixed(56),
        mass:         100,
        flags:        MF_PLAYER,
        pain_chance:  255,
        spawn_state:  sn(ids::S_NULL),
        see_state:    sn(ids::S_NULL),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 1: Trooper (Zombie Man) — MT_POSSESSED
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 20,
        speed:        fixed(8),
        radius:       fixed(20),
        height:       fixed(56),
        mass:         100,
        flags:        MF_MONSTER,
        pain_chance:  200,
        spawn_state:  sn(ids::S_POSS_STND),
        see_state:    sn(ids::S_POSS_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 2: Sergeant (Shotgun Guy) — MT_SHOTGUY
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 30,
        speed:        fixed(8),
        radius:       fixed(20),
        height:       fixed(56),
        mass:         100,
        flags:        MF_MONSTER,
        pain_chance:  170,
        spawn_state:  sn(ids::S_SPOS_STND),
        see_state:    sn(ids::S_SPOS_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 3: Imp — MT_TROOP
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 60,
        speed:        fixed(8),
        radius:       fixed(20),
        height:       fixed(56),
        mass:         100,
        flags:        MF_MONSTER,
        pain_chance:  200,
        spawn_state:  sn(ids::S_TROO_STND),
        see_state:    sn(ids::S_TROO_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 4: Demon (Pink Demon) — MT_SERGEANT
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 150,
        speed:        fixed(10),
        radius:       fixed(30),
        height:       fixed(56),
        mass:         400,
        flags:        MF_MONSTER,
        pain_chance:  180,
        spawn_state:  sn(ids::S_SARG_STND),
        see_state:    sn(ids::S_SARG_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 5: Spectre — same as Demon + MF_SHADOW
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 150,
        speed:        fixed(10),
        radius:       fixed(30),
        height:       fixed(56),
        mass:         400,
        flags:        MF_MONSTER | flags::MF_SHADOW,
        pain_chance:  180,
        spawn_state:  sn(ids::S_SARG_STND),
        see_state:    sn(ids::S_SARG_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 6: Lost Soul — MT_SKULL  (no state table entry yet)
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 100,
        speed:        fixed(8),
        radius:       fixed(16),
        height:       fixed(56),
        mass:         50,
        flags:        MF_FLOAT_MONSTER | flags::MF_SKULLFLY,
        pain_chance:  0,
        spawn_state:  sn(ids::S_NULL),
        see_state:    sn(ids::S_NULL),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 7: Cacodemon — MT_HEAD
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 400,
        speed:        fixed(8),
        radius:       fixed(31),
        height:       fixed(56),
        mass:         400,
        flags:        MF_FLOAT_MONSTER,
        pain_chance:  128,
        spawn_state:  sn(ids::S_HEAD_STND),
        see_state:    sn(ids::S_HEAD_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 8: Baron of Hell — MT_BRUISER
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 1000,
        speed:        fixed(8),
        radius:       fixed(24),
        height:       fixed(64),
        mass:         1000,
        flags:        MF_MONSTER,
        pain_chance:  50,
        spawn_state:  sn(ids::S_BOSS_STND),
        see_state:    sn(ids::S_BOSS_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 9: Hell Knight — shares Baron states for now
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 500,
        speed:        fixed(8),
        radius:       fixed(24),
        height:       fixed(64),
        mass:         1000,
        flags:        MF_MONSTER,
        pain_chance:  50,
        spawn_state:  sn(ids::S_BOSS_STND),
        see_state:    sn(ids::S_BOSS_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 10: Arachnotron — MT_BABY  (state table entry Batch 3)
    // -----------------------------------------------------------------------
    MobjInfo {
        spawn_health: 500,
        speed:        fixed(12),
        radius:       fixed(64),
        height:       fixed(64),
        mass:         600,
        flags:        MF_MONSTER,
        pain_chance:  128,
        spawn_state:  sn(ids::S_NULL),
        see_state:    sn(ids::S_NULL),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // 11: Pain Elemental
    MobjInfo {
        spawn_health: 400,
        speed:        fixed(8),
        radius:       fixed(31),
        height:       fixed(56),
        mass:         400,
        flags:        MF_FLOAT_MONSTER,
        pain_chance:  128,
        spawn_state:  sn(ids::S_NULL),
        see_state:    sn(ids::S_NULL),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // 12: Revenant
    MobjInfo {
        spawn_health: 300,
        speed:        fixed(10),
        radius:       fixed(20),
        height:       fixed(56),
        mass:         500,
        flags:        MF_MONSTER,
        pain_chance:  100,
        spawn_state:  sn(ids::S_NULL),
        see_state:    sn(ids::S_NULL),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // 13: Mancubus
    MobjInfo {
        spawn_health: 600,
        speed:        fixed(8),
        radius:       fixed(48),
        height:       fixed(64),
        mass:         1000,
        flags:        MF_MONSTER,
        pain_chance:  80,
        spawn_state:  sn(ids::S_NULL),
        see_state:    sn(ids::S_NULL),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // 14: ArchVile
    MobjInfo {
        spawn_health: 700,
        speed:        fixed(15),
        radius:       fixed(20),
        height:       fixed(56),
        mass:         500,
        flags:        MF_MONSTER,
        pain_chance:  10,
        spawn_state:  sn(ids::S_NULL),
        see_state:    sn(ids::S_NULL),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // 15: Spider Mastermind — MT_SPIDER
    MobjInfo {
        spawn_health: 3000,
        speed:        fixed(12),
        radius:       fixed(128),
        height:       fixed(100),
        mass:         i32::MAX,
        flags:        MF_MONSTER,
        pain_chance:  40,
        spawn_state:  sn(ids::S_SPID_STND),
        see_state:    sn(ids::S_SPID_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // 16: Cyberdemon — MT_CYBORG
    MobjInfo {
        spawn_health: 4000,
        speed:        fixed(16),
        radius:       fixed(40),
        height:       fixed(110),
        mass:         i32::MAX,
        flags:        MF_MONSTER,
        pain_chance:  20,
        spawn_state:  sn(ids::S_CYBER_STND),
        see_state:    sn(ids::S_CYBER_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // 17: Wolf SS — reuses Trooper states
    MobjInfo {
        spawn_health: 50,
        speed:        fixed(8),
        radius:       fixed(20),
        height:       fixed(56),
        mass:         100,
        flags:        MF_MONSTER,
        pain_chance:  170,
        spawn_state:  sn(ids::S_POSS_STND),
        see_state:    sn(ids::S_POSS_RUN1),
        pain_state:   sn(ids::S_NULL),
        death_state:  sn(ids::S_NULL),
    },
    // -----------------------------------------------------------------------
    // 18..=26: Projectiles and visual effects
    // -----------------------------------------------------------------------
    // 18: BulletPuff
    MobjInfo { flags: flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY, ..ITEM },
    // 19: Blood
    MobjInfo { flags: flags::MF_NOBLOCKMAP, ..ITEM },
    // 20: SmokeTrail
    MobjInfo { flags: flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY, ..ITEM },
    // 21: SpawnFire
    MobjInfo { flags: flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY, ..ITEM },
    // 22: Rocket
    MobjInfo {
        speed: fixed(20), height: fixed(8), mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        ..ITEM
    },
    // 23: PlasmaBall
    MobjInfo {
        speed: fixed(25), height: fixed(8), mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        ..ITEM
    },
    // 24: BfgBall
    MobjInfo {
        speed: fixed(25), height: fixed(8), mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        ..ITEM
    },
    // 25: ArachPlaz
    MobjInfo {
        speed: fixed(25), height: fixed(8), mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
        ..ITEM
    },
    // 26: Tracer (Revenant missile)
    MobjInfo {
        speed: fixed(10), height: fixed(8), mass: 100,
        flags: flags::MF_NOBLOCKMAP | flags::MF_MISSILE | flags::MF_DROPOFF | flags::MF_NOGRAVITY,
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
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_COUNTITEM, ..ITEM }, // 42: HealthBonus
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_COUNTITEM, ..ITEM }, // 43: ArmorBonus
    ITEM, // 44: GreenArmor
    ITEM, // 45: BlueArmor
    ITEM, // 46: Stimpack
    ITEM, // 47: Medikit
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_COUNTITEM, ..ITEM }, // 48: Megasphere
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_COUNTITEM, ..ITEM }, // 49: Soulsphere
    // -----------------------------------------------------------------------
    // 50..=55: Key pickups (no deathmatch)
    // -----------------------------------------------------------------------
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH, ..ITEM }, // 50: BlueCard
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH, ..ITEM }, // 51: RedCard
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH, ..ITEM }, // 52: YellowCard
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH, ..ITEM }, // 53: BlueSkull
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH, ..ITEM }, // 54: RedSkull
    MobjInfo { flags: flags::MF_SPECIAL | flags::MF_NOTDMATCH, ..ITEM }, // 55: YellowSkull
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
    MobjInfo { flags: flags::MF_SOLID, ..ITEM }, // 61: Column
    MobjInfo { flags: flags::MF_SOLID, ..ITEM }, // 62: TechLamp
    MobjInfo { flags: flags::MF_SOLID, ..ITEM }, // 63: TechLamp2
    MobjInfo { flags: flags::MF_SOLID | flags::MF_SHOOTABLE, spawn_health: 20, ..ITEM }, // 64: Barrel
    // -----------------------------------------------------------------------
    // 65..=66: Specials
    // -----------------------------------------------------------------------
    MobjInfo { spawn_health: 250, flags: flags::MF_SOLID | flags::MF_SHOOTABLE, ..ITEM }, // 65: BossBrain
    MobjInfo { spawn_health: 100, flags: flags::MF_SOLID | flags::MF_SHOOTABLE, ..ITEM }, // 66: CommanderKeen
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::MobjKind;

    #[test]
    fn table_covers_all_kinds() {
        // MobjKind::CommanderKeen = 66 → table must have 67 entries.
        assert_eq!(MOBJINFO.len(), 67);
        assert_eq!(MobjKind::CommanderKeen as usize, 66);
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
        for kind_idx in 0..=17usize {
            let info = &MOBJINFO[kind_idx];
            let see = info.see_state.0 as usize;
            assert!(
                see < STATES.len(),
                "MobjKind {kind_idx} see_state {see} out of STATES bounds"
            );
        }
    }
}
