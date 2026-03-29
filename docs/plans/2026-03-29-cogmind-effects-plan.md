# Cogmind Visual Effects Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add lighting enhancements, particle effects, and a player sight-line ray to the Cogmind ASCII renderer.

**Architecture:** Three new modules (`lighting.rs`, `effects.rs`, `sight_line.rs`) under `doom-app/src/cogmind/` provide pure functions consumed by the existing `render.rs` compositor. No changes to `doom-game` or `doom-tui` crates — all new code is purely visual, no gameplay impact.

**Tech Stack:** Rust, doom-types `Bam` for angle math, doom-game `GameState`/`MobjSlab` for entity data (read-only). No new crate dependencies.

**Design doc:** `docs/plans/2026-03-29-cogmind-effects-design.md`

---

## Task 1: Lighting module — pure functions

**Files:**
- Create: `crates/doom-app/src/cogmind/lighting.rs`
- Modify: `crates/doom-app/src/cogmind/mod.rs` (add `pub mod lighting;`)

**Step 1: Write failing tests**

In `lighting.rs`, write a `#[cfg(test)] mod tests` block with tests for the four lighting functions:

```rust
//! Lighting helpers for cogmind-mode: flicker, radial, hazard glow, entity boost.

use super::glyphs::Rgb;

/// Maximum radius (in grid cells) for player radial light.
const PLAYER_LIGHT_RADIUS: i32 = 8;
/// Light bonus per cell of proximity to the player.
const LIGHT_PER_CELL: i32 = 15;

/// Sector specials that produce flickering lights in Doom.
const FLICKER_SPECIALS: [u16; 6] = [1, 2, 3, 12, 13, 17];

/// Simple cosmetic PRNG (xorshift32). NOT for gameplay — visual only.
#[must_use]
pub fn cosm_rand(state: u32) -> u32 {
    let mut s = state;
    s ^= s << 13;
    s ^= s >> 17;
    s ^= s << 5;
    s
}

/// Brightness delta for a flickering sector.
///
/// Returns 0 for non-flicker sector specials.
/// For flicker specials, returns a value in roughly -40..+40 range,
/// varying by `tic` and `sector_idx` so each sector flickers independently.
#[must_use]
pub fn flicker_offset(sector_special: u16, sector_idx: usize, tic: u32) -> i16 {
    if !FLICKER_SPECIALS.contains(&sector_special) {
        return 0;
    }
    let seed = (sector_idx as u32).wrapping_mul(2654435761) ^ tic;
    let r = cosm_rand(seed);
    // Map to -40..+40 range.
    ((r % 81) as i16) - 40
}

/// Additive light bonus from player proximity.
///
/// Uses Manhattan distance. Returns 0 if outside `PLAYER_LIGHT_RADIUS`.
#[must_use]
pub fn player_radial_bonus(player_tx: i32, player_ty: i32, tile_tx: i32, tile_ty: i32) -> i32 {
    let dist = (player_tx - tile_tx).abs() + (player_ty - tile_ty).abs();
    if dist >= PLAYER_LIGHT_RADIUS {
        0
    } else {
        (PLAYER_LIGHT_RADIUS - dist) * LIGHT_PER_CELL
    }
}

/// Blend a hazard glow tint into a tile color at ~20% intensity.
///
/// `base` is the tile's current fg color; `glow` is the hazard tint
/// (green for nukage, orange-red for lava).
#[must_use]
pub fn blend_hazard_glow(base: Rgb, glow: Rgb) -> Rgb {
    // 20% glow + 80% base
    (
        ((u16::from(base.0) * 4 + u16::from(glow.0)) / 5) as u8,
        ((u16::from(base.1) * 4 + u16::from(glow.1)) / 5) as u8,
        ((u16::from(base.2) * 4 + u16::from(glow.2)) / 5) as u8,
    )
}

/// Compute effective light for a tile, stacking all modifiers.
#[must_use]
pub fn effective_light(
    sector_light: u8,
    sector_special: u16,
    sector_idx: usize,
    tic: u32,
    player_tx: i32,
    player_ty: i32,
    tile_tx: i32,
    tile_ty: i32,
) -> u8 {
    let mut light = i32::from(sector_light);
    light += i32::from(flicker_offset(sector_special, sector_idx, tic));
    light += player_radial_bonus(player_tx, player_ty, tile_tx, tile_ty);
    light.clamp(0, 255) as u8
}

/// Boost light for entities in dark sectors so they remain readable.
///
/// If `light < 80`, bump to `min(light + 60, 120)`.
#[must_use]
pub fn entity_light_boost(light: u8) -> u8 {
    if light < 80 {
        (light as u16 + 60).min(120) as u8
    } else {
        light
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosm_rand_is_deterministic() {
        assert_eq!(cosm_rand(42), cosm_rand(42));
    }

    #[test]
    fn cosm_rand_different_seeds_differ() {
        assert_ne!(cosm_rand(1), cosm_rand(2));
    }

    #[test]
    fn flicker_offset_zero_for_normal_sector() {
        assert_eq!(flicker_offset(0, 0, 100), 0);
        assert_eq!(flicker_offset(5, 0, 100), 0); // nukage, not flicker
    }

    #[test]
    fn flicker_offset_nonzero_for_flicker_specials() {
        // At least some tics should produce nonzero offsets.
        let has_nonzero = (0..100).any(|tic| flicker_offset(1, 0, tic) != 0);
        assert!(has_nonzero, "flicker special 1 should flicker");
    }

    #[test]
    fn flicker_offset_in_range() {
        for tic in 0..200 {
            let off = flicker_offset(2, 7, tic);
            assert!((-40..=40).contains(&off), "offset {off} out of range");
        }
    }

    #[test]
    fn flicker_sectors_vary_independently() {
        let a = flicker_offset(1, 0, 10);
        let b = flicker_offset(1, 5, 10);
        // Different sector indices should (usually) give different offsets.
        // Not guaranteed for every pair, but over many tics they should diverge.
        let differ = (0..50).any(|t| flicker_offset(1, 0, t) != flicker_offset(1, 5, t));
        assert!(differ, "sectors 0 and 5 should flicker independently");
        let _ = (a, b);
    }

    #[test]
    fn player_radial_bonus_at_player() {
        let bonus = player_radial_bonus(10, 10, 10, 10);
        assert_eq!(bonus, PLAYER_LIGHT_RADIUS * LIGHT_PER_CELL); // 8*15=120
    }

    #[test]
    fn player_radial_bonus_adjacent() {
        let bonus = player_radial_bonus(10, 10, 11, 10);
        assert_eq!(bonus, (PLAYER_LIGHT_RADIUS - 1) * LIGHT_PER_CELL); // 7*15=105
    }

    #[test]
    fn player_radial_bonus_at_edge() {
        let bonus = player_radial_bonus(10, 10, 18, 10); // dist=8
        assert_eq!(bonus, 0);
    }

    #[test]
    fn player_radial_bonus_beyond_radius() {
        let bonus = player_radial_bonus(0, 0, 20, 20); // dist=40
        assert_eq!(bonus, 0);
    }

    #[test]
    fn blend_hazard_glow_20_percent() {
        let base = (100, 100, 100);
        let glow = (0, 255, 0); // green nukage
        let result = blend_hazard_glow(base, glow);
        // (100*4+0)/5=80, (100*4+255)/5=131, (100*4+0)/5=80
        assert_eq!(result, (80, 131, 80));
    }

    #[test]
    fn blend_hazard_glow_black_base() {
        let result = blend_hazard_glow((0, 0, 0), (200, 60, 0));
        // (0+200)/5=40, (0+60)/5=12, (0+0)/5=0
        assert_eq!(result, (40, 12, 0));
    }

    #[test]
    fn effective_light_clamps_high() {
        let light = effective_light(250, 0, 0, 0, 0, 0, 0, 0);
        // 250 + 0 flicker + 120 radial (at player pos) = 370 -> clamped to 255
        assert_eq!(light, 255);
    }

    #[test]
    fn effective_light_clamps_low() {
        // Far from player, dark sector, flicker might subtract.
        // Even if flicker subtracts 40 from light 10, result should be 0 not negative.
        let light = effective_light(10, 1, 0, 0, 0, 0, 100, 100);
        assert!(light <= 10); // radial is 0 at that distance, flicker might lower it
    }

    #[test]
    fn entity_light_boost_dark_sector() {
        assert_eq!(entity_light_boost(20), 80);
        assert_eq!(entity_light_boost(60), 120);
        assert_eq!(entity_light_boost(79), 120); // 79+60=139 clamped to 120
    }

    #[test]
    fn entity_light_boost_bright_sector_unchanged() {
        assert_eq!(entity_light_boost(80), 80);
        assert_eq!(entity_light_boost(200), 200);
    }
}
```

