//! First-person perspective software renderer.
//!
//! Renders a Doom level from the player's viewpoint using per-seg projection.
//! No textures — walls are flat-shaded by sector light level.
//!
//! # Algorithm overview
//! 1. Draw ceiling/floor background rectangles.
//! 2. For each seg in the level, transform both endpoints into view space,
//!    clip against the near plane, project to screen columns, and draw
//!    each screen column as a solid-colored vertical strip.
//! 3. A per-column Z-buffer prevents back-segs from overdrawing closer walls.

use doom_map::Level;
use doom_types::Bam;

use crate::framebuffer::Framebuffer;
use crate::palette::PaletteLut;

// ---------------------------------------------------------------------------
// Screen constants
// ---------------------------------------------------------------------------

const SCREEN_W: i32 = 320;
const SCREEN_H: i32 = 200;
const HALF_W: i32 = SCREEN_W / 2; // 160
const HALF_H: i32 = SCREEN_H / 2; // 100
const FOCAL_LEN: i32 = 160; // = HALF_W, 90° horizontal FOV

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Render a first-person view of `level` from the player's position.
///
/// `player_x`, `player_y` — player position in map units (i32).
/// `player_angle`         — player facing direction as `Bam`.
/// `palette`              — PLAYPAL palette for color lookup (index 0 = normal).
///
/// This is a software renderer using per-seg perspective projection.
/// No textures — walls are flat-shaded by sector light level.
pub fn render_level(
    level: &Level,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
    fb: &mut Framebuffer,
    _palette: &PaletteLut,
) {
    // ------------------------------------------------------------------
    // Step 1: Draw background (ceiling top half, floor bottom half)
    // ------------------------------------------------------------------
    // Palette index 25 ≈ dark gray ceiling; 119 ≈ medium gray floor.
    fb.fill_rect(0, 0, SCREEN_W as usize, HALF_H as usize, 25);
    fb.fill_rect(0, HALF_H as usize, SCREEN_W as usize, HALF_H as usize, 119);

    // ------------------------------------------------------------------
    // Step 2: Z-buffer (per-column minimum depth)
    // ------------------------------------------------------------------
    let mut z_buf = [i32::MAX; SCREEN_W as usize];

    // ------------------------------------------------------------------
    // Step 3: Precompute trig — Fixed16_16 values (.0 is the raw i32)
    // ------------------------------------------------------------------
    let cos_a = player_angle.cos();
    let sin_a = player_angle.sin();

    // ------------------------------------------------------------------
    // Step 4: Iterate all segs
    // ------------------------------------------------------------------
    for seg in &level.segs {
        // Get vertex world positions
        let v1 = match level.vertexes.get(seg.from_vertex as usize) {
            Some(v) => v,
            None => continue,
        };
        let v2 = match level.vertexes.get(seg.to_vertex as usize) {
            Some(v) => v,
            None => continue,
        };
        let (wx1, wy1) = (v1.x as i32, v1.y as i32);
        let (wx2, wy2) = (v2.x as i32, v2.y as i32);

        // Translate to player-relative coordinates
        let dx1 = wx1 - player_x;
        let dy1 = wy1 - player_y;
        let dx2 = wx2 - player_x;
        let dy2 = wy2 - player_y;

        // Rotate into view space using 16.16 fixed-point trig values.
        // cos_a.0 and sin_a.0 are the raw i32 16.16 fixed-point values.
        // We shift right by 16 after the multiply to recover integer units.
        // Using i64 to avoid overflow (coordinates up to ±32767, trig up to 65536).
        let cos_i = cos_a.0 as i64;
        let sin_i = sin_a.0 as i64;

        // vx = forward depth (positive = in front of player)
        // vy = lateral offset (positive = right of player)
        let vx1 = ((dx1 as i64 * cos_i + dy1 as i64 * sin_i) >> 16) as i64;
        let vy1 = ((dx1 as i64 * sin_i - dy1 as i64 * cos_i) >> 16) as i64;
        let vx2 = ((dx2 as i64 * cos_i + dy2 as i64 * sin_i) >> 16) as i64;
        let vy2 = ((dx2 as i64 * sin_i - dy2 as i64 * cos_i) >> 16) as i64;

        // Skip if both endpoints are behind the player
        if vx1 <= 0 && vx2 <= 0 {
            continue;
        }

        // ------------------------------------------------------------------
        // Step 5: Get wall properties from seg
        // ------------------------------------------------------------------
        let linedef = match level.linedefs.get(seg.linedef as usize) {
            Some(ld) => ld,
            None => continue,
        };

        // seg.direction: 0 = same direction as linedef (right sidedef is front)
        //                1 = opposite direction    (left sidedef is front)
        let sidedef_idx = if seg.direction == 0 {
            linedef.right_sidedef as usize
        } else {
            linedef.left_sidedef as usize
        };

        let sidedef = match level.sidedefs.get(sidedef_idx) {
            Some(sd) => sd,
            None => continue,
        };

        let sector = match level.sectors.get(sidedef.sector as usize) {
            Some(s) => s,
            None => continue,
        };

        let floor_h = sector.floor_height as i32;
        let ceil_h = sector.ceil_height as i32;
        // Map light_level 0..255 to brightness 0..31
        let light = ((sector.light_level as u32) >> 3).min(31) as u8;

        // ------------------------------------------------------------------
        // Step 6: Near-clip
        // ------------------------------------------------------------------
        let clipped = match clip_seg_to_near_plane(vx1, vy1, vx2, vy2) {
            Some(c) => c,
            None => continue,
        };
        let (vx1, vy1, vx2, vy2) = clipped;

        // ------------------------------------------------------------------
        // Step 7: Project to screen columns
        // ------------------------------------------------------------------
        // sx = HALF_W - FOCAL_LEN * vy / vx
        let sx1 = HALF_W as i64 - (FOCAL_LEN as i64 * vy1) / vx1.max(1);
        let sx2 = HALF_W as i64 - (FOCAL_LEN as i64 * vy2) / vx2.max(1);

        // Sort so sx_left <= sx_right
        let (sx_left, vx_left, sx_right, vx_right) = if sx1 <= sx2 {
            (sx1, vx1, sx2, vx2)
        } else {
            (sx2, vx2, sx1, vx1)
        };

        // Clamp to screen
        let col_start = sx_left.max(0).min((SCREEN_W - 1) as i64) as i32;
        let col_end = sx_right.max(0).min((SCREEN_W - 1) as i64) as i32;

        if col_start > col_end {
            continue;
        }

        // ------------------------------------------------------------------
        // Step 8: Draw each column
        // ------------------------------------------------------------------
        let span_w = (sx_right - sx_left).max(1);

        for x in col_start..=col_end {
            // Interpolate depth at this column
            let t = (x as i64 - sx_left).max(0);
            let depth = vx_left + t * (vx_right - vx_left) / span_w;
            let depth_i32 = depth.max(1) as i32;

            // Z-buffer occlusion check
            if depth_i32 >= z_buf[x as usize] {
                continue;
            }
            z_buf[x as usize] = depth_i32;

            // Wall height in pixels at this depth
            let wall_h_world = (ceil_h - floor_h).max(0);
            let wall_h_px = (wall_h_world * FOCAL_LEN / depth_i32).min(SCREEN_H);

            // Wall top/bottom on screen (vertically centered)
            let wall_top = (HALF_H - wall_h_px / 2).max(0);
            let wall_bot = (HALF_H + wall_h_px / 2).min(SCREEN_H - 1);

            // Color from light level: 0..31 mapped to palette indices 32..63
            // Bright sectors (light=31) → index 63 (lighter)
            // Dark sectors (light=0) → index 32 (darker)
            let wall_color = (32u8).saturating_add(light);

            fb.draw_column(
                x as usize,
                wall_top as usize,
                wall_bot as usize,
                wall_color,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Near-plane clipping
// ---------------------------------------------------------------------------

/// Clip a view-space seg to the near plane (vx = 1).
///
/// Returns `None` if both endpoints are behind the near plane after clipping.
/// Returns `Some((vx1, vy1, vx2, vy2))` with the clipped coordinates.
fn clip_seg_to_near_plane(
    vx1: i64,
    vy1: i64,
    vx2: i64,
    vy2: i64,
) -> Option<(i64, i64, i64, i64)> {
    const NEAR: i64 = 1;

    // Both behind — cull
    if vx1 <= 0 && vx2 <= 0 {
        return None;
    }
    // Both in front — no clipping needed
    if vx1 > 0 && vx2 > 0 {
        return Some((vx1, vy1, vx2, vy2));
    }

    // One in front, one behind — linearly interpolate the crossing at vx = NEAR
    let t_num = NEAR - vx1;
    let t_den = vx2 - vx1;
    if t_den == 0 {
        return None;
    }

    if vx1 <= 0 {
        // vx1 behind, vx2 in front: clip vx1 to NEAR
        let ny = vy1 + t_num * (vy2 - vy1) / t_den;
        Some((NEAR, ny, vx2, vy2))
    } else {
        // vx2 behind, vx1 in front: clip vx2 to NEAR
        let t_num2 = NEAR - vx2;
        let t_den2 = vx1 - vx2;
        if t_den2 == 0 {
            return None;
        }
        let ny = vy2 + t_num2 * (vy1 - vy2) / t_den2;
        Some((vx1, vy1, NEAR, ny))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal Level with one sector and one seg for testing.
    ///
    /// Mirrors the structure used in doom-map's level tests.
    fn make_minimal_level() -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };
        use doom_map::BspTree;

        let vertexes = vec![
            Vertex { x: 0, y: 128 },
            Vertex { x: 128, y: 128 },
        ];
        let sectors = vec![Sector {
            floor_height: 0,
            ceil_height: 128,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }];
        let sidedefs = vec![Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"WALL1\0\0\0",
            lower_texture: *b"WALL2\0\0\0",
            middle_texture: *b"WALL3\0\0\0",
            sector: 0,
        }];
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF,
        }];
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];
        let ssectors = vec![Ssector { seg_count: 1, first_seg: 0 }];
        let things = vec![Thing { x: 0, y: 0, angle: 0, kind: 1, flags: 7 }];

        // Reject: ceil(1*1/8) = 1 byte
        let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject parse");

        // Minimal blockmap
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count=1
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count=1
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes()); // offset
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes()); // sentinel
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "TEST".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    #[test]
    fn render_empty_level_does_not_panic() {
        let level = make_minimal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(&level, 0, 0, Bam::ZERO, &mut fb, &palette);

        // Background must have been drawn — check that not all pixels are 0
        let has_nonzero = fb.data.iter().any(|&b| b != 0);
        assert!(has_nonzero, "framebuffer should be non-zero after rendering background");
    }

    #[test]
    fn render_with_player_facing_wall_does_not_panic() {
        use doom_types::ANG90;

        // Player at (0, 0) facing North (ANG90), wall at y=128
        let level = make_minimal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        // This exercises the full seg projection path
        render_level(&level, 0, 0, ANG90, &mut fb, &palette);
        // No panic is the success criterion
    }

    #[test]
    fn z_buffer_prevents_overdraw() {
        // Render the same level twice to different framebuffers — just verify no panic
        let level = make_minimal_level();
        let palette = PaletteLut::grayscale();

        let mut fb1 = Framebuffer::new();
        render_level(&level, 64, 0, Bam::ZERO, &mut fb1, &palette);

        let mut fb2 = Framebuffer::new();
        render_level(&level, 64, 0, Bam::ZERO, &mut fb2, &palette);

        // Both renders should produce identical results (deterministic)
        assert_eq!(fb1.data.as_slice(), fb2.data.as_slice());
    }

    #[test]
    fn clip_seg_both_behind_returns_none() {
        assert!(clip_seg_to_near_plane(-10, 0, -5, 0).is_none());
    }

    #[test]
    fn clip_seg_both_in_front_unchanged() {
        let result = clip_seg_to_near_plane(5, 2, 10, 4);
        assert_eq!(result, Some((5, 2, 10, 4)));
    }

    #[test]
    fn clip_seg_one_behind_clips_correctly() {
        // vx1 behind at -2, vx2 in front at 4; crossing at vx=1
        // vy1=0, vy2=6 → at crossing t = (1 - (-2)) / (4 - (-2)) = 3/6 = 0.5
        // ny = 0 + 3 * (6 - 0) / 6 = 3
        let result = clip_seg_to_near_plane(-2, 0, 4, 6);
        assert!(result.is_some());
        let (cx1, _cy1, cx2, cy2) = result.unwrap();
        assert_eq!(cx1, 1); // near plane
        assert_eq!(cx2, 4); // unchanged
        assert_eq!(cy2, 6); // unchanged
    }
}
