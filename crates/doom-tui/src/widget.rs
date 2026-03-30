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

use crate::charset::CharSet;
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
    /// Character-mapped rendering: `Some(charset)` renders luminance-mapped characters
    /// instead of half-blocks.  `None` = half-block mode (default).
    pub char_set: Option<CharSet>,
}

impl<'a> DoomFramebufferWidget<'a> {
    /// Construct from a `Framebuffer` + `PaletteLut` using the default (nearest-neighbor) scaler.
    pub fn new(fb: &'a Framebuffer, lut: &'a PaletteLut, active_palette: usize) -> Self {
        Self {
            data: fb.as_slice(),
            lut,
            active_palette,
            scaling_mode: ScalingMode::Nearest,
            char_set: None,
        }
    }

    /// Set the scaling mode, returning `self` for chaining.
    #[must_use]
    pub fn with_scaling(mut self, mode: ScalingMode) -> Self {
        self.scaling_mode = mode;
        self
    }

    /// Set character-mapped rendering mode, returning `self` for chaining.
    #[must_use]
    pub fn with_char_set(mut self, cs: Option<CharSet>) -> Self {
        self.char_set = cs;
        self
    }

    /// Legacy helper: `true` = ascii mode (equivalent to `char_set == Some(CharSet::Ascii)`).
    #[must_use]
    pub fn with_ascii_mode(mut self, ascii_mode: bool) -> Self {
        self.char_set = if ascii_mode {
            Some(CharSet::Ascii)
        } else {
            None
        };
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

        // Pre-slice the active palette once: eliminates per-cell `.min()` + multiply.
        let pal_slice = self.lut.palette_slice(pal);

        // Compute the absolute index in buf.content for our area's top-left cell.
        // This lets us slice buf.content directly per row, replacing per-cell cell_mut() calls
        // (which each do a bounds-check, a Position conversion, and a stride multiply).
        let buf_stride = buf.area.width as usize;
        let area_origin_idx =
            (area.y - buf.area.y) as usize * buf_stride + (area.x - buf.area.x) as usize;

        // Hoist scaling mode and char_set outside the pixel loops to eliminate
        // per-cell branches on frame-invariant values.
        match (self.scaling_mode, self.char_set) {
            (ScalingMode::Nearest, None) => {
                // Precompute coordinate tables: replaces 3 divisions per cell with
                // table lookups.  Tables fit in L1 cache (≤220 + 2×55 = 330 usize entries).
                let x_map: Vec<usize> = (0..term_w).map(|cx| (cx * fb_w) / term_w).collect();
                // y_top: (cy * 2 * fb_h) / (term_h * 2) simplifies to (cy * fb_h) / term_h.
                let y_top_map: Vec<usize> = (0..term_h).map(|cy| (cy * fb_h) / term_h).collect();
                let y_bot_map: Vec<usize> = (0..term_h)
                    .map(|cy| (((cy * 2 + 1) * fb_h) / (term_h * 2)).min(fb_h - 1))
                    .collect();

                for cy in 0..term_h {
                    // Row offsets computed once per outer iteration (eliminates fb_w mul in inner loop).
                    let top_row_base = y_top_map[cy] * fb_w;
                    let bot_row_base = y_bot_map[cy] * fb_w;
                    let row_start = area_origin_idx + cy * buf_stride;
                    let row_cells = &mut buf.content[row_start..row_start + term_w];
                    for (cell, &fb_x) in row_cells.iter_mut().zip(x_map.iter()) {
                        let top = pal_slice[data[top_row_base + fb_x] as usize];
                        let bot = pal_slice[data[bot_row_base + fb_x] as usize];
                        cell.set_char('▀')
                            .set_fg(Color::Rgb(top.r, top.g, top.b))
                            .set_bg(Color::Rgb(bot.r, bot.g, bot.b));
                    }
                }
            }
            (ScalingMode::Nearest, Some(cs)) => {
                // Character-mapped mode: same fg/bg as halfblocks (top pixel = fg,
                // bottom pixel = bg), but the half-block `▀` is replaced by a
                // luminance-mapped character.  This preserves the exact palette colors
                // while adding character texture.
                let x_map: Vec<usize> = (0..term_w).map(|cx| (cx * fb_w) / term_w).collect();
                let y_top_map: Vec<usize> = (0..term_h).map(|cy| (cy * fb_h) / term_h).collect();
                let y_bot_map: Vec<usize> = (0..term_h)
                    .map(|cy| (((cy * 2 + 1) * fb_h) / (term_h * 2)).min(fb_h - 1))
                    .collect();

                for cy in 0..term_h {
                    let top_row_base = y_top_map[cy] * fb_w;
                    let bot_row_base = y_bot_map[cy] * fb_w;
                    let row_start = area_origin_idx + cy * buf_stride;
                    let row_cells = &mut buf.content[row_start..row_start + term_w];
                    for (cell, &fb_x) in row_cells.iter_mut().zip(x_map.iter()) {
                        let top = pal_slice[data[top_row_base + fb_x] as usize];
                        let bot = pal_slice[data[bot_row_base + fb_x] as usize];
                        let luma = (top.r as u32 * 2126 + top.g as u32 * 7152 + top.b as u32 * 722)
                            / 10000;
                        let c = cs.map_luma(luma as u8);
                        cell.set_char(c)
                            .set_fg(Color::Rgb(top.r, top.g, top.b))
                            .set_bg(Color::Rgb(bot.r, bot.g, bot.b));
                    }
                }
            }
            (ScalingMode::Bilinear, None) => {
                // Bilinear: precompute fixed-point coordinate tables.
                // u64 arithmetic needed to avoid overflow before the >> 16 shift.
                let fx_map: Vec<u32> = (0..term_w)
                    .map(|cx| (((cx as u64 * fb_w as u64) << 16) / term_w as u64) as u32)
                    .collect();
                let fy_top_map: Vec<u32> = (0..term_h)
                    .map(|cy| (((cy as u64 * 2 * fb_h as u64) << 16) / (term_h as u64 * 2)) as u32)
                    .collect();
                let fy_bot_map: Vec<u32> = (0..term_h)
                    .map(|cy| {
                        ((((cy as u64 * 2 + 1) * fb_h as u64) << 16) / (term_h as u64 * 2)) as u32
                    })
                    .collect();

                for cy in 0..term_h {
                    let fy_top = fy_top_map[cy];
                    let fy_bot = fy_bot_map[cy];
                    let row_start = area_origin_idx + cy * buf_stride;
                    let row_cells = &mut buf.content[row_start..row_start + term_w];
                    for (cell, &fx) in row_cells.iter_mut().zip(fx_map.iter()) {
                        let (tr, tg, tb) = sample_bilinear(data, self.lut, pal, fx, fy_top);
                        let (br, bg, bb) = sample_bilinear(data, self.lut, pal, fx, fy_bot);
                        cell.set_char('▀')
                            .set_fg(Color::Rgb(tr, tg, tb))
                            .set_bg(Color::Rgb(br, bg, bb));
                    }
                }
            }
            (ScalingMode::Bilinear, Some(cs)) => {
                // Bilinear + character-mapped: same fg/bg as bilinear halfblocks.
                let fx_map: Vec<u32> = (0..term_w)
                    .map(|cx| (((cx as u64 * fb_w as u64) << 16) / term_w as u64) as u32)
                    .collect();
                let fy_top_map: Vec<u32> = (0..term_h)
                    .map(|cy| (((cy as u64 * 2 * fb_h as u64) << 16) / (term_h as u64 * 2)) as u32)
                    .collect();
                let fy_bot_map: Vec<u32> = (0..term_h)
                    .map(|cy| {
                        ((((cy as u64 * 2 + 1) * fb_h as u64) << 16) / (term_h as u64 * 2)) as u32
                    })
                    .collect();

                for cy in 0..term_h {
                    let fy_top = fy_top_map[cy];
                    let fy_bot = fy_bot_map[cy];
                    let row_start = area_origin_idx + cy * buf_stride;
                    let row_cells = &mut buf.content[row_start..row_start + term_w];
                    for (cell, &fx) in row_cells.iter_mut().zip(fx_map.iter()) {
                        let (tr, tg, tb) = sample_bilinear(data, self.lut, pal, fx, fy_top);
                        let (br, bg, bb) = sample_bilinear(data, self.lut, pal, fx, fy_bot);
                        let luma = (tr as u32 * 2126 + tg as u32 * 7152 + tb as u32 * 722) / 10000;
                        let c = cs.map_luma(luma as u8);
                        cell.set_char(c)
                            .set_fg(Color::Rgb(tr, tg, tb))
                            .set_bg(Color::Rgb(br, bg, bb));
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
            char_set: None,
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
        assert_eq!(w.char_set, Some(crate::charset::CharSet::Ascii));
    }

    #[test]
    fn with_ascii_mode_false_builder_clears_mode() {
        let fb = make_fb_with(0);
        let lut = PaletteLut::grayscale();
        let w = DoomFramebufferWidget::new(&fb, &lut, 0)
            .with_char_set(Some(crate::charset::CharSet::Ascii))
            .with_ascii_mode(false);
        assert_eq!(w.char_set, None);
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
        // ASCII ramp has 12 chars: char_idx = (54 * 11) / 255 = 2
        // [' ', '.', ':', '+', '=', '!', '*', '?', '#', '%', '&', '@'][2] = ':'
        assert_eq!(cell.symbol(), ":");
        // fg = top pixel color (same as halfblocks)
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        // bg = bottom pixel color (uniform fill → same as top)
        assert_eq!(cell.bg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn charmap_mode_with_bilinear_scaling_renders_correct_character() {
        let mut fb = Framebuffer::new();
        // Palette index 1 = red in the test_primary LUT.
        fb.clear(1);
        let lut = PaletteLut::test_primary();
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        DoomFramebufferWidget::new(&fb, &lut, 0)
            .with_scaling(ScalingMode::Bilinear)
            .with_char_set(Some(crate::charset::CharSet::Ascii))
            .render(area, &mut buf);
        let cell = buf.cell((0, 0)).unwrap();
        // Bilinear blend of uniform red is still red (255, 0, 0).
        // Luma calculation: (255 * 2126) / 10000 = 54.
        // ASCII char idx = (54 * 11) / 255 = 2 -> ':'
        assert_eq!(cell.symbol(), ":");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn charmap_mode_with_nearest_scaling_renders_correct_character() {
        let mut fb = Framebuffer::new();
        // Palette index 1 = red in the test_primary LUT.
        fb.clear(1);
        let lut = PaletteLut::test_primary();
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        DoomFramebufferWidget::new(&fb, &lut, 0)
            .with_scaling(ScalingMode::Nearest)
            .with_char_set(Some(crate::charset::CharSet::Ascii))
            .render(area, &mut buf);
        let cell = buf.cell((0, 0)).unwrap();
        // Nearest neighbor of uniform red is red (255, 0, 0).
        // Luma calculation: (255 * 2126) / 10000 = 54.
        // ASCII char idx = (54 * 11) / 255 = 2 -> ':'
        assert_eq!(cell.symbol(), ":");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(255, 0, 0));
    }
}
