//! PLAYPAL palette lookup table.
//!
//! Doom stores 14 palettes in the PLAYPAL lump:
//! - Palette 0: normal
//! - Palettes 1–8: pain flash (reddening with each level)
//! - Palette 9–12: bonus item pickup flash
//! - Palette 13: radiation suit (green tint)
//!
//! The `PaletteLut` precomputes the full `[palette][color] → RGB` mapping
//! from the raw 10752-byte PLAYPAL lump so the widget can do a single
//! array index per pixel at blit time.
//!
//! # Layout of PLAYPAL lump
//! ```text
//! 14 palettes × 256 colors × 3 bytes (R, G, B) = 10752 bytes
//! ```

use doom_types::limits::{PLAYPAL_COLORS, PLAYPAL_COUNT};
use thiserror::Error;

/// Expected byte size of the PLAYPAL lump.
pub const PLAYPAL_SIZE: usize = PLAYPAL_COUNT * PLAYPAL_COLORS * 3;

/// Errors from palette construction.
#[derive(Debug, Error)]
pub enum PaletteError {
    #[error("PLAYPAL lump is {actual} bytes; expected {PLAYPAL_SIZE}")]
    /// The `PLAYPAL` lump did not match the expected size of 10,752 bytes.
    ///
    /// Doom expects exactly 14 palettes, each with 256 RGB tuples (14 * 256 * 3 = 10,752).
    /// If this fails, the WAD is likely corrupted or uses a different palette specification.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::palette::{PaletteError, PaletteLut};
    /// let bad_data = vec![0u8; 100];
    /// assert!(matches!(PaletteLut::from_playpal(&bad_data), Err(PaletteError::WrongSize { actual: 100 })));
    /// ```
    WrongSize {
        /// The actual size of the parsed lump.
        actual: usize,
    },
}

/// RGB color triple.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb {
    /// The red channel component (0-255).
    pub r: u8,
    /// The green channel component (0-255).
    pub g: u8,
    /// The blue channel component (0-255).
    pub b: u8,
}

impl Rgb {
    /// Pure black color constant.
    ///
    /// Used frequently as a fallback color for unmapped indices or void rendering.
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    /// Pure white color constant.
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    /// Creates a new `Rgb` color from the given red, green, and blue components.
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::palette::Rgb;
    /// let purple = Rgb::new(128, 0, 128);
    /// assert_eq!(purple.r, 128);
    /// assert_eq!(purple.g, 0);
    /// assert_eq!(purple.b, 128);
    /// ```
    #[inline]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Precomputed palette lookup table: `[palette_idx][color_idx] → Rgb`.
///
/// Owned in a `Vec` rather than a const-sized array to avoid 10 KB on the stack.
#[derive(Clone)]
pub struct PaletteLut {
    /// Flat storage: `data[palette * 256 + color]` → `Rgb`.
    data: Vec<Rgb>,
    /// Number of palettes stored (always `PLAYPAL_COUNT = 14` for a real WAD).
    n_palettes: usize,
}

impl PaletteLut {
    /// Build from the raw PLAYPAL lump bytes.
    ///
    /// # Errors
    /// Returns `PaletteError::WrongSize` if the lump isn't exactly 10752 bytes.
    pub fn from_playpal(data: &[u8]) -> Result<Self, PaletteError> {
        if data.len() != PLAYPAL_SIZE {
            return Err(PaletteError::WrongSize { actual: data.len() });
        }
        let mut lut = Vec::with_capacity(PLAYPAL_COUNT * PLAYPAL_COLORS);
        for chunk in data.chunks_exact(3) {
            lut.push(Rgb::new(chunk[0], chunk[1], chunk[2]));
        }
        Ok(Self {
            data: lut,
            n_palettes: PLAYPAL_COUNT,
        })
    }

    /// Build a default grayscale LUT for testing (all 14 palettes identical).
    ///
    /// Color index `i` maps to `Rgb(i, i, i)`.
    pub fn grayscale() -> Self {
        let mut data = Vec::with_capacity(PLAYPAL_COUNT * PLAYPAL_COLORS);
        for _ in 0..PLAYPAL_COUNT {
            for i in 0..PLAYPAL_COLORS {
                data.push(Rgb::new(i as u8, i as u8, i as u8));
            }
        }
        Self {
            data,
            n_palettes: PLAYPAL_COUNT,
        }
    }

