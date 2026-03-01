//! 2D overhead automap renderer.
//!
//! Draws all linedefs from a `Level` scaled to fit the 320×200 framebuffer,
//! colour-coded by linedef type, with a small player-position marker.
//!
//! # Coordinate system
//! Doom Y increases upward; screen Y increases downward.  The world→screen
//! transform flips Y so north remains up on the automap.

use doom_map::{Level, Linedef};
use doom_types::Bam;

use crate::framebuffer::Framebuffer;
use crate::palette::PaletteLut;

// ---------------------------------------------------------------------------
// Screen constants (mirrors FB_WIDTH / FB_HEIGHT)
// ---------------------------------------------------------------------------

const SCREEN_W: i32 = 320;
const SCREEN_H: i32 = 200;

// Palette indices matching Doom's automap colours.
const COLOR_ONE_SIDED: u8  = 176; // red/orange — solid wall
const COLOR_TWO_SIDED: u8  =  96; // gray-brown — passable line
const COLOR_SPECIAL:   u8  = 231; // yellow — two-sided with special
const COLOR_PLAYER:    u8  = 255; // white — player marker
const COLOR_BACKGROUND: u8 =   0; // black

// Padding fraction applied to each side of the computed map bounds.
const PADDING_FRAC: f32 = 0.05;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Render a 2D overhead automap of `level` into `fb`.
///
/// * `player_x`, `player_y` — player position in Doom map units.
/// * `player_angle` — player facing angle (used for future arrow rendering;
///   currently unused but kept in the signature for API stability).
/// * `_palette` — unused for now (automap uses fixed palette indices).
pub fn draw_automap(
    level: &Level,
    player_x: i32,
    player_y: i32,
    _player_angle: Bam,
    fb: &mut Framebuffer,
    _palette: &PaletteLut,
) {
    // 1. Fill background with black.
    fb.clear(COLOR_BACKGROUND);

    // 2. Compute map bounds from vertexes.
    let Some((min_x, max_x, min_y, max_y)) = map_bounds(level) else {
        // No vertexes — nothing to draw.
        return;
    };

    // 3. Compute scale with 5 % padding on each side.
    let map_w = (max_x - min_x) as f32;
    let map_h = (max_y - min_y) as f32;

    if map_w < 1.0 || map_h < 1.0 {
        return;
    }

    let padded_w = map_w * (1.0 + 2.0 * PADDING_FRAC);
    let padded_h = map_h * (1.0 + 2.0 * PADDING_FRAC);
    let pad_x = map_w * PADDING_FRAC;
    let pad_y = map_h * PADDING_FRAC;

    let scale = (SCREEN_W as f32 / padded_w).min(SCREEN_H as f32 / padded_h);

    // Effective map origin after padding.
    let origin_x = min_x as f32 - pad_x;
    let origin_y = min_y as f32 - pad_y;

    // Helper: world → screen coordinates.
    let world_to_screen = |wx: i16, wy: i16| -> (i32, i32) {
        let sx = ((wx as f32 - origin_x) * scale) as i32;
        // Flip Y: world up → screen up (screen origin is top-left).
        let sy = (SCREEN_H - 1) - ((wy as f32 - origin_y) * scale) as i32;
        (sx, sy)
    };

    // 4. Draw all linedefs.
    for ld in &level.linedefs {
        let color = linedef_color(ld);

        // Guard against out-of-bounds vertex references (validated at load
        // time, but we use get() to stay panic-free here).
        let Some(v1) = level.vertexes.get(ld.from_vertex as usize) else {
            continue;
        };
        let Some(v2) = level.vertexes.get(ld.to_vertex as usize) else {
            continue;
        };

        let (x0, y0) = world_to_screen(v1.x, v1.y);
        let (x1, y1) = world_to_screen(v2.x, v2.y);

        draw_line(fb, x0, y0, x1, y1, color);
    }

    // 5. Draw player marker (3×3 cross).
    let (px, py) = world_to_screen(player_x as i16, player_y as i16);

    // Horizontal arm of the cross.
    for dx in -1_i32..=1 {
        let sx = (px + dx).clamp(0, SCREEN_W - 1) as usize;
        let sy = py.clamp(0, SCREEN_H - 1) as usize;
        fb.set_pixel(sx, sy, COLOR_PLAYER);
    }
    // Vertical arm of the cross.
    for dy in -1_i32..=1 {
        let sx = px.clamp(0, SCREEN_W - 1) as usize;
        let sy = (py + dy).clamp(0, SCREEN_H - 1) as usize;
        fb.set_pixel(sx, sy, COLOR_PLAYER);
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Compute min/max world-coordinate bounds of all vertexes.
///
/// Returns `None` if the level has no vertexes.
fn map_bounds(level: &Level) -> Option<(i32, i32, i32, i32)> {
    let mut iter = level.vertexes.iter();
    let first = iter.next()?;

    let mut min_x = i32::from(first.x);
    let mut max_x = i32::from(first.x);
    let mut min_y = i32::from(first.y);
    let mut max_y = i32::from(first.y);

    for v in iter {
        let vx = i32::from(v.x);
        let vy = i32::from(v.y);
        if vx < min_x { min_x = vx; }
        if vx > max_x { max_x = vx; }
        if vy < min_y { min_y = vy; }
        if vy > max_y { max_y = vy; }
    }

    Some((min_x, max_x, min_y, max_y))
}

/// Choose an automap colour for a linedef.
#[inline]
fn linedef_color(ld: &Linedef) -> u8 {
    if ld.is_two_sided() {
        if ld.special != 0 {
            COLOR_SPECIAL
        } else {
            COLOR_TWO_SIDED
        }
    } else {
        COLOR_ONE_SIDED
    }
}

/// Bresenham line rasteriser.
///
/// Pixels outside `[0, 319] × [0, 199]` are silently skipped.
fn draw_line(fb: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };

    let mut x = x0;
    let mut y = y0;
    let mut err = dx - dy;

    loop {
        // Only plot pixels within screen bounds.
        if x >= 0 && x < SCREEN_W && y >= 0 && y < SCREEN_H {
            fb.set_pixel(x as usize, y as usize, color);
        }

        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_map::lumps::{Linedef as LdRaw, Vertex as VxRaw, Ssector, Sector, Reject, Blockmap};
    use doom_map::Level;

    // -----------------------------------------------------------------------
    // Minimal Level builder
    // -----------------------------------------------------------------------

    /// Build a `Level` with a single 1-cell grid-sized blockmap, no sectors,
    /// no nodes, no segs, but with caller-supplied vertexes and linedefs.
    ///
    /// This mirrors how `doom-game/src/movement.rs` constructs levels for
    /// tests without needing a real WAD file.
    fn make_level(vertexes: Vec<VxRaw>, linedefs: Vec<LdRaw>) -> Level {
        // One placeholder sector (needed for reject table size).
        let sector = Sector {
            floor_height: 0,
            ceil_height:  128,
            floor_flat:   *b"FLAT1\0\0\0",
            ceil_flat:    *b"FLAT2\0\0\0",
            light_level:  192,
            special:      0,
            tag:          0,
        };

        // One ssector that references zero segs (seg_count=0, first_seg=0).
        // N_SSECTORS(1) == N_NODES(0) + 1 — the BSP invariant holds.
        let ssector = Ssector { seg_count: 0, first_seg: 0 };

        // Minimal blockmap: 1×1 cell grid rooted at (0,0).
        let mut bm_bytes = vec![0u8; 8 + 2 + 4];
        bm_bytes[0..2].copy_from_slice(&0i16.to_le_bytes()); // x_origin
        bm_bytes[2..4].copy_from_slice(&0i16.to_le_bytes()); // y_origin
        bm_bytes[4..6].copy_from_slice(&1u16.to_le_bytes()); // x_count
        bm_bytes[6..8].copy_from_slice(&1u16.to_le_bytes()); // y_count
        bm_bytes[8..10].copy_from_slice(&5u16.to_le_bytes());      // offset words
        bm_bytes[10..12].copy_from_slice(&0u16.to_le_bytes());     // sentinel
        bm_bytes[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes()); // terminator
        let blockmap = Blockmap::parse_lump(&bm_bytes).expect("blockmap parse");

        // Reject: 1 sector → ceil(1/8) = 1 byte.
        let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject parse");

        Level {
            name:     "TEST".to_owned(),
            things:   vec![],
            linedefs,
            sidedefs: vec![],
            vertexes,
            segs:     vec![],
            ssectors: vec![ssector],
            nodes:    vec![],
            sectors:  vec![sector],
            reject,
            blockmap,
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: empty level must not panic
    // -----------------------------------------------------------------------

    #[test]
    fn draw_automap_empty_level_does_not_panic() {
        let level = make_level(vec![], vec![]);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        draw_automap(&level, 0, 0, Bam(0), &mut fb, &palette);
        // If we reach here, no panic occurred.
        // Background must be all black since there are no vertexes.
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    // -----------------------------------------------------------------------
    // Test 2: single linedef should produce at least one non-black pixel
    // -----------------------------------------------------------------------

    #[test]
    fn draw_automap_single_linedef_marks_pixels() {
        let vertexes = vec![
            VxRaw { x:   0, y:   0 },
            VxRaw { x: 100, y: 100 },
        ];
        let linedefs = vec![LdRaw {
            from_vertex:   0,
            to_vertex:     1,
            flags:         0,   // one-sided
            special:       0,
            tag:           0,
            right_sidedef: 0xFFFF,
            left_sidedef:  0xFFFF,
        }];
        let level = make_level(vertexes, linedefs);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        draw_automap(&level, 50, 50, Bam(0), &mut fb, &palette);

        // At least one pixel should be non-black.
        let has_non_black = fb.data.iter().any(|&b| b != 0);
        assert!(has_non_black, "expected at least one non-black pixel after drawing a linedef");
    }

    // -----------------------------------------------------------------------
    // Test 3: draw_line horizontal — pixel on the line must be set
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_horizontal() {
        let mut fb = Framebuffer::new();
        draw_line(&mut fb, 10, 50, 50, 50, 176);

        // Every pixel from x=10 to x=50 at y=50 should be set.
        for x in 10..=50 {
            assert_eq!(
                fb.get_pixel(x, 50),
                Some(176),
                "pixel ({x}, 50) should be 176"
            );
        }
        // Pixel just before the line should be untouched.
        assert_eq!(fb.get_pixel(9, 50), Some(0));
    }

    // -----------------------------------------------------------------------
    // Test 4: draw_line with out-of-bounds coords must not panic
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_clamps_out_of_bounds() {
        let mut fb = Framebuffer::new();
        // Coordinates far outside [0,319]×[0,199] — should be a no-op / no panic.
        draw_line(&mut fb, -5000, -5000, 5000, 5000, 255);
        // At minimum the test must not panic; the framebuffer may or may not
        // have pixels set depending on clipping, so we just verify no panic.
    }
}
