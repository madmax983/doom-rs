//! Half-block (`▀`) framebuffer widget for terminal rendering.
//!
//! Ported from `abrash/src/platform/framebuffer_widget.rs`.
//!
//! # Key difference from the abrash version
//! The abrash widget takes `&Framebuffer<u32>` (ARGB pixels already expanded).
//! This widget takes `data: &[u8]` (palette indices) + `lut: &PaletteLut` and
//! performs the index → RGB lookup at blit time.  This means:
//! - Changing the active palette (pain flash, rad suit, pickup) costs zero
//!   re-renders of the framebuffer — just change `active_palette`.
//! - The game sim and renderer never see RGB values at all.
//!
//! # Terminal cell encoding
//! Each terminal cell is 1 character wide × 2 pixels tall:
//! - `▀` (U+2580 UPPER HALF BLOCK)
//! - Foreground = top pixel RGB
//! - Background = bottom pixel RGB
//!
//! # Scaling
//! Nearest-neighbor: maps terminal cell `(cx, cy)` to framebuffer pixels
//! `(fb_x, fb_y_top)` and `(fb_x, fb_y_bot)` using the same formula as abrash.

use crate::scaler::{ScalingMode, sample_bilinear};
use doom_renderer::{Framebuffer, PaletteLut};
use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

/// Ratatui widget that blits a palette-indexed Doom framebuffer into the terminal.
pub struct DoomFramebufferWidget<'a> {
    /// Raw palette-indexed pixel data (must be exactly `320 * 200` bytes).
    pub data: &'a [u8],
    /// Precomputed palette LUT for index → RGB conversion.
    pub lut: &'a PaletteLut,
    /// Which of the 14 PLAYPAL palettes to use (0 = normal, 1–8 = pain, etc.).
    pub active_palette: usize,
    /// Scaling algorithm to use when blitting to the terminal.
    pub scaling_mode: ScalingMode,
    /// ASCII rendering mode: true to render as colored ASCII art.
    pub ascii_mode: bool,
}

impl<'a> DoomFramebufferWidget<'a> {
    /// Construct from a `Framebuffer` + `PaletteLut` using the default (nearest-neighbor) scaler.
    pub fn new(fb: &'a Framebuffer, lut: &'a PaletteLut, active_palette: usize) -> Self {
        Self {
            data: fb.as_slice(),
            lut,
            active_palette,
            scaling_mode: ScalingMode::Nearest,
            ascii_mode: false,
        }
    }

    /// Set the scaling mode, returning `self` for chaining.
    #[must_use]
    pub fn with_scaling(mut self, mode: ScalingMode) -> Self {
        self.scaling_mode = mode;
        self
    }

    /// Set the ascii mode, returning `self` for chaining.
    #[must_use]
    pub fn with_ascii_mode(mut self, ascii_mode: bool) -> Self {
        self.ascii_mode = ascii_mode;
        self
    }
}

