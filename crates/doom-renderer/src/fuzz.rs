//! Doom fuzz effect — partial invisibility column drawer.
//!
//! When a thing has the partial invisibility powerup (spectres, invisible
//! players), its sprite is NOT drawn normally.  Instead, for each pixel where
//! the sprite would have drawn, the renderer reads the *existing* framebuffer
//! pixel at a vertically-offset position, darkens it through colormap index 6,
//! and writes the result back.  This creates the signature shimmering
//! distortion effect.
//!
//! The vertical offset comes from `FUZZ_TABLE` — a fixed 50-entry pattern of
//! +1 / -1 values multiplied by screen width (320).  The pattern index advances
//! for every pixel drawn and wraps at 50.

use crate::colormap::ColormapCache;
use crate::framebuffer::Framebuffer;
use doom_types::{FB_HEIGHT, FB_WIDTH};

/// The classic Doom fuzz offset table — 50 entries of +1 or -1.
///
/// Each value is multiplied by `FB_WIDTH` (320) to convert a row offset into
/// a linear pixel offset in the framebuffer array.
pub const FUZZ_TABLE: [i32; 50] = [
    1, -1, 1, -1, 1, 1, -1, 1, 1, -1, 1, 1, 1, -1, 1, 1, 1, -1, -1, -1, -1, 1, -1, -1, 1, 1, 1, 1,
    -1, 1, -1, 1, 1, -1, -1, 1, 1, -1, -1, -1, -1, 1, 1, 1, 1, -1, 1, 1, -1, 1,
];

/// Colormap row index used by the fuzz effect for darkening.
///
/// Row 6 in the COLORMAP lump is a moderately-dark tint, matching
/// vanilla Doom's `FUZZDARK` constant.
const FUZZ_DARK_COLORMAP: u8 = 6;

/// Draw a fuzz (partial invisibility) column onto the framebuffer.
///
/// For each row `y` in `[y_top, y_bot]` (inclusive):
/// 1. Look up the fuzz offset from `FUZZ_TABLE[*fuzz_pos % 50]`.
/// 2. Compute a source pixel position: `(y + offset) * FB_WIDTH + x`, where
///    `offset` is the fuzz table entry (±1 row).  The source row is clamped
///    to `[0, FB_HEIGHT - 1]`.
/// 3. Read the existing palette index at that source position.
/// 4. Run it through colormap row 6 (dark) to darken it.
/// 5. Write the darkened pixel back to `fb.data[y * FB_WIDTH + x]`.
/// 6. Advance `*fuzz_pos`.
///
/// # Arguments
/// - `fb`        — the framebuffer to read from and write to.
/// - `x`         — the screen column (0..319).
/// - `y_top`     — top of the fuzz column (inclusive, clamped internally).
/// - `y_bot`     — bottom of the fuzz column (inclusive, clamped internally).
/// - `fuzz_pos`  — mutable cursor into `FUZZ_TABLE`; wraps automatically.
/// - `colormap`  — optional colormap cache for looking up darkening map.
///   When `None`, a simple fallback darkening is applied.
pub fn draw_fuzz_column(
    fb: &mut Framebuffer,
    x: usize,
    y_top: usize,
    y_bot: usize,
    fuzz_pos: &mut usize,
    colormap: Option<&ColormapCache>,
) {
    if x >= FB_WIDTH || y_top > y_bot {
        return;
    }

    let y_top = y_top.max(0);
    let y_bot = y_bot.min(FB_HEIGHT - 1);

    for y in y_top..=y_bot {
        let offset = FUZZ_TABLE[*fuzz_pos % FUZZ_TABLE.len()];

        // Compute the source row, clamping to valid framebuffer rows.
        let source_y = (y as i32 + offset).clamp(0, (FB_HEIGHT - 1) as i32) as usize;
        let source_idx = source_y * FB_WIDTH + x;

        // Read existing pixel from the offset position.
        let raw_pixel = fb.data[source_idx];

        // Darken through colormap row 6.
        let darkened = match colormap {
            Some(cm) => cm.get(FUZZ_DARK_COLORMAP)[raw_pixel as usize],
            None => fallback_darken(raw_pixel),
        };

        // Write back to the actual pixel position.
        fb.data[y * FB_WIDTH + x] = darkened;

        // Advance fuzz position and wrap.
        *fuzz_pos = (*fuzz_pos + 1) % FUZZ_TABLE.len();
    }
}

