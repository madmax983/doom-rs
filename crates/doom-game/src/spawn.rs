//! Thing spawning from level data.
//!
//! Iterates the level's THINGS lump and creates `Mobj` actors in the
//! `GameState`'s slab arena, applying skill filtering, multiplayer
//! filtering, MOBJINFO defaults, and tracking kill/item totals.

use doom_map::Level;
use doom_types::{Bam, Fixed16_16};

use crate::mobj::{Mobj, MobjHandle, flags};
use crate::mobjinfo::MOBJINFO;
use crate::pickups::doomed_type_to_kind;
use crate::player::PlayerState;
use crate::state::GameState;
use crate::states::STATES;
use doom_types::mobj_kind::MobjKind;

// ---------------------------------------------------------------------------
// Skill level
// ---------------------------------------------------------------------------

/// Skill level for thing filtering.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Skill {
    /// I'm Too Young To Die.
    Baby = 0,
    /// Hey, Not Too Rough.
    Easy = 1,
    /// Hurt Me Plenty.
    Medium = 2,
    /// Ultra-Violence.
    Hard = 3,
    /// Nightmare!
    Nightmare = 4,
}

impl Skill {
    /// Convert an integer to a `Skill`.
    ///
    /// Returns `None` for any out-of-range value.
    pub fn from_num(n: u8) -> Option<Self> {
        Self::from_repr(n)
    }
}

// ---------------------------------------------------------------------------
// Thing flag bits (from the WAD THINGS lump)
// ---------------------------------------------------------------------------

/// Monster is deaf / ambush — won't react to sound, only sight.
const MTF_AMBUSH: u16 = 0x0008;
/// Thing only appears in multiplayer (deathmatch / coop).
const MTF_MULTIPLAYER: u16 = 0x0010;

// ---------------------------------------------------------------------------
// Angle conversion
// ---------------------------------------------------------------------------

/// Maximum simultaneous players (vanilla `MAXPLAYERS`).  Used for the
/// `mobj->lastlook = P_Random() % MAXPLAYERS` draw in P_SpawnMobj.
const MAXPLAYERS: u8 = 4;

/// BAM units per degree: 2^32 / 360.
const BAM_PER_DEGREE: u32 = (0x1_0000_0000u64 / 360) as u32;

/// Convert a degrees value (0-359) from a Thing to a BAM angle.
///
/// Retained for reference/tests; spawn now uses [`spawn_angle_to_bam`] for
/// vanilla-exact spawn angles.
#[inline]
#[cfg_attr(not(test), allow(dead_code))]
fn degrees_to_bam(degrees: u16) -> Bam {
    Bam((degrees as u32).wrapping_mul(BAM_PER_DEGREE))
}

/// ANG45 in BAM units (45 degrees).
const ANG45_BAM: u32 = 0x2000_0000;

/// Convert a map thing's spawn angle to BAM exactly as vanilla
/// `P_SpawnMapThing` does: `mobj->angle = ANG45 * (mthing->angle / 45)`.
///
/// The integer divide-then-multiply makes cardinal/ordinal angles bit-exact
/// (e.g. 90° -> ANG45 * 2 = 0x4000_0000), matching demo playback. This differs
/// from the general [`degrees_to_bam`] scaling used elsewhere.
#[inline]
fn spawn_angle_to_bam(degrees: u16) -> Bam {
    Bam(ANG45_BAM.wrapping_mul((degrees as u32) / 45))
}

// ---------------------------------------------------------------------------
// MOBJINFO application
// ---------------------------------------------------------------------------

/// Apply default properties from the MOBJINFO table to a freshly-created Mobj.
///
/// Sets health, radius, height, speed, flags, and initial state/tics from
/// the table entry corresponding to `mo.kind`.
pub(crate) fn apply_mobjinfo_defaults(mo: &mut Mobj) {
    let idx = mo.kind as usize;
    if idx >= MOBJINFO.len() {
        return;
    }
    let info = &MOBJINFO[idx];
    mo.health = info.spawn_health;
    mo.radius = info.radius;
    mo.height = info.height;
    mo.flags = info.flags;
    // Vanilla P_SpawnMobj: `mobj->reactiontime = info->reactiontime;`
    mo.reactiontime = crate::mobjinfo::reactiontime(mo.kind);

    // Set initial state and tics from the spawn state.
    mo.state = info.spawn_state;
    let state_idx = info.spawn_state.0 as usize;
    if state_idx < STATES.len() {
        mo.tics = STATES[state_idx].tics;
    }
}

/// Sync a freshly spawned map thing to the floor and subsector it occupies.
fn sync_mobj_to_level(level: &Level, mo: &mut Mobj) {
    if let Some(subsector) = level.subsector_index_at(mo.x.to_int(), mo.y.to_int()) {
        mo.subsector = subsector as u32;
    }

    let sector_idx = level
        .sector_index_at(mo.x.to_int(), mo.y.to_int())
        .or_else(|| crate::sight::sector_from_subsector(level, mo.subsector as usize));
    if let Some(sector_idx) = sector_idx
        && let Some(sector) = level.sectors.get(sector_idx)
    {
        mo.z = if mo.flags & flags::MF_SPAWNCEILING != 0 {
            Fixed16_16::from_int(i32::from(sector.ceil_height)) - mo.height
        } else {
            Fixed16_16::from_int(i32::from(sector.floor_height))
        };
    }
}