**Step 2: Add module to mod.rs**

In `crates/doom-app/src/cogmind/mod.rs`, add `pub mod lighting;`.

**Step 3: Run tests to verify they pass**

Run: `cargo test -p doom-app cogmind::lighting`
Expected: all 14 tests pass.

**Step 4: Commit**

```
feat(cogmind): add lighting module with flicker, radial, hazard glow, entity boost
```

---

## Task 2: Hazard glow precomputation on TileGrid

**Files:**
- Modify: `crates/doom-app/src/cogmind/tile_grid.rs`
- Modify: `crates/doom-app/src/cogmind/glyphs.rs` (make `Rgb` type public at module level)

**Step 1: Add `glow` field to `Tile`**

In `tile_grid.rs`, add to `Tile`:

```rust
use super::glyphs::Rgb;

pub struct Tile {
    pub kind: TileKind,
    pub sector_idx: Option<usize>,
    pub light: u8,
    /// Hazard glow tint from adjacent nukage/lava tiles, if any.
    pub glow: Option<Rgb>,
}
```

Update `Default` impl to include `glow: None`.

**Step 2: Add `compute_hazard_glow` function**

After `from_level` constructs the grid, add a pass that checks each floor tile's 4 neighbors for Nukage/Lava and sets `glow`:

```rust
/// Nukage glow color (green).
const NUKAGE_GLOW: Rgb = (0, 180, 0);
/// Lava glow color (orange-red).
const LAVA_GLOW: Rgb = (200, 80, 0);

fn compute_hazard_glow(tiles: &mut [Tile], grid_w: usize, grid_h: usize) {
    // Snapshot kinds to avoid aliasing issues.
    let kinds: Vec<TileKind> = tiles.iter().map(|t| t.kind).collect();
    for gy in 0..grid_h {
        for gx in 0..grid_w {
            let idx = gy * grid_w + gx;
            // Only floors/height-changes can receive glow.
            if !matches!(kinds[idx], TileKind::Floor | TileKind::DoorOpen | TileKind::HeightChange) {
                continue;
            }
            let neighbors = [
                if gy + 1 < grid_h { Some((gy + 1) * grid_w + gx) } else { None },
                if gx + 1 < grid_w { Some(gy * grid_w + gx + 1) } else { None },
                if gy > 0 { Some((gy - 1) * grid_w + gx) } else { None },
                if gx > 0 { Some(gy * grid_w + gx - 1) } else { None },
            ];
            for ni in neighbors.into_iter().flatten() {
                match kinds[ni] {
                    TileKind::Nukage => { tiles[idx].glow = Some(NUKAGE_GLOW); break; }
                    TileKind::Lava => { tiles[idx].glow = Some(LAVA_GLOW); break; }
                    _ => {}
                }
            }
        }
    }
}
```

