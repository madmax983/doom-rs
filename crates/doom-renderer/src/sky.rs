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

/// Check if a flat name is the sky marker `F_SKY1`.
///
/// Compares the first 6 bytes; any trailing bytes (null padding) are ignored.
pub fn is_sky_flat(name: &[u8; 8]) -> bool {
    name[0] == b'F'
        && name[1] == b'_'
        && name[2] == b'S'
        && name[3] == b'K'
        && name[4] == b'Y'
        && name[5] == b'1'
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
    let tex_h = sky_tex.height as usize;

    if tex_w == 0 || tex_h == 0 {
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
        let col_offset = sky_u * tex_h;

        for y in top..=bot {
            // Map screen Y to sky texture V.
            // The sky texture covers the upper half of the screen (rows 0..100).
            // We scale: v = y * tex_h / 100, clamped to tex_h - 1.
            // For rows below 100, we clamp to the bottom of the sky texture.
            let v = ((y as usize) * tex_h / 100).min(tex_h - 1);
            let pixel = sky_tex.data[col_offset + v];
            fb.set_pixel(x, y as usize, pixel);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_sky_flat_detects_f_sky1() {
        assert!(is_sky_flat(b"F_SKY1\0\0"));
        assert!(!is_sky_flat(b"FLOOR4_8"));
        assert!(!is_sky_flat(b"CEIL3_5\0"));
    }

    #[test]
    fn is_sky_flat_rejects_partial_match() {
        assert!(!is_sky_flat(b"F_SKY2\0\0"));
        assert!(!is_sky_flat(b"F_SKY\0\0\0"));
        assert!(!is_sky_flat(b"G_SKY1\0\0"));
    }

    #[test]
    fn sky_texture_name_default_is_sky1() {
        assert_eq!(&sky_texture_name("E1M1")[..4], b"SKY1");
        assert_eq!(&sky_texture_name("E1M9")[..4], b"SKY1");
    }

    #[test]
    fn sky_texture_name_episode_mapping() {
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

    #[test]
    fn column_to_angle_left_is_positive() {
        // Column 0 (leftmost) should produce a positive angle (~+45 degrees).
        let angle = column_to_angle(0);
        // In BAM, 45 degrees = 0x2000_0000. Allow generous tolerance.
        let expected = 0x2000_0000u32;
        let diff = if angle.0 > expected {
            angle.0 - expected
        } else {
            expected - angle.0
        };
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
        let diff = if angle.0 > expected {
            angle.0 - expected
        } else {
            expected - angle.0
        };
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
        // In unsigned BAM, column 0 has a large positive angle and column 319
        // has a large "negative" (wrapped) angle.
        let left = column_to_angle(0).0 as i32;
        let center = column_to_angle(160).0 as i32;
        let right = column_to_angle(319).0 as i32;
        assert!(left > center, "left ({left}) > center ({center})");
        assert!(center > right, "center ({center}) > right ({right})");
    }

    #[test]
    fn draw_sky_columns_fills_pixels() {
        let mut fb = Framebuffer::new();
        // 4x4 sky texture, all pixels = 42.
        let sky_tex = WallTexture {
            width: 4,
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

    #[test]
    fn draw_sky_columns_no_sky_when_bot_less_than_top() {
        let mut fb = Framebuffer::new();
        let sky_tex = WallTexture {
            width: 4,
            height: 4,
            data: vec![42u8; 16],
        };
        let ceil_top = [0i32; 320];
        let ceil_bot = [-1i32; 320]; // all columns: bot < top → no sky

        draw_sky_columns(&mut fb, &ceil_top, &ceil_bot, Bam::ZERO, &sky_tex);

        // Entire framebuffer should remain zero.
        assert!(
            fb.data.iter().all(|&b| b == 0),
            "no pixels should be written when ceil_bot < ceil_top everywhere",
        );
    }

    #[test]
    fn draw_sky_columns_clamps_to_screen_bounds() {
        let mut fb = Framebuffer::new();
        let sky_tex = WallTexture {
            width: 4,
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

    #[test]
    fn draw_sky_columns_parallax_varies_with_angle() {
        // Two different player angles should produce different sky U mappings.
        // Use a texture where every column has a distinct value (1..=255, then 1 again),
        // and choose angles that do NOT land on the same texel.
        let sky_tex = WallTexture {
            width: 256,
            height: 128,
            // Column c has all pixels = (c + 1) so column 0 = 1, column 255 = 0 (wrap).
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
        // Use 45 degrees (0x2000_0000) which maps to a different column than 0 degrees.
        // 0x2000_0000 >> 22 = 128, 128 % 256 = 128, pixel = (128+1) & 0xFF = 129.
        // 0x0000_0000 >> 22 = 0, 0 % 256 = 0, pixel = (0+1) & 0xFF = 1.
        draw_sky_columns(
            &mut fb2,
            &ceil_top,
            &ceil_bot,
            Bam(0x2000_0000), // 45 degrees
            &sky_tex,
        );

        let px1 = fb1.get_pixel(160, 0).unwrap();
        let px2 = fb2.get_pixel(160, 0).unwrap();
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
}
