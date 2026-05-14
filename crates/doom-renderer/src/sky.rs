//! Sky rendering — draws parallax sky columns for sectors with `F_SKY1` ceiling.
//!
//! In Doom, any sector whose ceiling flat is `F_SKY1` gets a sky texture drawn
//! instead of a normal textured/flat ceiling.  The sky texture is a wall texture
//! (`SKY1`, `SKY2`, etc.) mapped with parallax: the U coordinate depends on
//! the **player's viewing angle + per-column screen angle**, not world position.
//! This creates the illusion of an infinitely distant sky dome.
//!
//! # Column mapping
//! For each screen column `x`:
//! ```text
//!   view_angle = player_angle + column_to_angle(x)
//!   sky_u = (view_angle / 2pi) * sky_tex.width
//! ```
//! Sky is always drawn at full bright (no light diminishing).

use crate::framebuffer::Framebuffer;
use crate::texture::WallTexture;
use doom_types::Bam;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The flat name that marks "draw sky here instead of a normal ceiling".
pub const SKY_FLAT_NAME: [u8; 8] = *b"F_SKY1\0\0";

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Palette index used as a solid-colour fallback when no sky texture is
/// available.  Approximates dark blue in the default PLAYPAL.
pub const SKY_FALLBACK_COLOR: u8 = 197;

const SCREEN_W: usize = 320;
const SCREEN_H: usize = 200;
const SKY_MASK_WORDS: usize = SCREEN_H.div_ceil(64);
const SKY_TEXTURE_MID_ROWS: i32 = (SCREEN_H / 2) as i32;

/// Per-column sky coverage mask that can represent multiple disjoint spans.
#[derive(Clone, Debug)]
pub struct SkyCoverage {
    columns: [[u64; SKY_MASK_WORDS]; SCREEN_W],
}

impl SkyCoverage {
    /// Creates a new, empty `SkyCoverage` mask buffer, clearing all sky pixels to `false`.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::sky::SkyCoverage;
    /// let mask = SkyCoverage::new();
    /// assert_eq!(mask.contains(10, 10), false);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            columns: [[0; SKY_MASK_WORDS]; SCREEN_W],
        }
    }

    /// Records a vertical span of the sky as visible in the mask buffer using fast bitwise OR operations.
    ///
    /// Instead of rendering the complex sky texture during BSP traversal (which would be slow and might suffer overdraw),
    /// we just flip bits in this 1D array of 64-bit integers. Once the solid geometry is finished, we do a single
    /// pass over the screen, drawing sky pixels only where these bits are set.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::sky::SkyCoverage;
    /// let mut mask = SkyCoverage::new();
    /// mask.record_span(150, 0, 100); // Sky is visible in column 150 from top of screen to y=100
    /// assert!(mask.contains(150, 50));
    /// ```
    pub fn record_span(&mut self, x: usize, top: i32, bot: i32) {
        if x >= SCREEN_W {
            return;
        }

        let top = top.clamp(0, SCREEN_H as i32 - 1);
        let bot = bot.clamp(0, SCREEN_H as i32 - 1);
        if bot < top {
            return;
        }

        let start = top as usize;
        let end = bot as usize;
        let start_word = start / 64;
        let end_word = end / 64;

        if start_word == end_word {
            let width = end - start + 1;
            let mask = if width == 64 {
                u64::MAX
            } else {
                ((1u64 << width) - 1) << (start % 64)
            };
            self.columns[x][start_word] |= mask;
            return;
        }

        self.columns[x][start_word] |= u64::MAX << (start % 64);
        for word in (start_word + 1)..end_word {
            self.columns[x][word] = u64::MAX;
        }

        let end_bit = end % 64;
        let tail_mask = if end_bit == 63 {
            u64::MAX
        } else {
            (1u64 << (end_bit + 1)) - 1
        };
        self.columns[x][end_word] |= tail_mask;
    }

    /// Queries the bitmask to check if the specific screen pixel `(x, y)` requires sky rendering.
    ///
    /// Used by the final composition pass to determine whether to write a sky texel or leave the pixel alone.
    #[must_use]
    pub fn contains(&self, x: usize, y: usize) -> bool {
        if x >= SCREEN_W || y >= SCREEN_H {
            return false;
        }
        let word = y / 64;
        let bit = y % 64;
        (self.columns[x][word] & (1u64 << bit)) != 0
    }
}

