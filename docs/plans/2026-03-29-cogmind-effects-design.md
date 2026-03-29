# Cogmind Visual Effects Enhancement

**Date:** 2026-03-29
**Status:** Approved
**Crates:** doom-app (cogmind module), doom-tui (CogmindFrame)

## Summary

Enhance the Cogmind-mode ASCII renderer with:
1. Player sight line (2-3 cell directional ray)
2. Radial player lighting
3. Hazard glow bleed (nukage/lava)
4. Flickering sector lights
5. Entity light boost for readability
6. Combat debris particles
7. Projectile trails
8. Ambient dust motes

## Architecture

### Effect Overlay Layer

New `EffectLayer` struct in `doom-app/src/cogmind/effects.rs`:

```rust
struct Effect {
    x: i32,              // map-unit position
    y: i32,
    glyph: char,
    fg: Rgb,
    lifetime: u8,        // tics remaining (max ~15)
    fade: bool,          // dim fg proportional to remaining life
}

struct EffectLayer {
    effects: Vec<Effect>,          // hard cap 128
    dust_timer: u8,                // cooldown for ambient dust spawns
    rng_state: u32,                // cosmetic PRNG (not P_Random)
    spawned_puffs: HashSet<u32>,   // mobj handles already spawned debris for
}
```

- Separate PRNG from game's `P_Random` — no netplay desync risk
- Effects tick down each frame, removed when lifetime hits 0
- Oldest evicted if vec hits 128 capacity

### Lighting Pipeline

Phase 1 tile rendering replaces flat `apply_light(color, sector_light)` with a stacked pipeline:

```
effective_light = sector_light
  |> apply_flicker(sector_special, tic_counter)   // ±20-40 for types 1/2/3/12/13/17
  |> apply_player_radial(tile_pos, player_pos)     // +15/cell, radius 8 cells
  |> apply_hazard_glow(neighbor_tiles)             // +tinted boost if adjacent nukage/lava
  |> clamp(0, 255)
```

**Flicker:** `flicker_offset(special, tic)` returns i16 delta. Cosmetic PRNG seeded by `(sector_idx ^ tic)`. Sector specials 1/2/3/12/13/17.

**Player radial:** `bonus = max(0, (RADIUS - manhattan_dist) * LIGHT_PER_CELL)`. Radius=8, per-cell=15. Tiles adjacent to player get +120, 8 cells out get +0.

**Hazard glow:** Precomputed `glow: Option<Rgb>` on `Tile` during grid construction. If any of 4 neighbors is Nukage/Lava, tile gets tint blended at ~20% intensity.

**Entity boost (#5):** Phase 2 entities with `effective_light < 80` get bumped to `min(light + 60, 120)`.

### Player Sight Line

Phase 3 — after entities and effects.

Player `Bam` angle → 8 octants → `(dx, dy)` step vector:

| Octant | Direction | Step |
|--------|-----------|------|
| 0 | East | (1, 0) |
| 1 | NE | (1, 1) |
| 2 | North | (0, 1) |
| 3 | NW | (-1, 1) |
| 4 | West | (-1, 0) |
| 5 | SW | (-1, -1) |
| 6 | South | (0, -1) |
| 7 | SE | (1, -1) |

Ray steps 1-3 cells from player:
- Cell 1: directional arrow glyph (▸/▴/▾/◂ or ╲/╱), bright cyan (0, 180, 220)
- Cell 2: `·`, mid cyan (0, 120, 160)
- Cell 3: `·`, dim cyan (0, 60, 80)

Ray stops at wall tiles. Y-flip applied same as tile/entity rendering.

### Particle Effects

**Combat debris (#6):** Scan mobj slab for BulletPuff/Blood. Per mobj, spawn 1-2 particles in adjacent cells (±1 x/y). Track spawned handles to avoid re-spawn. BulletPuff: `'` yellow (180, 160, 80). Blood: `.` dark red (160, 30, 30). Lifetime 3-5 tics, fading.

**Projectile trails (#7):** Per projectile mobj per frame, spawn `·` at current position. Color = projectile entity color at 50% brightness. Lifetime 4-6 tics, fading. Trail forms naturally as projectile moves.

**Ambient dust (#8):** Every 8-12 tics, spawn 1-2 `·`/`'` on random visible floor tiles. Gray (40, 40, 45). Lifetime 6-10 tics, fading. Cap 15 concurrent dust particles. Only in Visible sectors.

### Render Order

1. Phase 1: Tile grid (with enhanced lighting pipeline)
2. Phase 2: Entity overlay (with light boost)
3. Phase 2.5: Effect particles (never overwrite player `@`)
4. Phase 3: Player sight line

## Files to Create/Modify

| File | Action |
|------|--------|
| `doom-app/src/cogmind/effects.rs` | **Create** — EffectLayer, Effect, particle spawn/tick logic |
| `doom-app/src/cogmind/lighting.rs` | **Create** — flicker, radial, hazard glow, entity boost functions |
| `doom-app/src/cogmind/sight_line.rs` | **Create** — angle-to-octant, ray stepping, sight line rendering |
| `doom-app/src/cogmind/mod.rs` | **Modify** — add new submodules |
| `doom-app/src/cogmind/render.rs` | **Modify** — integrate lighting pipeline, effects, sight line into compositor |
| `doom-app/src/cogmind/tile_grid.rs` | **Modify** — add `glow: Option<Rgb>` to Tile, precompute hazard neighbor glow |
| `doom-app/src/cogmind/glyphs.rs` | **Modify** — export Rgb type, add sight-line glyph constants |