    /// Build a simple 4-color test LUT: 0=black, 1=red, 2=green, 3=blue, 4+=white.
    pub fn test_primary() -> Self {
        let mut data = Vec::with_capacity(PLAYPAL_COUNT * PLAYPAL_COLORS);
        for _ in 0..PLAYPAL_COUNT {
            for i in 0..PLAYPAL_COLORS {
                let rgb = match i {
                    0 => Rgb::BLACK,
                    1 => Rgb::new(255, 0, 0),
                    2 => Rgb::new(0, 255, 0),
                    3 => Rgb::new(0, 0, 255),
                    _ => Rgb::WHITE,
                };
                data.push(rgb);
            }
        }
        Self {
            data,
            n_palettes: PLAYPAL_COUNT,
        }
    }

    /// Look up `Rgb` for `(palette, color_index)`.
    ///
    /// Clamps `palette` to `[0, n_palettes)` silently.
    #[inline]
    pub fn get(&self, palette: usize, color: u8) -> Rgb {
        let pal = palette.min(self.n_palettes.saturating_sub(1));
        self.data[pal * PLAYPAL_COLORS + color as usize]
    }

    /// Return the 256-entry `Rgb` slice for `palette`, ready for direct `slice[color as usize]` indexing.
    ///
    /// Prefer this over repeated [`Self::get`] calls in tight rendering loops: computing the
    /// palette offset once and indexing directly eliminates the per-call `.min()` + multiply.
    ///
    /// Clamps `palette` to `[0, n_palettes)` silently.
    #[inline]
    pub fn palette_slice(&self, palette: usize) -> &[Rgb] {
        let pal = palette.min(self.n_palettes.saturating_sub(1));
        &self.data[pal * PLAYPAL_COLORS..(pal + 1) * PLAYPAL_COLORS]
    }

    /// Number of palettes.
    pub fn n_palettes(&self) -> usize {
        self.n_palettes
    }

