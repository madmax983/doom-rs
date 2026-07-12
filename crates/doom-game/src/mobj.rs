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

use doom_types::mobj_kind::MobjKind;
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
    /// X-axis momentum (velocity).
    pub momx: Fixed16_16,
    /// Y-axis momentum (velocity).
    pub momy: Fixed16_16,
    /// Z-axis momentum (vertical velocity).
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

    // --- Blockmap thing-list links (vanilla `bnext`/`bprev`) ---
    /// Next actor in this actor's blockmap-cell list, or `NULL` if this is the
    /// tail / not linked.  Maintained by [`MobjSlab::set_thing_position`] /
    /// [`MobjSlab::unset_thing_position`], mirroring vanilla `mobj_t::bnext`.
    pub bnext: MobjHandle,
    /// Previous actor in this actor's blockmap-cell list, or `NULL` if this is
    /// the head / not linked (vanilla `mobj_t::bprev`).
    pub bprev: MobjHandle,

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
            bnext: MobjHandle::NULL,
            bprev: MobjHandle::NULL,
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
/// use doom_game::mobj::{MobjSlab, Mobj, MobjHandle};
/// use doom_types::mobj_kind::MobjKind;
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

    // --- Blockmap thing-list (vanilla `blocklinks`) ---
    /// Per-cell list heads: `blocklinks[row * bmap_width + col]` is the handle
    /// of the most-recently-linked actor whose origin is in that cell, or
    /// `NULL` for an empty cell.  Sized by [`Self::setup_blockmap`].  Iterate a
    /// cell via `bnext` (head-insertion order = newest first), matching vanilla
    /// `P_BlockThingsIterator`.
    blocklinks: Vec<MobjHandle>,
    /// Blockmap X origin in raw fixed-point (`x_origin << FRACBITS`).
    bmap_orgx: i32,
    /// Blockmap Y origin in raw fixed-point (`y_origin << FRACBITS`).
    bmap_orgy: i32,
    /// Blockmap width in cells.  Zero until [`Self::setup_blockmap`] runs; while
    /// zero all things are treated as off-map and are not linked.
    bmap_width: i32,
    /// Blockmap height in cells.
    bmap_height: i32,
}

impl MobjSlab {
    /// Blockmap-block shift in fixed-point space (vanilla `MAPBLOCKSHIFT`,
    /// `FRACBITS + 7` → 128-unit cells).
    const MAPBLOCKSHIFT: u32 = 16 + 7;