impl Widget for DoomFramebufferWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let term_w = area.width as usize;
        let term_h = area.height as usize;
        let fb_w = Framebuffer::width();
        let fb_h = Framebuffer::height();
        let data = self.data;
        let pal = self.active_palette;

        // Bounds check: if the data slice is wrong size, bail silently rather
        // than panic — the game may not have rendered a frame yet.
        if data.len() != fb_w * fb_h {
            return;
        }

        for cy in 0..term_h {
            for cx in 0..term_w {
                let (top_r, top_g, top_b, bot_r, bot_g, bot_b) = match self.scaling_mode {
                    ScalingMode::Nearest => {
                        // Nearest-neighbor scaling (identical formula to abrash).
                        let fb_x = (cx * fb_w) / term_w;
                        let fb_y_top = (cy * 2 * fb_h) / (term_h * 2);
                        let fb_y_bot = ((cy * 2 + 1) * fb_h) / (term_h * 2);

                        if fb_x >= fb_w || fb_y_top >= fb_h {
                            continue;
                        }

                        let idx_top = fb_y_top * fb_w + fb_x;
                        let idx_bot = (fb_y_bot.min(fb_h - 1)) * fb_w + fb_x;

                        let top = self.lut.get(pal, data[idx_top]);
                        let bot = self.lut.get(pal, data[idx_bot]);
                        (top.r, top.g, top.b, bot.r, bot.g, bot.b)
                    }
                    ScalingMode::Bilinear => {
                        // Bilinear: compute fixed-point source coordinates.
                        // fx maps cx in [0, term_w) to [0, fb_w<<16).
                        // fy_top maps the top sub-pixel; fy_bot maps the bottom.
                        // We use u64 intermediate to avoid overflow before >> 16.
                        let fx: u32 = (((cx as u64 * (fb_w as u64)) << 16) / term_w as u64) as u32;
                        let fy_top: u32 =
                            (((cy as u64 * 2 * (fb_h as u64)) << 16) / (term_h as u64 * 2)) as u32;
                        let fy_bot: u32 = ((((cy as u64 * 2 + 1) * (fb_h as u64)) << 16)
                            / (term_h as u64 * 2)) as u32;

                        let (tr, tg, tb) = sample_bilinear(data, self.lut, pal, fx, fy_top);
                        let (br, bg, bb) = sample_bilinear(data, self.lut, pal, fx, fy_bot);
                        (tr, tg, tb, br, bg, bb)
                    }
                };

                if let Some(cell) = buf.cell_mut((area.x + cx as u16, area.y + cy as u16)) {
                    if self.ascii_mode {
                        let luma = (top_r as u32 * 2126 + top_g as u32 * 7152 + top_b as u32 * 722) / 10000;
                        let chars = b" .:-=+*#%@";
                        let char_idx = (luma * (chars.len() as u32 - 1)) / 255;
                        let c = chars[char_idx as usize] as char;
                        cell.set_char(c)
                            .set_fg(Color::Rgb(top_r, top_g, top_b))
                            .set_bg(Color::Rgb(0, 0, 0));
                    } else {
                        cell.set_char('▀')
                            .set_fg(Color::Rgb(top_r, top_g, top_b))
                            .set_bg(Color::Rgb(bot_r, bot_g, bot_b));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use doom_renderer::PaletteLut;

    fn make_fb_with(color_index: u8) -> Framebuffer {
        let mut fb = Framebuffer::new();
        fb.clear(color_index);
        fb
    }

    #[test]
    fn zero_size_area_is_noop() {
        let fb = make_fb_with(0);
        let lut = PaletteLut::grayscale();
        let w = DoomFramebufferWidget::new(&fb, &lut, 0);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));
        w.render(area, &mut buf);
        // No panic = success.
    }

    #[test]
    fn renders_correct_color_from_palette() {
        let mut fb = Framebuffer::new();
        // Palette index 1 = red in the test_primary LUT.
        fb.clear(1);
        let lut = PaletteLut::test_primary();
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        DoomFramebufferWidget::new(&fb, &lut, 0).render(area, &mut buf);
        let cell = buf.cell((0, 0)).unwrap();
        assert_eq!(cell.symbol(), "▀");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn palette_switch_changes_color_without_fb_change() {
        // Index 0 = black in all palettes (by convention grayscale).
        let fb = make_fb_with(0);
        let lut = PaletteLut::grayscale();
        for pal in 0..14 {
            let area = Rect::new(0, 0, 1, 1);
            let mut buf = Buffer::empty(area);
            DoomFramebufferWidget::new(&fb, &lut, pal).render(area, &mut buf);
            let cell = buf.cell((0, 0)).unwrap();
            // Index 0 in grayscale = (0,0,0) in all palettes.
            assert_eq!(cell.fg, Color::Rgb(0, 0, 0));
        }
    }

    #[test]
    fn wrong_data_length_is_noop() {
        let lut = PaletteLut::grayscale();
        let bad = [0u8; 10]; // wrong size
        let area = Rect::new(0, 0, 4, 2);
        let mut buf = Buffer::empty(area);
        // Manually construct to bypass Framebuffer type.
        DoomFramebufferWidget {
            data: &bad,
            lut: &lut,
            active_palette: 0,
            scaling_mode: ScalingMode::Nearest,
            ascii_mode: false,
        }
        .render(area, &mut buf);
        // No panic; buffer remains empty/default.
    }

    #[test]
    fn scaling_nearest_neighbor_fills_all_cells() {
        // Fill with color index 2 (green in test_primary).
        let mut fb = Framebuffer::new();
        fb.clear(2);
        let lut = PaletteLut::test_primary();
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        DoomFramebufferWidget::new(&fb, &lut, 0).render(area, &mut buf);
        for cy in 0..10u16 {
            for cx in 0..20u16 {
                let cell = buf.cell((cx, cy)).unwrap();
                assert_eq!(cell.symbol(), "▀", "cell ({cx},{cy}) missing ▀");
                assert_eq!(
                    cell.fg,
                    Color::Rgb(0, 255, 0),
                    "cell ({cx},{cy}) wrong color"
                );
            }
        }
    }

    #[test]
    fn bilinear_uniform_field_fills_all_cells() {
        // Uniform green field → bilinear should produce the same color as nearest.
        let mut fb = Framebuffer::new();
        fb.clear(2); // green in test_primary
        let lut = PaletteLut::test_primary();
        let area = Rect::new(0, 0, 20, 10);
        let mut buf = Buffer::empty(area);
        DoomFramebufferWidget::new(&fb, &lut, 0)
            .with_scaling(ScalingMode::Bilinear)
            .render(area, &mut buf);
        for cy in 0..10u16 {
            for cx in 0..20u16 {
                let cell = buf.cell((cx, cy)).unwrap();
                assert_eq!(cell.symbol(), "▀", "cell ({cx},{cy}) missing ▀");
                // Uniform green: bilinear blend is still (0,255,0).
                assert_eq!(cell.fg, Color::Rgb(0, 255, 0), "cell ({cx},{cy}) wrong fg");
                assert_eq!(cell.bg, Color::Rgb(0, 255, 0), "cell ({cx},{cy}) wrong bg");
            }
        }
    }

    #[test]
    fn bilinear_zero_size_area_is_noop() {
        let fb = make_fb_with(0);
        let lut = PaletteLut::grayscale();
        let w = DoomFramebufferWidget::new(&fb, &lut, 0).with_scaling(ScalingMode::Bilinear);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));
        w.render(area, &mut buf);
        // No panic = success.
    }

    #[test]
    fn with_scaling_builder_sets_mode() {
        let fb = make_fb_with(0);
        let lut = PaletteLut::grayscale();
        let w = DoomFramebufferWidget::new(&fb, &lut, 0).with_scaling(ScalingMode::Bilinear);
        assert_eq!(w.scaling_mode, ScalingMode::Bilinear);
    }

    #[test]
    fn with_ascii_mode_builder_sets_mode() {
        let fb = make_fb_with(0);
        let lut = PaletteLut::grayscale();
        let w = DoomFramebufferWidget::new(&fb, &lut, 0).with_ascii_mode(true);
        assert!(w.ascii_mode);
    }

    #[test]
    fn ascii_mode_renders_correct_character() {
        let mut fb = Framebuffer::new();
        // Palette index 1 = red in the test_primary LUT.
        fb.clear(1);
        let lut = PaletteLut::test_primary();
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        DoomFramebufferWidget::new(&fb, &lut, 0)
            .with_ascii_mode(true)
            .render(area, &mut buf);
        let cell = buf.cell((0, 0)).unwrap();
        // top_r = 255, top_g = 0, top_b = 0 -> luma = (255 * 2126) / 10000 = 54
        // char_idx = (54 * 9) / 255 = 486 / 255 = 1
        // b" .:-=+*#%@"[1] = '.'
        assert_eq!(cell.symbol(), ".");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(0, 0, 0));
    }
}
