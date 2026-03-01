//! `R_DrawSpan` — textured horizontal floor/ceiling span renderer.
//!
//! Flats (floor/ceiling textures) are 64×64 palette-index arrays.
//! Texture coordinates step affinely across the span in fixed-point.
//!
//! # Coordinate mapping
//! Both `ds_xfrac` and `ds_yfrac` are 16.16 fixed-point numbers.
//! The flat row is `(yfrac >> 16) & 63`, column is `(xfrac >> 16) & 63`.

use crate::framebuffer::Framebuffer;
use doom_types::limits::{FB_WIDTH, FLAT_SIZE};

/// Side length of a flat texture (64).
pub const FLAT_DIM: usize = 64;

/// Mask for wrapping flat coordinates (63 = 0x3F).
pub const FLAT_MASK: u32 = (FLAT_DIM as u32) - 1;

/// Parameters for a single `R_DrawSpan` call.
pub struct DrawSpanParams<'a> {
    /// Screen Y row.
    pub y: usize,
    /// Starting screen X (inclusive).
    pub x1: usize,
    /// Ending screen X (inclusive).
    pub x2: usize,
    /// Initial horizontal texture coordinate (16.16 fixed-point).
    pub ds_xfrac: u32,
    /// Initial vertical texture coordinate (16.16 fixed-point).
    pub ds_yfrac: u32,
    /// Per-pixel step in the X texture direction.
    pub ds_xstep: u32,
    /// Per-pixel step in the Y texture direction.
    pub ds_ystep: u32,
    /// Source flat (must be exactly `FLAT_SIZE = 4096` bytes).
    pub source: &'a [u8; FLAT_SIZE],
    /// Colormap for light-level application.
    pub colormap: &'a [u8; 256],
}

/// Draw a textured floor/ceiling span.
///
/// Equivalent to the original `R_DrawSpan`.
pub fn draw_span(fb: &mut Framebuffer, p: &DrawSpanParams<'_>) {
    if p.y >= Framebuffer::height() || p.x1 > p.x2 {
        return;
    }
    let x_end = (p.x2 + 1).min(FB_WIDTH);
    let mut xfrac = p.ds_xfrac;
    let mut yfrac = p.ds_yfrac;

    let row_base = p.y * FB_WIDTH;
    for x in p.x1..x_end {
        let col = (xfrac >> 16) & FLAT_MASK;
        let row = (yfrac >> 16) & FLAT_MASK;
        let flat_idx = (row * FLAT_DIM as u32 + col) as usize;
        let raw = p.source[flat_idx];
        fb.data[row_base + x] = p.colormap[raw as usize];
        xfrac = xfrac.wrapping_add(p.ds_xstep);
        yfrac = yfrac.wrapping_add(p.ds_ystep);
    }
}

/// Draw a flat-shaded (solid-color) span — used for testing and skies.
pub fn draw_span_solid(fb: &mut Framebuffer, y: usize, x1: usize, x2: usize, index: u8) {
    fb.draw_span(y, x1, x2, index);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::column::IDENTITY_COLORMAP;

    #[test]
    fn draw_span_solid_pixels() {
        let mut fb = Framebuffer::new();
        draw_span_solid(&mut fb, 50, 10, 20, 55);
        for x in 10..=20 {
            assert_eq!(fb.get_pixel(x, 50), Some(55));
        }
        assert_eq!(fb.get_pixel(9, 50), Some(0));
    }

    #[test]
    fn draw_span_textured_wraps_coords() {
        let mut fb = Framebuffer::new();
        let mut source = [0u8; FLAT_SIZE];
        // Fill the flat with value 7 everywhere.
        source.fill(7);
        let p = DrawSpanParams {
            y: 10, x1: 0, x2: 9,
            ds_xfrac: 0, ds_yfrac: 0,
            ds_xstep: 1 << 16, ds_ystep: 0,
            source: &source,
            colormap: &IDENTITY_COLORMAP,
        };
        draw_span(&mut fb, &p);
        for x in 0..=9 {
            assert_eq!(fb.get_pixel(x, 10), Some(7));
        }
    }

    #[test]
    fn flat_mask_and_dim_consistent() {
        assert_eq!(FLAT_DIM * FLAT_DIM, FLAT_SIZE);
        assert_eq!(FLAT_MASK, 63);
    }
}