Call `compute_hazard_glow(&mut tiles, grid_w, grid_h)` at the end of `from_level`, before constructing `Self`.

**Step 3: Write test**

```rust
#[test]
fn hazard_glow_adjacent_to_nukage() {
    // Manually build a small grid: floor next to nukage.
    let mut tiles = vec![Tile::default(); 4]; // 2x2
    tiles[0] = Tile { kind: TileKind::Floor, sector_idx: Some(0), light: 128, glow: None };
    tiles[1] = Tile { kind: TileKind::Nukage, sector_idx: Some(1), light: 128, glow: None };
    tiles[2] = Tile { kind: TileKind::Floor, sector_idx: Some(0), light: 128, glow: None };
    tiles[3] = Tile { kind: TileKind::Floor, sector_idx: Some(0), light: 128, glow: None };
    compute_hazard_glow(&mut tiles, 2, 2);
    // tiles[0] is east-neighbor of nukage -> should have glow
    assert!(tiles[0].glow.is_some());
    assert_eq!(tiles[0].glow.unwrap(), NUKAGE_GLOW);
    // tiles[1] is nukage itself -> no glow (not a floor)
    assert!(tiles[1].glow.is_none());
}

#[test]
fn hazard_glow_not_on_walls() {
    let mut tiles = vec![Tile::default(); 4];
    tiles[0] = Tile { kind: TileKind::Wall, sector_idx: Some(0), light: 128, glow: None };
    tiles[1] = Tile { kind: TileKind::Lava, sector_idx: Some(1), light: 128, glow: None };
    tiles[2] = Tile::default();
    tiles[3] = Tile::default();
    compute_hazard_glow(&mut tiles, 2, 2);
    assert!(tiles[0].glow.is_none(), "walls should not receive glow");
}
```

**Step 4: Run tests**

Run: `cargo test -p doom-app cogmind::tile_grid`
Expected: all existing + 2 new tests pass.

**Step 5: Commit**

```
feat(cogmind): precompute hazard glow on tile grid for nukage/lava adjacency
```

---

## Task 3: Sight-line module

**Files:**
- Create: `crates/doom-app/src/cogmind/sight_line.rs`
- Modify: `crates/doom-app/src/cogmind/mod.rs` (add `pub mod sight_line;`)

**Step 1: Write the module with tests**