    /// Expand a palette-indexed [`Framebuffer`](crate::framebuffer::Framebuffer)
    /// into packed 32-bit `0xFF_RR_GG_BB` (opaque ARGB) pixels.
    ///
    /// This is the **canonical** palette→ARGB conversion shared by presenters
    /// that need packed 32-bit pixels (e.g. the windowed `doom-present` host,
    /// which hands the result to `abrash`'s software presenter). The alpha byte
    /// is always `0xFF`. Each output pixel `y * 320 + x` corresponds to the same
    /// index in `fb.as_slice()`.
    ///
    /// `palette` is clamped to `[0, n_palettes)` (via [`Self::palette_slice`]).
    ///
    /// ## Examples
    /// ```
    /// use doom_renderer::framebuffer::Framebuffer;
    /// use doom_renderer::palette::PaletteLut;
    ///
    /// let lut = PaletteLut::test_primary(); // 0=black,1=red,2=green,3=blue
    /// let mut fb = Framebuffer::new();
    /// fb.set_pixel(0, 0, 1); // red
    /// fb.set_pixel(1, 0, 2); // green
    /// let argb = lut.expand_argb(&fb, 0);
    /// assert_eq!(argb[0], 0xFF_FF_00_00); // red
    /// assert_eq!(argb[1], 0xFF_00_FF_00); // green
    /// ```
    #[must_use]
    pub fn expand_argb(&self, fb: &crate::framebuffer::Framebuffer, palette: usize) -> Vec<u32> {
        let slice = self.palette_slice(palette);
        fb.as_slice()
            .iter()
            .map(|&idx| {
                let c = slice[idx as usize];
                0xFF00_0000 | (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b)
            })
            .collect()
    }
}

impl Default for PaletteLut {
    fn default() -> Self {
        Self::grayscale()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grayscale_lut_roundtrip() {
        let lut = PaletteLut::grayscale();
        for i in 0..=255u8 {
            let rgb = lut.get(0, i);
            assert_eq!(rgb.r, i);
            assert_eq!(rgb.g, i);
            assert_eq!(rgb.b, i);
        }
    }

    #[test]
    fn palette_clamping() {
        let lut = PaletteLut::grayscale();
        // Asking for palette 99 should not panic.
        let _ = lut.get(99, 0);
    }

    #[test]
    fn from_playpal_rejects_wrong_size() {
        let bad = vec![0u8; 100];
        assert!(
            PaletteError::WrongSize { actual: 100 }
                .to_string()
                .contains("100")
        );
        assert!(matches!(
            PaletteLut::from_playpal(&bad),
            Err(PaletteError::WrongSize { .. })
        ));
    }

    #[test]
    fn from_playpal_parses_correctly() {
        let mut raw = vec![0u8; PLAYPAL_SIZE];
        // Palette 0, color 0 = (10, 20, 30)
        raw[0] = 10;
        raw[1] = 20;
        raw[2] = 30;
        // Palette 1, color 0 = (40, 50, 60)
        raw[PLAYPAL_COLORS * 3] = 40;
        raw[PLAYPAL_COLORS * 3 + 1] = 50;
        raw[PLAYPAL_COLORS * 3 + 2] = 60;
        let lut = PaletteLut::from_playpal(&raw).expect("value must exist in test");
        assert_eq!(lut.get(0, 0), Rgb::new(10, 20, 30));
        assert_eq!(lut.get(1, 0), Rgb::new(40, 50, 60));
    }

    #[test]
    fn test_primary_lut_colors() {
        let lut = PaletteLut::test_primary();
        assert_eq!(lut.get(0, 0), Rgb::BLACK);
        assert_eq!(lut.get(0, 1), Rgb::new(255, 0, 0));
        assert_eq!(lut.get(0, 2), Rgb::new(0, 255, 0));
        assert_eq!(lut.get(0, 3), Rgb::new(0, 0, 255));
    }

    #[test]
    fn expand_argb_packs_known_indices() {
        use crate::framebuffer::Framebuffer;
        let lut = PaletteLut::test_primary(); // 0=black,1=red,2=green,3=blue,4+=white
        let mut fb = Framebuffer::new();
        fb.set_pixel(0, 0, 0); // black
        fb.set_pixel(1, 0, 1); // red
        fb.set_pixel(2, 0, 2); // green
        fb.set_pixel(3, 0, 3); // blue
        fb.set_pixel(4, 0, 4); // white
        let argb = lut.expand_argb(&fb, 0);
        assert_eq!(argb.len(), 64_000);
        assert_eq!(argb[0], 0xFF_00_00_00); // black, opaque alpha
        assert_eq!(argb[1], 0xFF_FF_00_00); // red
        assert_eq!(argb[2], 0xFF_00_FF_00); // green
        assert_eq!(argb[3], 0xFF_00_00_FF); // blue
        assert_eq!(argb[4], 0xFF_FF_FF_FF); // white
    }

    #[test]
    fn expand_argb_uses_selected_palette() {
        use crate::framebuffer::Framebuffer;
        // Build a LUT whose palette 0 and palette 1 differ for color index 0.
        let mut raw = vec![0u8; PLAYPAL_SIZE];
        // Palette 0, color 0 = (10, 20, 30)
        raw[0] = 10;
        raw[1] = 20;
        raw[2] = 30;
        // Palette 1, color 0 = (40, 50, 60)
        raw[PLAYPAL_COLORS * 3] = 40;
        raw[PLAYPAL_COLORS * 3 + 1] = 50;
        raw[PLAYPAL_COLORS * 3 + 2] = 60;
        let lut = PaletteLut::from_playpal(&raw).expect("valid playpal");
        let fb = Framebuffer::new(); // all index 0
        assert_eq!(lut.expand_argb(&fb, 0)[0], 0xFF_0A_14_1E);
        assert_eq!(lut.expand_argb(&fb, 1)[0], 0xFF_28_32_3C);
    }
}
