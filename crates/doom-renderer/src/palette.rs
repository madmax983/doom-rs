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
/// Indicates a fatal mismatch reading the `PLAYPAL` lump.
///
/// Doom expects `PLAYPAL` to contain exactly 14 palettes, each with 256 colors,
/// where each color is a 3-byte `(R, G, B)` sequence. Total bytes must be 10,752.
pub enum PaletteError {
    #[error("PLAYPAL lump is {actual} bytes; expected {PLAYPAL_SIZE}")]
    #[doc(hidden)]
    WrongSize { actual: usize },
}

/// An unpacked 24-bit TrueColor pixel representation.
///
/// Under the hood, the entire `doom-renderer` engine only deals in 8-bit palette indices.
/// It doesn't actually know what 'red' or 'green' means. This struct exists solely at the
/// final blit stage where those 8-bit integers are translated through the active palette
/// into full-color triples suitable for the terminal or an OS window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rgb {
    #[doc(hidden)]
    pub r: u8,
    #[doc(hidden)]
    pub g: u8,
    #[doc(hidden)]
    pub b: u8,
}

impl Rgb {
    #[doc(hidden)]
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };
    #[doc(hidden)]
    pub const WHITE: Self = Self {
        r: 255,
        g: 255,
        b: 255,
    };

    /// Translates raw `r`, `g`, `b` bytes into a standard `Rgb` bundle.
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
        let lut = PaletteLut::from_playpal(&raw).unwrap();
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
}