```rust
//! Player sight-line: 2-3 cell directional ray from the player glyph.

use doom_types::Bam;

use super::glyphs::Rgb;

/// A single cell of the sight-line ray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SightCell {
    /// Terminal-space offset from the player cell.
    pub dx: i32,
    pub dy: i32,
    /// Glyph to render.
    pub glyph: char,
    /// Foreground color.
    pub fg: Rgb,
}

/// Bright cyan for cell 1 (closest to player).
const CYAN_BRIGHT: Rgb = (0, 180, 220);
/// Mid cyan for cell 2.
const CYAN_MID: Rgb = (0, 120, 160);
/// Dim cyan for cell 3.
const CYAN_DIM: Rgb = (0, 60, 80);

/// Direction vectors and glyphs for each of 8 octants.
///
/// Index = octant (0=East, 1=NE, 2=North, ... counter-clockwise).
/// `(dx, dy)` is in **Doom space** (Y+ = north). The caller must negate dy
/// for terminal rendering.
const OCTANTS: [(i32, i32, char); 8] = [
    ( 1,  0, '\u{25B8}'), // ▸ East
    ( 1,  1, '\u{2571}'), // ╱ NE
    ( 0,  1, '\u{25B4}'), // ▴ North
    (-1,  1, '\u{2572}'), // ╲ NW
    (-1,  0, '\u{25C2}'), // ◂ West
    (-1, -1, '\u{2571}'), // ╱ SW
    ( 0, -1, '\u{25BE}'), // ▾ South
    ( 1, -1, '\u{2572}'), // ╲ SE
];

/// Convert a `Bam` angle to an octant index (0..7).
///
/// Each octant spans 45 degrees. Octant 0 is centered on East (0 degrees).
/// We shift by half an octant (22.5 deg = ANG45/2) so boundaries fall between
/// cardinal/diagonal directions.
#[must_use]
pub fn angle_to_octant(angle: Bam) -> usize {
    // ANG45/2 = 0x1000_0000
    let shifted = angle.raw().wrapping_add(0x1000_0000);
    // Top 3 bits give octant index 0..7.
    ((shifted >> 29) & 7) as usize
}

/// Produce up to 3 sight-line cells for the given player angle.
///
/// `is_wall` is a callback: `is_wall(dx, dy)` returns true if the tile at
/// (player_tx + dx, player_ty + dy) in terminal space is a wall. The ray
/// stops at walls.
///
/// The `dy` values in the returned cells are in **terminal space** (Y+ = down),
/// already flipped from Doom's Y+ = north.
#[must_use]
pub fn sight_line_cells(angle: Bam, is_wall: impl Fn(i32, i32) -> bool) -> Vec<SightCell> {
    let octant = angle_to_octant(angle);
    let (dx, dy, glyph) = OCTANTS[octant];

    let colors = [CYAN_BRIGHT, CYAN_MID, CYAN_DIM];
    let glyphs = [glyph, '\u{00B7}', '\u{00B7}']; // cell 1: arrow, cells 2-3: ·

    let mut cells = Vec::with_capacity(3);
    for i in 1..=3 {
        let sx = dx * i;
        let sy = -(dy * i); // flip Y for terminal
        if is_wall(sx, sy) {
            break;
        }
        cells.push(SightCell {
            dx: sx,
            dy: sy,
            glyph: glyphs[(i - 1) as usize],
            fg: colors[(i - 1) as usize],
        });
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octant_east() {
        assert_eq!(angle_to_octant(Bam::from_raw(0x0000_0000)), 0); // 0 deg = East
    }

    #[test]
    fn octant_north() {
        assert_eq!(angle_to_octant(Bam::from_raw(0x4000_0000)), 2); // 90 deg = North
    }

    #[test]
    fn octant_west() {
        assert_eq!(angle_to_octant(Bam::from_raw(0x8000_0000)), 4); // 180 deg = West
    }

    #[test]
    fn octant_south() {
        assert_eq!(angle_to_octant(Bam::from_raw(0xC000_0000)), 6); // 270 deg = South
    }

    #[test]
    fn octant_ne() {
        assert_eq!(angle_to_octant(Bam::from_raw(0x2000_0000)), 1); // 45 deg = NE
    }

    #[test]
    fn octant_wraps_around() {
        // Just below 360 deg should still be octant 0 (East).
        assert_eq!(angle_to_octant(Bam::from_raw(0xF000_0000)), 7); // 337.5 deg = SE
        assert_eq!(angle_to_octant(Bam::from_raw(0xFFFF_FFFF)), 0); // ~360 deg = East
    }

    #[test]
    fn sight_line_east_no_walls() {
        let cells = sight_line_cells(Bam::from_raw(0), |_, _| false);
        assert_eq!(cells.len(), 3);
        // East: dx=+1 per step, dy=0
        assert_eq!(cells[0].dx, 1);
        assert_eq!(cells[0].dy, 0);
        assert_eq!(cells[1].dx, 2);
        assert_eq!(cells[2].dx, 3);
        // First cell should have arrow glyph
        assert_eq!(cells[0].glyph, '\u{25B8}'); // ▸
        assert_eq!(cells[0].fg, CYAN_BRIGHT);
        // Second and third should be dots
        assert_eq!(cells[1].glyph, '\u{00B7}'); // ·
        assert_eq!(cells[2].glyph, '\u{00B7}');
    }

    #[test]
    fn sight_line_north_flips_y() {
        let cells = sight_line_cells(Bam::from_raw(0x4000_0000), |_, _| false);
        // North in Doom = Y+, but terminal flips: dy should be negative
        assert_eq!(cells[0].dx, 0);
        assert_eq!(cells[0].dy, -1);
    }

    #[test]
    fn sight_line_stops_at_wall() {
        // Wall at step 2
        let cells = sight_line_cells(Bam::from_raw(0), |dx, _| dx == 2);
        assert_eq!(cells.len(), 1); // only cell 1 before the wall
    }

    #[test]
    fn sight_line_wall_at_step_1() {
        // Wall right in front of player
        let cells = sight_line_cells(Bam::from_raw(0), |_, _| true);
        assert_eq!(cells.len(), 0);
    }
}
```

**Step 2: Add to mod.rs**

Add `pub mod sight_line;` to `crates/doom-app/src/cogmind/mod.rs`.

**Step 3: Run tests**

Run: `cargo test -p doom-app cogmind::sight_line`
Expected: all 9 tests pass.

