//! Map objects (actors): generational slab arena and type definitions.
//!
//! `MobjHandle` is a stable token; the generation counter prevents stale
//! handles from silently accessing reallocated slots (use-after-free safety).
//!
//! # Arena design
//! Each `MobjSlab` slot holds either an occupied `Mobj` + generation tag, or
//! a free-list link.  On `alloc`, the global generation counter is bumped.
//! `free()` invalidates all existing handles for that slot.  `get()` checks
//! the generation before returning a reference, so stale handles return `None`.

use doom_types::{Bam, Fixed16_16};

// ---------------------------------------------------------------------------
// MobjHandle
// ---------------------------------------------------------------------------

/// Stable handle to an actor in the `MobjSlab`.
///
/// Handles are `Copy` and can be stored in `GameState` without lifetime issues.
/// After `MobjSlab::free(handle)`, the handle becomes stale and all `get`
/// calls return `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct MobjHandle {
    /// Index into `MobjSlab::slots`.
    pub index: u32,
    /// Generation tag — must equal the slot's current generation.
    pub generation: u32,
}

impl MobjHandle {
    /// The null / sentinel handle.  Never resolves to an actor.
    pub const NULL: Self = Self {
        index: u32::MAX,
        generation: 0,
    };
}

// ---------------------------------------------------------------------------
// MobjKind — species/type
// ---------------------------------------------------------------------------

/// Species / type of a map object.
///
/// Values are stable (`repr u16`) for wire serialization and demo compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, strum_macros::FromRepr)]
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

// ---------------------------------------------------------------------------
// Mobj flags (MF_* constants)
// ---------------------------------------------------------------------------

/// Mobj behavior flags — combine with `|`.
pub mod flags {
    /// Can be picked up as a bonus item.
    pub const MF_SPECIAL: u32 = 0x0000_0001;
    /// Blocks movement of other solid actors.
    pub const MF_SOLID: u32 = 0x0000_0002;
    /// Can receive hitscan / projectile damage.
    pub const MF_SHOOTABLE: u32 = 0x0000_0004;
    /// Not linked into sector thing lists (no position query).
    pub const MF_NOSECTOR: u32 = 0x0000_0008;
    /// Not linked into blockmap.
    pub const MF_NOBLOCKMAP: u32 = 0x0000_0010;
    /// Won't react until first hit (ambush/deaf monster).
    pub const MF_AMBUSH: u32 = 0x0000_0020;
    /// Will try to attack immediately on next tic.
    pub const MF_JUSTHIT: u32 = 0x0000_0040;
    /// Has just attacked.
    pub const MF_JUSTATTACKED: u32 = 0x0000_0080;
    /// Spawned hanging from ceiling.
    pub const MF_SPAWNCEILING: u32 = 0x0000_0100;
    /// Floats — unaffected by gravity (cacodemon, etc.).
    pub const MF_NOGRAVITY: u32 = 0x0000_0200;
    /// Can fall off ledges.
    pub const MF_DROPOFF: u32 = 0x0000_0400;
    /// Actor picks up items.
    pub const MF_PICKUP: u32 = 0x0000_0800;
    /// No clipping — passes through walls/actors.
    pub const MF_NOCLIP: u32 = 0x0000_1000;
    /// Float-target altitude adjustment in progress.
    pub const MF_FLOAT: u32 = 0x0000_2000;
    /// Teleporting; bypass collision this tic.
    pub const MF_TELEPORT: u32 = 0x0000_4000;
    /// Is a missile projectile; explodes on contact.
    pub const MF_MISSILE: u32 = 0x0000_8000;
    /// Dropped by a dying enemy (counts differently for item %).
    pub const MF_DROPPED: u32 = 0x0001_0000;
    /// Partial invisibility (spectre blur effect).
    pub const MF_SHADOW: u32 = 0x0002_0000;
    /// No blood splat on hit.
    pub const MF_NOBLOOD: u32 = 0x0004_0000;
    /// Lying dead as a corpse.
    pub const MF_CORPSE: u32 = 0x0008_0000;
    /// Altitude-adjusting float in progress.
    pub const MF_INFLOAT: u32 = 0x0010_0000;
    /// Counts toward kill percentage.
    pub const MF_COUNTKILL: u32 = 0x0020_0000;
    /// Counts toward item percentage.
    pub const MF_COUNTITEM: u32 = 0x0040_0000;
    /// Lost soul flying skull attack.
    pub const MF_SKULLFLY: u32 = 0x0080_0000;
    /// Not placed in deathmatch games.
    pub const MF_NOTDMATCH: u32 = 0x0100_0000;
    /// Death scream has fired (set by A_Scream so audio layer can react).
    pub const MF_SCREAMED: u32 = 0x0200_0000;
}

