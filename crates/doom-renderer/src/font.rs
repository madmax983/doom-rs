//! Simple 8x8 bitmap font for rendering menu text and HUD messages.
//!
//! In Doom, menu text uses STCFN* lumps from the WAD. For now we provide
//! a hardcoded 8x8 font covering ASCII printable characters (32-126).
//! Each glyph is stored as 8 bytes, one per row, MSB = leftmost pixel.

use crate::framebuffer::Framebuffer;
use doom_types::FB_WIDTH;

/// A simple 8x8 bitmap font for rendering text onto the framebuffer.
pub struct BitmapFont {
    /// 8x8 bitmaps for characters 32..127 (96 glyphs).
    /// Each glyph is 8 bytes, one per row, MSB = leftmost pixel.
    glyphs: [[u8; 8]; 96],
    /// Character width in pixels.
    pub char_width: u8,
    /// Character height in pixels.
    pub char_height: u8,
}

impl BitmapFont {
    /// Create a new bitmap font with hardcoded 8x8 glyphs.
    ///
    /// Covers all ASCII printable characters (32-126). Lowercase letters
    /// are mapped to their uppercase equivalents.
    #[allow(clippy::too_many_lines)]
    pub fn new() -> Self {
        let mut glyphs = [[0u8; 8]; 96];

        // Space (32) - blank
        glyphs[0] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

        // ! (33)
        glyphs[1] = [
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
            0b0001_1000,
            0b0000_0000,
        ];

        // " (34)
        glyphs[2] = [
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
        ];

        // # (35)
        glyphs[3] = [
            0b0110_0110,
            0b0110_0110,
            0b1111_1111,
            0b0110_0110,
            0b1111_1111,
            0b0110_0110,
            0b0110_0110,
            0b0000_0000,
        ];

        // $ (36)
        glyphs[4] = [
            0b0001_1000,
            0b0011_1110,
            0b0110_0000,
            0b0011_1100,
            0b0000_0110,
            0b0111_1100,
            0b0001_1000,
            0b0000_0000,
        ];

        // % (37)
        glyphs[5] = [
            0b0110_0010,
            0b0110_0100,
            0b0000_1000,
            0b0001_0000,
            0b0010_0000,
            0b0100_1100,
            0b1000_1100,
            0b0000_0000,
        ];

        // & (38)
        glyphs[6] = [
            0b0011_1000,
            0b0110_1100,
            0b0011_1000,
            0b0111_0110,
            0b1101_1100,
            0b1100_1100,
            0b0111_0110,
            0b0000_0000,
        ];

        // ' (39)
        glyphs[7] = [
            0b0001_1000,
            0b0001_1000,
            0b0011_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
        ];

        // ( (40)
        glyphs[8] = [
            0b0000_1100,
            0b0001_1000,
            0b0011_0000,
            0b0011_0000,
            0b0011_0000,
            0b0001_1000,
            0b0000_1100,
            0b0000_0000,
        ];

        // ) (41)
        glyphs[9] = [
            0b0011_0000,
            0b0001_1000,
            0b0000_1100,
            0b0000_1100,
            0b0000_1100,
            0b0001_1000,
            0b0011_0000,
            0b0000_0000,
        ];

        // * (42)
        glyphs[10] = [
            0b0000_0000,
            0b0110_0110,
            0b0011_1100,
            0b1111_1111,
            0b0011_1100,
            0b0110_0110,
            0b0000_0000,
            0b0000_0000,
        ];

        // + (43)
        glyphs[11] = [
            0b0000_0000,
            0b0001_1000,
            0b0001_1000,
            0b0111_1110,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
            0b0000_0000,
        ];

        // , (44)
        glyphs[12] = [
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0001_1000,
            0b0001_1000,
            0b0011_0000,
        ];

        // - (45)
        glyphs[13] = [
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0111_1110,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
        ];

        // . (46)
        glyphs[14] = [
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
        ];

        // / (47)
        glyphs[15] = [
            0b0000_0010,
            0b0000_0110,
            0b0000_1100,
            0b0001_1000,
            0b0011_0000,
            0b0110_0000,
            0b0100_0000,
            0b0000_0000,
        ];

        // 0 (48)
        glyphs[16] = [
            0b0011_1100,
            0b0110_0110,
            0b0110_1110,
            0b0111_0110,
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // 1 (49)
        glyphs[17] = [
            0b0001_1000,
            0b0011_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0111_1110,
            0b0000_0000,
        ];

        // 2 (50)
        glyphs[18] = [
            0b0011_1100,
            0b0110_0110,
            0b0000_0110,
            0b0000_1100,
            0b0001_1000,
            0b0011_0000,
            0b0111_1110,
            0b0000_0000,
        ];

        // 3 (51)
        glyphs[19] = [
            0b0011_1100,
            0b0110_0110,
            0b0000_0110,
            0b0001_1100,
            0b0000_0110,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // 4 (52)
        glyphs[20] = [
            0b0000_1100,
            0b0001_1100,
            0b0011_1100,
            0b0110_1100,
            0b0111_1110,
            0b0000_1100,
            0b0000_1100,
            0b0000_0000,
        ];

        // 5 (53)
        glyphs[21] = [
            0b0111_1110,
            0b0110_0000,
            0b0111_1100,
            0b0000_0110,
            0b0000_0110,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // 6 (54)
        glyphs[22] = [
            0b0011_1100,
            0b0110_0000,
            0b0110_0000,
            0b0111_1100,
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // 7 (55)
        glyphs[23] = [
            0b0111_1110,
            0b0000_0110,
            0b0000_1100,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
        ];

        // 8 (56)
        glyphs[24] = [
            0b0011_1100,
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // 9 (57)
        glyphs[25] = [
            0b0011_1100,
            0b0110_0110,
            0b0110_0110,
            0b0011_1110,
            0b0000_0110,
            0b0000_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // : (58)
        glyphs[26] = [
            0b0000_0000,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
            0b0000_0000,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
        ];

        // ; (59)
        glyphs[27] = [
            0b0000_0000,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
            0b0000_0000,
            0b0001_1000,
            0b0001_1000,
            0b0011_0000,
        ];

        // < (60)
        glyphs[28] = [
            0b0000_0110,
            0b0000_1100,
            0b0001_1000,
            0b0011_0000,
            0b0001_1000,
            0b0000_1100,
            0b0000_0110,
            0b0000_0000,
        ];

        // = (61)
        glyphs[29] = [
            0b0000_0000,
            0b0000_0000,
            0b0111_1110,
            0b0000_0000,
            0b0111_1110,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
        ];

        // > (62)
        glyphs[30] = [
            0b0110_0000,
            0b0011_0000,
            0b0001_1000,
            0b0000_1100,
            0b0001_1000,
            0b0011_0000,
            0b0110_0000,
            0b0000_0000,
        ];

        // ? (63)
        glyphs[31] = [
            0b0011_1100,
            0b0110_0110,
            0b0000_0110,
            0b0000_1100,
            0b0001_1000,
            0b0000_0000,
            0b0001_1000,
            0b0000_0000,
        ];

        // @ (64)
        glyphs[32] = [
            0b0011_1100,
            0b0110_0110,
            0b0110_1110,
            0b0110_1010,
            0b0110_1110,
            0b0110_0000,
            0b0011_1100,
            0b0000_0000,
        ];

        // A (65)
        glyphs[33] = [
            0b0001_1000,
            0b0011_1100,
            0b0110_0110,
            0b0110_0110,
            0b0111_1110,
            0b0110_0110,
            0b0110_0110,
            0b0000_0000,
        ];

        // B (66)
        glyphs[34] = [
            0b0111_1100,
            0b0110_0110,
            0b0110_0110,
            0b0111_1100,
            0b0110_0110,
            0b0110_0110,
            0b0111_1100,
            0b0000_0000,
        ];

        // C (67)
        glyphs[35] = [
            0b0011_1100,
            0b0110_0110,
            0b0110_0000,
            0b0110_0000,
            0b0110_0000,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // D (68)
        glyphs[36] = [
            0b0111_1000,
            0b0110_1100,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0110_1100,
            0b0111_1000,
            0b0000_0000,
        ];

        // E (69)
        glyphs[37] = [
            0b0111_1110,
            0b0110_0000,
            0b0110_0000,
            0b0111_1100,
            0b0110_0000,
            0b0110_0000,
            0b0111_1110,
            0b0000_0000,
        ];

        // F (70)
        glyphs[38] = [
            0b0111_1110,
            0b0110_0000,
            0b0110_0000,
            0b0111_1100,
            0b0110_0000,
            0b0110_0000,
            0b0110_0000,
            0b0000_0000,
        ];

        // G (71)
        glyphs[39] = [
            0b0011_1100,
            0b0110_0110,
            0b0110_0000,
            0b0110_1110,
            0b0110_0110,
            0b0110_0110,
            0b0011_1110,
            0b0000_0000,
        ];

        // H (72)
        glyphs[40] = [
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0111_1110,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0000_0000,
        ];

        // I (73)
        glyphs[41] = [
            0b0111_1110,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0111_1110,
            0b0000_0000,
        ];

        // J (74)
        glyphs[42] = [
            0b0000_0110,
            0b0000_0110,
            0b0000_0110,
            0b0000_0110,
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // K (75)
        glyphs[43] = [
            0b0110_0110,
            0b0110_1100,
            0b0111_1000,
            0b0111_0000,
            0b0111_1000,
            0b0110_1100,
            0b0110_0110,
            0b0000_0000,
        ];

        // L (76)
        glyphs[44] = [
            0b0110_0000,
            0b0110_0000,
            0b0110_0000,
            0b0110_0000,
            0b0110_0000,
            0b0110_0000,
            0b0111_1110,
            0b0000_0000,
        ];

        // M (77)
        glyphs[45] = [
            0b1100_0110,
            0b1110_1110,
            0b1111_1110,
            0b1101_0110,
            0b1100_0110,
            0b1100_0110,
            0b1100_0110,
            0b0000_0000,
        ];

        // N (78)
        glyphs[46] = [
            0b0110_0110,
            0b0111_0110,
            0b0111_1110,
            0b0111_1110,
            0b0110_1110,
            0b0110_0110,
            0b0110_0110,
            0b0000_0000,
        ];

        // O (79)
        glyphs[47] = [
            0b0011_1100,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // P (80)
        glyphs[48] = [
            0b0111_1100,
            0b0110_0110,
            0b0110_0110,
            0b0111_1100,
            0b0110_0000,
            0b0110_0000,
            0b0110_0000,
            0b0000_0000,
        ];

        // Q (81)
        glyphs[49] = [
            0b0011_1100,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0110_1010,
            0b0110_1100,
            0b0011_0110,
            0b0000_0000,
        ];

        // R (82)
        glyphs[50] = [
            0b0111_1100,
            0b0110_0110,
            0b0110_0110,
            0b0111_1100,
            0b0110_1100,
            0b0110_0110,
            0b0110_0110,
            0b0000_0000,
        ];

        // S (83)
        glyphs[51] = [
            0b0011_1100,
            0b0110_0110,
            0b0110_0000,
            0b0011_1100,
            0b0000_0110,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // T (84)
        glyphs[52] = [
            0b0111_1110,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
        ];

        // U (85)
        glyphs[53] = [
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0000_0000,
        ];

        // V (86)
        glyphs[54] = [
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
        ];

        // W (87)
        glyphs[55] = [
            0b1100_0110,
            0b1100_0110,
            0b1100_0110,
            0b1101_0110,
            0b1111_1110,
            0b1110_1110,
            0b1100_0110,
            0b0000_0000,
        ];

        // X (88)
        glyphs[56] = [
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0001_1000,
            0b0011_1100,
            0b0110_0110,
            0b0110_0110,
            0b0000_0000,
        ];

        // Y (89)
        glyphs[57] = [
            0b0110_0110,
            0b0110_0110,
            0b0110_0110,
            0b0011_1100,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
        ];

        // Z (90)
        glyphs[58] = [
            0b0111_1110,
            0b0000_0110,
            0b0000_1100,
            0b0001_1000,
            0b0011_0000,
            0b0110_0000,
            0b0111_1110,
            0b0000_0000,
        ];

        // [ (91)
        glyphs[59] = [
            0b0011_1100,
            0b0011_0000,
            0b0011_0000,
            0b0011_0000,
            0b0011_0000,
            0b0011_0000,
            0b0011_1100,
            0b0000_0000,
        ];

        // \ (92)
        glyphs[60] = [
            0b0100_0000,
            0b0110_0000,
            0b0011_0000,
            0b0001_1000,
            0b0000_1100,
            0b0000_0110,
            0b0000_0010,
            0b0000_0000,
        ];

        // ] (93)
        glyphs[61] = [
            0b0011_1100,
            0b0000_1100,
            0b0000_1100,
            0b0000_1100,
            0b0000_1100,
            0b0000_1100,
            0b0011_1100,
            0b0000_0000,
        ];

        // ^ (94)
        glyphs[62] = [
            0b0001_1000,
            0b0011_1100,
            0b0110_0110,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
        ];

        // _ (95)
        glyphs[63] = [
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b1111_1111,
            0b0000_0000,
        ];

        // ` (96)
        glyphs[64] = [
            0b0001_1000,
            0b0001_1000,
            0b0000_1100,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
        ];

        // Lowercase a-z (97-122) map to uppercase A-Z (glyphs 33..59)
        for i in 0..26 {
            glyphs[65 + i] = glyphs[33 + i]; // a=65+0 maps to A=33+0
        }

        // { (123)
        glyphs[91] = [
            0b0000_1100,
            0b0001_1000,
            0b0001_1000,
            0b0011_0000,
            0b0001_1000,
            0b0001_1000,
            0b0000_1100,
            0b0000_0000,
        ];

        // | (124)
        glyphs[92] = [
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0001_1000,
            0b0000_0000,
        ];

        // } (125)
        glyphs[93] = [
            0b0011_0000,
            0b0001_1000,
            0b0001_1000,
            0b0000_1100,
            0b0001_1000,
            0b0001_1000,
            0b0011_0000,
            0b0000_0000,
        ];

        // ~ (126)
        glyphs[94] = [
            0b0000_0000,
            0b0011_0010,
            0b0111_1110,
            0b0100_1100,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
            0b0000_0000,
        ];

        // DEL (127) placeholder — blank
        glyphs[95] = [0x00; 8];

        Self {
            glyphs,
            char_width: 8,
            char_height: 8,
        }
    }

    /// Access the raw glyph data for a given index (0..96).
    ///
    /// Returns the 8-byte bitmap for the glyph. If `idx` is out of range,
    /// returns the space glyph (all zeros).
    #[inline]
    pub fn glyph(&self, idx: usize) -> &[u8; 8] {
        if idx < 96 {
            &self.glyphs[idx]
        } else {
            &self.glyphs[0] // space fallback
        }
    }

    /// Draw a single character at `(x, y)` with the given palette color.
    ///
    /// Transparent pixels (where the bitmap bit is 0) are skipped.
    /// Out-of-bounds coordinates are silently ignored per `Framebuffer::set_pixel`.
    pub fn draw_char(&self, fb: &mut Framebuffer, x: i32, y: i32, ch: u8, color: u8) {
        let idx = if (32..128).contains(&ch) {
            (ch - 32) as usize
        } else {
            0 // unmapped characters render as space
        };

        let glyph = &self.glyphs[idx];
        for (row, &bits) in glyph.iter().enumerate() {
            for col in 0..8 {
                if bits & (0x80 >> col) != 0 {
                    let px = x + col;
                    let py = y + row as i32;
                    if px >= 0 && py >= 0 {
                        fb.set_pixel(px as usize, py as usize, color);
                    }
                }
            }
        }
    }

    /// Draw a string starting at `(x, y)`, advancing by `char_width` per character.
    ///
    /// Characters outside the printable ASCII range are rendered as spaces.
    pub fn draw_string(&self, fb: &mut Framebuffer, x: i32, y: i32, text: &str, color: u8) {
        for (i, ch) in text.bytes().enumerate() {
            let cx = x + (i as i32) * i32::from(self.char_width);
            self.draw_char(fb, cx, y, ch, color);
        }
    }

    /// Calculate the pixel width of a string.
    #[inline]
    pub fn string_width(&self, text: &str) -> i32 {
        (text.len() as i32).saturating_mul(i32::from(self.char_width))
    }

    /// Draw a string centered horizontally on the 320-pixel-wide framebuffer.
    pub fn draw_string_centered(&self, fb: &mut Framebuffer, y: i32, text: &str, color: u8) {
        let w = self.string_width(text);
        let x = (FB_WIDTH as i32 - w) / 2;
        self.draw_string(fb, x, y, text, color);
    }
}

impl Default for BitmapFont {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod havoc_tests {
    use super::*;

    #[test]
    fn test_font_string_width_overflow() {
        let font = BitmapFont::new();
        // A huge string causes an i32 overflow if simply multiplied
        let s = "A".repeat(300_000_000);
        let _w = font.string_width(&s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_font() -> BitmapFont {
        BitmapFont::new()
    }

    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    #[test]
    fn new_creates_valid_font() {
        let font = make_font();
        assert_eq!(font.char_width, 8);
        assert_eq!(font.char_height, 8);
        assert_eq!(font.glyphs.len(), 96);
    }

    #[test]
    fn default_creates_same_as_new() {
        let a = BitmapFont::new();
        let b = BitmapFont::default();
        assert_eq!(a.char_width, b.char_width);
        assert_eq!(a.char_height, b.char_height);
        assert_eq!(a.glyphs, b.glyphs);
    }

    // -----------------------------------------------------------------------
    // Glyph access
    // -----------------------------------------------------------------------

    #[test]
    fn space_glyph_is_blank() {
        let font = make_font();
        let glyph = font.glyph(0); // space = char 32, index 0
        assert!(
            glyph.iter().all(|&b| b == 0),
            "Space glyph should be all zeros"
        );
    }

    #[test]
    fn a_glyph_has_correct_bitmap() {
        let font = make_font();
        // 'A' = char 65, index 33
        let glyph = font.glyph(33);
        assert_eq!(glyph[0], 0b0001_1000, "Row 0 of 'A'");
        assert_eq!(glyph[1], 0b0011_1100, "Row 1 of 'A'");
        assert_eq!(glyph[2], 0b0110_0110, "Row 2 of 'A'");
        assert_eq!(glyph[4], 0b0111_1110, "Row 4 of 'A' (crossbar)");
    }

    #[test]
    fn all_ascii_32_to_126_have_glyphs() {
        let font = make_font();
        // Ensure no index panics and all return valid references.
        for ch in 32u8..=126 {
            let idx = (ch - 32) as usize;
            let glyph = font.glyph(idx);
            assert_eq!(glyph.len(), 8, "Glyph for char {ch} should have 8 rows");
        }
    }

    #[test]
    fn glyph_out_of_range_returns_space() {
        let font = make_font();
        let glyph = font.glyph(200);
        assert!(
            glyph.iter().all(|&b| b == 0),
            "Out-of-range should return space glyph"
        );
    }

    #[test]
    fn lowercase_maps_to_uppercase() {
        let font = make_font();
        // 'a' = char 97, index 65;  'A' = char 65, index 33
        let upper_a = font.glyph(33);
        let lower_a = font.glyph(65);
        assert_eq!(
            upper_a, lower_a,
            "Lowercase 'a' should map to uppercase 'A'"
        );
    }

    #[test]
    fn all_lowercase_maps_to_uppercase() {
        let font = make_font();
        for offset in 0..26 {
            let upper = font.glyph(33 + offset); // A=33, B=34, ...
            let lower = font.glyph(65 + offset); // a=65, b=66, ...
            assert_eq!(
                upper, lower,
                "Lowercase letter at offset {offset} should map to uppercase"
            );
        }
    }

    #[test]
    fn digit_glyphs_are_non_blank() {
        let font = make_font();
        for digit in 0..10 {
            let idx = 16 + digit; // '0' = char 48, index 16
            let glyph = font.glyph(idx);
            let total_bits: u32 = glyph.iter().map(|&b| b.count_ones()).sum();
            assert!(total_bits > 0, "Digit {digit} glyph should not be blank");
        }
    }

    #[test]
    fn punctuation_glyphs_are_non_blank() {
        let font = make_font();
        let chars = [b'!', b'.', b',', b'?', b':', b'-', b'\'', b'(', b')', b'/'];
        for &ch in &chars {
            let idx = (ch - 32) as usize;
            let glyph = font.glyph(idx);
            let total_bits: u32 = glyph.iter().map(|&b| b.count_ones()).sum();
            assert!(
                total_bits > 0,
                "Punctuation '{}' should not be blank",
                ch as char
            );
        }
    }

    // -----------------------------------------------------------------------
    // draw_char
    // -----------------------------------------------------------------------

    #[test]
    fn draw_char_sets_pixels() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        // Draw 'A' at (10, 10) with color 42
        font.draw_char(&mut fb, 10, 10, b'A', 42);

        // 'A' row 0 = 0b0001_1000 → bits at cols 3,4
        // So pixels at (10+3, 10) and (10+4, 10) should be set.
        assert_eq!(
            fb.get_pixel(13, 10),
            Some(42),
            "Pixel at (13,10) should be color 42"
        );
        assert_eq!(
            fb.get_pixel(14, 10),
            Some(42),
            "Pixel at (14,10) should be color 42"
        );
        // Pixel at (10,10) should NOT be set (bit 7 of row 0 is 0).
        assert_eq!(
            fb.get_pixel(10, 10),
            Some(0),
            "Pixel at (10,10) should still be 0"
        );
    }

    #[test]
    fn draw_char_with_color_sets_correct_palette_index() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        font.draw_char(&mut fb, 0, 0, b'!', 200);

        // '!' row 0 = 0b0001_1000 → cols 3,4
        assert_eq!(fb.get_pixel(3, 0), Some(200));
        assert_eq!(fb.get_pixel(4, 0), Some(200));
    }

    #[test]
    fn draw_char_out_of_bounds_does_not_panic() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        // Negative coordinates — should not panic.
        font.draw_char(&mut fb, -5, -5, b'X', 10);
        // Way off screen.
        font.draw_char(&mut fb, 400, 300, b'X', 10);
        // Partially off screen.
        font.draw_char(&mut fb, 316, 196, b'X', 10);
    }

    #[test]
    fn draw_char_space_sets_no_pixels() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        font.draw_char(&mut fb, 50, 50, b' ', 99);

        // Check that no pixels in the 8x8 region were set.
        for dy in 0..8 {
            for dx in 0..8 {
                assert_eq!(
                    fb.get_pixel(50 + dx, 50 + dy),
                    Some(0),
                    "Space should set no pixels at ({}, {})",
                    50 + dx,
                    50 + dy
                );
            }
        }
    }

    #[test]
    fn draw_char_unprintable_renders_as_space() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        // Control character (char 1) should render as space (blank).
        font.draw_char(&mut fb, 20, 20, 1, 50);
        for dy in 0..8 {
            for dx in 0..8 {
                assert_eq!(fb.get_pixel(20 + dx, 20 + dy), Some(0));
            }
        }
    }

    // -----------------------------------------------------------------------
    // draw_string
    // -----------------------------------------------------------------------

    #[test]
    fn draw_string_renders_multiple_chars() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        font.draw_string(&mut fb, 10, 10, "AB", 7);

        // 'A' at x=10 — check a known pixel (row 0, col 3 → x=13)
        assert_eq!(fb.get_pixel(13, 10), Some(7));
        // 'B' at x=18 — 'B' row 0 = 0b0111_1100 → col 1 → x=18+1=19
        assert_eq!(fb.get_pixel(19, 10), Some(7));
    }

    #[test]
    fn draw_string_empty_sets_no_pixels() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        let before = fb.data.clone();
        font.draw_string(&mut fb, 10, 10, "", 7);
        assert_eq!(
            &*fb.data, &*before,
            "Empty string should not modify framebuffer"
        );
    }

    // -----------------------------------------------------------------------
    // string_width
    // -----------------------------------------------------------------------

    #[test]
    fn string_width_returns_correct_pixel_width() {
        let font = make_font();
        assert_eq!(font.string_width("DOOM"), 32); // 4 chars * 8 = 32
        assert_eq!(font.string_width("A"), 8);
        assert_eq!(font.string_width(""), 0);
        assert_eq!(font.string_width("Hello World"), 88); // 11 * 8 = 88
    }

    #[test]
    fn string_width_matches_char_width_times_len() {
        let font = make_font();
        let text = "TESTING 123";
        let expected = text.len() as i32 * 8;
        assert_eq!(font.string_width(text), expected);
    }

    // -----------------------------------------------------------------------
    // draw_string_centered
    // -----------------------------------------------------------------------

    #[test]
    fn draw_string_centered_centers_on_320px() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        // "DOOM" = 4 chars * 8 = 32px wide.
        // Center: (320 - 32) / 2 = 144.
        font.draw_string_centered(&mut fb, 10, "DOOM", 5);

        // 'D' starts at x=144. D row 0 = 0b0111_1000 → col 1 → x=144+1=145
        assert_eq!(fb.get_pixel(145, 10), Some(5));
    }

    #[test]
    fn draw_string_centered_single_char() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        // "X" = 1 char * 8 = 8px. Center: (320-8)/2 = 156
        font.draw_string_centered(&mut fb, 50, "X", 3);

        // 'X' row 0 = 0b0110_0110 → col 1 → x=156+1=157
        assert_eq!(fb.get_pixel(157, 50), Some(3));
    }

    #[test]
    fn draw_string_centered_long_text() {
        let font = make_font();
        let mut fb = Framebuffer::new();
        // 40 chars = 320px → x = 0
        let text = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        assert_eq!(text.len(), 40);
        font.draw_string_centered(&mut fb, 0, text, 1);
        // First 'A' at x=0, row 0 col 3 → pixel at (3, 0)
        assert_eq!(fb.get_pixel(3, 0), Some(1));
    }
}