**Step 4: Commit**

```
feat(cogmind): add sight-line module with angle-to-octant and directional ray
```

---

## Task 4: Effects module — particle system

**Files:**
- Create: `crates/doom-app/src/cogmind/effects.rs`
- Modify: `crates/doom-app/src/cogmind/mod.rs` (add `pub mod effects;`)

**Step 1: Write the module with tests**

```rust
//! Particle effect layer for cogmind-mode rendering.
//!
//! Manages transient visual effects (combat debris, projectile trails, ambient
//! dust) that overlay the tile grid. Uses a cosmetic PRNG separate from the
//! game's `P_Random` to avoid netplay desync.

use std::collections::HashSet;

use doom_game::mobj::MobjHandle;
use doom_game::{GameState, MobjKind};

use super::glyphs::Rgb;
use super::lighting::cosm_rand;
use super::tile_grid::CELL_SIZE;
use super::visibility::{SectorVisibility, VisibilityMap};

/// Hard cap on total concurrent effects.
const MAX_EFFECTS: usize = 128;
/// Max concurrent ambient dust particles.
const MAX_DUST: usize = 15;

/// A single transient visual effect.
#[derive(Clone, Debug)]
pub struct Effect {
    /// Map-unit X position.
    pub x: i32,
    /// Map-unit Y position.
    pub y: i32,
    /// Display character.
    pub glyph: char,
    /// Foreground color (faded over lifetime if `fade` is set).
    pub fg: Rgb,
    /// Tics remaining before removal.
    pub lifetime: u8,
    /// Maximum lifetime (for fade calculation).
    pub max_lifetime: u8,
    /// Whether to dim fg proportional to remaining life.
    pub fade: bool,
    /// Whether this is a dust particle (counted toward dust cap).
    pub is_dust: bool,
}

impl Effect {
    /// Current fg color, dimmed if fading.
    #[must_use]
    pub fn current_fg(&self) -> Rgb {
        if !self.fade || self.max_lifetime == 0 {
            return self.fg;
        }
        let ratio = u16::from(self.lifetime) * 255 / u16::from(self.max_lifetime);
        (
            (u16::from(self.fg.0) * ratio / 255) as u8,
            (u16::from(self.fg.1) * ratio / 255) as u8,
            (u16::from(self.fg.2) * ratio / 255) as u8,
        )
    }
}

/// Manages the particle overlay layer.
pub struct EffectLayer {
    /// Active effects.
    pub effects: Vec<Effect>,
    /// Cooldown timer for ambient dust spawns.
    dust_timer: u8,
    /// Cosmetic PRNG state (visual-only, no gameplay impact).
    rng: u32,
    /// Mobj handles we've already spawned combat debris for (avoid re-spawn).
    spawned_puffs: HashSet<MobjHandle>,
}

impl EffectLayer {
    /// Create a new empty effect layer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            effects: Vec::with_capacity(64),
            dust_timer: 0,
            rng: 0xDEAD_BEEF,
            spawned_puffs: HashSet::new(),
        }
    }

    /// Advance the PRNG and return a pseudo-random u32.
    fn rand(&mut self) -> u32 {
        self.rng = cosm_rand(self.rng);
        self.rng
    }

    /// Tick all effects: decrement lifetimes, remove expired ones.
    pub fn tick(&mut self) {
        self.effects.retain_mut(|e| {
            e.lifetime = e.lifetime.saturating_sub(1);
            e.lifetime > 0
        });
    }

    /// Add an effect, evicting oldest if at capacity.
    pub fn push(&mut self, effect: Effect) {
        if self.effects.len() >= MAX_EFFECTS {
            self.effects.remove(0);
        }
        self.effects.push(effect);
    }

    /// Spawn combat debris around BulletPuff and Blood mobjs.
    pub fn spawn_combat_debris(&mut self, gs: &GameState) {
        for handle in gs.mobjslab.iter_handles() {
            if self.spawned_puffs.contains(&handle) {
                continue;
            }
            let mobj = match gs.mobjslab.get(handle) {
                Some(m) => m,
                None => continue,
            };
            let (glyph, fg) = match mobj.kind {
                MobjKind::BulletPuff => ('\'', (180, 160, 80)),
                MobjKind::Blood => ('.', (160, 30, 30)),
                _ => continue,
            };
            self.spawned_puffs.insert(handle);

            let mx = mobj.x.to_int();
            let my = mobj.y.to_int();
            // Spawn 1-2 particles in adjacent cells.
            let count = (self.rand() % 2) + 1;
            for _ in 0..count {
                let r = self.rand();
                let ox = ((r % 3) as i32 - 1) * CELL_SIZE;
                let oy = (((r >> 8) % 3) as i32 - 1) * CELL_SIZE;
                let lifetime = ((self.rand() % 3) + 3) as u8; // 3-5 tics
                self.push(Effect {
                    x: mx + ox,
                    y: my + oy,
                    glyph,
                    fg,
                    lifetime,
                    max_lifetime: lifetime,
                    fade: true,
                    is_dust: false,
                });
            }
        }
    }

    /// Spawn trail dots behind in-flight projectiles.
    pub fn spawn_projectile_trails(&mut self, gs: &GameState) {
        for handle in gs.mobjslab.iter_handles() {
            let mobj = match gs.mobjslab.get(handle) {
                Some(m) => m,
                None => continue,
            };
            let fg = match mobj.kind {
                MobjKind::Rocket => (127, 80, 0),
                MobjKind::PlasmaBall => (40, 40, 127),
                MobjKind::BfgBall => (0, 127, 0),
                MobjKind::ArachPlaz => (0, 100, 0),
                MobjKind::Tracer => (127, 50, 0),
                MobjKind::ImpFireball => (100, 50, 20),
                MobjKind::CacoFireball => (100, 0, 100),
                MobjKind::BaronBall => (0, 100, 0),
                MobjKind::FatShot => (127, 40, 0),
                MobjKind::BossCube => (100, 0, 0),
                _ => continue,
            };

            let lifetime = ((self.rand() % 3) + 4) as u8; // 4-6 tics
            self.push(Effect {
                x: mobj.x.to_int(),
                y: mobj.y.to_int(),
                glyph: '\u{00B7}', // ·
                fg,
                lifetime,
                max_lifetime: lifetime,
                fade: true,
                is_dust: false,
            });
        }
    }

    /// Spawn ambient dust motes on random visible floor tiles.
    ///
    /// `visible_floors` is a list of `(map_x, map_y)` positions for visible
    /// floor-type tiles. Called once per frame; internally throttled.
    pub fn spawn_ambient_dust(&mut self, visible_floors: &[(i32, i32)]) {
        if self.dust_timer > 0 {
            self.dust_timer -= 1;
            return;
        }
        // Reset timer: 8-12 tics.
        self.dust_timer = ((self.rand() % 5) + 8) as u8;

        if visible_floors.is_empty() {
            return;
        }

        let dust_count = self.effects.iter().filter(|e| e.is_dust).count();
        if dust_count >= MAX_DUST {
            return;
        }

        let count = ((self.rand() % 2) + 1) as usize; // 1-2 particles
        for _ in 0..count {
            let idx = (self.rand() as usize) % visible_floors.len();
            let (mx, my) = visible_floors[idx];
            let glyph = if self.rand() % 2 == 0 { '\u{00B7}' } else { '\'' };
            let lifetime = ((self.rand() % 5) + 6) as u8; // 6-10 tics
            self.push(Effect {
                x: mx,
                y: my,
                glyph,
                fg: (40, 40, 45),
                lifetime,
                max_lifetime: lifetime,
                fade: true,
                is_dust: true,
            });
        }
    }

    /// Clear stale puff handles that no longer exist in the slab.
    pub fn clean_stale_handles(&mut self, gs: &GameState) {
        self.spawned_puffs.retain(|h| gs.mobjslab.get(*h).is_some());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_fade_full_life() {
        let e = Effect {
            x: 0, y: 0, glyph: '.', fg: (200, 100, 50),
            lifetime: 10, max_lifetime: 10, fade: true, is_dust: false,
        };
        assert_eq!(e.current_fg(), (200, 100, 50));
    }

    #[test]
    fn effect_fade_half_life() {
        let e = Effect {
            x: 0, y: 0, glyph: '.', fg: (200, 100, 50),
            lifetime: 5, max_lifetime: 10, fade: true, is_dust: false,
        };
        let fg = e.current_fg();
        // 200 * (5*255/10) / 255 = 200 * 127 / 255 ≈ 99
        assert!(fg.0 < 110 && fg.0 > 90, "r={}", fg.0);
    }

    #[test]
    fn effect_no_fade() {
        let e = Effect {
            x: 0, y: 0, glyph: '.', fg: (200, 100, 50),
            lifetime: 1, max_lifetime: 10, fade: false, is_dust: false,
        };
        assert_eq!(e.current_fg(), (200, 100, 50));
    }

    #[test]
    fn tick_decrements_and_removes() {
        let mut layer = EffectLayer::new();
        layer.push(Effect {
            x: 0, y: 0, glyph: '.', fg: (255, 0, 0),
            lifetime: 2, max_lifetime: 2, fade: false, is_dust: false,
        });
        layer.push(Effect {
            x: 0, y: 0, glyph: '.', fg: (0, 255, 0),
            lifetime: 1, max_lifetime: 1, fade: false, is_dust: false,
        });
        assert_eq!(layer.effects.len(), 2);
        layer.tick();
        // Green effect (lifetime 1) should be removed, red (now lifetime 1) remains.
        assert_eq!(layer.effects.len(), 1);
        assert_eq!(layer.effects[0].fg, (255, 0, 0));
        layer.tick();
        assert_eq!(layer.effects.len(), 0);
    }

    #[test]
    fn push_evicts_oldest_at_capacity() {
        let mut layer = EffectLayer::new();
        for i in 0..MAX_EFFECTS {
            layer.push(Effect {
                x: i as i32, y: 0, glyph: '.', fg: (0, 0, 0),
                lifetime: 10, max_lifetime: 10, fade: false, is_dust: false,
            });
        }
        assert_eq!(layer.effects.len(), MAX_EFFECTS);
        // Push one more — should evict the oldest (x=0).
        layer.push(Effect {
            x: 999, y: 0, glyph: '.', fg: (0, 0, 0),
            lifetime: 10, max_lifetime: 10, fade: false, is_dust: false,
        });
        assert_eq!(layer.effects.len(), MAX_EFFECTS);
        assert_eq!(layer.effects[0].x, 1); // oldest (x=0) evicted
        assert_eq!(layer.effects.last().unwrap().x, 999);
    }

    #[test]
    fn dust_throttled_by_timer() {
        let mut layer = EffectLayer::new();
        let floors = vec![(100, 100), (200, 200)];
        layer.spawn_ambient_dust(&floors);
        let count_after_first = layer.effects.len();
        assert!(count_after_first > 0, "should spawn on first call");
        // Immediately calling again should be throttled.
        layer.spawn_ambient_dust(&floors);
        assert_eq!(layer.effects.len(), count_after_first, "should be throttled");
    }

    #[test]
    fn dust_respects_cap() {
        let mut layer = EffectLayer::new();
        let floors = vec![(100, 100)];
        // Fill up to dust cap.
        for _ in 0..MAX_DUST {
            layer.push(Effect {
                x: 0, y: 0, glyph: '.', fg: (40, 40, 45),
                lifetime: 10, max_lifetime: 10, fade: true, is_dust: true,
            });
        }
        layer.dust_timer = 0; // force timer to allow spawn
        layer.spawn_ambient_dust(&floors);
        let dust_count = layer.effects.iter().filter(|e| e.is_dust).count();
        assert!(dust_count <= MAX_DUST + 1, "dust cap should be respected"); // +1 tolerance for boundary
    }

    #[test]
    fn effect_layer_new_is_empty() {
        let layer = EffectLayer::new();
        assert!(layer.effects.is_empty());
    }
}
```

