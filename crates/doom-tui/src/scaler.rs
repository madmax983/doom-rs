//! Scaling algorithms for the 320×200 Doom framebuffer → terminal cell grid.
//!
//! Provides both nearest-neighbor (fast, chunky) and bilinear (smooth)
//! scaling modes.  All arithmetic is integer-only — no f32/f64.
//!
//! # Nearest-neighbor helpers
//! Shared math for mapping terminal cell coordinates to framebuffer
//! pixel coordinates — identical to the abrash port and confirmed by
//! the widget implementation.

use doom_renderer::PaletteLut;

/// Scaling algorithm for the 320×200 Doom framebuffer → terminal cell grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScalingMode {
    /// Nearest-neighbor: each terminal cell maps to the closest source pixel.
    /// Fast, pixel-perfect, "chunky" appearance.
    #[default]
    Nearest,
    /// Bilinear: each terminal cell samples 4 source pixels with weighted average.
    /// Smoother appearance at the cost of a small extra computation per cell.
    Bilinear,
}

/// Map terminal column `cx` to framebuffer X pixel, given `term_w` and `fb_w`.
#[inline]
#[allow(dead_code)]
pub fn cell_to_fb_x(cx: usize, term_w: usize, fb_w: usize) -> usize {
    (cx * fb_w) / term_w
}

/// Map terminal row `cy` (top sub-pixel) to framebuffer Y, given `term_h` and `fb_h`.
#[inline]
#[allow(dead_code)]
pub fn cell_to_fb_y_top(cy: usize, term_h: usize, fb_h: usize) -> usize {
    (cy * 2 * fb_h) / (term_h * 2)
}

/// Map terminal row `cy` (bottom sub-pixel) to framebuffer Y.
#[inline]
#[allow(dead_code)]
pub fn cell_to_fb_y_bot(cy: usize, term_h: usize, fb_h: usize) -> usize {
    ((cy * 2 + 1) * fb_h) / (term_h * 2)
}

// ── Bilinear sampling ──────────────────────────────────────────────────────

/// Sample the 320×200 palette-indexed framebuffer at a sub-pixel position,
/// returning an interpolated `(r, g, b)` triple using the given palette.
///
/// `fx`, `fy` are fixed-point coordinates in `[0, (320<<16)) × [0, (200<<16))`.
/// The upper bits give the integer pixel index; the lower 16 bits are the
/// fractional part used for blending.
///
/// # Bounds safety
/// All array accesses are clamped to `[0, 319] × [0, 199]` before indexing,
/// so this function never panics regardless of input values.
pub fn sample_bilinear(
    fb_data: &[u8], // 320*200 palette indices
    palette: &PaletteLut,
    palette_idx: usize, // active palette (0 = normal)
    fx: u32,            // fixed-point x: integer part = fx >> 16, frac = fx & 0xFFFF
    fy: u32,            // fixed-point y
) -> (u8, u8, u8) {
    const W: u32 = 320;
    const H: u32 = 200;

    let x0 = (fx >> 16).min(W - 1) as usize;
    let y0 = (fy >> 16).min(H - 1) as usize;
    let x1 = (x0 + 1).min((W - 1) as usize);
    let y1 = (y0 + 1).min((H - 1) as usize);

    // Fractional parts scaled to 0..256.
    // We use 8 bits of the 16-bit fractional part, giving a range of 0..=255.
    // Weight 0 means "all from the left/top sample"; 255 means "almost all from
    // the right/bottom sample".  The denominator in bilerp is 256 so that
    // integer coordinates (wx=0) produce the exact corner value.
    let wx = (fx & 0xFFFF) >> 8; // 0..=255
    let wy = (fy & 0xFFFF) >> 8;

    // Sample 4 corners.
    let c00 = lookup(fb_data, palette, palette_idx, x0, y0);
    let c10 = lookup(fb_data, palette, palette_idx, x1, y0);
    let c01 = lookup(fb_data, palette, palette_idx, x0, y1);
    let c11 = lookup(fb_data, palette, palette_idx, x1, y1);

    // Bilinear blend per channel.
    let r = bilerp(c00.0, c10.0, c01.0, c11.0, wx, wy);
    let g = bilerp(c00.1, c10.1, c01.1, c11.1, wx, wy);
    let b = bilerp(c00.2, c10.2, c01.2, c11.2, wx, wy);
    (r, g, b)
}