/// Port of `P_SpawnMobj` (`p_mobj.c:547`) for direct (non-map-thing) spawns
/// such as puffs, blood, and projectiles.
///
/// Draws exactly one `P_Random()` for `lastlook` (as vanilla `P_SpawnMobj`
/// does for every mobj), applies the type's `mobjinfo` defaults, and places
/// the mobj at the explicit `z`.  Unlike map-thing spawning it does **not**
/// randomize the initial animation tics (that is a `P_SpawnMapThing` step).
pub fn p_spawn_mobj(
    gs: &mut GameState,
    level: Option<&Level>,
    x: Fixed16_16,
    y: Fixed16_16,
    z: Fixed16_16,
    kind: MobjKind,
) -> MobjHandle {
    // P_SpawnMobj (p_mobj.c:547): mobj->lastlook = P_Random() % MAXPLAYERS.
    let _lastlook = (gs.p_random() as u32) % (MAXPLAYERS as u32);

    let mut mo = Mobj::new(kind, x, y, Bam::ZERO);
    apply_mobjinfo_defaults(&mut mo);
    mo.z = z;
    if let Some(level) = level
        && let Some(ss) = level.subsector_index_at(x.to_int(), y.to_int())
    {
        mo.subsector = ss as u32;
    }
    gs.mobjslab.alloc(mo)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Dictates whether to spawn multiplayer-only things.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum GameMode {
    /// Standard single-player mode.
    SinglePlayer,
    /// Deathmatch multiplayer mode.
    Deathmatch,
}

/// Spawn all things from the level's THINGS lump into the game state.
///
/// Filters by skill level and multiplayer flag. Returns the player handle
/// if a player-1 start (DoomEd type 1) was found and spawned.
///
/// Player starts 2-4 (DoomEd types 2-4) are skipped in single-player
/// (`is_deathmatch == false`).
pub fn spawn_level_things(
    gs: &mut GameState,
    level: &Level,
    skill: Skill,
    game_mode: GameMode,
) -> Option<MobjHandle> {
    let mut player_handle: Option<MobjHandle> = None;

    // Size the sound-propagation state to this level's sector count.  Vanilla
    // clears soundtargets in P_SpawnMapThing/P_SetupLevel; without this the
    // `sound_targets` vector stays empty and `P_NoiseAlert` can never wake a
    // monster (weapon fire would silently fail to alert nearby monsters).
    crate::sound::init_sound_state(gs, level.sectors.len());

    // Vanilla P_SpawnMapThing skill bit (p_mobj.c): sk_baby->1, sk_nightmare->4,
    // otherwise 1 << (gameskill-1).  A thing spawns only if `options & bit`.
    let skill_bit: u16 = match skill {
        Skill::Baby | Skill::Easy => 1,
        Skill::Medium => 2,
        Skill::Hard | Skill::Nightmare => 4,
    };

    // Iterate the THINGS lump in file order, mirroring vanilla P_SpawnMapThing
    // call order so the P_Random consumption matches vanilla draw-for-draw.
    for thing in &level.things {
        let ttype = thing.kind; // DoomEd number.

        // --- Deathmatch start (type 11): counted, never spawns (no RNG). ---
        if ttype == 11 {
            continue;
        }

        // --- "player -1 start" / invalid (vanilla: type <= 0). ---
        if ttype == 0 {
            continue;
        }

        // --- Player starts (types 1..=4), handled BEFORE skill/MP filters. ---
        if ttype <= 4 {
            // In single player only player 1 is in-game, so only a type-1 start
            // spawns a mobj (drawing one P_Random for lastlook).  Coop starts
            // (2..=4) reserve a respawn spot but spawn nothing here.
            if ttype == 1 && game_mode != GameMode::Deathmatch {
                let angle = spawn_angle_to_bam(thing.angle);
                let x = Fixed16_16::from_int(thing.x as i32);
                let y = Fixed16_16::from_int(thing.y as i32);
                // P_SpawnMobj (p_mobj.c:547): mobj->lastlook = P_Random() % MAXPLAYERS.
                let _lastlook = (gs.p_random() as u32) % (MAXPLAYERS as u32);
                let mut mo = Mobj::new(MobjKind::Player, x, y, angle);
                apply_mobjinfo_defaults(&mut mo);
                sync_mobj_to_level(level, &mut mo);
                let handle = gs.mobjslab.alloc(mo);
                gs.player = PlayerState::pistol_start(handle);
                crate::weapons::setup_psprites(&mut gs.player);
                player_handle = Some(handle);
            }
            continue;
        }

        // --- Multiplayer-only filter (vanilla: !netgame && options & 16). ---
        if thing.flags & MTF_MULTIPLAYER != 0 && game_mode != GameMode::Deathmatch {
            continue;
        }

        // --- Skill-bit filter (vanilla: if !(options & bit) return). ---
        if thing.flags & skill_bit == 0 {
            continue;
        }

        // --- Look up the placeable DoomEd type. ---
        // Vanilla scans mobjinfo[] for a matching doomednum (I_Error if none).
        // `spawn_thing_tics` returns that type's spawnstate tic count, which
        // drives the tic-randomization draw.  We consume the spawn RNG for
        // EVERY placeable type so the stream matches vanilla, and build a
        // fully-simulated Mobj for the subset of types we model (monsters,
        // items, barrels); other placeable types (static/animated decorations)
        // currently only consume their RNG draws.
        let Some(spawnstate_tics) = spawn_thing_tics(ttype) else {
            // Not a placeable vanilla type (vanilla would I_Error). Skip.
            continue;
        };

        let angle = spawn_angle_to_bam(thing.angle);
        let x = Fixed16_16::from_int(thing.x as i32);
        let y = Fixed16_16::from_int(thing.y as i32);

        // P_SpawnMobj (p_mobj.c:547): mobj->lastlook = P_Random() % MAXPLAYERS.
        let _lastlook = (gs.p_random() as u32) % (MAXPLAYERS as u32);

        // P_SpawnMapThing (p_mobj.c:851): after P_SpawnMobj,
        //   if (mobj->tics > 0)
        //       mobj->tics = 1 + (P_Random() % mobj->tics);
        // Randomizes the initial animation phase. One draw when tics > 0.
        let randomized_tics: Option<i16> = if spawnstate_tics > 0 {
            Some((1 + (gs.p_random() as i32 % spawnstate_tics as i32)) as i16)
        } else {
            None
        };

        // Build a simulated Mobj for the types we model.  Decorations we don't
        // model have already consumed their RNG draws above.
        let Some(kind) = doomed_type_to_kind(ttype) else {
            // Unmodeled placeable type. If it is a vanilla SOLID decoration
            // (tech pillars, torches, hanging bodies, …), it still occupies
            // space: vanilla `P_CheckPosition` blocks any monster/player/missile
            // that would overlap it. We were silently dropping these, so a
            // monster whose vanilla chase step is blocked by a pillar instead
            // walked straight through — flipping its `A_Chase` into the
            // `P_NewChaseDir`/`P_TryWalk` branch (extra P_Random draws) one tic
            // apart from vanilla and desyncing the demo (DEMO3/E1M7 lt970: an
            // imp at (640,-160) walked through the MT_MISC48 techno-pillar the
            // vanilla imp was blocked by). Spawn the collision-only body so the
            // clip test matches. These decorations are inert (static `S_NULL`
            // hold, no thinker RNG), so this adds no draws to the stream.
            if let Some((radius_units, height_units, spawn_ceiling)) = solid_decoration_dims(ttype)
            {
                let mut deco = Mobj::new(MobjKind::Column, x, y, angle);
                apply_mobjinfo_defaults(&mut deco);
                deco.radius = Fixed16_16::from_int(radius_units);
                deco.height = Fixed16_16::from_int(height_units);
                deco.flags = if spawn_ceiling {
                    flags::MF_SOLID | flags::MF_SPAWNCEILING | flags::MF_NOGRAVITY
                } else {
                    flags::MF_SOLID
                };
                sync_mobj_to_level(level, &mut deco);
                deco.spawn_x = x;
                deco.spawn_y = y;
                deco.spawn_angle = angle;
                deco.spawn_type = ttype;
                gs.mobjslab.alloc(deco);
            }
            continue;
        };
        if kind == MobjKind::Player {
            continue; // unreachable for ttype > 4, but guard defensively.
        }

        let mut mo = Mobj::new(kind, x, y, angle);
        apply_mobjinfo_defaults(&mut mo);
        sync_mobj_to_level(level, &mut mo);

        // Apply the vanilla tic-randomization result to the simulated mobj, but
        // ONLY for mobjs whose spawnstate we actually model (monsters, barrels).
        // Pickups use the placeholder `ITEM` mobjinfo whose spawn_state is
        // `S_NULL` (tics = -1, an infinite hold): in vanilla these items loop a
        // bright bobbing animation forever and never disappear until collected.
        // If we overwrote their infinite tics with the positive randomized count,
        // the state machine would count down and transition `S_NULL -> S_NULL`,
        // removing the item a few tics after level start (e.g. the E1M5 green
        // armor vanished before the player reached it). The `P_Random` draw above
        // is still consumed unconditionally, preserving demo RNG-stream parity.
        if let Some(t) = randomized_tics
            && mo.state != crate::mobj::StateNum::NULL
        {
            mo.tics = t;
        }

        // Apply ambush flag from thing flags (deaf monsters).
        if thing.flags & MTF_AMBUSH != 0 {
            mo.flags |= flags::MF_AMBUSH;
        }

        // Save original spawn point for Nightmare respawning.
        mo.spawn_x = x;
        mo.spawn_y = y;
        mo.spawn_angle = angle;
        mo.spawn_type = ttype;

        let handle = gs.mobjslab.alloc(mo);

        // --- Track totals for intermission screen ---
        if let Some(spawned) = gs.mobjslab.get(handle) {
            if spawned.flags & flags::MF_COUNTKILL != 0 {
                gs.stats.total_kills += 1;
            }
            if spawned.flags & flags::MF_COUNTITEM != 0 {
                gs.stats.total_items += 1;
            }
        }
    }

    player_handle
}

/// Collision dimensions of every placeable vanilla `MF_SOLID` decoration we do
/// not otherwise model (tech pillars, torches, stalagmites, gutted/hanging
/// bodies, …). Returns `Some((radius, height, spawn_ceiling))` in map units, or
/// `None` for a `doomednum` that is not one of these (a modeled type, a
/// non-solid decoration, or a non-placeable number).
///
/// Values are transcribed from chocolate-doom `src/doom/info.c`: every
/// `mobjinfo` entry with a non-negative `doomednum` and `MF_SOLID` set that is
/// not a monster (`MF_COUNTKILL`), the barrel, Keen, the boss brain, or one of
/// the items/lamps already mapped by `doomed_type_to_kind`. All have
/// `radius = 16`; the ceiling-hung bodies carry `MF_SPAWNCEILING|MF_NOGRAVITY`.
fn solid_decoration_dims(doomednum: u16) -> Option<(i32, i32, bool)> {
    // (radius, height, spawn_ceiling)
    let dims = match doomednum {
        // Floor-standing solid props (MF_SOLID), radius 16, height 16.
        25 | 26 | 27 | 28 | 29 | 30 | 31 | 32 | 33 | 35 | 36 | 37 | 41 | 42 | 43 | 44 | 45 | 46
        | 47 | 48 | 55 | 56 | 57 | 70 | 2028 => (16, 16, false),
        // Large brown tree (MT_MISC76): radius 32, height 16.
        54 => (32, 16, false),
        // Ceiling-hung bodies (MF_SOLID|MF_SPAWNCEILING|MF_NOGRAVITY), radius 16.
        49 => (16, 68, true), // MT_MISC51
        50 => (16, 84, true), // MT_MISC52
        51 => (16, 84, true), // MT_MISC53
        52 => (16, 68, true), // MT_MISC54
        53 => (16, 52, true), // MT_MISC55
        73 => (16, 88, true), // MT_MISC78
        74 => (16, 88, true), // MT_MISC79
        75 => (16, 64, true), // MT_MISC80
        76 => (16, 64, true), // MT_MISC81
        77 => (16, 64, true), // MT_MISC82
        78 => (16, 64, true), // MT_MISC83
        _ => return None,
    };
    Some(dims)
}

/// Spawnstate tic count for every placeable vanilla DoomEd thing type.
///
/// Returns `Some(tics)` — the `tics` field of the type's spawnstate in vanilla
/// `states[]` — for any placeable `doomednum`, or `None` for a number that has
/// no `mobjinfo` entry (which vanilla treats as a fatal `I_Error`).
///
/// This drives the P_SpawnMapThing tic-randomization draw:
/// `if (mobj->tics > 0) mobj->tics = 1 + (P_Random() % mobj->tics)`.  The table
/// is generated from chocolate-doom `src/doom/info.c` (mobjinfo doomednum +
/// spawnstate, resolved through states[]).  Player/coop/deathmatch starts
/// (types 1..=4, 11) are intentionally absent — vanilla handles them before the
/// mobjinfo lookup.
fn spawn_thing_tics(doomednum: u16) -> Option<i16> {
    let tics: i16 = match doomednum {
        5 => 10,    // S_BKEY
        6 => 10,    // S_YKEY
        7 => 10,    // S_SPID_STND
        8 => -1,    // S_BPAK
        9 => 10,    // S_SPOS_STND
        10 => -1,   // S_PLAY_XDIE9
        12 => -1,   // S_PLAY_XDIE9
        13 => 10,   // S_RKEY
        14 => -1,   // S_NULL (teleport landing)
        15 => -1,   // S_PLAY_DIE7
        16 => 10,   // S_CYBER_STND
        17 => -1,   // S_CELP
        18 => -1,   // S_POSS_DIE5
        19 => -1,   // S_SPOS_DIE5
        20 => -1,   // S_TROO_DIE5
        21 => -1,   // S_SARG_DIE6
        22 => -1,   // S_HEAD_DIE6
        23 => 6,    // S_SKULL_DIE6
        24 => -1,   // S_GIBS
        25 => -1,   // S_DEADSTICK
        26 => 6,    // S_LIVESTICK
        27 => -1,   // S_HEADONASTICK
        28 => -1,   // S_HEADSONSTICK
        29 => 6,    // S_HEADCANDLES
        30 => -1,   // S_TALLGRNCOL
        31 => -1,   // S_SHRTGRNCOL
        32 => -1,   // S_TALLREDCOL
        33 => -1,   // S_SHRTREDCOL
        34 => -1,   // S_CANDLESTIK
        35 => -1,   // S_CANDELABRA
        36 => 14,   // S_HEARTCOL
        37 => -1,   // S_SKULLCOL
        38 => 10,   // S_RSKULL
        39 => 10,   // S_YSKULL
        40 => 10,   // S_BSKULL
        41 => 6,    // S_EVILEYE
        42 => 6,    // S_FLOATSKULL
        43 => -1,   // S_TORCHTREE
        44 => 4,    // S_BLUETORCH
        45 => 4,    // S_GREENTORCH
        46 => 4,    // S_REDTORCH
        47 => -1,   // S_STALAGTITE
        48 => -1,   // S_TECHPILLAR
        49 => 10,   // S_BLOODYTWITCH
        50 => -1,   // S_MEAT2
        51 => -1,   // S_MEAT3
        52 => -1,   // S_MEAT4
        53 => -1,   // S_MEAT5
        54 => -1,   // S_BIGTREE
        55 => 4,    // S_BTORCHSHRT
        56 => 4,    // S_GTORCHSHRT
        57 => 4,    // S_RTORCHSHRT
        58 => 10,   // S_SARG_STND
        59 => -1,   // S_MEAT2
        60 => -1,   // S_MEAT4
        61 => -1,   // S_MEAT3
        62 => -1,   // S_MEAT5
        63 => 10,   // S_BLOODYTWITCH
        64 => 10,   // S_VILE_STND
        65 => 10,   // S_CPOS_STND
        66 => 10,   // S_SKEL_STND
        67 => 15,   // S_FATT_STND
        68 => 10,   // S_BSPI_STND
        69 => 10,   // S_BOS2_STND
        70 => 4,    // S_BBAR1
        71 => 10,   // S_PAIN_STND
        72 => -1,   // S_KEENSTND
        73 => -1,   // S_HANGNOGUTS
        74 => -1,   // S_HANGBNOBRAIN
        75 => -1,   // S_HANGTLOOKDN
        76 => -1,   // S_HANGTSKULL
        77 => -1,   // S_HANGTLOOKUP
        78 => -1,   // S_HANGTNOBRAIN
        79 => -1,   // S_COLONGIBS
        80 => -1,   // S_SMALLPOOL
        81 => -1,   // S_BRAINSTEM
        82 => -1,   // S_SHOT2
        83 => 6,    // S_MEGA
        84 => 10,   // S_SSWV_STND
        85 => 4,    // S_TECHLAMP
        86 => 4,    // S_TECH2LAMP
        87 => -1,   // S_NULL (spawn spot)
        88 => -1,   // S_BRAIN
        89 => 10,   // S_BRAINEYE
        2001 => -1, // S_SHOT
        2002 => -1, // S_MGUN
        2003 => -1, // S_LAUN
        2004 => -1, // S_PLAS
        2005 => -1, // S_CSAW
        2006 => -1, // S_BFUG
        2007 => -1, // S_CLIP
        2008 => -1, // S_SHEL
        2010 => -1, // S_ROCK
        2011 => -1, // S_STIM
        2012 => -1, // S_MEDI
        2013 => 6,  // S_SOUL
        2014 => 6,  // S_BON1
        2015 => 6,  // S_BON2
        2018 => 6,  // S_ARM1
        2019 => 6,  // S_ARM2
        2022 => 6,  // S_PINV
        2023 => -1, // S_PSTR
        2024 => 6,  // S_PINS
        2025 => -1, // S_SUIT
        2026 => 6,  // S_PMAP
        2028 => -1, // S_COLU
        2035 => 6,  // S_BAR1
        2045 => 6,  // S_PVIS
        2046 => -1, // S_BROK
        2047 => -1, // S_CELL
        2048 => -1, // S_AMMO
        2049 => -1, // S_SBOX
        3001 => 10, // S_TROO_STND
        3002 => 10, // S_SARG_STND
        3003 => 10, // S_BOSS_STND
        3004 => 10, // S_POSS_STND
        3005 => 10, // S_HEAD_STND
        3006 => 10, // S_SKULL_STND
        _ => return None,
    };
    Some(tics)
}

// ---------------------------------------------------------------------------
// GameState convenience method
// ---------------------------------------------------------------------------

impl GameState {
    /// Spawn all things from the level and return the player handle.
    ///
    /// This is a convenience wrapper around [`spawn_level_things`] with
    /// `game_mode = GameMode::SinglePlayer`.
    pub fn spawn_things(&mut self, level: &Level, skill: Skill) -> Option<MobjHandle> {
        spawn_level_things(self, level, skill, GameMode::SinglePlayer)
    }
}

// ---------------------------------------------------------------------------
// Nightmare respawn
// ---------------------------------------------------------------------------

/// Simulation tics per second (Doom runs at 35 Hz).
pub const TICRATE: u32 = 35;

/// Number of tics a dead monster waits before respawning on Nightmare.
///
/// 12 seconds * 35 tics/sec = 420 tics.
pub const NIGHTMARE_RESPAWN_TICS: i32 = 12 * TICRATE as i32;

/// Attempt to respawn a dead monster at its original spawn point.
///
/// On Nightmare difficulty, dead monsters (those with `MF_COUNTKILL` that
/// are in their death state) use `movecount` as a respawn timer.  Each tic,
/// `movecount` is incremented.  Once it reaches `NIGHTMARE_RESPAWN_TICS`
/// (420), the corpse is removed and a fresh monster is spawned at its
/// original map position with teleport fog effects.
///
/// Returns `true` if the monster respawned (corpse should be removed by
/// the caller), `false` if the timer is still counting or the mobj is
/// ineligible.
pub fn p_nightmare_respawn(gs: &mut GameState, level: Option<&Level>, handle: MobjHandle) -> bool {
    // Read all the data we need from the corpse before mutating.
    let (spawn_x, spawn_y, spawn_angle, spawn_type, corpse_x, corpse_y, movecount) = {
        let Some(mo) = gs.mobjslab.get(handle) else {
            return false;
        };

        // Must be a dead monster with a valid spawn type.
        if mo.spawn_type == 0 {
            return false;
        }

        (
            mo.spawn_x,
            mo.spawn_y,
            mo.spawn_angle,
            mo.spawn_type,
            mo.x,
            mo.y,
            mo.movecount,
        )
    };

    // Increment the respawn counter.
    if movecount < NIGHTMARE_RESPAWN_TICS {
        if let Some(mo) = gs.mobjslab.get_mut(handle) {
            mo.movecount += 1;
        }
        return false;
    }

    let Some(k) = doomed_type_to_kind(spawn_type) else {
        return false;
    };
    let kind = k;

    let spawn_radius = MOBJINFO[kind as usize].radius;
    let spawn_blocked = gs.mobjslab.iter_handles().any(|other| {
        if other == handle {
            return false;
        }
        let Some(mo) = gs.mobjslab.get(other) else {
            return false;
        };
        if mo.flags & flags::MF_SOLID == 0 || mo.health <= 0 {
            return false;
        }

        let combined_radius = spawn_radius + mo.radius;
        (spawn_x - mo.x).abs() < combined_radius && (spawn_y - mo.y).abs() < combined_radius
    });
    if spawn_blocked {
        return false;
    }
    if let Some(level) = level
        && !crate::movement::p_try_move(&gs.mobjslab, handle, spawn_x, spawn_y, level)
    {
        return false;
    }

    // --- Timer expired: respawn the monster ---

    // Spawn teleport fog at the corpse location.
    let mut fog_corpse = Mobj::new(MobjKind::SpawnFire, corpse_x, corpse_y, Bam::ZERO);
    fog_corpse.state = crate::mobj::StateNum(crate::states::ids::S_TFOG1);
    if let Some(entry) = STATES.get(crate::states::ids::S_TFOG1 as usize) {
        fog_corpse.tics = entry.tics;
    }
    fog_corpse.flags = flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY;
    if let Some(level) = level {
        sync_mobj_to_level(level, &mut fog_corpse);
    }
    gs.mobjslab.alloc(fog_corpse);

    // Spawn teleport fog at the original spawn point.
    let mut fog_spawn = Mobj::new(MobjKind::SpawnFire, spawn_x, spawn_y, Bam::ZERO);
    fog_spawn.state = crate::mobj::StateNum(crate::states::ids::S_TFOG1);
    if let Some(entry) = STATES.get(crate::states::ids::S_TFOG1 as usize) {
        fog_spawn.tics = entry.tics;
    }
    fog_spawn.flags = flags::MF_NOBLOCKMAP | flags::MF_NOGRAVITY;
    if let Some(level) = level {
        sync_mobj_to_level(level, &mut fog_spawn);
    }
    gs.mobjslab.alloc(fog_spawn);

    // Spawn a fresh monster at the original position.
    let mut fresh = Mobj::new(kind, spawn_x, spawn_y, spawn_angle);
    apply_mobjinfo_defaults(&mut fresh);
    if let Some(level) = level {
        sync_mobj_to_level(level, &mut fresh);
    }

    // Carry over the spawn-point data so it can respawn again.
    fresh.spawn_x = spawn_x;
    fresh.spawn_y = spawn_y;
    fresh.spawn_angle = spawn_angle;
    fresh.spawn_type = spawn_type;

    gs.mobjslab.alloc(fresh);

    // Remove the corpse.
    gs.mobjslab.free(handle);

    true
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobj::flags;
    use doom_map::lumps::NODE_SUBSECTOR_BIT;
    use doom_map::{Node, NodeBBox, Thing};

    /// Build a minimal valid Level with the given things list and floor height.
    fn make_test_level_with_things_and_floor(things: Vec<Thing>, floor_height: i16) -> Level {
        // 1x1 blockmap at origin with one empty block.
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes()); // offset
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes()); // sentinel
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).expect("value must exist in test");

        let reject = doom_map::Reject::parse_lump(&[0u8], 1).expect("value must exist in test");

        Level {
            name: "TEST".to_string(),
            things,
            linedefs: vec![doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: doom_map::SIDEDEF_NONE,
            }],
            sidedefs: vec![doom_map::Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"        ",
                lower_texture: *b"        ",
                middle_texture: *b"WALL1   ",
                sector: 0,
            }],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: 0 },
                doom_map::Vertex { x: 128, y: 0 },
            ],
            segs: vec![doom_map::Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            }],
            ssectors: vec![doom_map::Ssector {
                seg_count: 1,
                first_seg: 0,
            }],
            nodes: vec![],
            sectors: vec![doom_map::Sector {
                floor_height,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            }],
            reject,
            blockmap,
        }
    }

    /// Build a minimal valid Level with the given things list.
    fn make_test_level_with_things(things: Vec<Thing>) -> Level {
        make_test_level_with_things_and_floor(things, 0)
    }

    fn make_partition_test_level(things: Vec<Thing>, right_floor: i16, left_floor: i16) -> Level {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = doom_map::Blockmap::parse_lump(&bm_data).expect("value must exist in test");
        let reject = doom_map::Reject::parse_lump(&[0u8; 1], 2).expect("value must exist in test");

        Level {
            name: "TEST".to_string(),
            things,
            linedefs: vec![
                doom_map::Linedef {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: doom_map::FLAG_TWO_SIDED,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
                doom_map::Linedef {
                    from_vertex: 2,
                    to_vertex: 3,
                    flags: doom_map::FLAG_TWO_SIDED,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0,
                    left_sidedef: 1,
                },
            ],
            sidedefs: vec![
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"        ",
                    lower_texture: *b"        ",
                    middle_texture: *b"WALL0   ",
                    sector: 0,
                },
                doom_map::Sidedef {
                    x_offset: 0,
                    y_offset: 0,
                    upper_texture: *b"        ",
                    lower_texture: *b"        ",
                    middle_texture: *b"WALL1   ",
                    sector: 1,
                },
            ],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: -128 },
                doom_map::Vertex { x: 0, y: 128 },
                doom_map::Vertex { x: 64, y: -128 },
                doom_map::Vertex { x: 64, y: 128 },
            ],
            segs: vec![
                doom_map::Seg {
                    from_vertex: 0,
                    to_vertex: 1,
                    angle: 0,
                    linedef: 0,
                    direction: 0,
                    offset: 0,
                },
                doom_map::Seg {
                    from_vertex: 3,
                    to_vertex: 2,
                    angle: 0,
                    linedef: 1,
                    direction: 1,
                    offset: 0,
                },
            ],
            ssectors: vec![
                doom_map::Ssector {
                    seg_count: 1,
                    first_seg: 0,
                },
                doom_map::Ssector {
                    seg_count: 1,
                    first_seg: 1,
                },
            ],
            nodes: vec![Node {
                x: 0,
                y: 0,
                dx: 0,
                dy: 1,
                right_bbox: NodeBBox {
                    ymax: 128,
                    ymin: -128,
                    xmin: 0,
                    xmax: 128,
                },
                left_bbox: NodeBBox {
                    ymax: 128,
                    ymin: -128,
                    xmin: -128,
                    xmax: 0,
                },
                right_child: NODE_SUBSECTOR_BIT,
                left_child: NODE_SUBSECTOR_BIT | 1,
            }],
            sectors: vec![
                doom_map::Sector {
                    floor_height: right_floor,
                    ceil_height: 128,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
                doom_map::Sector {
                    floor_height: left_floor,
                    ceil_height: 192,
                    floor_flat: *b"FLAT1\0\0\0",
                    ceil_flat: *b"FLAT2\0\0\0",
                    light_level: 192,
                    special: 0,
                    tag: 0,
                },
            ],
            reject,
            blockmap,
        }
    }

    // ===================================================================
    // Player spawning
    // ===================================================================

    #[test]
    fn spawn_level_things_creates_player() {
        let level = make_test_level_with_things(vec![Thing {
            x: 100,
            y: 200,
            angle: 90,
            kind: 1,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        assert!(handle.is_some());
        let mo = gs
            .mobjslab
            .get(handle.expect("value must exist in test"))
            .expect("value must exist in test");
        assert_eq!(mo.kind, MobjKind::Player);
        assert_eq!(mo.x, Fixed16_16::from_int(100));
        assert_eq!(mo.y, Fixed16_16::from_int(200));
    }

    #[test]
    fn spawn_player_sets_player_state() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        assert!(handle.is_some());
        assert_eq!(gs.player.handle, handle.expect("value must exist in test"));
        assert_eq!(gs.player.health(), 100);
    }

    #[test]
    fn spawn_player_applies_mobjinfo() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer)
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        // Player MOBJINFO: health=100, radius=16, height=56
        assert_eq!(mo.health, 100);
        assert_eq!(mo.radius, Fixed16_16::from_int(16));
        assert_eq!(mo.height, Fixed16_16::from_int(56));
    }

    #[test]
    fn spawn_player_snaps_to_sector_floor_height() {
        let level = make_test_level_with_things_and_floor(
            vec![Thing {
                x: 32,
                y: 0,
                angle: 0,
                kind: 1,
                flags: 7,
            }],
            24,
        );
        let mut gs = GameState::new("E1M1");

        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer)
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");

        assert_eq!(mo.z, Fixed16_16::from_int(24));
    }

    #[test]
    fn spawn_nonplayer_snaps_to_sector_floor_height() {
        let level = make_test_level_with_things_and_floor(
            vec![Thing {
                x: 64,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 7,
            }],
            -32,
        );
        let mut gs = GameState::new("E1M1");

        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let trooper = gs
            .mobjslab
            .iter_handles()
            .find_map(|h| gs.mobjslab.get(h).filter(|mo| mo.kind == MobjKind::Trooper))
            .expect("trooper should spawn");

        assert_eq!(trooper.z, Fixed16_16::from_int(-32));
    }

    #[test]
    fn spawned_animated_item_persists_and_is_not_removed() {
        // Regression: animated pickups (e.g. green armor, doomednum 2018) use
        // the placeholder ITEM mobjinfo whose spawn_state is S_NULL (infinite
        // tics, -1). Vanilla P_SpawnMapThing randomizes the initial animation
        // phase, but the item loops its bright bobbing animation forever and
        // never disappears until collected. A prior bug applied the positive
        // randomized tics to the simulated item, so its state machine counted
        // down and transitioned S_NULL -> S_NULL, deleting the item a few tics
        // after level start. On E1M5 (DEMO1) this deleted the green armor before
        // the player reached it, so the player took the full 3-damage shotgun
        // pellet at lt109 instead of the armor-absorbed 2, diverging from the
        // vanilla demo (health 97 vs 98 at leveltime 110).
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 2018, // Green armor: vanilla spawnstate S_ARM1 has 6 tics.
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);

        let find_armor = |gs: &GameState| {
            gs.mobjslab.iter_handles().find(|&h| {
                gs.mobjslab
                    .get(h)
                    .is_some_and(|m| m.kind == MobjKind::GreenArmor)
            })
        };

        let handle = find_armor(&gs).expect("green armor should spawn");
        let armor = gs.mobjslab.get(handle).expect("value must exist in test");
        // Invariant: the item keeps its infinite tics so its state machine never
        // counts down to a removing S_NULL transition.
        assert_eq!(armor.tics, -1, "animated pickup must retain infinite tics");
        assert_ne!(
            armor.flags & flags::MF_SPECIAL,
            0,
            "pickup must remain MF_SPECIAL so it can be collected"
        );

        // Tick well past the 1..=6 tic window the bug used to remove it in.
        for _ in 0..20 {
            crate::tic::tick_all_mobjs(&mut gs, Some(&level));
        }
        assert!(
            find_armor(&gs).is_some(),
            "animated item must persist and not be removed by state countdown"
        );
    }

    #[test]
    fn spawn_nonplayer_on_partition_line_uses_doom_subsector_tiebreak_for_floor() {
        let level = make_partition_test_level(
            vec![Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 7,
            }],
            0,
            64,
        );
        let mut gs = GameState::new("E1M1");

        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let trooper = gs
            .mobjslab
            .iter_handles()
            .find_map(|h| gs.mobjslab.get(h).filter(|mo| mo.kind == MobjKind::Trooper))
            .expect("trooper should spawn");

        assert_eq!(
            trooper.z,
            Fixed16_16::from_int(64),
            "thing on a partition line should inherit the floor from Doom's chosen subsector"
        );
        assert_eq!(
            trooper.subsector, 1,
            "partition-line thing should resolve to the left subsector"
        );
    }

    #[test]
    fn sync_mobj_to_level_honors_spawnceiling() {
        let mut level = make_test_level_with_things(vec![]);
        level.sectors[0].ceil_height = 128;

        let mut mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(32),
            Fixed16_16::from_int(0),
            Bam::ZERO,
        );
        apply_mobjinfo_defaults(&mut mo);
        mo.flags |= flags::MF_SPAWNCEILING;

        sync_mobj_to_level(&level, &mut mo);

        assert_eq!(
            mo.z,
            Fixed16_16::from_int(72),
            "spawnceiling thing should hang from ceiling minus actor height"
        );
    }

    #[test]
    fn spawn_nonplayer_randomizes_tics_and_advances_rng() {
        // Vanilla P_SpawnMobj draws `lastlook = P_Random() % MAXPLAYERS` for
        // every spawned mobj, then P_SpawnMapThing does
        // `if (mobj->tics > 0) mobj->tics = 1 + (P_Random() % mobj->tics)`.
        // For a Trooper (spawnstate tics = 10) that is two draws total, and the
        // resulting tics must equal 1 + (draw % 10) in [1, 10].
        let level = make_test_level_with_things(vec![Thing {
            x: 64,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);

        let mut gs = GameState::new("E1M1");
        gs.rng.set_index(3);
        let index_before = gs.rng.index();

        // Predict the exact vanilla draws: lastlook then tics-randomization.
        let mut predictor = GameState::new("E1M1");
        predictor.rng.set_index(3);
        let _lastlook = (predictor.p_random() as u32) % 4;
        let expected_tics = (1 + (predictor.p_random() as i32 % 10)) as i16;

        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let trooper = gs
            .mobjslab
            .iter_handles()
            .find_map(|h| gs.mobjslab.get(h).filter(|mo| mo.kind == MobjKind::Trooper))
            .expect("trooper should spawn");

        assert_eq!(
            trooper.tics, expected_tics,
            "spawn tics must be the vanilla-randomized 1 + (P_Random() % spawnstate_tics)"
        );
        assert!(
            (1..=10).contains(&trooper.tics),
            "randomized tics must be in [1, spawnstate_tics]"
        );
        assert_eq!(
            gs.rng.index(),
            (index_before + 2) & 255,
            "spawn must consume exactly two RNG bytes (lastlook + tic randomization)"
        );
    }

    // ===================================================================
    // Skill filtering
    // ===================================================================

    #[test]
    fn spawn_filters_by_skill_easy_only() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 1,
            }, // easy only (Trooper)
            Thing {
                x: 100,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 4,
            }, // hard only
        ]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Easy, GameMode::SinglePlayer);
        // Only the easy-skill trooper should exist.
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn spawn_filters_by_skill_hard_only() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 1,
            }, // easy only
            Thing {
                x: 100,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 4,
            }, // hard only
        ]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Hard, GameMode::SinglePlayer);
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn spawn_all_skills_when_bits_are_7() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        for skill in [
            Skill::Baby,
            Skill::Easy,
            Skill::Medium,
            Skill::Hard,
            Skill::Nightmare,
        ] {
            let mut gs = GameState::new("TEST");
            spawn_level_things(&mut gs, &level, skill, GameMode::SinglePlayer);
            let count = gs
                .mobjslab
                .iter_handles()
                .filter(|&h| {
                    gs.mobjslab
                        .get(h)
                        .map(|m| m.kind == MobjKind::Trooper)
                        .unwrap_or(false)
                })
                .count();
            assert_eq!(
                count, 1,
                "Skill {:?} should spawn thing with flags=7",
                skill
            );
        }
    }

    #[test]
    fn spawn_skips_thing_with_no_skill_bit() {
        // Vanilla P_SpawnMapThing spawns a thing only when `options & bit` is
        // set for the active skill.  A thing with no skill bits (flags & 7 == 0)
        // fails that test at every skill and is NOT spawned.
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 0,
        }]);
        let mut gs = GameState::new("TEST");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(
            count, 0,
            "Things with no skill bit for the active skill must not spawn (vanilla)"
        );
    }

    // ===================================================================
    // Multiplayer filter
    // ===================================================================

    #[test]
    fn spawn_filters_multiplayer_only() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7 | 16,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer); // singleplayer
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 0, "Multiplayer-only thing should be filtered in SP");
    }

    #[test]
    fn spawn_multiplayer_in_deathmatch() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7 | 16,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::Deathmatch); // deathmatch
        let count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(count, 1, "Multiplayer-only thing should spawn in DM");
    }

    // ===================================================================
    // Ambush flag
    // ===================================================================

    #[test]
    fn spawn_sets_ambush_flag() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7 | 8,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert!(
            mo.flags & flags::MF_AMBUSH != 0,
            "MF_AMBUSH should be set for deaf monsters"
        );
    }

    #[test]
    fn spawn_no_ambush_when_not_flagged() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert!(
            mo.flags & flags::MF_AMBUSH == 0,
            "MF_AMBUSH should NOT be set without thing flag bit 3"
        );
    }

    // ===================================================================
    // Kill/item counters
    // ===================================================================

    #[test]
    fn spawn_counts_kills_and_items() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 7,
            }, // Trooper (MF_COUNTKILL)
            Thing {
                x: 100,
                y: 0,
                angle: 0,
                kind: 2014,
                flags: 7,
            }, // HealthBonus (MF_COUNTITEM)
        ]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        assert_eq!(gs.stats.total_kills, 1, "Trooper should count as a kill");
        assert_eq!(
            gs.stats.total_items, 1,
            "HealthBonus should count as an item"
        );
    }

    #[test]
    fn spawn_counts_multiple_kills() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags: 7,
            }, // Trooper
            Thing {
                x: 100,
                y: 0,
                angle: 0,
                kind: 9,
                flags: 7,
            }, // Sergeant
            Thing {
                x: 200,
                y: 0,
                angle: 0,
                kind: 3001,
                flags: 7,
            }, // Imp
        ]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        assert_eq!(
            gs.stats.total_kills, 3,
            "All three monsters should be counted"
        );
    }

    // ===================================================================
    // MOBJINFO defaults
    // ===================================================================

    #[test]
    fn spawn_applies_mobjinfo_defaults() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        // Trooper MOBJINFO: health=20, radius=20, height=56, MF_SOLID|MF_SHOOTABLE|MF_COUNTKILL
        assert_eq!(mo.health, 20);
        assert_eq!(mo.radius, Fixed16_16::from_int(20));
        assert_eq!(mo.height, Fixed16_16::from_int(56));
        assert!(mo.flags & flags::MF_SOLID != 0);
        assert!(mo.flags & flags::MF_SHOOTABLE != 0);
        assert!(mo.flags & flags::MF_COUNTKILL != 0);
    }

    #[test]
    fn spawn_applies_barrel_defaults() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 2035,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Barrel)
                    .unwrap_or(false)
            })
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(mo.health, 20, "Barrel health should be 20");
        assert!(mo.flags & flags::MF_SOLID != 0, "Barrel should be solid");
        assert!(
            mo.flags & flags::MF_SHOOTABLE != 0,
            "Barrel should be shootable"
        );
    }

    // ===================================================================
    // Angle conversion
    // ===================================================================

    #[test]
    fn spawn_converts_angle_to_bam() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 180,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        // 180 degrees should be approximately ANG180 (0x8000_0000)
        let expected = Bam(0x8000_0000);
        let diff = mo.angle.0.wrapping_sub(expected.0);
        assert!(
            !(0x0100_0000..=0xFF00_0000).contains(&diff),
            "180 degrees should convert to approximately ANG180, got {:08X}",
            mo.angle.0
        );
    }

    #[test]
    fn spawn_converts_angle_0_degrees() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        assert_eq!(mo.angle.0, 0, "0 degrees should be BAM 0");
    }

    #[test]
    fn spawn_converts_angle_90_degrees() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 90,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        // 90 degrees = ANG90 = 0x4000_0000
        let expected = Bam(0x4000_0000);
        let diff = mo.angle.0.wrapping_sub(expected.0);
        assert!(
            !(0x0100_0000..=0xFF00_0000).contains(&diff),
            "90 degrees should convert to approximately ANG90, got {:08X}",
            mo.angle.0
        );
    }

    // ===================================================================
    // GameState convenience method
    // ===================================================================

    #[test]
    fn gs_spawn_things_convenience() {
        let level = make_test_level_with_things(vec![
            Thing {
                x: 50,
                y: 50,
                angle: 0,
                kind: 1,
                flags: 7,
            },
            Thing {
                x: 100,
                y: 100,
                angle: 45,
                kind: 3004,
                flags: 7,
            },
        ]);
        let mut gs = GameState::new("E1M1");
        let player = gs.spawn_things(&level, Skill::Medium);
        assert!(player.is_some(), "Player should be spawned");
        // Should have player + trooper = 2 actors
        assert_eq!(gs.mobjslab.len(), 2);
    }

    // ===================================================================
    // Unknown thing types
    // ===================================================================

    #[test]
    fn spawn_skips_unknown_thing_types() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 9999,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        assert!(handle.is_none(), "No player should be spawned");
        assert!(gs.mobjslab.is_empty(), "Unknown type should be skipped");
    }

    // ===================================================================
    // Empty level
    // ===================================================================

    #[test]
    fn spawn_empty_level() {
        let level = make_test_level_with_things(vec![]);
        let mut gs = GameState::new("E1M1");
        let handle = spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        assert!(handle.is_none());
        assert!(gs.mobjslab.is_empty());
        assert_eq!(gs.stats.total_kills, 0);
        assert_eq!(gs.stats.total_items, 0);
    }

    // ===================================================================
    // Spawn state from MOBJINFO
    // ===================================================================

    #[test]
    fn spawn_sets_spawn_state_from_mobjinfo() {
        let level = make_test_level_with_things(vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 3004,
            flags: 7,
        }]);
        let mut gs = GameState::new("E1M1");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);
        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");
        let expected_state = MOBJINFO[MobjKind::Trooper as usize].spawn_state;
        assert_eq!(mo.state, expected_state);
    }

    // ===================================================================
    // Skill filter behavior (vanilla `options & bit`)
    // ===================================================================

    #[test]
    fn spawn_skill_bit_filter_matches_vanilla() {
        // Vanilla bit: baby/easy -> 1, medium -> 2, hard/nightmare -> 4.
        // A thing spawns iff `options & bit`.
        let cases = [
            (1u16, Skill::Easy, true),
            (1u16, Skill::Medium, false),
            (1u16, Skill::Hard, false),
            (2u16, Skill::Easy, false),
            (2u16, Skill::Medium, true),
            (2u16, Skill::Hard, false),
            (4u16, Skill::Easy, false),
            (4u16, Skill::Medium, false),
            (4u16, Skill::Hard, true),
            (7u16, Skill::Easy, true),
            (7u16, Skill::Medium, true),
            (7u16, Skill::Hard, true),
            (0u16, Skill::Medium, false),
        ];
        for (flags, skill, should_spawn) in cases {
            let level = make_test_level_with_things(vec![Thing {
                x: 0,
                y: 0,
                angle: 0,
                kind: 3004,
                flags,
            }]);
            let mut gs = GameState::new("TEST");
            spawn_level_things(&mut gs, &level, skill, GameMode::SinglePlayer);
            let count = gs
                .mobjslab
                .iter_handles()
                .filter(|&h| {
                    gs.mobjslab
                        .get(h)
                        .map(|m| m.kind == MobjKind::Trooper)
                        .unwrap_or(false)
                })
                .count();
            assert_eq!(
                count,
                usize::from(should_spawn),
                "flags={flags:#x} skill={skill:?} expected spawn={should_spawn}"
            );
        }
    }

    #[test]
    fn spawn_thing_tics_matches_vanilla_table() {
        // Spot-check the generated doomednum -> spawnstate-tics table against
        // vanilla info.c values.
        assert_eq!(spawn_thing_tics(3004), Some(10)); // Trooper S_POSS_STND
        assert_eq!(spawn_thing_tics(3001), Some(10)); // Imp S_TROO_STND
        assert_eq!(spawn_thing_tics(2035), Some(6)); // Barrel S_BAR1
        assert_eq!(spawn_thing_tics(2001), Some(-1)); // Shotgun (static)
        assert_eq!(spawn_thing_tics(2014), Some(6)); // HealthBonus S_BON1
        assert_eq!(spawn_thing_tics(48), Some(-1)); // Tech pillar (static decoration)
        assert_eq!(spawn_thing_tics(9999), None); // not placeable
        // Player/coop/deathmatch starts are handled before the mobjinfo lookup.
        assert_eq!(spawn_thing_tics(1), None);
        assert_eq!(spawn_thing_tics(11), None);
    }

    #[test]
    fn solid_decoration_dims_matches_vanilla_table() {
        // Regression: these MF_SOLID decorations must be spawned as collision
        // bodies so monsters/players/missiles clip them exactly as vanilla does.
        // Dropping them let a chasing imp walk through a techno-pillar in
        // DEMO3/E1M7 (lt970), flipping A_Chase into the P_NewChaseDir branch a
        // tic apart from vanilla and desyncing the demo. Values from info.c.
        assert_eq!(solid_decoration_dims(48), Some((16, 16, false))); // MT_MISC48 techno pillar
        assert_eq!(solid_decoration_dims(30), Some((16, 16, false))); // MT_MISC32 tall green pillar
        assert_eq!(solid_decoration_dims(54), Some((32, 16, false))); // MT_MISC76 large brown tree (radius 32)
        assert_eq!(solid_decoration_dims(49), Some((16, 68, true))); // MT_MISC51 hanging body (ceiling)
        assert_eq!(solid_decoration_dims(73), Some((16, 88, true))); // MT_MISC78 hanging body (ceiling)
        // Modeled / non-solid / non-placeable types must NOT be handled here.
        assert_eq!(solid_decoration_dims(3001), None); // imp (monster, modeled)
        assert_eq!(solid_decoration_dims(2035), None); // barrel (modeled)
        assert_eq!(solid_decoration_dims(2014), None); // health bonus (item, non-solid)
        assert_eq!(solid_decoration_dims(9999), None); // not placeable
    }

    // ===================================================================
    // Degrees to BAM conversion
    // ===================================================================

    #[test]
    fn degrees_to_bam_conversion() {
        assert_eq!(degrees_to_bam(0).0, 0);
        // 90 degrees ~ 0x4000_0000
        let bam90 = degrees_to_bam(90);
        let diff90 = bam90.0.wrapping_sub(0x4000_0000);
        assert!(!(0x0100_0000..=0xFF00_0000).contains(&diff90));
        // 180 degrees ~ 0x8000_0000
        let bam180 = degrees_to_bam(180);
        let diff180 = bam180.0.wrapping_sub(0x8000_0000);
        assert!(!(0x0100_0000..=0xFF00_0000).contains(&diff180));
        // 270 degrees ~ 0xC000_0000
        let bam270 = degrees_to_bam(270);
        let diff270 = bam270.0.wrapping_sub(0xC000_0000);
        assert!(!(0x0100_0000..=0xFF00_0000).contains(&diff270));
    }

    // ===================================================================
    // Nightmare respawn
    // ===================================================================

    /// Create a dead Trooper corpse with spawn-point data set.
    fn make_dead_trooper_corpse(gs: &mut GameState) -> crate::mobj::MobjHandle {
        let spawn_x = Fixed16_16::from_int(200);
        let spawn_y = Fixed16_16::from_int(300);
        let spawn_angle = degrees_to_bam(90);

        let mut mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(500),
            Fixed16_16::from_int(600),
            Bam::ZERO,
        );
        crate::spawn::apply_mobjinfo_defaults(&mut mo);

        // Simulate death: zero health, set MF_COUNTKILL (already set by mobjinfo).
        mo.health = 0;
        mo.movecount = 0;

        // Set spawn point data.
        mo.spawn_x = spawn_x;
        mo.spawn_y = spawn_y;
        mo.spawn_angle = spawn_angle;
        mo.spawn_type = 3004; // DoomEd type for Trooper

        gs.mobjslab.alloc(mo)
    }

    #[test]
    fn nightmare_respawn_constants() {
        assert_eq!(TICRATE, 35);
        assert_eq!(NIGHTMARE_RESPAWN_TICS, 420);
    }

    #[test]
    fn nightmare_respawn_timer_increments() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);

        // First call: movecount goes from 0 to 1, returns false.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .movecount,
            1
        );

        // Second call: movecount goes to 2.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .movecount,
            2
        );
    }

    #[test]
    fn nightmare_respawn_triggers_at_420_tics() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);

        // Set movecount just below threshold.
        gs.mobjslab
            .get_mut(handle)
            .expect("value must exist in test")
            .movecount = NIGHTMARE_RESPAWN_TICS - 1;

        // One more increment, still not at threshold.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .movecount,
            NIGHTMARE_RESPAWN_TICS
        );

        // Now at threshold — respawn should happen.
        assert!(p_nightmare_respawn(&mut gs, None, handle));

        // Original corpse handle should be freed.
        assert!(gs.mobjslab.get(handle).is_none());
    }

    #[test]
    fn nightmare_respawn_creates_fresh_monster_at_spawn_point() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);

        // Set movecount to threshold so respawn fires immediately.
        gs.mobjslab
            .get_mut(handle)
            .expect("value must exist in test")
            .movecount = NIGHTMARE_RESPAWN_TICS;

        let initial_count = gs.mobjslab.len();
        assert!(p_nightmare_respawn(&mut gs, None, handle));

        // Corpse removed, but fresh monster + 2 fog effects added.
        // Net: -1 corpse + 1 monster + 2 fog = +2
        assert_eq!(gs.mobjslab.len(), initial_count + 2);

        // Find the freshly-spawned Trooper (not SpawnFire).
        let fresh_handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("Fresh trooper should exist after respawn");

        let fresh = gs
            .mobjslab
            .get(fresh_handle)
            .expect("value must exist in test");

        // Verify spawn point position.
        assert_eq!(fresh.x, Fixed16_16::from_int(200));
        assert_eq!(fresh.y, Fixed16_16::from_int(300));

        // Verify full health restored.
        assert_eq!(fresh.health, 20); // Trooper spawn_health = 20

        // Verify spawn data carried over for re-respawning.
        assert_eq!(fresh.spawn_x, Fixed16_16::from_int(200));
        assert_eq!(fresh.spawn_y, Fixed16_16::from_int(300));
        assert_eq!(fresh.spawn_type, 3004);
    }

    #[test]
    fn nightmare_respawn_snaps_fresh_monster_and_fog_to_sector_floor() {
        let level = make_test_level_with_things_and_floor(vec![], 40);
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab
            .get_mut(handle)
            .expect("value must exist in test")
            .movecount = NIGHTMARE_RESPAWN_TICS;

        assert!(p_nightmare_respawn(&mut gs, Some(&level), handle));

        let fresh = gs
            .mobjslab
            .iter_handles()
            .find_map(|h| gs.mobjslab.get(h).filter(|mo| mo.kind == MobjKind::Trooper))
            .expect("fresh trooper should exist after respawn");
        assert_eq!(fresh.z, Fixed16_16::from_int(40));

        let fog_zs: Vec<Fixed16_16> = gs
            .mobjslab
            .iter_handles()
            .filter_map(|h| {
                gs.mobjslab
                    .get(h)
                    .filter(|mo| mo.kind == MobjKind::SpawnFire)
                    .map(|mo| mo.z)
            })
            .collect();
        assert_eq!(fog_zs.len(), 2);
        assert!(fog_zs.iter().all(|&z| z == Fixed16_16::from_int(40)));
    }

    #[test]
    fn nightmare_respawn_spawns_teleport_fog() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab
            .get_mut(handle)
            .expect("value must exist in test")
            .movecount = NIGHTMARE_RESPAWN_TICS;

        assert!(p_nightmare_respawn(&mut gs, None, handle));

        // Should have exactly 2 SpawnFire fog effects.
        let fog_count = gs
            .mobjslab
            .iter_handles()
            .filter(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::SpawnFire)
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(fog_count, 2, "Should spawn fog at corpse and spawn point");
    }

    #[test]
    fn nightmare_respawn_fails_when_spawn_spot_is_blocked_by_solid_actor() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab
            .get_mut(handle)
            .expect("value must exist in test")
            .movecount = NIGHTMARE_RESPAWN_TICS;

        let mut blocker = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::from_int(200),
            Fixed16_16::from_int(300),
            Bam::ZERO,
        );
        apply_mobjinfo_defaults(&mut blocker);
        blocker.health = 20;
        blocker.flags |= flags::MF_SOLID;
        gs.mobjslab.alloc(blocker);

        let initial_count = gs.mobjslab.len();
        assert!(
            !p_nightmare_respawn(&mut gs, None, handle),
            "blocked spawn spot should prevent Nightmare respawn"
        );
        assert!(
            gs.mobjslab.get(handle).is_some(),
            "corpse should remain when respawn is blocked"
        );
        assert_eq!(
            gs.mobjslab.len(),
            initial_count,
            "blocked respawn must not spawn fog or a fresh monster"
        );
    }

    #[test]
    fn nightmare_respawn_no_action_without_spawn_type() {
        let mut gs = GameState::new("TEST");
        let mut mo = Mobj::new(
            MobjKind::Trooper,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 0;
        mo.flags = flags::MF_COUNTKILL;
        mo.spawn_type = 0; // No spawn type — cannot respawn.
        let handle = gs.mobjslab.alloc(mo);

        // Should return false because spawn_type == 0.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
    }

    #[test]
    fn nightmare_respawn_does_not_happen_below_threshold() {
        let mut gs = GameState::new("TEST");
        let handle = make_dead_trooper_corpse(&mut gs);

        // Run 419 tics — should not respawn.
        for _ in 0..419 {
            assert!(!p_nightmare_respawn(&mut gs, None, handle));
        }

        // Monster should still be alive in the slab (corpse).
        assert!(gs.mobjslab.get(handle).is_some());
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .movecount,
            419
        );

        // 420th call pushes to threshold.
        assert!(!p_nightmare_respawn(&mut gs, None, handle));
        assert_eq!(
            gs.mobjslab
                .get(handle)
                .expect("value must exist in test")
                .movecount,
            NIGHTMARE_RESPAWN_TICS
        );

        // 421st call (at threshold) triggers respawn.
        assert!(p_nightmare_respawn(&mut gs, None, handle));
        assert!(gs.mobjslab.get(handle).is_none());
    }

    #[test]
    fn tick_all_mobjs_nightmare_triggers_respawn() {
        let mut gs = GameState::new("TEST");
        gs.skill = Skill::Nightmare;

        // Spawn a player so tick_all_mobjs can skip it.
        let player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let player_handle = gs.mobjslab.alloc(player_mo);
        gs.player = crate::player::PlayerState::pistol_start(player_handle);

        // Spawn a dead trooper with spawn data.
        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab
            .get_mut(handle)
            .expect("value must exist in test")
            .movecount = NIGHTMARE_RESPAWN_TICS;

        // Run tick_all_mobjs — should trigger respawn on Nightmare.
        crate::tic::tick_all_mobjs(&mut gs, None);

        // Corpse should be gone.
        assert!(gs.mobjslab.get(handle).is_none());

        // Fresh trooper should exist.
        let has_trooper = gs.mobjslab.iter_handles().any(|h| {
            gs.mobjslab
                .get(h)
                .map(|m| m.kind == MobjKind::Trooper && m.health > 0)
                .unwrap_or(false)
        });
        assert!(
            has_trooper,
            "Fresh trooper should exist after Nightmare respawn"
        );
    }

    #[test]
    fn tick_all_mobjs_no_respawn_on_lower_skill() {
        let mut gs = GameState::new("TEST");
        gs.skill = Skill::Hard; // Not Nightmare

        let player_mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        let player_handle = gs.mobjslab.alloc(player_mo);
        gs.player = crate::player::PlayerState::pistol_start(player_handle);

        let handle = make_dead_trooper_corpse(&mut gs);
        gs.mobjslab
            .get_mut(handle)
            .expect("value must exist in test")
            .movecount = NIGHTMARE_RESPAWN_TICS;

        // Run tick_all_mobjs on Hard — should NOT trigger respawn.
        crate::tic::tick_all_mobjs(&mut gs, None);

        // Corpse should still be there (not respawned).
        assert!(gs.mobjslab.get(handle).is_some());
    }

    #[test]
    fn spawn_level_things_saves_spawn_point() {
        let level = make_test_level_with_things(vec![Thing {
            x: 150,
            y: 250,
            angle: 45,
            kind: 3004, // Trooper
            flags: 7,
        }]);
        let mut gs = GameState::new("TEST");
        spawn_level_things(&mut gs, &level, Skill::Medium, GameMode::SinglePlayer);

        let handle = gs
            .mobjslab
            .iter_handles()
            .find(|&h| {
                gs.mobjslab
                    .get(h)
                    .map(|m| m.kind == MobjKind::Trooper)
                    .unwrap_or(false)
            })
            .expect("value must exist in test");
        let mo = gs.mobjslab.get(handle).expect("value must exist in test");

        assert_eq!(mo.spawn_x, Fixed16_16::from_int(150));
        assert_eq!(mo.spawn_y, Fixed16_16::from_int(250));
        assert_eq!(mo.spawn_type, 3004);
    }
}