**Step 2: Add to mod.rs**

Add `pub mod effects;` to `crates/doom-app/src/cogmind/mod.rs`.

**Step 3: Run tests**

Run: `cargo test -p doom-app cogmind::effects`
Expected: all 7 tests pass.

**Step 4: Commit**

```
feat(cogmind): add particle effect layer with combat debris, projectile trails, ambient dust
```

---

## Task 5: Wire everything into the compositor

**Files:**
- Modify: `crates/doom-app/src/cogmind/render.rs`

This is the integration task. Modify `CogmindState` and `render_frame` to use all three new modules.

**Step 1: Add `EffectLayer` to `CogmindState`**

```rust
use super::effects::EffectLayer;
use super::lighting::{effective_light, entity_light_boost, blend_hazard_glow};
use super::sight_line::sight_line_cells;

pub struct CogmindState {
    pub tile_grid: Option<TileGrid>,
    pub visibility: Option<VisibilityMap>,
    cached_level_name: String,
    pub effects: EffectLayer,
}
```

Update `new()` to include `effects: EffectLayer::new()`. Update `ensure_grid` to reset effects when the level changes: `self.effects = EffectLayer::new();`.

**Step 2: Modify render_frame — Phase 1 lighting**

Replace the flat `apply_light(tg.fg, light)` calls in Phase 1 with the new pipeline. For each tile:

```rust
// Get tic counter from game state.
let tic = gs.tic_num;
let player_tx = i32::from(tw / 2); // player is always centered
let player_ty = i32::from(th / 2);

// Inside the Phase 1 loop, replace the Visible branch:
SectorVisibility::Visible(light) => {
    let sector_special = level.sectors.get(sector_idx)
        .map_or(0, |s| s.special);
    let eff_light = effective_light(
        light, sector_special, sector_idx, tic,
        player_tx, player_ty,
        i32::from(tx), i32::from(ty),
    );
    let mut fg = apply_light(tg.fg, eff_light);
    let mut bg = apply_light(tg.bg, eff_light);
    // Apply hazard glow if present.
    if let Some(glow) = tile.glow {
        fg = blend_hazard_glow(fg, glow);
        bg = blend_hazard_glow(bg, glow);
    }
    CogmindCell { glyph: tg.glyph, fg, bg }
}
```

**Step 3: Modify render_frame — Phase 2 entity light boost**

In the entity overlay, replace:
```rust
let fg = apply_light(eg.fg, light);
```
with:
```rust
let boosted = entity_light_boost(light);
let fg = apply_light(eg.fg, boosted);
let bg = apply_light(eg.bg, boosted);
```

**Step 4: Add Phase 2.5 — effect overlay**

After the entity loop, add effect rendering. Also collect `visible_floors` during Phase 1 for dust spawning:

```rust
// Before Phase 1, declare:
let mut visible_floors: Vec<(i32, i32)> = Vec::new();

// In Phase 1 Visible branch, after writing the cell:
if matches!(effective_kind, TileKind::Floor | TileKind::DoorOpen) {
    visible_floors.push((map_x, map_y));
}
```