// ---------------------------------------------------------------------------
// State machine types
// ---------------------------------------------------------------------------

/// Index into the global Mobj state table.
///
/// `StateNum::NULL (0)` is the "stop / remove" sentinel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct StateNum(pub u16);

impl StateNum {
    /// Sentinel: no further state transitions.
    pub const NULL: Self = Self(0);
}

/// One step in an actor's state machine.
#[derive(Clone, Copy, Debug)]
pub struct MobjStateEntry {
    /// Sprite number (index into `sprite_names::SPRITE_NAMES`).
    pub sprite: u16,
    /// Frame letter (0=A, 1=B, etc.) with optional fullbright bit (`0x80`).
    pub frame: u8,
    /// Tics to remain in this state.  Negative = stay indefinitely.
    pub tics: i16,
    /// Next state when `tics` reaches zero.
    pub next_state: StateNum,
    /// Action function index (0 = none; dispatched in `actions.rs`).
    pub action: u8,
}

impl MobjStateEntry {
    /// Fullbright flag — OR with `frame` to disable lighting on this frame.
    pub const FF_FULLBRIGHT: u8 = 0x80;

    /// A permanent idle state with no action and no sprite.
    pub const IDLE: Self = Self {
        sprite: 0xFFFF,
        frame: 0,
        tics: -1,
        next_state: StateNum::NULL,
        action: 0,
    };
}

// ---------------------------------------------------------------------------
// Mobj struct
// ---------------------------------------------------------------------------

/// A map object — player, monster, projectile, item, or decoration.
///
/// Lives inside `MobjSlab`; external code references it via `MobjHandle`.
#[derive(Clone, Debug)]
pub struct Mobj {
    /// Species / type.
    pub kind: MobjKind,

    // --- Position ---
    /// World X coordinate (fixed-point map units).
    pub x: Fixed16_16,
    /// World Y coordinate.
    pub y: Fixed16_16,
    /// Height above sector floor.
    pub z: Fixed16_16,
    /// Facing direction (binary angle measure).
    pub angle: Bam,

    // --- Behavior ---
    /// Behavior flags (`flags::MF_*` bitmask).
    pub flags: u32,
    /// Approximate radius for collision (fixed-point map units).
    pub radius: Fixed16_16,
    /// Height for collision (fixed-point map units).
    pub height: Fixed16_16,

    // --- Health ---
    /// Current hit points.  ≤ 0 means dead.
    pub health: i32,

    // --- Velocity ---
    pub momx: Fixed16_16,
    pub momy: Fixed16_16,
    pub momz: Fixed16_16,

    // --- State machine ---
    /// Current state index in the global state table.
    pub state: StateNum,
    /// Tics remaining in the current state.
    pub tics: i16,

    // --- AI ---
    /// Primary target (for monsters: who they're chasing/attacking).
    pub target: MobjHandle,
    /// Secondary target (Arch-Vile fire tracking, Revenant homing).
    pub tracer: MobjHandle,
    /// 8-way move direction (0 = east, 1 = northeast, ..., 7 = southeast).
    pub movedir: u8,
    /// Tics left before AI reconsiders its move direction.
    pub movecount: i32,
    /// Tics before the monster tries to attack (0 = can attack now).
    pub reactiontime: i32,
    /// Alert threshold: tics the monster stays alert after losing sight.
    pub threshold: i32,

    // --- World linkage ---
    /// Index of the subsector this actor occupies.
    pub subsector: u32,

    // --- Nightmare respawn ---
    /// Original spawn X (map units, fixed-point).
    pub spawn_x: Fixed16_16,
    /// Original spawn Y.
    pub spawn_y: Fixed16_16,
    /// Original facing angle at spawn.
    pub spawn_angle: Bam,
    /// DoomEd thing type for respawning.  0 = cannot respawn.
    pub spawn_type: u16,
}

impl Mobj {
    /// Construct an actor at `(x, y)` facing `angle` with no velocity.
    pub fn new(kind: MobjKind, x: Fixed16_16, y: Fixed16_16, angle: Bam) -> Self {
        Self {
            kind,
            x,
            y,
            z: Fixed16_16::ZERO,
            angle,
            flags: 0,
            radius: Fixed16_16::from_int(20), // default 20-unit radius
            height: Fixed16_16::from_int(56), // default 56-unit height
            health: 0,
            momx: Fixed16_16::ZERO,
            momy: Fixed16_16::ZERO,
            momz: Fixed16_16::ZERO,
            state: StateNum::NULL,
            tics: -1,
            target: MobjHandle::NULL,
            tracer: MobjHandle::NULL,
            movedir: 0,
            movecount: 0,
            reactiontime: 0,
            threshold: 0,
            subsector: 0,
            spawn_x: Fixed16_16::ZERO,
            spawn_y: Fixed16_16::ZERO,
            spawn_angle: Bam::ZERO,
            spawn_type: 0,
        }
    }