/// Simple fallback darkening when no colormap is available.
///
/// Shifts the palette index towards 0 (darker end of the Doom palette).
/// This is a rough approximation — the real COLORMAP-based darkening is
/// much more accurate but requires a loaded WAD.
#[inline]
fn fallback_darken(index: u8) -> u8 {
    // Halve the index as a very crude brightness reduction.
    index / 2
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzz_table_has_50_entries() {
        assert_eq!(FUZZ_TABLE.len(), 50);
    }

    #[test]
    fn fuzz_table_entries_are_plus_or_minus_one() {
        for (i, &val) in FUZZ_TABLE.iter().enumerate() {
            assert!(
                val == 1 || val == -1,
                "FUZZ_TABLE[{i}] = {val}, expected +1 or -1"
            );
        }
    }

    #[test]
    fn draw_fuzz_column_modifies_framebuffer() {
        let mut fb = Framebuffer::new();
        // Fill a region with a known value so fuzz has something to darken.
        fb.fill_rect(100, 50, 20, 20, 128);

        let original = fb.get_pixel(110, 60).unwrap();
        assert_eq!(original, 128);

        let mut fuzz_pos = 0;
        draw_fuzz_column(&mut fb, 110, 55, 65, &mut fuzz_pos, None);

        // At least one pixel in the column should have changed.
        let mut any_changed = false;
        for y in 55..=65 {
            if fb.get_pixel(110, y).unwrap() != 128 {
                any_changed = true;
                break;
            }
        }
        assert!(any_changed, "fuzz column should modify at least one pixel");
    }

    #[test]
    fn draw_fuzz_column_does_not_draw_sprite_pixels() {
        // Fuzz reads existing fb pixels, not sprite texture.
        // Draw a single-pixel fuzz column. The surrounding fb is all 200.
        // Fuzz should read from an offset position (also 200) and darken it.
        let mut fb = Framebuffer::new();
        fb.clear(200);

        // Draw just one pixel so we don't hit cascading darkening.
        let mut fuzz_pos = 0;
        draw_fuzz_column(&mut fb, 50, 10, 10, &mut fuzz_pos, None);

        let pixel = fb.get_pixel(50, 10).unwrap();
        // Fuzz reads from offset row (FUZZ_TABLE[0] = +1 → row 11, value 200).
        // fallback_darken(200) = 100.
        assert_eq!(
            pixel, 100,
            "fuzz should darken existing fb pixel, not inject sprite data"
        );

        // The pixel at the offset position (row 11) should be unchanged since
        // we only drew fuzz at row 10.
        assert_eq!(
            fb.get_pixel(50, 11).unwrap(),
            200,
            "pixels not in the fuzz column should be unmodified"
        );
    }

    #[test]
    fn draw_fuzz_column_advances_fuzz_pos() {
        let mut fb = Framebuffer::new();
        fb.clear(100);

        let mut fuzz_pos = 0;
        draw_fuzz_column(&mut fb, 10, 0, 9, &mut fuzz_pos, None);

        // We drew 10 pixels (rows 0..=9), so fuzz_pos should have advanced by 10.
        assert_eq!(fuzz_pos, 10);
    }

    #[test]
    fn draw_fuzz_column_wraps_fuzz_pos_at_table_length() {
        let mut fb = Framebuffer::new();
        fb.clear(100);

        let mut fuzz_pos = 45; // Start near the end of the table.
        draw_fuzz_column(&mut fb, 10, 0, 9, &mut fuzz_pos, None);

        // 45 + 10 = 55 → wraps: 55 % 50 = 5.
        assert_eq!(fuzz_pos, 5, "fuzz_pos should wrap around at 50");
    }

    #[test]
    fn draw_fuzz_column_clamps_offset_to_screen_bounds() {
        let mut fb = Framebuffer::new();
        fb.clear(150);

        // Draw fuzz at the very top row (y=0).
        // FUZZ_TABLE[0] = 1 (reads from row 1), FUZZ_TABLE[1] = -1 but we
        // only draw 1 pixel. The offset -1 from row 0 should clamp to row 0.
        let mut fuzz_pos = 1; // Start at entry -1 so offset is -1.
        draw_fuzz_column(&mut fb, 10, 0, 0, &mut fuzz_pos, None);
        // Should not panic (the clamp prevents negative index).
        let pixel = fb.get_pixel(10, 0).unwrap();
        assert_eq!(pixel, 75, "fallback_darken(150) = 75");

        // Draw fuzz at the very bottom row (y=199).
        let mut fuzz_pos = 0; // Entry +1 from row 199 should clamp to row 199.
        draw_fuzz_column(&mut fb, 10, 199, 199, &mut fuzz_pos, None);
        // Should not panic.
        let pixel = fb.get_pixel(10, 199).unwrap();
        // The pixel was already modified at row 0 for x=10, but row 199 was 150.
        // fallback_darken(150) = 75.
        assert_eq!(pixel, 75, "fallback_darken(150) = 75");
    }

    #[test]
    fn fuzz_column_produces_darker_pixels() {
        let mut fb = Framebuffer::new();
        // Fill with a bright value.
        fb.clear(200);

        let mut fuzz_pos = 0;
        draw_fuzz_column(&mut fb, 100, 50, 60, &mut fuzz_pos, None);

        for y in 50..=60 {
            let pixel = fb.get_pixel(100, y).unwrap();
            assert!(
                pixel < 200,
                "fuzz pixel at y={y} should be darker than original (got {pixel})"
            );
        }
    }

    #[test]
    fn draw_fuzz_column_with_colormap() {
        // Build a ColormapCache where row 6 maps everything to index 42.
        let mut cm_data = vec![0u8; 34 * 256];
        // Fill row 6 with a distinctive mapping.
        let row6_start = 6 * 256;
        for i in 0..256 {
            cm_data[row6_start + i] = 42;
        }
        let cm = ColormapCache::from_test_data(cm_data);

        let mut fb = Framebuffer::new();
        fb.clear(100);

        let mut fuzz_pos = 0;
        draw_fuzz_column(&mut fb, 50, 10, 15, &mut fuzz_pos, Some(&cm));

        for y in 10..=15 {
            let pixel = fb.get_pixel(50, y).unwrap();
            assert_eq!(pixel, 42, "with colormap, fuzz should map to 42 (y={y})");
        }
    }

    #[test]
    fn draw_fuzz_column_noop_for_invalid_x() {
        let mut fb = Framebuffer::new();
        fb.clear(100);

        let mut fuzz_pos = 0;
        draw_fuzz_column(&mut fb, 320, 10, 20, &mut fuzz_pos, None);

        // fuzz_pos should not advance.
        assert_eq!(fuzz_pos, 0);
        // fb should be unchanged.
        assert!(fb.data.iter().all(|&b| b == 100));
    }

    #[test]
    fn draw_fuzz_column_noop_for_inverted_range() {
        let mut fb = Framebuffer::new();
        fb.clear(100);

        let mut fuzz_pos = 0;
        draw_fuzz_column(&mut fb, 50, 20, 10, &mut fuzz_pos, None);

        // Inverted range should be a no-op.
        assert_eq!(fuzz_pos, 0);
        assert!(fb.data.iter().all(|&b| b == 100));
    }
}
