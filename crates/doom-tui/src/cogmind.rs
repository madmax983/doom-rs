//! Cogmind-style top-down ASCII roguelike frame and widget.
//!
//! [`CogmindFrame`] is a grid of pre-styled terminal cells that bypasses the
//! palette-indexed framebuffer entirely.  Each [`CogmindCell`] carries its own
//! glyph, foreground, and background RGB color.
//!
//! [`CogmindWidget`] is a dumb ratatui [`Widget`] that blits a `CogmindFrame`
//! to the terminal buffer, centering it if the frame is smaller than the
//! available area.

use ratatui::{buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

/// A single cell in a [`CogmindFrame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CogmindCell {
    /// The character glyph to display.
    pub glyph: char,
    /// Foreground color as `(r, g, b)`.
    pub fg: (u8, u8, u8),
    /// Background color as `(r, g, b)`.
    pub bg: (u8, u8, u8),
}

impl Default for CogmindCell {
    fn default() -> Self {
        Self {
            glyph: ' ',
            fg: (0, 0, 0),
            bg: (0, 0, 0),
        }
    }
}

/// A grid of pre-styled terminal cells for Cogmind-mode rendering.
///
/// Unlike the palette-indexed [`Framebuffer`](doom_renderer::Framebuffer),
/// each cell carries its own glyph and RGB colors directly.
#[derive(Debug, Clone)]
pub struct CogmindFrame {
    cells: Vec<CogmindCell>,
    width: u16,
    height: u16,
}

impl CogmindFrame {
    /// Create a blank frame filled with default (space, black/black) cells.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let count = usize::from(width) * usize::from(height);
        Self {
            cells: vec![CogmindCell::default(); count],
            width,
            height,
        }
    }

    /// Frame width in cells.
    #[must_use]
    pub fn width(&self) -> u16 {
        self.width
    }

    /// Frame height in cells.
    #[must_use]
    pub fn height(&self) -> u16 {
        self.height
    }

    /// Set the cell at `(x, y)`.  Out-of-bounds coordinates are silently ignored.
    pub fn set(&mut self, x: u16, y: u16, cell: CogmindCell) {
        if x < self.width && y < self.height {
            let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
            self.cells[idx] = cell;
        }
    }

    /// Get the cell at `(x, y)`, or `None` if out of bounds.
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> Option<&CogmindCell> {
        if x < self.width && y < self.height {
            let idx = usize::from(y) * usize::from(self.width) + usize::from(x);
            Some(&self.cells[idx])
        } else {
            None
        }
    }
}

/// Ratatui widget that blits a [`CogmindFrame`] to the terminal buffer.
///
/// Centers the frame within the available area if the frame is smaller.
pub struct CogmindWidget<'a> {
    frame: &'a CogmindFrame,
}

impl<'a> CogmindWidget<'a> {
    /// Wrap a frame reference for rendering.
    #[must_use]
    pub fn new(frame: &'a CogmindFrame) -> Self {
        Self { frame }
    }
}

