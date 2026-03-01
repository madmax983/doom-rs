//! `R_DrawColumn` — textured vertical column renderer.
//!
//! This is the innermost rendering loop, called once per screen column per wall.
//! The texture is a flat array of palette indices; we step through it vertically
//! using a fixed-point texture coordinate accumulator.
//!
//! # Fixed-point texture mapping
//! `frac` is a 16.16 fixed-point number representing the current vertical
//! position in the texture.  `fracstep` is added per pixel.  The texture
//! row is `(frac >> FRACBITS) & (texture_height - 1)`.

use crate::framebuffer::Framebuffer;
use doom_types::limits::FB_WIDTH;

/// Parameters for a single `R_DrawColumn` call.
pub struct DrawColumnParams<'a> {
    /// Screen X coordinate (0..319).
    pub x: usize,
    /// Top screen Y coordinate (inclusive).
    pub y_top: usize,
    /// Bottom screen Y coordinate (inclusive).
    pub y_bot: usize,
    /// Initial texture-coordinate accumulator (16.16 fixed-point).
    pub frac: u32,
    /// Amount to add to `frac` per pixel (16.16 fixed-point).
    pub fracstep: u32,
    /// Source texture column (palette indices), height must be a power of 2.
    pub source: &'a [u8],
    /// Colormap to apply (translates texture index → final palette index).
    /// Slice of 256 bytes; `colormap[source_pixel] → final_palette_index`.
    pub colormap: &'a [u8; 256],
}

/// Draw a textured vertical column onto the framebuffer.
///
/// Equivalent to the original `R_DrawColumn` / `R_DrawColumnLow`.
pub fn draw_column(fb: &mut Framebuffer, p: &DrawColumnParams<'_>) {
    if p.x >= FB_WIDTH || p.y_top > p.y_bot {
        return;
    }
    let y_end = (p.y_bot + 1).min(Framebuffer::height());
    let height_mask = (p.source.len() as u32).wrapping_sub(1); // works for power-of-2 heights
    let mut frac = p.frac;

    for y in p.y_top..y_end {
        let tex_row = ((frac >> 16) & height_mask) as usize;
        let raw = p.source[tex_row];
        fb.data[y * FB_WIDTH + p.x] = p.colormap[raw as usize];
        frac = frac.wrapping_add(p.fracstep);
    }
}

/// Draw a flat-shaded (solid-color) vertical column — used for sky and debug.
pub fn draw_column_solid(fb: &mut Framebuffer, x: usize, y_top: usize, y_bot: usize, index: u8) {
    fb.draw_column(x, y_top, y_bot, index);
}

/// The identity colormap: `colormap[i] == i` for all i.
///
/// Used when no light diminishment is applied (full-bright sectors).
pub const IDENTITY_COLORMAP: [u8; 256] = {
    let mut c = [0u8; 256];
    let mut i = 0usize;
    while i < 256 {
        c[i] = i as u8;
        i += 1;
    }
    c
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_column_solid_pixels() {
        let mut fb = Framebuffer::new();
        draw_column_solid(&mut fb, 10, 5, 10, 77);
        for y in 5..=10 {
            assert_eq!(fb.get_pixel(10, y), Some(77));
        }
        assert_eq!(fb.get_pixel(10, 4), Some(0));
    }

    #[test]
    fn draw_column_textured_applies_colormap() {
        let mut fb = Framebuffer::new();
        // Texture: 4 rows, all value 1.
        let source = [1u8; 4];
        // Colormap: 1 → 42.
        let mut colormap = IDENTITY_COLORMAP;
        colormap[1] = 42;

        let p = DrawColumnParams {
            x: 0, y_top: 0, y_bot: 3,
            frac: 0, fracstep: 1 << 16, // step 1 texel/pixel
            source: &source,
            colormap: &colormap,
        };
        draw_column(&mut fb, &p);
        for y in 0..=3 {
            assert_eq!(fb.get_pixel(0, y), Some(42), "row {y}");
        }
    }

    #[test]
    fn identity_colormap_is_identity() {
        for (i, &v) in IDENTITY_COLORMAP.iter().enumerate() {
            assert_eq!(v, i as u8);
        }
    }
}
