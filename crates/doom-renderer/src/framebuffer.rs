//! Palette-indexed 320×200 framebuffer — the canvas Doom renders into.
//!
//! Each byte is an index (0–255) into the active PLAYPAL palette.
//! RGB conversion happens at blit time in `doom-tui`, never during rendering.
//! This keeps the game sim and renderer free of any color-space concerns.

use crate::texture_compose::PatchImage;
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
    pub const fn width() -> usize {
        FB_WIDTH
    }

    /// Height in pixels.
    pub const fn height() -> usize {
        FB_HEIGHT
    }

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
            let end = row * FB_WIDTH + x_end;
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
        let end = y * FB_WIDTH + (x_end + 1).min(FB_WIDTH);
        if start < end {
            self.data[start..end].fill(index);
        }
    }

    /// Draw a Doom picture-format patch at screen position `(x, y)`.
    ///
    /// `x` and `y` are the screen coordinates of the patch origin **after**
    /// applying `left_offset` / `top_offset` (i.e. callers pass the adjusted
    /// position).  Transparent pixels (gaps between posts) are skipped.
    /// Pixels that land outside the 320×200 screen are silently clipped.
    pub fn draw_patch(&mut self, x: i32, y: i32, patch: &PatchImage) {
        for (col, posts) in patch.columns.iter().enumerate() {
            let px = x + col as i32;
            if px < 0 || px >= FB_WIDTH as i32 {
                continue;
            }
            let px = px as usize;
            for post in posts {
                for (row, &color) in post.pixels.iter().enumerate() {
                    let py = y + post.y_offset as i32 + row as i32;
                    if py >= 0 && py < FB_HEIGHT as i32 {
                        self.data[py as usize * FB_WIDTH + px] = color;
                    }
                }
            }
        }
    }

    /// Draw a patch centered horizontally at the given `y` coordinate.
    pub fn draw_patch_centered(&mut self, y: i32, patch: &PatchImage) {
        let x = (FB_WIDTH as i32 - patch.width as i32) / 2;
        self.draw_patch(x, y, patch);
    }

    /// Return an immutable view of the raw pixel data.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.data.as_slice()
    }
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
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
        fb.set_pixel(320, 0, 1); // x == FB_WIDTH
        fb.set_pixel(0, 200, 1); // y == FB_HEIGHT
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

    // -----------------------------------------------------------------------
    // draw_patch
    // -----------------------------------------------------------------------

    fn make_patch(width: u16, height: u16, color: u8) -> PatchImage {
        use crate::texture_compose::PatchPost;
        // One solid post per column covering the full height.
        let columns = (0..width)
            .map(|_| {
                vec![PatchPost {
                    y_offset: 0,
                    pixels: vec![color; height as usize],
                }]
            })
            .collect();
        PatchImage {
            width,
            height,
            left_offset: 0,
            top_offset: 0,
            columns,
        }
    }

    #[test]
    fn draw_patch_solid_rect() {
        let mut fb = Framebuffer::new();
        let patch = make_patch(10, 8, 42);
        fb.draw_patch(5, 10, &patch);
        for y in 10..18usize {
            for x in 5..15usize {
                assert_eq!(fb.get_pixel(x, y), Some(42), "({x},{y}) should be 42");
            }
        }
        // Outside left edge
        assert_eq!(fb.get_pixel(4, 10), Some(0));
    }

    #[test]
    fn draw_patch_clips_negative_x() {
        let mut fb = Framebuffer::new();
        let patch = make_patch(4, 4, 7);
        // Draw at x=-2: only columns 2 and 3 should appear (screen x=0,1).
        fb.draw_patch(-2, 0, &patch);
        assert_eq!(fb.get_pixel(0, 0), Some(7));
        assert_eq!(fb.get_pixel(1, 0), Some(7));
        assert_eq!(fb.get_pixel(2, 0), Some(0)); // column 4 would be x=2, out of original width
    }

    #[test]
    fn draw_patch_clips_negative_y() {
        let mut fb = Framebuffer::new();
        let patch = make_patch(2, 4, 9);
        // y=-2: rows 0,1 off screen, rows 2,3 land at y=0,1.
        fb.draw_patch(0, -2, &patch);
        assert_eq!(fb.get_pixel(0, 0), Some(9));
        assert_eq!(fb.get_pixel(0, 1), Some(9));
    }

    #[test]
    fn draw_patch_transparent_gap() {
        use crate::texture_compose::PatchPost;
        // A patch with a gap: post at y=0 (2px), then post at y=4 (2px).
        let patch = PatchImage {
            width: 1,
            height: 6,
            left_offset: 0,
            top_offset: 0,
            columns: vec![vec![
                PatchPost {
                    y_offset: 0,
                    pixels: vec![11, 11],
                },
                PatchPost {
                    y_offset: 4,
                    pixels: vec![22, 22],
                },
            ]],
        };
        let mut fb = Framebuffer::new();
        fb.draw_patch(0, 0, &patch);
        assert_eq!(fb.get_pixel(0, 0), Some(11));
        assert_eq!(fb.get_pixel(0, 1), Some(11));
        assert_eq!(fb.get_pixel(0, 2), Some(0)); // transparent gap
        assert_eq!(fb.get_pixel(0, 3), Some(0)); // transparent gap
        assert_eq!(fb.get_pixel(0, 4), Some(22));
        assert_eq!(fb.get_pixel(0, 5), Some(22));
    }
}