    /// Returns `true` if this actor is dead (health ≤ 0).
    #[inline]
    pub fn is_dead(&self) -> bool {
        self.health <= 0
    }

    /// Returns `true` if this actor blocks movement.
    #[inline]
    pub fn is_solid(&self) -> bool {
        self.flags & flags::MF_SOLID != 0
    }

    /// Returns `true` if this actor can be damaged.
    #[inline]
    pub fn is_shootable(&self) -> bool {
        self.flags & flags::MF_SHOOTABLE != 0
    }
}

// ---------------------------------------------------------------------------
// Generational slab arena
// ---------------------------------------------------------------------------

enum Slot {
    Occupied { mobj: Mobj, generation: u32 },
    Free { next_free: Option<u32> },
}

impl Clone for Slot {
    fn clone(&self) -> Self {
        match self {
            Slot::Occupied { mobj, generation } => Slot::Occupied {
                mobj: mobj.clone(),
                generation: *generation,
            },
            Slot::Free { next_free } => Slot::Free {
                next_free: *next_free,
            },
        }
    }
}

/// Generational slab arena — the backing store for all live actors.
///
/// Slot indices are stable; the generation counter prevents use-after-free.
/// Clone produces a fully independent deep copy for rollback snapshots.
///
/// # Usage
/// ```
/// use doom_game::mobj::{MobjSlab, Mobj, MobjHandle, MobjKind};
/// use doom_types::{Fixed16_16, Bam};
/// let mut slab = MobjSlab::new();
/// let mobj = Mobj::new(MobjKind::Player, Fixed16_16::ZERO, Fixed16_16::ZERO, Bam::ZERO);
/// let handle: MobjHandle = slab.alloc(mobj);
/// assert!(slab.get(handle).is_some());
/// slab.free(handle);
/// assert!(slab.get(handle).is_none());
/// ```
#[derive(Clone)]
pub struct MobjSlab {
    slots: Vec<Slot>,
    free_head: Option<u32>,
    live_count: usize,
    /// Next generation value to assign (never 0).
    next_generation: u32,
}