impl Widget for CogmindWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let fw = self.frame.width;
        let fh = self.frame.height;

        if fw == 0 || fh == 0 {
            return;
        }

        // Center the frame within the area.
        let offset_x = area.x + (area.width.saturating_sub(fw)) / 2;
        let offset_y = area.y + (area.height.saturating_sub(fh)) / 2;

        // Visible region of the frame (clamp to area bounds).
        let visible_w = fw.min(area.width);
        let visible_h = fh.min(area.height);

        for fy in 0..visible_h {
            for fx in 0..visible_w {
                let bx = offset_x + fx;
                let by = offset_y + fy;

                // Only write cells that fall within the buffer area.
                if bx >= area.x + area.width || by >= area.y + area.height {
                    continue;
                }

                if let Some(cell) = self.frame.get(fx, fy) {
                    if let Some(buf_cell) = buf.cell_mut((bx, by)) {
                        buf_cell
                            .set_char(cell.glyph)
                            .set_fg(Color::Rgb(cell.fg.0, cell.fg.1, cell.fg.2))
                            .set_bg(Color::Rgb(cell.bg.0, cell.bg.1, cell.bg.2));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_new_is_blank() {
        let frame = CogmindFrame::new(10, 5);
        assert_eq!(frame.width(), 10);
        assert_eq!(frame.height(), 5);
        for y in 0..5 {
            for x in 0..10 {
                let cell = frame.get(x, y).unwrap();
                assert_eq!(*cell, CogmindCell::default());
            }
        }
    }

    #[test]
    fn frame_set_get_roundtrip() {
        let mut frame = CogmindFrame::new(4, 4);
        let cell = CogmindCell {
            glyph: '@',
            fg: (255, 0, 0),
            bg: (0, 0, 128),
        };
        frame.set(2, 3, cell);
        assert_eq!(*frame.get(2, 3).unwrap(), cell);
    }

    #[test]
    fn frame_oob_set_is_noop() {
        let mut frame = CogmindFrame::new(4, 4);
        let cell = CogmindCell {
            glyph: '#',
            fg: (255, 255, 255),
            bg: (0, 0, 0),
        };
        // These should not panic.
        frame.set(4, 0, cell);
        frame.set(0, 4, cell);
        frame.set(100, 100, cell);
        // Frame should still be all defaults.
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(*frame.get(x, y).unwrap(), CogmindCell::default());
            }
        }
    }

    #[test]
    fn frame_oob_get_returns_none() {
        let frame = CogmindFrame::new(4, 4);
        assert!(frame.get(4, 0).is_none());
        assert!(frame.get(0, 4).is_none());
        assert!(frame.get(100, 100).is_none());
    }

    #[test]
    fn widget_renders_cell_colors() {
        let mut frame = CogmindFrame::new(2, 2);
        frame.set(
            0,
            0,
            CogmindCell {
                glyph: '@',
                fg: (255, 0, 0),
                bg: (0, 255, 0),
            },
        );
        frame.set(
            1,
            0,
            CogmindCell {
                glyph: '#',
                fg: (0, 0, 255),
                bg: (128, 128, 128),
            },
        );

        let area = Rect::new(0, 0, 2, 2);
        let mut buf = Buffer::empty(area);
        CogmindWidget::new(&frame).render(area, &mut buf);

        let c00 = buf.cell((0, 0)).unwrap();
        assert_eq!(c00.symbol(), "@");
        assert_eq!(c00.fg, Color::Rgb(255, 0, 0));
        assert_eq!(c00.bg, Color::Rgb(0, 255, 0));

        let c10 = buf.cell((1, 0)).unwrap();
        assert_eq!(c10.symbol(), "#");
        assert_eq!(c10.fg, Color::Rgb(0, 0, 255));
        assert_eq!(c10.bg, Color::Rgb(128, 128, 128));
    }

    #[test]
    fn widget_zero_size_is_noop() {
        let frame = CogmindFrame::new(4, 4);
        let area = Rect::new(0, 0, 0, 0);
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 10));
        CogmindWidget::new(&frame).render(area, &mut buf);
        // No panic = success.
    }

    #[test]
    fn widget_blank_cells_are_spaces() {
        let frame = CogmindFrame::new(3, 3);
        let area = Rect::new(0, 0, 3, 3);
        let mut buf = Buffer::empty(area);
        CogmindWidget::new(&frame).render(area, &mut buf);

        for y in 0..3u16 {
            for x in 0..3u16 {
                let cell = buf.cell((x, y)).unwrap();
                assert_eq!(cell.symbol(), " ", "cell ({x},{y}) should be space");
                assert_eq!(
                    cell.fg,
                    Color::Rgb(0, 0, 0),
                    "cell ({x},{y}) fg should be black"
                );
                assert_eq!(
                    cell.bg,
                    Color::Rgb(0, 0, 0),
                    "cell ({x},{y}) bg should be black"
                );
            }
        }
    }

    #[test]
    fn widget_centers_small_frame_in_large_area() {
        let mut frame = CogmindFrame::new(2, 2);
        frame.set(
            0,
            0,
            CogmindCell {
                glyph: 'X',
                fg: (255, 255, 255),
                bg: (0, 0, 0),
            },
        );

        // 6x6 area, 2x2 frame => offset (2,2)
        let area = Rect::new(0, 0, 6, 6);
        let mut buf = Buffer::empty(area);
        CogmindWidget::new(&frame).render(area, &mut buf);

        // The 'X' should appear at (2, 2) in the buffer.
        let cell = buf.cell((2, 2)).unwrap();
        assert_eq!(cell.symbol(), "X");
        assert_eq!(cell.fg, Color::Rgb(255, 255, 255));

        // Origin (0,0) should still be the default reset character.
        let origin = buf.cell((0, 0)).unwrap();
        assert_ne!(origin.symbol(), "X");
    }
}
