//! WAD-based HU font renderer using STCFN patch lumps.
//!
//! Doom stores individual character glyphs as picture-format patches named
//! `STCFN033` through `STCFN095` (ASCII 33 '!' through 95 '_').
//! Space (ASCII 32) has no patch — callers advance by `SPACE_WIDTH` pixels.
//!
//! Glyph widths vary (unlike the hardcoded 8×8 `BitmapFont`), so `string_width`
//! must iterate glyphs to compute layout.

use doom_wad::WadStack;

use crate::framebuffer::Framebuffer;
use crate::patch_cache::PatchCache;
use crate::texture_compose::PatchImage;

/// Space glyph advance width in pixels.
pub const SPACE_WIDTH: i32 = 4;

/// Extra pixels between consecutive glyphs.
pub const GLYPH_GAP: i32 = 1;

/// Parsed WAD font: STCFN033–STCFN095 glyphs.
pub struct WadFont {
    /// Glyphs for ASCII 33–95. Index 0 = '!', index 62 = '_'.
    glyphs: Vec<Option<PatchImage>>,
}

impl WadFont {
    /// Load the HU font glyphs from the WAD via `patch_cache`.
    pub fn load(cache: &mut PatchCache, wad: &WadStack) -> Self {
        let mut glyphs = Vec::with_capacity(63);
        for ascii in 33u8..=95 {
            let name = format!("STCFN{ascii:03}");
            // Clone so we're not fighting borrow checker with the cache.
            let patch = cache.get(&name, wad).cloned();
            glyphs.push(patch);
        }
        Self { glyphs }
    }

    /// Return the glyph for ASCII character `ch`, or `None` for missing/space.
    fn glyph(&self, ch: u8) -> Option<&PatchImage> {
        if !(33..=95).contains(&ch) {
            return None;
        }
        self.glyphs[(ch - 33) as usize].as_ref()
    }

    /// Total pixel width of `text` using WAD glyph metrics.
    ///
    /// Space contributes `SPACE_WIDTH`. Unknown chars are treated as space.
    pub fn string_width(&self, text: &str) -> i32 {
        let mut width = 0i32;
        for ch in text.bytes() {
            let ch_upper = ch.to_ascii_uppercase();
            if ch_upper == b' ' {
                width += SPACE_WIDTH + GLYPH_GAP;
            } else if let Some(g) = self.glyph(ch_upper) {
                width += g.width as i32 + GLYPH_GAP;
            } else {
                width += SPACE_WIDTH + GLYPH_GAP;
            }
        }
        // Remove trailing gap.
        if width > 0 {
            width -= GLYPH_GAP;
        }
        width
    }

    /// Draw `text` onto `fb` at `(x, y)`.
    ///
    /// Advances x per glyph width.  Lowercase is mapped to uppercase.
    pub fn draw_string(&self, fb: &mut Framebuffer, mut x: i32, y: i32, text: &str) {
        for ch in text.bytes() {
            let ch_upper = ch.to_ascii_uppercase();
            if ch_upper == b' ' {
                x += SPACE_WIDTH + GLYPH_GAP;
                continue;
            }
            if let Some(g) = self.glyph(ch_upper) {
                fb.draw_patch(x, y, g);
                x += g.width as i32 + GLYPH_GAP;
            } else {
                x += SPACE_WIDTH + GLYPH_GAP;
            }
        }
    }

    /// Draw `text` centered horizontally at `y`.
    pub fn draw_string_centered(&self, fb: &mut Framebuffer, y: i32, text: &str) {
        let w = self.string_width(text);
        let x = (320 - w) / 2;
        self.draw_string(fb, x, y, text);
    }

    /// `true` if at least one glyph was loaded successfully.
    pub fn is_loaded(&self) -> bool {
        self.glyphs.iter().any(|g| g.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_font() -> WadFont {
        WadFont {
            glyphs: vec![None; 63],
        }
    }

    #[test]
    fn space_width_for_empty_font() {
        let font = empty_font();
        assert_eq!(font.string_width(" "), SPACE_WIDTH);
    }

    #[test]
    fn single_char_width_is_zero_for_missing_glyph() {
        let font = empty_font();
        // '!' maps to glyph index 0, which is None.
        assert_eq!(font.string_width("!"), SPACE_WIDTH);
    }

    #[test]
    fn empty_string_width_is_zero() {
        let font = empty_font();
        assert_eq!(font.string_width(""), 0);
    }

    #[test]
    fn is_loaded_false_for_empty_font() {
        let font = empty_font();
        assert!(!font.is_loaded());
    }

    #[test]
    fn string_width_with_valid_glyphs() {
        let mut glyphs = vec![None; 63];
        // 'A' is ascii 65. 65 - 33 = 32
        glyphs[32] = Some(PatchImage {
            width: 10,
            height: 10,
            left_offset: 0,
            top_offset: 0,
            columns: vec![],
        });
        // 'B' is ascii 66. 66 - 33 = 33
        glyphs[33] = Some(PatchImage {
            width: 15,
            height: 10,
            left_offset: 0,
            top_offset: 0,
            columns: vec![],
        });

        let font = WadFont { glyphs };

        // "A B" = width(A) + GLYPH_GAP + width(space) + GLYPH_GAP + width(B)
        // 10 + 1 + 4 + 1 + 15 = 31
        assert_eq!(font.string_width("A B"), 31);
    }

    #[test]
    fn draw_string_centered() {
        let mut glyphs = vec![None; 63];
        // 'A' is ascii 65. 65 - 33 = 32
        glyphs[32] = Some(PatchImage {
            width: 10,
            height: 10,
            left_offset: 0,
            top_offset: 0,
            columns: vec![],
        });

        let font = WadFont { glyphs };

        let mut fb = Framebuffer::new();
        // Draw "A". width = 10. Center x = (320 - 10) / 2 = 155
        font.draw_string_centered(&mut fb, 10, "A");
    }

    #[test]
    fn is_loaded_true_when_has_glyphs() {
        let mut glyphs = vec![None; 63];
        glyphs[32] = Some(PatchImage {
            width: 10,
            height: 10,
            left_offset: 0,
            top_offset: 0,
            columns: vec![],
        });
        let font = WadFont { glyphs };
        assert!(font.is_loaded());
    }
}