    /// Create an empty slab.
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            free_head: None,
            live_count: 0,
            next_generation: 1,
            blocklinks: Vec::new(),
            bmap_orgx: 0,
            bmap_orgy: 0,
            bmap_width: 0,
            bmap_height: 0,
        }
    }

    /// Size the per-cell blockmap thing-list heads for a level and clear them.
    ///
    /// Mirrors vanilla `P_LoadBlockMap`'s allocation of `blocklinks`
    /// (`bmapwidth * bmapheight` head pointers, all NULL).  Call once at level
    /// setup, **before** spawning any actor, so that each subsequent
    /// [`Self::alloc`] head-inserts in vanilla spawn order.  Every currently
    /// linked actor is implicitly unlinked (heads reset to NULL); callers that
    /// resize mid-level must re-link existing actors.
    pub fn setup_blockmap(&mut self, x_origin: i16, y_origin: i16, x_count: u16, y_count: u16) {
        self.bmap_orgx = i32::from(x_origin) << 16;
        self.bmap_orgy = i32::from(y_origin) << 16;
        self.bmap_width = i32::from(x_count);
        self.bmap_height = i32::from(y_count);
        let n = self.bmap_width as usize * self.bmap_height as usize;
        self.blocklinks = vec![MobjHandle::NULL; n];
    }

    /// Blockmap cell index for a fixed-point position, or `None` if the position
    /// is off the map (or the blockmap is unsized).  Uses the exact vanilla
    /// `(x - bmaporgx) >> MAPBLOCKSHIFT` derivation with the unclamped in-range
    /// test — mirroring `P_SetThingPosition` / `P_BlockThingsIterator`.
    #[inline]
    fn block_cell(&self, x: Fixed16_16, y: Fixed16_16) -> Option<usize> {
        if self.bmap_width == 0 || self.bmap_height == 0 {
            return None;
        }
        let bx = x.raw().wrapping_sub(self.bmap_orgx) >> Self::MAPBLOCKSHIFT;
        let by = y.raw().wrapping_sub(self.bmap_orgy) >> Self::MAPBLOCKSHIFT;
        if bx < 0 || bx >= self.bmap_width || by < 0 || by >= self.bmap_height {
            return None;
        }
        Some((by * self.bmap_width + bx) as usize)
    }

    /// Blockmap fixed-point origin X (vanilla `bmaporgx`), for consumers that
    /// derive a cell bbox from a fixed-point position (e.g. `P_RadiusAttack`).
    #[inline]
    pub fn bmap_orgx(&self) -> i32 {
        self.bmap_orgx
    }

    /// Blockmap fixed-point origin Y (vanilla `bmaporgy`).
    #[inline]
    pub fn bmap_orgy(&self) -> i32 {
        self.bmap_orgy
    }

    /// Blockmap width in cells (vanilla `bmapwidth`).
    #[inline]
    pub fn bmap_width(&self) -> i32 {
        self.bmap_width
    }

    /// Blockmap height in cells (vanilla `bmapheight`).
    #[inline]
    pub fn bmap_height(&self) -> i32 {
        self.bmap_height
    }

    /// Head of the blockmap thing-list for cell `(col, row)`, or `NULL` if the
    /// cell is out of range or empty.  Read-only accessor for consumers that
    /// walk the list via [`Mobj::bnext`] (used from Stage 2 onward).
    #[inline]
    pub fn block_things_head(&self, col: i32, row: i32) -> MobjHandle {
        if col < 0 || col >= self.bmap_width || row < 0 || row >= self.bmap_height {
            return MobjHandle::NULL;
        }
        self.blocklinks[(row * self.bmap_width + col) as usize]
    }

    /// Port of `P_SetThingPosition`'s blockmap-link step (`p_maputl.c:391`).
    ///
    /// Head-inserts `handle` into its home cell's list (new node becomes the
    /// head; the old head's `bprev` points back), so per-cell iteration order is
    /// the reverse of link order (newest first).  No-op for `MF_NOBLOCKMAP`
    /// actors and for actors off the blockmap (their `bnext`/`bprev` are left
    /// NULL — "thing is off the map").  Call **after** the actor's `x`/`y` and
    /// `flags` are final.
    pub fn set_thing_position(&mut self, handle: MobjHandle) {
        let (flags, x, y) = match self.get(handle) {
            Some(mo) => (mo.flags, mo.x, mo.y),
            None => return,
        };
        if flags & flags::MF_NOBLOCKMAP != 0 {
            return;
        }
        let Some(cell) = self.block_cell(x, y) else {
            // Off the map: not linked.
            if let Some(mo) = self.get_mut(handle) {
                mo.bnext = MobjHandle::NULL;
                mo.bprev = MobjHandle::NULL;
            }
            return;
        };
        let old_head = self.blocklinks[cell];
        if let Some(mo) = self.get_mut(handle) {
            mo.bprev = MobjHandle::NULL;
            mo.bnext = old_head;
        }
        if old_head != MobjHandle::NULL
            && let Some(oh) = self.get_mut(old_head)
        {
            oh.bprev = handle;
        }
        self.blocklinks[cell] = handle;
    }

    /// Port of `P_UnsetThingPosition`'s blockmap-unlink step (`p_maputl.c:343`).
    ///
    /// O(1) unlink of `handle` from its current cell list (patch the neighbors'
    /// `bnext`/`bprev`, and the cell head if this actor was the head — the head
    /// case re-derives the cell from `x`/`y`, so this MUST run **before** any
    /// `x`/`y` change).  No-op for `MF_NOBLOCKMAP` actors.  Safe to call more
    /// than once: the head is only cleared when it actually points at `handle`.
    pub fn unset_thing_position(&mut self, handle: MobjHandle) {
        let (flags, x, y, bnext, bprev) = match self.get(handle) {
            Some(mo) => (mo.flags, mo.x, mo.y, mo.bnext, mo.bprev),
            None => return,
        };
        if flags & flags::MF_NOBLOCKMAP != 0 {
            return;
        }
        if bnext != MobjHandle::NULL
            && let Some(n) = self.get_mut(bnext)
        {
            n.bprev = bprev;
        }
        if bprev != MobjHandle::NULL {
            if let Some(p) = self.get_mut(bprev) {
                p.bnext = bnext;
            }
        } else if let Some(cell) = self.block_cell(x, y)
            && self.blocklinks[cell] == handle
        {
            // This actor was the cell head; the guard makes a redundant unlink
            // safe (never clobbers a different actor's head).
            self.blocklinks[cell] = bnext;
        }
        if let Some(mo) = self.get_mut(handle) {
            mo.bnext = MobjHandle::NULL;
            mo.bprev = MobjHandle::NULL;
        }
    }

    /// Allocate a slot for `mobj`, returning a stable handle.
    pub fn alloc(&mut self, mobj: Mobj) -> MobjHandle {
        let new_gen = self.next_generation;
        // Advance generation, skip 0 (reserved as null sentinel).
        self.next_generation = self.next_generation.wrapping_add(1).max(1);

        self.live_count += 1;

        let handle = if let Some(free_idx) = self.free_head {
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
        };

        // Vanilla `P_SpawnMobj` ends with `P_SetThingPosition`, which links the
        // fresh actor into the blockmap.  The actor's `x`/`y`/`flags` are final
        // by the time it reaches `alloc`, so link here — no-op until a level has
        // called `setup_blockmap`, and for `MF_NOBLOCKMAP` / off-map actors.
        self.set_thing_position(handle);
        handle
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
        // Vanilla `P_RemoveMobj` calls `P_UnsetThingPosition` before releasing
        // the actor; unlink from the blockmap while `x`/`y` are still valid.
        self.unset_thing_position(handle);
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
        assert_eq!(
            slab.get(handle).expect("value must exist in test").kind,
            MobjKind::Player
        );
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
        slab2
            .get_mut(handle)
            .expect("value must exist in test")
            .health = 50;
        // Original unchanged.
        assert_eq!(
            slab.get(handle).expect("value must exist in test").health,
            100
        );
        assert_eq!(
            slab2.get(handle).expect("value must exist in test").health,
            50
        );
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
        slab.get_mut(handle)
            .expect("value must exist in test")
            .health = 42;
        assert_eq!(
            slab.get(handle).expect("value must exist in test").health,
            42
        );
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

        // This should trigger the unreachable!("free list points to occupied slot") panic
        slab.alloc(make_player_mobj());

        let _handle2 = slab.alloc(make_player_mobj());
    }

    #[test]
    #[should_panic(expected = "free list points to occupied slot")]
    fn alloc_free_list_occupied_panic() {
        let mut slab = MobjSlab::new();
        let handle1 = slab.alloc(make_player_mobj());
        slab.free(handle1);

        // Corrupt the free list to point to an occupied slot
        // In this case, `handle1.index` is the free head.
        // We will force it to be occupied.
        slab.slots[handle1.index as usize] = Slot::Occupied {
            mobj: make_player_mobj(),
            generation: 2,
        };

        // Attempting to allocate should follow the free list, encounter the Occupied slot, and panic.
        slab.alloc(make_player_mobj());
    }

    // -----------------------------------------------------------------------
    // Blockmap thing-list (blocklinks) — vanilla P_SetThingPosition /
    // P_UnsetThingPosition semantics.  These prove the structure is
    // vanilla-correct before any consumer relies on it (Stage 2+).
    // -----------------------------------------------------------------------

    /// A shootable, blockmap-linked actor at integer map position `(x, y)`.
    fn positioned_mobj(x: i32, y: i32) -> Mobj {
        let mut mo = Mobj::new(
            MobjKind::Imp,
            Fixed16_16::from_int(x),
            Fixed16_16::from_int(y),
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE; // NOT MF_NOBLOCKMAP
        mo
    }

    /// Collect a cell's list by walking `bnext` from the head (vanilla
    /// `P_BlockThingsIterator` order).
    fn cell_chain(slab: &MobjSlab, col: i32, row: i32) -> Vec<MobjHandle> {
        let mut out = Vec::new();
        let mut h = slab.block_things_head(col, row);
        while h != MobjHandle::NULL {
            out.push(h);
            h = slab.get(h).expect("linked handle must be live").bnext;
        }
        out
    }

    #[test]
    fn blocklinks_head_insertion_order_is_newest_first() {
        let mut slab = MobjSlab::new();
        slab.setup_blockmap(0, 0, 4, 4);
        // (10,10),(20,20),(30,30) all resolve to cell (0,0): 30<<16 >> 23 == 0.
        let a = slab.alloc(positioned_mobj(10, 10));
        let b = slab.alloc(positioned_mobj(20, 20));
        let c = slab.alloc(positioned_mobj(30, 30));
        // Iteration order = reverse of insertion (newest first).
        assert_eq!(cell_chain(&slab, 0, 0), vec![c, b, a]);
        // bprev threads the other way; head has NULL bprev, tail NULL bnext.
        assert_eq!(slab.get(c).expect("c").bprev, MobjHandle::NULL);
        assert_eq!(slab.get(b).expect("b").bprev, c);
        assert_eq!(slab.get(a).expect("a").bprev, b);
        assert_eq!(slab.get(a).expect("a").bnext, MobjHandle::NULL);
    }

    #[test]
    fn blocklinks_move_relinks_into_new_cell() {
        let mut slab = MobjSlab::new();
        slab.setup_blockmap(0, 0, 4, 4);
        let a = slab.alloc(positioned_mobj(10, 10));
        let b = slab.alloc(positioned_mobj(20, 20));
        let c = slab.alloc(positioned_mobj(30, 30));
        // Move B (middle of the (0,0) chain) to cell (1,0): x=200 -> col 1.
        slab.unset_thing_position(b);
        slab.get_mut(b).expect("b").x = Fixed16_16::from_int(200);
        slab.set_thing_position(b);
        // (0,0) now holds C -> A with B spliced out.
        assert_eq!(cell_chain(&slab, 0, 0), vec![c, a]);
        assert_eq!(slab.get(c).expect("c").bnext, a);
        assert_eq!(slab.get(a).expect("a").bprev, c);
        // B is the sole head of cell (1,0).
        assert_eq!(cell_chain(&slab, 1, 0), vec![b]);
        assert_eq!(slab.get(b).expect("b").bnext, MobjHandle::NULL);
        assert_eq!(slab.get(b).expect("b").bprev, MobjHandle::NULL);
    }

    #[test]
    fn blocklinks_remove_unlinks_from_cell() {
        let mut slab = MobjSlab::new();
        slab.setup_blockmap(0, 0, 4, 4);
        let a = slab.alloc(positioned_mobj(10, 10));
        let b = slab.alloc(positioned_mobj(20, 20));
        let c = slab.alloc(positioned_mobj(30, 30));
        // Free the tail (A): chain becomes C -> B, B is new tail.
        slab.free(a);
        assert_eq!(cell_chain(&slab, 0, 0), vec![c, b]);
        assert_eq!(slab.get(b).expect("b").bnext, MobjHandle::NULL);
        // Free the head (C): chain becomes just B, now both head and tail.
        slab.free(c);
        assert_eq!(cell_chain(&slab, 0, 0), vec![b]);
        assert_eq!(slab.get(b).expect("b").bprev, MobjHandle::NULL);
    }

    #[test]
    fn blocklinks_noblockmap_and_offmap_not_linked() {
        let mut slab = MobjSlab::new();
        slab.setup_blockmap(0, 0, 4, 4);
        // A NOBLOCKMAP actor (missile/puff/blood) is never linked.
        let mut miss = positioned_mobj(30, 30);
        miss.flags |= flags::MF_NOBLOCKMAP;
        let m = slab.alloc(miss);
        assert_eq!(slab.get(m).expect("m").bnext, MobjHandle::NULL);
        assert_eq!(slab.get(m).expect("m").bprev, MobjHandle::NULL);
        assert_eq!(slab.block_things_head(0, 0), MobjHandle::NULL);
        // An off-map actor (cell outside the 4x4 grid) is not linked either.
        let off = slab.alloc(positioned_mobj(100_000, 100_000));
        assert_eq!(slab.get(off).expect("off").bnext, MobjHandle::NULL);
        assert_eq!(slab.block_things_head(0, 0), MobjHandle::NULL);
    }

    #[test]
    fn blocklinks_every_linked_handle_home_cell_matches_position() {
        let mut slab = MobjSlab::new();
        slab.setup_blockmap(0, 0, 8, 8);
        // Scatter actors across several cells, two sharing a cell.
        let handles = [
            slab.alloc(positioned_mobj(10, 10)),   // (0,0)
            slab.alloc(positioned_mobj(300, 10)),  // (2,0)
            slab.alloc(positioned_mobj(300, 300)), // (2,2)
            slab.alloc(positioned_mobj(40, 20)),   // (0,0) again
        ];
        // Invariant: every linked handle hashes back to the cell it is in.
        let mut seen = 0;
        for row in 0..8 {
            for col in 0..8 {
                let mut h = slab.block_things_head(col, row);
                while h != MobjHandle::NULL {
                    let mo = slab.get(h).expect("linked handle must be live");
                    assert_eq!(
                        slab.block_cell(mo.x, mo.y),
                        Some((row * 8 + col) as usize),
                        "handle in cell ({col},{row}) has mismatched home cell"
                    );
                    seen += 1;
                    h = mo.bnext;
                }
            }
        }
        // Every allocated (blockmap-linked) handle appears exactly once.
        assert_eq!(seen, handles.len());
    }
}