After Phase 2:
```rust
// --- Phase 2.5: effect particles ---
for effect in &self.effects.effects {
    let tx = (effect.x - vp_origin_x) / CELL_SIZE;
    let ty = (effect.y - vp_origin_y) / CELL_SIZE;
    if tx < 0 || ty < 0 || tx >= tw || ty >= th {
        continue;
    }
    let screen_y = (term_h.saturating_sub(1)).saturating_sub(ty as u16);
    let tx_u16 = tx as u16;
    // Don't overwrite the player glyph.
    if tx == player_tx && ty == player_ty {
        continue;
    }
    let fg = effect.current_fg();
    frame.set(tx_u16, screen_y, CogmindCell {
        glyph: effect.glyph,
        fg,
        bg: (0, 0, 0),
    });
}
```

**Step 5: Add Phase 3 — sight line**

After Phase 2.5:
```rust
// --- Phase 3: player sight line ---
let player_screen_x = player_tx as u16;
let player_screen_y = (term_h.saturating_sub(1)).saturating_sub(player_ty as u16);

let cells = sight_line_cells(player_mobj.angle, |dx, dy| {
    // Check if the tile at (player_tx + dx, player_ty + dy) is a wall.
    let check_tx = player_tx + dx;
    let check_ty = player_ty + dy;
    if check_tx < 0 || check_ty < 0 || check_tx >= tw || check_ty >= th {
        return true; // out of bounds = wall
    }
    let map_x = vp_origin_x + check_tx * CELL_SIZE + CELL_SIZE / 2;
    let map_y = vp_origin_y + check_ty * CELL_SIZE + CELL_SIZE / 2;
    let (gx, gy) = grid.map_to_grid(map_x, map_y);
    if gx < 0 || gy < 0 {
        return true;
    }
    grid.get(gx as usize, gy as usize)
        .map_or(true, |t| t.kind == TileKind::Wall)
});

for sc in &cells {
    let sx = player_screen_x as i32 + sc.dx;
    let sy = player_screen_y as i32 + sc.dy;
    if sx >= 0 && sy >= 0 && sx < i32::from(term_w) && sy < i32::from(term_h) {
        frame.set(sx as u16, sy as u16, CogmindCell {
            glyph: sc.glyph,
            fg: sc.fg,
            bg: (0, 0, 0),
        });
    }
}
```

**Step 6: Add tick/spawn call site**

`render_frame` is called each frame but effect ticking and spawning should happen once per game tic. Add a public method on `CogmindState`:

```rust
/// Tick effects and spawn new particles. Call once per game tic.
pub fn tick_effects(&mut self, gs: &GameState, vis: &VisibilityMap, visible_floors: &[(i32, i32)]) {
    self.effects.tick();
    self.effects.spawn_combat_debris(gs);
    self.effects.spawn_projectile_trails(gs);
    self.effects.spawn_ambient_dust(visible_floors);
    self.effects.clean_stale_handles(gs);
}
```

Since we need `visible_floors` which is computed during rendering, we'll instead call the effect methods directly inside `render_frame` (it's called once per frame which is once per tic in Doom). Move the tick/spawn to the beginning of `render_frame`, after locating the player but before Phase 1. Collect `visible_floors` will need to come from the *previous* frame or be done as a pre-pass. The simplest approach: tick + spawn combat + spawn trails at the start of render_frame (these don't need visible_floors), and spawn dust at the end using the visible_floors collected during Phase 1, for use next frame.

Actually, the cleanest approach: make `render_frame` take `&mut self` instead of `&self`, and do all effect work inside it:

```rust
pub fn render_frame(
    &mut self,  // was &self
    gs: &GameState,
    level: &Level,
    term_w: u16,
    term_h: u16,
) -> CogmindFrame {
    // ... existing grid/vis setup ...

    // Tick + spawn effects
    self.effects.tick();
    self.effects.spawn_combat_debris(gs);
    self.effects.spawn_projectile_trails(gs);

    // ... Phase 1 (collect visible_floors) ...
    // ... Phase 2 (entities) ...
    // ... Phase 2.5 (effects) ...
    // ... Phase 3 (sight line) ...

    // Spawn dust for next frame
    self.effects.spawn_ambient_dust(&visible_floors);
    self.effects.clean_stale_handles(gs);

    frame
}
```

**Step 7: Update call site in doom-app main.rs**

In `render_cogmind` (line ~1348 of `crates/doom-app/src/main.rs`), the call is `self.cogmind_state.render_frame(...)`. Since we changed `&self` to `&mut self`, this should already work because the outer method has `&mut self`.

**Step 8: Run full test suite**

Run: `cargo test -p doom-app`
Expected: all existing + new tests pass.

Run: `cargo clippy -p doom-app -- -D warnings`
Expected: no warnings.

**Step 9: Commit**

```
feat(cogmind): wire lighting pipeline, particle effects, and sight line into compositor
```

---

## Task 6: Full workspace verification

**Step 1:** Run `cargo fmt --all`
**Step 2:** Run `cargo clippy --workspace -- -D warnings`
**Step 3:** Run `cargo test --workspace`
**Step 4:** Fix any issues.
**Step 5:** Final commit if any formatting/lint fixes needed.