/// Look up the RGB triple for pixel `(x, y)` in `fb_data`.
///
/// Assumes `x < 320` and `y < 200` (caller must clamp).
#[inline]
fn lookup(fb: &[u8], palette: &PaletteLut, pal: usize, x: usize, y: usize) -> (u8, u8, u8) {
    // Safety: caller clamps x/y; fb_data is 320*200 bytes.
    // We defensively use saturating indexing to avoid any panic if the slice
    // is under-sized (widget.rs already guards for this, but belt-and-suspenders).
    let offset = y * 320 + x;
    let idx = fb.get(offset).copied().unwrap_or(0);
    let rgb = palette.get(pal, idx);
    (rgb.r, rgb.g, rgb.b)
}

/// Bilinear interpolation for a single channel.
///
/// `c00`, `c10`, `c01`, `c11` are the four corner values (top-left, top-right,
/// bottom-left, bottom-right).  `wx`, `wy` are fractional weights in `0..=255`
/// where 0 = full weight on the `0` sample and 255 = weight almost fully on
/// the `1` sample.  The denominator is 256, so `wx=0` produces the exact `c00`
/// value without rounding error.
#[inline]
pub fn bilerp(c00: u8, c10: u8, c01: u8, c11: u8, wx: u32, wy: u32) -> u8 {
    // Use 256 as denominator: (a*(256-w) + b*w) >> 8.
    // At wx=0: top = c00*(256>>8) = c00 exactly.
    // At wx=255: top ≈ c10 (within 1 LSB due to 255/256 instead of 1.0).
    let top = (c00 as u32 * (256 - wx) + c10 as u32 * wx) >> 8;
    let bottom = (c01 as u32 * (256 - wx) + c11 as u32 * wx) >> 8;
    ((top * (256 - wy) + bottom * wy) >> 8) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_renderer::PaletteLut;

    // ── ScalingMode tests ──────────────────────────────────────────────────

    #[test]
    fn scaling_mode_default_is_nearest() {
        assert_eq!(ScalingMode::default(), ScalingMode::Nearest);
    }

    // ── bilerp unit tests ──────────────────────────────────────────────────

    #[test]
    fn bilerp_corners() {
        // wx=0, wy=0 → exact top-left corner (c00). No rounding error.
        assert_eq!(bilerp(0, 255, 0, 0, 0, 0), 0);
        assert_eq!(bilerp(255, 0, 0, 0, 0, 0), 255);

        // wx=255, wy=0 → c10 with weight 255/256 ≈ c10 (within 1 LSB).
        // top = (0*(256-255) + 255*255) >> 8 = (0 + 65025) >> 8 = 254
        // result = (254*(256-0) + 0*0) >> 8 = 65024 >> 8 = 254
        assert_eq!(bilerp(0, 255, 0, 0, 255, 0), 254);

        // wx=0, wy=255 → c01 with weight 255/256 ≈ c01 (within 1 LSB).
        // top    = (0*(256-0) + 0*0) >> 8 = 0
        // bottom = (255*(256-0) + 0*0) >> 8 = 65280 >> 8 = 255
        // result = (0*(256-255) + 255*255) >> 8 = 65025 >> 8 = 254
        assert_eq!(bilerp(0, 0, 255, 0, 0, 255), 254);

        // All same value → identity regardless of weights.
        assert_eq!(bilerp(128, 128, 128, 128, 127, 127), 128);
        assert_eq!(bilerp(0, 0, 0, 0, 255, 255), 0);
        assert_eq!(bilerp(255, 255, 255, 255, 255, 255), 255);
    }

    #[test]
    fn bilerp_midpoint_of_uniform_is_same() {
        // Uniform field: any weights produce the same value.
        assert_eq!(bilerp(100, 100, 100, 100, 128, 128), 100);
        assert_eq!(bilerp(200, 200, 200, 200, 0, 0), 200);
        assert_eq!(bilerp(50, 50, 50, 50, 255, 255), 50);
    }

    // ── sample_bilinear tests ──────────────────────────────────────────────

    /// Build a 320×200 framebuffer with a specific palette index at (px, py)
    /// and 0 everywhere else.
    fn make_fb_with_pixel(px: usize, py: usize, idx: u8) -> Vec<u8> {
        let mut fb = vec![0u8; 320 * 200];
        fb[py * 320 + px] = idx;
        fb
    }

    /// Build a 320×200 framebuffer filled uniformly with `idx`.
    fn make_fb_uniform(idx: u8) -> Vec<u8> {
        vec![idx; 320 * 200]
    }

    #[test]
    fn sample_bilinear_uniform_field() {
        // All pixels index 42 → bilinear blends of equal values always return same value.
        let fb = make_fb_uniform(42);
        let lut = PaletteLut::grayscale();
        let expected = lut.get(0, 42); // (42, 42, 42) in grayscale

        // At integer coordinates (no fractional part), wx=0, wy=0 → exact corner value.
        let (r, g, b) = sample_bilinear(&fb, &lut, 0, 160 << 16, 100 << 16);
        assert_eq!(r, expected.r, "r at integer coord");
        assert_eq!(g, expected.g, "g at integer coord");
        assert_eq!(b, expected.b, "b at integer coord");

        // At a fractional position with uniform field, bilerp(42,42,42,42,wx,wy)==42.
        let (r2, g2, b2) =
            sample_bilinear(&fb, &lut, 0, (160 << 16) | 0x8000, (100 << 16) | 0x8000);
        assert_eq!(r2, expected.r, "r at fractional coord");
        assert_eq!(g2, expected.g, "g at fractional coord");
        assert_eq!(b2, expected.b, "b at fractional coord");
    }

    #[test]
    fn sample_bilinear_no_panic_at_edges() {
        let fb = make_fb_uniform(0);
        let lut = PaletteLut::grayscale();

        // Maximum fixed-point values for 320×200 (319<<16 and 199<<16).
        let _ = sample_bilinear(&fb, &lut, 0, 319 << 16, 199 << 16);
        // One past end (should clamp, not panic).
        let _ = sample_bilinear(&fb, &lut, 0, 320 << 16, 200 << 16);
        // Zero.
        let _ = sample_bilinear(&fb, &lut, 0, 0, 0);
        // Max u32.
        let _ = sample_bilinear(&fb, &lut, 0, u32::MAX, u32::MAX);
    }

    #[test]
    fn sample_nearest_center_pixel() {
        // Pixel (160,100) = palette index 42; everything else = 0.
        // Sample at exactly (160<<16, 100<<16): no fractional part → wx=0, wy=0.
        // bilerp(42, 0, 0, 0, 0, 0):
        //   top    = (42*(256-0) + 0*0) >> 8 = (42*256) >> 8 = 42
        //   bottom = (0*(256-0)  + 0*0) >> 8 = 0
        //   result = (42*(256-0) + 0*0) >> 8 = 42
        // With 256-based weights, integer coordinates produce the exact corner value.
        let fb = make_fb_with_pixel(160, 100, 42);
        let lut = PaletteLut::grayscale();
        let expected = lut.get(0, 42); // grayscale: (42, 42, 42)

        let (r, g, b) = sample_bilinear(&fb, &lut, 0, 160 << 16, 100 << 16);
        assert_eq!(r, expected.r, "r should be palette value at (160,100)");
        assert_eq!(g, expected.g, "g should be palette value at (160,100)");
        assert_eq!(b, expected.b, "b should be palette value at (160,100)");
    }

    #[test]
    fn sample_bilinear_at_exact_integer_uses_nearest_corner() {
        // When fx & 0xFFFF == 0 and fy & 0xFFFF == 0, wx=0 and wy=0.
        // bilerp(c,c,c,c,0,0) = (c*(256-0)+c*0)>>8 * (256-0) >> 8
        //                     = c*256>>8 = c exactly.
        let fb = make_fb_uniform(200);
        let lut = PaletteLut::grayscale();
        let (r, g, b) = sample_bilinear(&fb, &lut, 0, 50 << 16, 50 << 16);
        // Uniform field with integer coordinates → exact value.
        assert_eq!(r, 200);
        assert_eq!(g, 200);
        assert_eq!(b, 200);
    }

    // ── nearest-neighbor helper tests (preserved from original) ───────────

    #[test]
    fn identity_mapping() {
        assert_eq!(cell_to_fb_x(5, 320, 320), 5);
        assert_eq!(cell_to_fb_y_top(10, 100, 200), 20);
    }

    #[test]
    fn half_size_terminal_scales_up() {
        assert_eq!(cell_to_fb_x(0, 160, 320), 0);
        assert_eq!(cell_to_fb_x(1, 160, 320), 2);
    }

    #[test]
    fn top_and_bot_differ_within_bounds() {
        for cy in 0..50 {
            let top = cell_to_fb_y_top(cy, 50, 200);
            let bot = cell_to_fb_y_bot(cy, 50, 200);
            assert!(bot >= top, "cy={cy}: bot={bot} < top={top}");
            assert!(top < 200, "cy={cy}: top out of bounds");
            assert!(bot < 200, "cy={cy}: bot out of bounds");
        }
    }
}