impl MobjSlab {
    /// Create an empty slab.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            live_count: 0,
            next_generation: 1,
        }
    }

    /// Allocate a slot for `mobj`, returning a stable handle.
    pub fn alloc(&mut self, mobj: Mobj) -> MobjHandle {
        let new_gen = self.next_generation;
        // Advance generation, skip 0 (reserved as null sentinel).
        self.next_generation = self.next_generation.wrapping_add(1).max(1);

        self.live_count += 1;

        if let Some(free_idx) = self.free_head {
            let next = match &self.slots[free_idx as usize] {
                Slot::Free { next_free } => *next_free,
                Slot::Occupied { .. } => unreachable!("free list points to occupied slot"),
            };
            self.free_head = next;
            self.slots[free_idx as usize] = Slot::Occupied {
                mobj,
                generation: new_gen,
            };
            MobjHandle {
                index: free_idx,
                generation: new_gen,
            }
        } else {
            let idx = self.slots.len() as u32;
            self.slots.push(Slot::Occupied {
                mobj,
                generation: new_gen,
            });
            MobjHandle {
                index: idx,
                generation: new_gen,
            }
        }
    }

    /// Release the slot for `handle`.  Returns `true` if the handle was valid.
    pub fn free(&mut self, handle: MobjHandle) -> bool {
        let idx = handle.index as usize;
        if idx >= self.slots.len() {
            return false;
        }
        match &self.slots[idx] {
            Slot::Occupied { generation, .. } if *generation == handle.generation => {}
            _ => return false,
        }
        self.slots[idx] = Slot::Free {
            next_free: self.free_head,
        };
        self.free_head = Some(handle.index);
        self.live_count -= 1;
        true
    }

    /// Borrow the `Mobj` for `handle`, or `None` if stale or out-of-bounds.
    pub fn get(&self, handle: MobjHandle) -> Option<&Mobj> {
        let idx = handle.index as usize;
        match self.slots.get(idx)? {
            Slot::Occupied { mobj, generation } if *generation == handle.generation => Some(mobj),
            _ => None,
        }
    }

    /// Mutably borrow the `Mobj` for `handle`.
    pub fn get_mut(&mut self, handle: MobjHandle) -> Option<&mut Mobj> {
        let idx = handle.index as usize;
        match self.slots.get_mut(idx)? {
            Slot::Occupied { mobj, generation } if *generation == handle.generation => Some(mobj),
            _ => None,
        }
    }

    /// Iterate all live handles in slot order.
    pub fn iter_handles(&self) -> impl Iterator<Item = MobjHandle> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| match slot {
                Slot::Occupied { generation, .. } => Some(MobjHandle {
                    index: i as u32,
                    generation: *generation,
                }),
                Slot::Free { .. } => None,
            })
    }

    /// Number of live actors.
    ///
    /// ⚡ Bolt: Tracking `live_count` explicitly reduces `len()` to an O(1) operation
    /// instead of requiring an O(N) iteration over the entire generational arena.
    #[inline]
    pub fn len(&self) -> usize {
        self.live_count
    }

    /// Total number of slots currently in the slab.
    pub fn slot_count(&self) -> u32 {
        self.slots.len() as u32
    }

    /// Returns the next generation value (used for non-allocating snapshots).
    #[inline]
    pub fn next_generation(&self) -> u32 {
        self.next_generation
    }

    /// Retrieves the live actor handle assigned to a specific generation slot index, ensuring the slot has not been recycled.
    pub fn handle_at(&self, index: u32) -> Option<MobjHandle> {
        match self.slots.get(index as usize)? {
            Slot::Occupied { generation, .. } => Some(MobjHandle {
                index,
                generation: *generation,
            }),
            Slot::Free { .. } => None,
        }
    }

    /// `true` if no actors are alive.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for MobjSlab {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for MobjSlab {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MobjSlab")
            .field("live_count", &self.len())
            .field("total_slots", &self.slots.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_player_mobj() -> Mobj {
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        mo
    }

    #[test]
    fn alloc_and_get() {
        let mut slab = MobjSlab::new();
        let handle = slab.alloc(make_player_mobj());
        assert!(slab.get(handle).is_some());
        assert_eq!(slab.get(handle).unwrap().kind, MobjKind::Player);
    }

    #[test]
    fn free_invalidates_handle() {
        let mut slab = MobjSlab::new();
        let handle = slab.alloc(make_player_mobj());
        assert!(slab.free(handle));
        assert!(slab.get(handle).is_none(), "stale handle must return None");
    }

    #[test]
    fn double_free_returns_false() {
        let mut slab = MobjSlab::new();
        let handle = slab.alloc(make_player_mobj());
        assert!(slab.free(handle));
        assert!(!slab.free(handle), "double-free must return false");
    }

    #[test]
    fn free_list_recycles_slot_with_new_generation() {
        let mut slab = MobjSlab::new();
        let h1 = slab.alloc(make_player_mobj());
        slab.free(h1);
        let h2 = slab.alloc(make_player_mobj());
        // Same slot index reused...
        assert_eq!(h1.index, h2.index);
        // ...but different generation.
        assert_ne!(h1.generation, h2.generation);
        // Old handle is now stale.
        assert!(slab.get(h1).is_none());
        assert!(slab.get(h2).is_some());
    }

    #[test]
    fn null_handle_returns_none() {
        let slab = MobjSlab::new();
        assert!(slab.get(MobjHandle::NULL).is_none());
    }

    #[test]
    fn iter_handles_skips_freed() {
        let mut slab = MobjSlab::new();
        let h1 = slab.alloc(make_player_mobj());
        let h2 = slab.alloc(make_player_mobj());
        let _h3 = slab.alloc(make_player_mobj());
        slab.free(h1);
        let live: Vec<_> = slab.iter_handles().collect();
        assert_eq!(live.len(), 2);
        assert!(!live.contains(&h1));
        assert!(live.contains(&h2));
    }

    #[test]
    fn clone_slab_is_deep_copy() {
        let mut slab = MobjSlab::new();
        let handle = slab.alloc(make_player_mobj());
        let mut slab2 = slab.clone();
        slab2.get_mut(handle).unwrap().health = 50;
        // Original unchanged.
        assert_eq!(slab.get(handle).unwrap().health, 100);
        assert_eq!(slab2.get(handle).unwrap().health, 50);
    }

    #[test]
    fn is_dead_and_shootable_flags() {
        let mut mo = make_player_mobj();
        assert!(!mo.is_dead());
        mo.health = 0;
        assert!(mo.is_dead());
        assert!(mo.is_solid());
        assert!(mo.is_shootable());
    }

    #[test]
    fn get_mut_allows_mutation() {
        let mut slab = MobjSlab::new();
        let handle = slab.alloc(make_player_mobj());
        slab.get_mut(handle).unwrap().health = 42;
        assert_eq!(slab.get(handle).unwrap().health, 42);
    }

    #[test]
    #[should_panic(expected = "free list points to occupied slot")]
    fn alloc_unreachable_panic() {
        let mut slab = MobjSlab::new();
        let handle1 = slab.alloc(make_player_mobj());
        slab.free(handle1);

        // Artificially corrupt the slab to hit the unreachable arm
        slab.slots[0] = Slot::Occupied {
            mobj: make_player_mobj(),
            generation: 1,
        };

        let _handle2 = slab.alloc(make_player_mobj());
    }
}
