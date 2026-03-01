//! Palette-indexed 320×200 framebuffer — the canvas Doom renders into.
//!
//! Each byte is an index (0–255) into the active PLAYPAL palette.
//! RGB conversion happens at blit time in `doom-tui`, never during rendering.
//! This keeps the game sim and renderer free of any color-space concerns.

use doom_types::limits::{FB_HEIGHT, FB_SIZE, FB_WIDTH};

/// The primary render target: 320×200 palette-indexed pixels.
///
/// Pixel `(x, y)` lives at index `y * FB_WIDTH + x`.
/// All coordinates are in the range `[0, 320) × [0, 200)`.
#[derive(Clone)]
pub struct Framebuffer {
    /// Raw palette indices, row-major, top-left origin.
    pub data: Box<[u8; FB_SIZE]>,
}

impl Framebuffer {
    /// Create a new framebuffer filled with palette index 0 (black).
    pub fn new() -> Self {
        Self {
            data: Box::new([0u8; FB_SIZE]),
        }
    }

    /// Width in pixels.
    pub const fn width() -> usize { FB_WIDTH }

    /// Height in pixels.
    pub const fn height() -> usize { FB_HEIGHT }

    /// Clear the entire framebuffer to a single palette index.
    #[inline]
    pub fn clear(&mut self, index: u8) {
        self.data.fill(index);
    }

    /// Set a single pixel. Silently ignores out-of-bounds coordinates.
    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, index: u8) {
        if x < FB_WIDTH && y < FB_HEIGHT {
            self.data[y * FB_WIDTH + x] = index;
        }
    }

    /// Get the palette index at `(x, y)`. Returns `None` if out of bounds.
    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> Option<u8> {
        if x < FB_WIDTH && y < FB_HEIGHT {
            Some(self.data[y * FB_WIDTH + x])
        } else {
            None
        }
    }

    /// Fill a rectangular region with a palette index.
    ///
    /// Coordinates are clamped to the framebuffer bounds.
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, index: u8) {
        let x_end = (x + w).min(FB_WIDTH);
        let y_end = (y + h).min(FB_HEIGHT);
        for row in y..y_end {
            let start = row * FB_WIDTH + x;
            let end   = row * FB_WIDTH + x_end;
            self.data[start..end].fill(index);
        }
    }

    /// Draw a vertical column of pixels from `y_top` to `y_bot` (inclusive).
    ///
    /// Used by `R_DrawColumn` — the innermost renderer loop.
    #[inline]
    pub fn draw_column(&mut self, x: usize, y_top: usize, y_bot: usize, index: u8) {
        if x >= FB_WIDTH {
            return;
        }
        let top = y_top.min(FB_HEIGHT - 1);
        let bot = (y_bot + 1).min(FB_HEIGHT);
        for y in top..bot {
            self.data[y * FB_WIDTH + x] = index;
        }
    }

    /// Draw a horizontal span of pixels from `x_start` to `x_end` (inclusive) at row `y`.
    ///
    /// Used by `R_DrawSpan` — floor/ceiling fill.
    #[inline]
    pub fn draw_span(&mut self, y: usize, x_start: usize, x_end: usize, index: u8) {
        if y >= FB_HEIGHT {
            return;
        }
        let start = y * FB_WIDTH + x_start.min(FB_WIDTH);
        let end   = y * FB_WIDTH + (x_end + 1).min(FB_WIDTH);
        if start < end {
            self.data[start..end].fill(index);
        }
    }

    /// Return an immutable view of the raw pixel data.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.data.as_slice()
    }
}

impl Default for Framebuffer {
    fn default() -> Self { Self::new() }
}

impl core::fmt::Debug for Framebuffer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Framebuffer({}×{})", FB_WIDTH, FB_HEIGHT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_all_zeros() {
        let fb = Framebuffer::new();
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    #[test]
    fn clear_sets_all_pixels() {
        let mut fb = Framebuffer::new();
        fb.clear(42);
        assert!(fb.data.iter().all(|&b| b == 42));
    }

    #[test]
    fn set_get_pixel_roundtrip() {
        let mut fb = Framebuffer::new();
        fb.set_pixel(10, 20, 99);
        assert_eq!(fb.get_pixel(10, 20), Some(99));
    }

    #[test]
    fn out_of_bounds_set_is_noop() {
        let mut fb = Framebuffer::new();
        fb.set_pixel(320, 0, 1);  // x == FB_WIDTH
        fb.set_pixel(0, 200, 1);  // y == FB_HEIGHT
        // No panic, no change to valid pixels.
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    #[test]
    fn out_of_bounds_get_returns_none() {
        let fb = Framebuffer::new();
        assert_eq!(fb.get_pixel(320, 0), None);
        assert_eq!(fb.get_pixel(0, 200), None);
    }

    #[test]
    fn fill_rect_covers_region() {
        let mut fb = Framebuffer::new();
        fb.fill_rect(10, 10, 20, 20, 7);
        for y in 10..30 {
            for x in 10..30 {
                assert_eq!(fb.get_pixel(x, y), Some(7), "({x},{y}) should be 7");
            }
        }
        // Pixels outside the rect are unchanged.
        assert_eq!(fb.get_pixel(9, 10), Some(0));
        assert_eq!(fb.get_pixel(10, 9), Some(0));
    }

    #[test]
    fn fill_rect_clamps_to_bounds() {
        let mut fb = Framebuffer::new();
        // Start within bounds but extends past the edge.
        fb.fill_rect(310, 190, 100, 100, 5);
        // Should not panic; pixels in [310..320, 190..200] are set.
        assert_eq!(fb.get_pixel(315, 195), Some(5));
        assert_eq!(fb.get_pixel(319, 199), Some(5));
    }

    #[test]
    fn draw_column_sets_vertical_pixels() {
        let mut fb = Framebuffer::new();
        fb.draw_column(50, 10, 20, 3);
        for y in 10..=20 {
            assert_eq!(fb.get_pixel(50, y), Some(3));
        }
        assert_eq!(fb.get_pixel(50, 9), Some(0));
        assert_eq!(fb.get_pixel(50, 21), Some(0));
    }

    #[test]
    fn draw_span_sets_horizontal_pixels() {
        let mut fb = Framebuffer::new();
        fb.draw_span(100, 50, 60, 9);
        for x in 50..=60 {
            assert_eq!(fb.get_pixel(x, 100), Some(9));
        }
        assert_eq!(fb.get_pixel(49, 100), Some(0));
        assert_eq!(fb.get_pixel(61, 100), Some(0));
    }

    #[test]
    fn framebuffer_size_matches_constants() {
        assert_eq!(FB_SIZE, FB_WIDTH * FB_HEIGHT);
        assert_eq!(FB_SIZE, 64_000);
    }
}