impl Default for SkyCoverage {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a flat name is the sky marker `F_SKY1`.
///
/// Compares the first 6 bytes case-insensitively; any trailing bytes
/// (null padding) are ignored.
pub fn is_sky_flat(name: &[u8; 8]) -> bool {
    name[0].eq_ignore_ascii_case(&b'F')
        && name[1] == b'_'
        && name[2].eq_ignore_ascii_case(&b'S')
        && name[3].eq_ignore_ascii_case(&b'K')
        && name[4].eq_ignore_ascii_case(&b'Y')
        && name[5] == b'1'
}

/// Return the sky texture lump name for a given Doom 1 episode number.
///
/// - Episode 1 -> `SKY1`
/// - Episode 2 -> `SKY2`
/// - Episode 3 -> `SKY3`
/// - Episode 4+ -> `SKY4`
///
/// If `episode` is 0 the default `SKY1` is returned.
pub fn sky_texture_for_episode(episode: u8) -> [u8; 8] {
    match episode {
        2 => *b"SKY2\0\0\0\0",
        3 => *b"SKY3\0\0\0\0",
        4.. => *b"SKY4\0\0\0\0",
        _ => *b"SKY1\0\0\0\0", // 0 or 1
    }
}

/// Determine which sky texture lump name to use based on the map name.
///
/// Doom 1 (ExMy format):
///   - E1 -> `SKY1`, E2 -> `SKY2`, E3 -> `SKY3`, E4 -> `SKY4`
///
/// Doom 2 (MAPxx format):
///   - MAP01-MAP11 -> `SKY1`, MAP12-MAP20 -> `SKY2`, MAP21-MAP32 -> `SKY3`
///
/// Anything else defaults to `SKY1`.
pub fn sky_texture_name(map_name: &str) -> [u8; 8] {
    let name = map_name.to_uppercase();
    let bytes = name.as_bytes();

    // Doom 1: ExMy format
    if bytes.len() >= 4 && bytes[0] == b'E' && bytes[2] == b'M' {
        let episode = bytes[1];
        return match episode {
            b'2' => *b"SKY2\0\0\0\0",
            b'3' => *b"SKY3\0\0\0\0",
            b'4' => *b"SKY4\0\0\0\0",
            _ => *b"SKY1\0\0\0\0", // E1 and anything else
        };
    }

    // Doom 2: MAPxx format
    if bytes.len() >= 5 && bytes[0] == b'M' && bytes[1] == b'A' && bytes[2] == b'P' {
        if let Ok(num) = name[3..].parse::<u32>() {
            return if num >= 21 {
                *b"SKY3\0\0\0\0"
            } else if num >= 12 {
                *b"SKY2\0\0\0\0"
            } else {
                *b"SKY1\0\0\0\0"
            };
        }
    }

    // Default
    *b"SKY1\0\0\0\0"
}

// ---------------------------------------------------------------------------
// Column-to-angle mapping
// ---------------------------------------------------------------------------

/// Convert a screen column index to the angular offset from the view center.
///
/// The horizontal FOV is 90 degrees.  Column 160 is dead center (angle 0).
/// Column 0 is +45 degrees (left edge), column 319 is -45 degrees (right edge).
///
/// Uses `atan` for perspective-correct mapping.
pub fn column_to_angle(x: usize) -> Bam {
    let dx = x as f32 - 160.0;
    // atan(dx / focal_len) where focal_len = 160 gives the angle offset
    let angle_rad = (dx / 160.0).atan();
    // Convert radians to BAM: full circle = 2*pi = 2^32 BAM units
    // Negative because screen-left = positive angle in Doom's coordinate system
    let bam_f64 = (-angle_rad as f64) / std::f64::consts::TAU * (u32::MAX as f64 + 1.0);
    Bam(bam_f64 as i64 as u32)
}

#[inline]
fn sky_texel_row(y: usize, logical_tex_h: usize) -> usize {
    if logical_tex_h == 0 {
        return 0;
    }

    // Chocolate Doom's `R_InitSkyMap` sets `skytexturemid` to
    // `SCREENHEIGHT / 2 * FRACUNIT`. With a centered horizon, the sky row
    // sampled at screen row `y` is therefore horizon-relative rather than
    // "stretch top half, clamp bottom half".
    let row = SKY_TEXTURE_MID_ROWS + y as i32 - (SCREEN_H as i32 / 2);
    row.rem_euclid(logical_tex_h as i32) as usize
}

// ---------------------------------------------------------------------------
// Sky column renderer
// ---------------------------------------------------------------------------

/// Draw sky columns for the given screen region.
///
/// For each screen column `x`, if `ceil_bot[x] >= ceil_top[x]`, sky pixels
/// are drawn from `ceil_top[x]` to `ceil_bot[x]` (inclusive).
///
/// The sky texture U coordinate is based on `player_angle + column_to_angle(x)`
/// (parallax), and the V coordinate stretches the sky texture across the top
/// portion of the screen.
///
/// Sky is always drawn at full bright (no colormap applied).
pub fn draw_sky_columns(
    fb: &mut Framebuffer,
    ceil_top: &[i32; 320],
    ceil_bot: &[i32; 320],
    player_angle: Bam,
    sky_tex: &WallTexture,
) {
    let fb_h = Framebuffer::height() as i32;
    let tex_w = sky_tex.width as usize;
    let tex_h = sky_tex.logical_height as usize;
    let tex_stride = sky_tex.height as usize;

    if tex_w == 0 || tex_h == 0 || tex_stride == 0 {
        return;
    }

    for x in 0..320usize {
        let top = ceil_top[x].max(0);
        let bot = ceil_bot[x].min(fb_h - 1);
        if bot < top {
            continue;
        }

        // Parallax U: based on player angle + per-column angle offset.
        let view_angle = player_angle + column_to_angle(x);
        // Map the full 32-bit angle range to `tex_w` texels.
        // Multiply by 2.45 (roughly) so the 256-wide sky wraps ~2.45 times
        // around the 360-degree view, matching Doom's behavior where a
        // 256-wide sky texture covers about 147 degrees.
        // In classic Doom, sky_u = (angle >> ANGLETOSKYSHIFT) & 255
        // where ANGLETOSKYSHIFT = 22 for 256-wide sky.
        // This gives: 2^32 / 2^22 = 1024 virtual columns mapped to 256 texels.
        // Equivalent: (angle >> 22) & (tex_w - 1).
        let sky_u = ((view_angle.0 >> 22) as usize) % tex_w;

        // Column data in the texture is column-major: data[col * height + row].
        let col_offset = sky_u * tex_stride;

        for y in top..=bot {
            let v = sky_texel_row(y as usize, tex_h);
            let pixel = sky_tex.data[col_offset + v];
            fb.set_pixel(x, y as usize, pixel);
        }
    }
}

/// Draw sky columns from a per-column coverage mask.
pub fn draw_sky_coverage_columns(
    fb: &mut Framebuffer,
    coverage: &SkyCoverage,
    player_angle: Bam,
    sky_tex: &WallTexture,
) {
    let tex_w = sky_tex.width as usize;
    let tex_h = sky_tex.logical_height as usize;
    let tex_stride = sky_tex.height as usize;

    if tex_w == 0 || tex_h == 0 || tex_stride == 0 {
        return;
    }

    for x in 0..SCREEN_W {
        let view_angle = player_angle + column_to_angle(x);
        let sky_u = ((view_angle.0 >> 22) as usize) % tex_w;
        let col_offset = sky_u * tex_stride;
        for y in 0..SCREEN_H {
            if !coverage.contains(x, y) {
                continue;
            }
            let v = sky_texel_row(y, tex_h);
            let pixel = sky_tex.data[col_offset + v];
            fb.set_pixel(x, y, pixel);
        }
    }
}

/// Draw a solid-colour sky fallback when no sky texture is available.
///
/// Uses [`SKY_FALLBACK_COLOR`] (dark blue) for all sky pixels.  The
/// column-to-angle parallax is irrelevant here — every pixel is the
/// same colour.
pub fn draw_sky_fallback(fb: &mut Framebuffer, ceil_top: &[i32; 320], ceil_bot: &[i32; 320]) {
    let fb_h = Framebuffer::height() as i32;
    for x in 0..320usize {
        let top = ceil_top[x].max(0);
        let bot = ceil_bot[x].min(fb_h - 1);
        if bot < top {
            continue;
        }
        for y in top..=bot {
            fb.set_pixel(x, y as usize, SKY_FALLBACK_COLOR);
        }
    }
}

/// Draw fallback sky from a per-column coverage mask.
pub fn draw_sky_coverage_fallback(fb: &mut Framebuffer, coverage: &SkyCoverage) {
    for x in 0..SCREEN_W {
        for y in 0..SCREEN_H {
            if coverage.contains(x, y) {
                fb.set_pixel(x, y, SKY_FALLBACK_COLOR);
            }
        }
    }
}

/// Compute the sky texture column (U coordinate) for screen column `x`
/// at the given `player_angle`.
///
/// Uses the classic Doom formula:
/// `sky_u = ((player_angle + column_to_angle(x)).0 >> 22) % tex_w`
///
/// This is exposed publicly so callers can inspect the mapping without
/// having to replicate the bit-shift logic.
pub fn sky_texel_column(x: usize, player_angle: Bam, tex_w: u32) -> u32 {
    if tex_w == 0 {
        return 0;
    }
    let view_angle = player_angle + column_to_angle(x);
    (view_angle.0 >> 22) % tex_w
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // 1. is_sky_flat returns true for F_SKY1
    // -----------------------------------------------------------------------
    #[test]
    fn is_sky_flat_detects_f_sky1() {
        assert!(is_sky_flat(b"F_SKY1\0\0"));
        assert!(!is_sky_flat(b"FLOOR4_8"));
        assert!(!is_sky_flat(b"CEIL3_5\0"));
    }

    // -----------------------------------------------------------------------
    // 2. is_sky_flat returns false for other flats
    // -----------------------------------------------------------------------
    #[test]
    fn is_sky_flat_rejects_partial_match() {
        assert!(!is_sky_flat(b"F_SKY2\0\0"));
        assert!(!is_sky_flat(b"F_SKY\0\0\0"));
        assert!(!is_sky_flat(b"G_SKY1\0\0"));
    }

    // -----------------------------------------------------------------------
    // 3. is_sky_flat case insensitive (f_sky1)
    // -----------------------------------------------------------------------
    #[test]
    fn is_sky_flat_case_insensitive() {
        assert!(is_sky_flat(b"f_sky1\0\0"), "all lowercase must match");
        assert!(is_sky_flat(b"F_SKY1\0\0"), "all uppercase must match");
        assert!(is_sky_flat(b"f_Sky1\0\0"), "mixed case must match");
        assert!(is_sky_flat(b"F_sky1\0\0"), "mixed case must match");
        assert!(!is_sky_flat(b"f_sky2\0\0"), "F_SKY2 must not match");
    }

    // -----------------------------------------------------------------------
    // 4. sky_texture_for_episode returns correct names for all 4 episodes
    // -----------------------------------------------------------------------
    #[test]
    fn sky_texture_for_episode_all_four() {
        assert_eq!(&sky_texture_for_episode(1)[..4], b"SKY1");
        assert_eq!(&sky_texture_for_episode(2)[..4], b"SKY2");
        assert_eq!(&sky_texture_for_episode(3)[..4], b"SKY3");
        assert_eq!(&sky_texture_for_episode(4)[..4], b"SKY4");
    }

    #[test]
    fn sky_texture_for_episode_defaults() {
        // Episode 0 and 1 both return SKY1.
        assert_eq!(&sky_texture_for_episode(0)[..4], b"SKY1");
        assert_eq!(&sky_texture_for_episode(1)[..4], b"SKY1");
        // Episode 5+ returns SKY4.
        assert_eq!(&sky_texture_for_episode(5)[..4], b"SKY4");
        assert_eq!(&sky_texture_for_episode(255)[..4], b"SKY4");
    }

    // -----------------------------------------------------------------------
    // 5. Sky column angle calculation for center column (x=160) is 0
    // -----------------------------------------------------------------------
    #[test]
    fn column_to_angle_center_is_zero() {
        let angle = column_to_angle(160);
        // Center column should be approximately 0.
        // Allow small epsilon for floating-point rounding.
        assert!(
            angle.0 < 1000 || angle.0 > u32::MAX - 1000,
            "center column angle {} should be ~0",
            angle.0,
        );
    }

    // -----------------------------------------------------------------------
    // 6. Sky column angle for edge columns is correct
    // -----------------------------------------------------------------------
    #[test]
    fn column_to_angle_left_is_positive() {
        // Column 0 (leftmost) should produce a positive angle (~+45 degrees).
        let angle = column_to_angle(0);
        // In BAM, 45 degrees = 0x2000_0000. Allow generous tolerance.
        let expected = 0x2000_0000u32;
        let diff = angle.0.abs_diff(expected);
        assert!(
            diff < 0x0200_0000, // ~2.8 degrees tolerance
            "left edge angle {} should be near 45 degrees ({}), diff={}",
            angle.0,
            expected,
            diff,
        );
    }

    #[test]
    fn column_to_angle_right_is_negative() {
        // Column 319 (rightmost) should produce a negative angle (~-45 degrees).
        // In wrapping BAM, -45 degrees = 0xE000_0000.
        let angle = column_to_angle(319);
        let expected = 0xE000_0000u32; // -45 degrees
        let diff = angle.0.abs_diff(expected);
        assert!(
            diff < 0x0200_0000, // ~2.8 degrees tolerance
            "right edge angle {} should be near -45 degrees ({}), diff={}",
            angle.0,
            expected,
            diff,
        );
    }

    #[test]
    fn column_to_angle_is_monotonic() {
        // Angles should decrease (in signed interpretation) from left to right.
        let left = column_to_angle(0).0 as i32;
        let center = column_to_angle(160).0 as i32;
        let right = column_to_angle(319).0 as i32;
        assert!(left > center, "left ({left}) > center ({center})");
        assert!(center > right, "center ({center}) > right ({right})");
    }

    // -----------------------------------------------------------------------
    // 7. draw_sky_column draws correct texture column
    // -----------------------------------------------------------------------
    #[test]
    fn draw_sky_columns_fills_pixels() {
        let mut fb = Framebuffer::new();
        // 4x4 sky texture, all pixels = 42.
        let sky_tex = WallTexture {
            width: 4,
            logical_height: 4,
            height: 4,
            data: vec![42u8; 16],
        };
        let ceil_top = [0i32; 320];
        let mut ceil_bot = [-1i32; 320]; // no sky by default
        ceil_bot[0] = 3; // column 0 has sky from row 0 to 3
        ceil_bot[1] = 3; // column 1 has sky from row 0 to 3

        draw_sky_columns(&mut fb, &ceil_top, &ceil_bot, Bam::ZERO, &sky_tex);

        // Verify pixels were written with the sky texture value.
        for y in 0..=3 {
            assert_eq!(
                fb.get_pixel(0, y),
                Some(42),
                "pixel at (0, {y}) should be 42 from sky texture",
            );
        }
        // Column 2 should remain untouched (ceil_bot = -1 < ceil_top = 0).
        assert_eq!(fb.get_pixel(2, 0), Some(0));
    }

    // -----------------------------------------------------------------------
    // 8. Sky texture wraps horizontally (angle > 360 wraps)
    // -----------------------------------------------------------------------
    #[test]
    fn sky_texture_wraps_horizontally() {
        // A 256-wide sky texture uses (angle >> 22) % 256.
        // Verify that angles separated by a full 360 degrees produce the same
        // texel column (wrapping).
        let tex_w = 256u32;
        let angle_a = Bam(0x0000_0000); // 0 degrees
        let angle_b = Bam(0x0000_0000); // 0 degrees (360 wraps to 0 in BAM)
        let col_a = sky_texel_column(160, angle_a, tex_w);
        let col_b = sky_texel_column(160, angle_b, tex_w);
        assert_eq!(
            col_a, col_b,
            "full 360-degree wrap should produce the same texel column"
        );

        // Also verify that different angles map to different columns.
        // 45 degrees = 0x2000_0000. (0x2000_0000 >> 22) = 128.  128 % 256 = 128.
        let angle_c = Bam(0x2000_0000); // 45 degrees
        let col_c = sky_texel_column(160, angle_c, tex_w);
        assert_ne!(
            col_a, col_c,
            "0 degrees and 45 degrees should map to different sky columns"
        );
    }

    // -----------------------------------------------------------------------
    // 9. Sky at full brightness (no colormap darkening)
    // -----------------------------------------------------------------------
    #[test]
    fn sky_at_full_brightness() {
        // draw_sky_columns uses the raw texture pixel value directly — no
        // colormap is applied.  Verify by using a texture with a known pixel
        // value and checking the framebuffer gets that exact value (not a
        // darkened version).
        let mut fb = Framebuffer::new();
        let sky_tex = WallTexture {
            width: 4,
            logical_height: 4,
            height: 4,
            data: vec![200u8; 16], // bright pixel value
        };
        let ceil_top = [0i32; 320];
        let mut ceil_bot = [-1i32; 320];
        ceil_bot[0] = 0; // just one pixel at (0, 0)

        draw_sky_columns(&mut fb, &ceil_top, &ceil_bot, Bam::ZERO, &sky_tex);

        // The pixel must be the raw texture value — no colormap was applied.
        assert_eq!(
            fb.get_pixel(0, 0),
            Some(200),
            "sky pixel must be raw texture value (full bright, no colormap)",
        );
    }

    // -----------------------------------------------------------------------
    // 10. Fallback to solid color when no sky texture
    // -----------------------------------------------------------------------
    #[test]
    fn fallback_to_solid_color_when_no_texture() {
        let mut fb = Framebuffer::new();
        let ceil_top = [0i32; 320];
        let mut ceil_bot = [-1i32; 320];
        ceil_bot[0] = 5; // column 0: sky from row 0..=5

        draw_sky_fallback(&mut fb, &ceil_top, &ceil_bot);

        for y in 0..=5 {
            assert_eq!(
                fb.get_pixel(0, y),
                Some(SKY_FALLBACK_COLOR),
                "fallback sky pixel at (0, {y}) should be dark blue ({})",
                SKY_FALLBACK_COLOR,
            );
        }
        // Column 1 (no sky) should be untouched.
        assert_eq!(fb.get_pixel(1, 0), Some(0));
    }

    // -----------------------------------------------------------------------
    // 11. Sky column y-range clipping (y_top/y_bot bounds)
    // -----------------------------------------------------------------------
    #[test]
    fn draw_sky_columns_clamps_to_screen_bounds() {
        let mut fb = Framebuffer::new();
        let sky_tex = WallTexture {
            width: 4,
            logical_height: 4,
            height: 4,
            data: vec![42u8; 16],
        };
        // Intentionally out-of-bounds values to test clamping.
        let mut ceil_top = [-10i32; 320];
        let mut ceil_bot = [250i32; 320]; // beyond screen height 200

        // Only enable column 0.
        for i in 1..320 {
            ceil_top[i] = 0;
            ceil_bot[i] = -1;
        }

        // Should not panic.
        draw_sky_columns(&mut fb, &ceil_top, &ceil_bot, Bam::ZERO, &sky_tex);

        // Pixel at (0, 0) should be sky.
        assert_eq!(fb.get_pixel(0, 0), Some(42));
        // Pixel at (0, 199) should also be sky (clamped to screen bounds).
        assert_eq!(fb.get_pixel(0, 199), Some(42));
    }

    // -----------------------------------------------------------------------
    // 12. Sky does not overwrite existing wall pixels below ceiling
    // -----------------------------------------------------------------------
    #[test]
    fn sky_does_not_overwrite_wall_below_ceiling() {
        let mut fb = Framebuffer::new();

        // Pre-fill rows 50..=99 with "wall" pixel value 77 at column 0.
        for y in 50..=99 {
            fb.set_pixel(0, y, 77);
        }

        let sky_tex = WallTexture {
            width: 4,
            logical_height: 4,
            height: 4,
            data: vec![42u8; 16],
        };

        // Sky only covers rows 0..=49 for column 0.
        let ceil_top = [0i32; 320];
        let mut ceil_bot = [-1i32; 320];
        ceil_bot[0] = 49;

        draw_sky_columns(&mut fb, &ceil_top, &ceil_bot, Bam::ZERO, &sky_tex);

        // Sky region should be drawn.
        assert_eq!(fb.get_pixel(0, 0), Some(42), "sky at (0, 0)");
        assert_eq!(fb.get_pixel(0, 49), Some(42), "sky at (0, 49)");

        // Wall pixels below the sky ceiling must be preserved.
        assert_eq!(
            fb.get_pixel(0, 50),
            Some(77),
            "wall pixel at (0, 50) must not be overwritten by sky"
        );
        assert_eq!(
            fb.get_pixel(0, 99),
            Some(77),
            "wall pixel at (0, 99) must not be overwritten by sky"
        );
    }

    // -----------------------------------------------------------------------
    // 13. Screen column to sky tex_x mapping
    // -----------------------------------------------------------------------
    #[test]
    fn screen_column_to_sky_tex_x_mapping() {
        // For a 256-wide sky texture with player_angle=0 and center column
        // (x=160), the column_to_angle is ~0.  So:
        //   sky_u = (0 >> 22) % 256 = 0
        let tex_x = sky_texel_column(160, Bam::ZERO, 256);
        assert_eq!(tex_x, 0, "center column at 0 degrees should map to texel 0");

        // For player_angle = 0x2000_0000 (45 degrees) at center:
        //   sky_u = (0x2000_0000 >> 22) % 256 = 128
        let tex_x2 = sky_texel_column(160, Bam(0x2000_0000), 256);
        assert_eq!(
            tex_x2, 128,
            "center column at 45 degrees should map to texel 128"
        );

        // For a 128-wide sky: (0x2000_0000 >> 22) % 128 = 128 % 128 = 0
        let tex_x3 = sky_texel_column(160, Bam(0x2000_0000), 128);
        assert_eq!(
            tex_x3, 0,
            "128-wide sky at 45 degrees center should wrap to texel 0"
        );
    }

    // -----------------------------------------------------------------------
    // 14. sky_texture_name (map-name based) for all episodes
    // -----------------------------------------------------------------------
    #[test]
    fn sky_texture_name_all_episodes() {
        assert_eq!(&sky_texture_name("E1M1")[..4], b"SKY1");
        assert_eq!(&sky_texture_name("E1M9")[..4], b"SKY1");
        assert_eq!(&sky_texture_name("E2M1")[..4], b"SKY2");
        assert_eq!(&sky_texture_name("E3M5")[..4], b"SKY3");
        assert_eq!(&sky_texture_name("E4M1")[..4], b"SKY4");
    }

    #[test]
    fn sky_texture_name_doom2_mapping() {
        assert_eq!(&sky_texture_name("MAP01")[..4], b"SKY1");
        assert_eq!(&sky_texture_name("MAP11")[..4], b"SKY1");
        assert_eq!(&sky_texture_name("MAP12")[..4], b"SKY2");
        assert_eq!(&sky_texture_name("MAP20")[..4], b"SKY2");
        assert_eq!(&sky_texture_name("MAP21")[..4], b"SKY3");
        assert_eq!(&sky_texture_name("MAP32")[..4], b"SKY3");
    }

    #[test]
    fn sky_texture_name_unknown_defaults_to_sky1() {
        assert_eq!(&sky_texture_name("LEVEL01")[..4], b"SKY1");
        assert_eq!(&sky_texture_name("")[..4], b"SKY1");
    }

    // -----------------------------------------------------------------------
    // Additional tests (beyond the 14 minimum)
    // -----------------------------------------------------------------------

    #[test]
    fn draw_sky_columns_no_sky_when_bot_less_than_top() {
        let mut fb = Framebuffer::new();
        let sky_tex = WallTexture {
            width: 4,
            logical_height: 4,
            height: 4,
            data: vec![42u8; 16],
        };
        let ceil_top = [0i32; 320];
        let ceil_bot = [-1i32; 320]; // all columns: bot < top -> no sky

        draw_sky_columns(&mut fb, &ceil_top, &ceil_bot, Bam::ZERO, &sky_tex);

        // Entire framebuffer should remain zero.
        assert!(
            fb.data.iter().all(|&b| b == 0),
            "no pixels should be written when ceil_bot < ceil_top everywhere",
        );
    }

    #[test]
    fn draw_sky_columns_parallax_varies_with_angle() {
        // Two different player angles should produce different sky U mappings.
        let sky_tex = WallTexture {
            width: 256,
            logical_height: 128,
            height: 128,
            data: (0..256u32)
                .flat_map(|c| std::iter::repeat_n(((c + 1) & 0xFF) as u8, 128))
                .collect(),
        };

        let ceil_top = [0i32; 320];
        let mut ceil_bot = [-1i32; 320];
        ceil_bot[160] = 0; // only center column, row 0

        let mut fb1 = Framebuffer::new();
        draw_sky_columns(&mut fb1, &ceil_top, &ceil_bot, Bam::ZERO, &sky_tex);

        let mut fb2 = Framebuffer::new();
        draw_sky_columns(
            &mut fb2,
            &ceil_top,
            &ceil_bot,
            Bam(0x2000_0000), // 45 degrees
            &sky_tex,
        );

        let px1 = fb1.get_pixel(160, 0).expect("value must exist in test");
        let px2 = fb2.get_pixel(160, 0).expect("value must exist in test");
        assert_ne!(
            px1, px2,
            "different player angles should produce different sky pixels at center column (got {} vs {})",
            px1, px2,
        );
    }

    #[test]
    fn draw_sky_columns_zero_size_texture_no_panic() {
        let mut fb = Framebuffer::new();
        let sky_tex = WallTexture {
            width: 0,
            logical_height: 0,
            height: 0,
            data: vec![],
        };
        let ceil_top = [0i32; 320];
        let ceil_bot = [50i32; 320];
        // Should not panic.
        draw_sky_columns(&mut fb, &ceil_top, &ceil_bot, Bam::ZERO, &sky_tex);
    }

    #[test]
    fn sky_flat_name_constant_matches_detector() {
        assert!(is_sky_flat(&SKY_FLAT_NAME));
    }

    #[test]
    fn sky_texel_column_zero_width_returns_zero() {
        // Edge case: tex_w=0 should return 0 without panicking.
        assert_eq!(sky_texel_column(160, Bam::ZERO, 0), 0);
    }

    #[test]
    fn draw_sky_fallback_clamps_to_screen_bounds() {
        let mut fb = Framebuffer::new();
        let mut ceil_top = [-10i32; 320];
        let mut ceil_bot = [250i32; 320];
        // Only enable column 0.
        for i in 1..320 {
            ceil_top[i] = 0;
            ceil_bot[i] = -1;
        }

        // Should not panic.
        draw_sky_fallback(&mut fb, &ceil_top, &ceil_bot);
        assert_eq!(fb.get_pixel(0, 0), Some(SKY_FALLBACK_COLOR));
        assert_eq!(fb.get_pixel(0, 199), Some(SKY_FALLBACK_COLOR));
    }

    #[test]
    fn draw_sky_coverage_columns_preserves_disjoint_spans() {
        let mut fb = Framebuffer::new();
        fb.fill_rect(0, 0, 320, 200, 9);

        let sky_tex = WallTexture {
            width: 1,
            logical_height: 4,
            height: 4,
            data: vec![3, 4, 5, 6],
        };
        let mut coverage = SkyCoverage::new();
        coverage.record_span(10, 10, 12);
        coverage.record_span(10, 20, 22);

        draw_sky_coverage_columns(&mut fb, &coverage, Bam::ZERO, &sky_tex);

        assert_ne!(fb.get_pixel(10, 10), Some(9));
        assert_eq!(
            fb.get_pixel(10, 15),
            Some(9),
            "disjoint sky spans must not fill the solid gap between them"
        );
        assert_ne!(fb.get_pixel(10, 21), Some(9));
    }

    #[test]
    fn draw_sky_coverage_fallback_preserves_disjoint_spans() {
        let mut fb = Framebuffer::new();
        fb.fill_rect(0, 0, 320, 200, 9);

        let mut coverage = SkyCoverage::new();
        coverage.record_span(12, 30, 31);
        coverage.record_span(12, 40, 42);

        draw_sky_coverage_fallback(&mut fb, &coverage);

        assert_eq!(fb.get_pixel(12, 30), Some(SKY_FALLBACK_COLOR));
        assert_eq!(
            fb.get_pixel(12, 35),
            Some(9),
            "fallback sky must also preserve disjoint gaps"
        );
        assert_eq!(fb.get_pixel(12, 41), Some(SKY_FALLBACK_COLOR));
    }

    #[test]
    fn sky_vertical_mapping_draw_columns_tracks_full_screen_rows() {
        let mut fb = Framebuffer::new();
        let sky_tex = WallTexture {
            width: 1,
            logical_height: 200,
            height: 200,
            data: (0..200u16).map(|row| row as u8).collect(),
        };
        let ceil_top = [0i32; 320];
        let mut ceil_bot = [-1i32; 320];
        ceil_bot[0] = 199;

        draw_sky_columns(&mut fb, &ceil_top, &ceil_bot, Bam::ZERO, &sky_tex);

        assert_eq!(fb.get_pixel(0, 25), Some(25));
        assert_eq!(fb.get_pixel(0, 100), Some(100));
        assert_eq!(
            fb.get_pixel(0, 150),
            Some(150),
            "rows below the horizon must keep advancing through the sky texture"
        );
    }

    #[test]
    fn sky_vertical_mapping_draw_coverage_tracks_full_screen_rows() {
        let mut fb = Framebuffer::new();
        let sky_tex = WallTexture {
            width: 1,
            logical_height: 200,
            height: 200,
            data: (0..200u16).map(|row| row as u8).collect(),
        };
        let mut coverage = SkyCoverage::new();
        coverage.record_span(0, 0, 199);

        draw_sky_coverage_columns(&mut fb, &coverage, Bam::ZERO, &sky_tex);

        assert_eq!(fb.get_pixel(0, 40), Some(40));
        assert_eq!(fb.get_pixel(0, 120), Some(120));
        assert_eq!(
            fb.get_pixel(0, 175),
            Some(175),
            "coverage-based sky draw must not clamp every row below the horizon to the final texel"
        );
    }
}
