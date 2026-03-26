# BSP Raycaster Design — abrash-raycast Integration

**Date:** 2026-03-26
**Goal:** Move doom-rs's rendering engine into abrash as a reusable BSP raycaster, then add a GPU backend for GZDoom-style visuals.

## Context

doom-rs has a working 320x200 software renderer (~7,600 lines in `render.rs`). abrash has a DDA grid raycaster (`abrash-raycast`) and a wgpu deferred GPU renderer (`abrash-gpu-render`). The DDA raycaster uses a `GridMap` trait for wall lookups.

Doom uses BSP tree traversal instead of DDA grid stepping. Both answer "what does this ray hit?" but the traversal strategy is fundamentally different — BSP iterates segs front-to-back, each claiming a range of screen columns, while DDA casts one ray per column.

The plan: add a BSP traversal target to abrash-raycast alongside DDA, then wire doom-rs as a consumer. This makes the BSP engine reusable for any BSP-based game.

## Architecture

### New Modules

- `abrash-raycast/src/bsp.rs` — BSP types, `BspMap` trait, `BspHit`
- `abrash-raycast/src/bsp_clip.rs` — Frustum clipping for BSP segs
- `abrash-raycast/src/bsp_visplane.rs` — Floor/ceiling visplane allocator and span extraction
- `abrash-render/src/raycaster/bsp.rs` — `render_bsp_view()` software column/span renderer
- `abrash-render/src/raycaster/bsp_lighting.rs` — Colormap-based distance shading

### Traits

```rust
// abrash-raycast/src/bsp.rs

/// A BSP-structured map that rays can be cast against.
pub trait BspMap {
    /// Traverse the BSP tree front-to-back from the camera position.
    /// Returns subsector indices in front-to-back order.
    fn subsectors_front_to_back(&self, pos: Vec2Fixed) -> Vec<usize>;

    /// Get the segs (wall segments) belonging to a subsector.
    fn subsector_segs(&self, ssector_idx: usize) -> &[BspSeg];

    /// Get the sector properties for a seg's front side.
    fn seg_front_sector(&self, seg: &BspSeg) -> &BspSector;

    /// Get the back sector for a two-sided seg (portals), if any.
    fn seg_back_sector(&self, seg: &BspSeg) -> Option<&BspSector>;
}

// abrash-render/src/raycaster/bsp.rs

/// Texture data provider for BSP rendering.
pub trait BspTextures {
    /// Get a single column of wall texture data (palette-indexed).
    fn wall_column(&self, texture_id: u16, col: usize) -> &[u8];

    /// Get a 64x64 flat texture (4096 bytes, palette-indexed).
    fn flat_data(&self, texture_id: u16) -> &[u8; 4096];

    /// Get a 256-byte colormap row for light-level shading.
    fn colormap(&self, index: u8) -> &[u8; 256];
}
```

### Data Types

```rust
// abrash-raycast/src/bsp.rs

/// A wall segment in BSP space.
pub struct BspSeg {
    pub v1: Vec2Fixed,
    pub v2: Vec2Fixed,
    pub offset: Fixed16_16,
    pub angle: Bam,
    pub front_sector: u16,
    pub back_sector: Option<u16>,
    pub upper_texture: u16,
    pub middle_texture: u16,
    pub lower_texture: u16,
    pub line_flags: u16,
}

/// Sector geometry and appearance.
pub struct BspSector {
    pub floor_height: i16,
    pub ceil_height: i16,
    pub light_level: u16,
    pub floor_texture: u16,
    pub ceil_texture: u16,
}

/// Result of a ray hitting a BSP wall segment.
pub struct BspHit {
    pub distance: Fixed16_16,
    pub seg_index: usize,
    pub texture_u: Fixed16_16,
    pub hit_point: Vec2Fixed,
}
```

### Rendering Entry Point

```rust
// abrash-render/src/raycaster/bsp.rs

pub fn render_bsp_view(
    fb: &mut Framebuffer,
    zbuf: &mut ZBuffer,
    map: &impl BspMap,
    textures: &impl BspTextures,
    camera_pos: Vec2Fixed,
    camera_angle: Bam,
    camera_z: Fixed16_16,
    fov: Bam,
) { ... }
```

### Rendering Loop

1. Traverse BSP front-to-back → ordered list of subsectors
2. For each subsector, iterate its segs
3. For each seg, project both endpoints to screen columns (angle → viewangletox)
4. For each column in the seg's span:
   - Compute wall scale (R_ScaleFromGlobalAngle)
   - Draw upper/middle/lower wall bands
   - Emit floor/ceiling visplane strips
   - Update clip arrays (open_top/open_bot)
5. After all segs: draw visplane spans (floor/ceiling)
6. Draw sprites sorted by depth against the z-buffer

## Migration Path

### Phase 1: Extract

Move doom-rs's rendering math into abrash as the BSP module. Mostly a code move, not a rewrite.

| doom-rs source | abrash destination |
|---|---|
| `render.rs` projection functions | `abrash-raycast/src/bsp.rs` |
| `visplane.rs` | `abrash-raycast/src/bsp_visplane.rs` |
| `clip.rs` frustum clipping | `abrash-raycast/src/bsp_clip.rs` |
| `lighting.rs` colormap logic | `abrash-render/src/raycaster/bsp_lighting.rs` |
| Column/span drawing | `abrash-render/src/raycaster/bsp.rs` |

doom-rs implements `BspMap` as a thin wrapper around `Level` and `BspTextures` as a wrapper around WAD texture/flat/colormap caches.

**Constraint:** No rendering behavior changes in this phase. Pure refactor.

### Phase 2: Verify

doom-rs switches from its own `render_level` to calling `render_bsp_view`. Frame-by-frame pixel comparison against trunk confirms zero diff.

**Constraint:** Identical output to trunk. No "improvements" mixed in.

### Phase 3: GPU Path

`abrash-gpu-render` gains a BSP mode that takes the same `BspMap` input but renders through wgpu:

- Convert sectors to triangle meshes (sector floor/ceiling polygons)
- Convert segs to textured quads (wall surfaces)
- Apply sector lighting as per-vertex or per-sector uniforms
- Use the existing deferred pipeline for lighting passes

This is where GZDoom-style visuals unlock: true-color, higher resolution, dynamic lights, smooth lighting gradients.

### Phase 4: GZDoom Features

With the GPU path working, extend the BSP types for modern Doom features:

- **UDMF map format** — Extend `BspSeg`/`BspSector` with optional slope/3D-floor fields
- **Dynamic lights** — Feed point/spot lights through abrash's existing `Light` system
- **True-color textures** — `BspTextures` returns RGBA instead of palette-indexed
- **Slopes** — `BspSector` gains floor/ceil slope vectors
- **3D floors** — Sector stacking with multiple floor/ceiling pairs
- **ZScript** — Runtime scripting (long-term, not part of rendering)

## Key Decisions

1. **Texture IDs are opaque u16 handles** — abrash-raycast doesn't know about WAD lumps. The consumer maps IDs to actual texture data.

2. **BSP traversal lives in abrash-raycast** — Not in doom-rs. This makes it reusable for Build engine games, Quake BSP, etc.

3. **Phase 2 must be pixel-identical to trunk** — We learned the hard way that mixing "improvements" with refactoring causes regressions.

4. **Software and GPU renderers share the same BspMap trait** — Switching backends is a one-line change in doom-rs.
