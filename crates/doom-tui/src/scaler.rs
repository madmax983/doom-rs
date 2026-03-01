//! Nearest-neighbor scaling helpers.
//!
//! Shared math for mapping terminal cell coordinates to framebuffer
//! pixel coordinates — identical to the abrash port and confirmed by
//! the widget implementation.

/// Map terminal column `cx` to framebuffer X pixel, given `term_w` and `fb_w`.
#[inline]
pub fn cell_to_fb_x(cx: usize, term_w: usize, fb_w: usize) -> usize {
    (cx * fb_w) / term_w
}

/// Map terminal row `cy` (top sub-pixel) to framebuffer Y, given `term_h` and `fb_h`.
#[inline]
pub fn cell_to_fb_y_top(cy: usize, term_h: usize, fb_h: usize) -> usize {
    (cy * 2 * fb_h) / (term_h * 2)
}

/// Map terminal row `cy` (bottom sub-pixel) to framebuffer Y.
#[inline]
pub fn cell_to_fb_y_bot(cy: usize, term_h: usize, fb_h: usize) -> usize {
    ((cy * 2 + 1) * fb_h) / (term_h * 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_mapping() {
        assert_eq!(cell_to_fb_x(5, 320, 320), 5);
        assert_eq!(cell_to_fb_y_top(10, 100, 200), 20);
    }

    #[test]
    fn half_size_terminal_scales_up() {
        // Terminal 160 wide, FB 320 wide → cell 0 → fb pixel 0, cell 1 → fb pixel 2.
        assert_eq!(cell_to_fb_x(0, 160, 320), 0);
        assert_eq!(cell_to_fb_x(1, 160, 320), 2);
    }

    #[test]
    fn top_and_bot_differ_within_bounds() {
        // For any cy, bot >= top and both < fb_h (assuming fb_h > 1).
        for cy in 0..50 {
            let top = cell_to_fb_y_top(cy, 50, 200);
            let bot = cell_to_fb_y_bot(cy, 50, 200);
            assert!(bot >= top, "cy={cy}: bot={bot} < top={top}");
            assert!(top < 200, "cy={cy}: top out of bounds");
            assert!(bot < 200, "cy={cy}: bot out of bounds");
        }
    }
}
