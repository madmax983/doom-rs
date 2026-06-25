//! Fast palette-aware Sixel widget for Doom's indexed framebuffer.
//!
//! ## Performance design
//!
//! The generic ratatui-image path quantizes 192 KB of RGB data per frame (~20-50 ms).
//! Doom already has a 256-color indexed buffer, so we skip quantization entirely and
//! encode directly from palette indices.
//!
//! Three bottlenecks addressed vs. a naive implementation:
//!
//! 1. **`col_bits` zeroing** — use a dirty-color list; only zero the ~30-50 colors that
//!    actually appeared last band, not all 256 × dst_w bytes.
//! 2. **Accumulation** — iterate in *source* pixel coordinates (src_w = 320) and fill
//!    contiguous runs of dst columns, instead of iterating every dst pixel (dst_w = 1760+)
//!    with a scatter-write per pixel.  Division is replaced by a precomputed table.
//! 3. **Active-color scan** — skip the O(256 × dst_w) `any()` check; dirty tracking
//!    tells us exactly which colors need emission.
//!
//! ## Sixel format summary
//! ```text
//! ESC P <params> q          DCS introducer
//! #n;2;R;G;B                color register n = RGB (0-100 scale)
//! #n <chars> $              color n's pixels for this 6-row band, CR
//! ...
//! #n <chars> -              last color in band → Graphics New Line
//! ESC \                     String Terminator
//! ```
//! Each sixel character encodes one column:
//!   `char = '?' (63) + 6-bit mask`  (bit 0 = top row, bit 5 = bottom row)
//! Runs of identical characters are RLE-compressed as `!count char`.

use std::fmt::Write as FmtWrite;

use doom_renderer::{Framebuffer, PaletteLut};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

/// Palette-aware Sixel widget for a Doom framebuffer.
pub struct DoomSixelWidget<'a> {
    /// Raw indexed pixel data from the `Framebuffer`.
    pub data: &'a [u8],
    /// RGB color lookup table mapping `data` indices to actual colors.
    pub lut: &'a PaletteLut,
    /// The currently active palette index in the `lut` (usually 0, unless taking damage).
    pub active_palette: usize,
    /// Terminal font cell size in pixels `(width, height)` — from `Picker::font_size()`.
    pub font_size: (u16, u16),
}

impl<'a> DoomSixelWidget<'a> {
    /// Creates a new `DoomSixelWidget` referencing the current frame state.
    ///
    /// The actual scaling and bounds calculation occurs in `Widget::render`
    /// to dynamically fit the terminal window.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_renderer::{Framebuffer, PaletteLut};
    /// use doom_tui::sixel::DoomSixelWidget;
    ///
    /// let fb = Framebuffer::new();
    /// let lut = PaletteLut::grayscale();
    /// let active_palette = 0; // Normal view, no damage tint
    /// let font_size = (8, 16); // Typical font cell size
    ///
    /// let widget = DoomSixelWidget::new(&fb, &lut, active_palette, font_size);
    /// ```
    pub fn new(
        fb: &'a Framebuffer,
        lut: &'a PaletteLut,
        active_palette: usize,
        font_size: (u16, u16),
    ) -> Self {
        Self {
            data: fb.as_slice(),
            lut,
            active_palette,
            font_size,
        }
    }
}

impl Widget for DoomSixelWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let src_w = Framebuffer::width();
        let src_h = Framebuffer::height();

        if area.width == 0 || area.height == 0 || self.data.len() != src_w * src_h {
            return;
        }

        let (fw, fh) = self.font_size;
        let dst_w = (area.width as usize) * (fw as usize);
        let dst_h = (area.height as usize) * (fh as usize);

        let sixel = encode_doom_sixel(
            self.data,
            self.lut,
            self.active_palette,
            src_w,
            src_h,
            dst_w,
            dst_h,
            area.width,
        );

        if let Some(cell) = buf.cell_mut((area.x, area.y)) {
            cell.set_symbol(&sixel);
        }
        let mut skip_first = false;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if !skip_first {
                    skip_first = true;
                    continue;
                }
                buf.cell_mut((x, y))
                    .map(|cell| {
                        #[allow(deprecated)]
                        cell.set_skip(true)
                    });
            }
        }
    }
}

/// Directly encodes a paletted source buffer into a Sixel string, applying
/// nearest-neighbor scaling dynamically to match the output size.
///
/// Converts from indexed pixels straight to DCS output without any generic
/// RGB conversions. Calculates differences from previous frames implicitly
/// using Ratatui's state caching (on the main event loop thread) but handles
/// the actual bitwise image building here.
///
/// ## Parameters
///
/// - `data`: Raw framebuffer indices (`src_w` × `src_h` length).
/// - `lut`: RGB lookup table.
/// - `pal`: The active palette to index into the lookup table.
/// - `src_w`: Width of the source buffer.
/// - `src_h`: Height of the source buffer.
/// - `dst_w`: Desired width in Sixel pixels.
/// - `dst_h`: Desired height in Sixel pixels.
/// - `area_w`: Column width of the target terminal cell area (used for aspect ratios).
///
/// ## Examples
///
/// ```
/// use doom_renderer::PaletteLut;
/// use doom_tui::sixel::encode_doom_sixel;
///
/// let lut = PaletteLut::grayscale();
/// let data = vec![0; 320 * 200];
/// let encoded_string = encode_doom_sixel(&data, &lut, 0, 320, 200, 640, 400, 80);
/// ```
pub fn encode_doom_sixel(
    data: &[u8],
    lut: &PaletteLut,
    pal: usize,
    src_w: usize,
    src_h: usize,
    dst_w: usize,
    dst_h: usize,
    area_w: u16,
) -> String {
    // ── Pass 1: which colors appear in the source ─────────────────────────
    let mut used = [false; 256];
    for &idx in data {
        used[idx as usize] = true;
    }

    // ── Precompute column ranges ──────────────────────────────────────────
    // col_ranges[src_col] = (dst_start, dst_end): which dst columns this
    // source column maps to.  Computed once; used in every band's accumulation.
    let mut col_ranges = vec![(0usize, 0usize); src_w];
    for (src_col, range) in col_ranges.iter_mut().enumerate() {
        let start = (src_col * dst_w) / src_w;
        let end = ((src_col + 1) * dst_w) / src_w;
        *range = (start, end);
    }

    // ── Precompute source row for each dst row ────────────────────────────
    let row_src: Vec<usize> = (0..dst_h).map(|r| (r * src_h) / dst_h).collect();

    // ── Working buffers ───────────────────────────────────────────────────
    // col_bits[color * dst_w + col]: 6-bit mask of which rows (in the current
    // band) have `color` at `col`.
    let mut col_bits = vec![0u8; 256 * dst_w];
    // dirty_colors: colors written during accumulation — only these need
    // zeroing at the start of the next band and emitting in the current one.
    let mut dirty_colors: Vec<u8> = Vec::with_capacity(256);
    let mut color_dirty = [false; 256];

    // ── Output string ─────────────────────────────────────────────────────
    let mut out = String::with_capacity(8192 + dst_w * 16);

    // DCS header: aspect-ratio=7 (1:1), background=0 (no change).
    write!(out, "\x1bP7;0;{}q", area_w).unwrap();

    // Color register definitions: #n;2;R;G;B (values 0-100).
    for (i, is_used) in used.iter().enumerate() {
        if !*is_used {
            continue;
        }
        let rgb = lut.get(pal, i as u8);
        let r = (rgb.r as u32 * 100 + 127) / 255;
        let g = (rgb.g as u32 * 100 + 127) / 255;
        let b = (rgb.b as u32 * 100 + 127) / 255;
        write!(out, "#{};2;{};{};{}", i, r, g, b).unwrap();
    }

    // ── Band loop ─────────────────────────────────────────────────────────
    let bands = dst_h.div_ceil(6);

    for band in 0..bands {
        let row_start = band * 6;
        let row_end = (row_start + 6).min(dst_h);

        // Zero only the colors that were dirty last band.
        for &c in &dirty_colors {
            let base = c as usize * dst_w;
            col_bits[base..base + dst_w].fill(0);
        }
        dirty_colors.clear();

        // Collapse the 6 dst rows in this band to at most 3 distinct source
        // rows with their combined row-bit masks.  At 5× scale typically 1-2
        // source rows appear per band.
        let mut src_bands = [(0usize, 0u8); 3];
        let mut n_src = 0usize;

        for (dst_row, &src_row) in row_src.iter().enumerate().take(row_end).skip(row_start) {
            let bit = 1u8 << (dst_row - row_start);
            // Linear search over at most 3 entries — faster than a HashMap.
            let mut found = false;
            for src_band in src_bands.iter_mut().take(n_src) {
                if src_band.0 == src_row {
                    src_band.1 |= bit;
                    found = true;
                    break;
                }
            }
            if !found && n_src < 3 {
                src_bands[n_src] = (src_row, bit);
                n_src += 1;
            }
        }

        // Accumulate in source-pixel coordinates: iterate src_w (320) columns
        // and fill contiguous dst-column runs, instead of iterating dst_w
        // (1760+) columns with a scatter-write per pixel.
        for &(src_row, bit) in src_bands.iter().take(n_src) {
            let src_row_data = &data[src_row * src_w..(src_row + 1) * src_w];

            for (src_col, &color) in src_row_data.iter().enumerate() {
                let (dst_start, dst_end) = col_ranges[src_col];
                let base = color as usize * dst_w;
                // Fill the contiguous run of dst columns — sequential writes,
                // great cache behavior.
                for j in dst_start..dst_end {
                    col_bits[base + j] |= bit;
                }
                // Track dirty colors for zeroing next band and emission this band.
                if !color_dirty[color as usize] {
                    color_dirty[color as usize] = true;
                    dirty_colors.push(color);
                }
            }
        }

        // Sort dirty colors so sixel color selectors are emitted in order.
        dirty_colors.sort_unstable();
        // Reset color_dirty flags.
        for &c in &dirty_colors {
            color_dirty[c as usize] = false;
        }

        // Emit sixel rows for each dirty color.
        let mut first_in_band = true;
        for &color in &dirty_colors {
            let base = color as usize * dst_w;
            let slice = &col_bits[base..base + dst_w];

            if !first_in_band {
                out.push('$'); // Graphics Carriage Return
            }
            first_in_band = false;

            write!(out, "#{}", color).unwrap();

            // RLE-encode sixel characters for this color's columns.
            let mut run_ch = slice[0] + 63;
            let mut run_len = 1usize;
            for &bits in &slice[1..] {
                let ch = bits + 63;
                if ch == run_ch {
                    run_len += 1;
                } else {
                    emit_rle(&mut out, run_ch, run_len);
                    run_ch = ch;
                    run_len = 1;
                }
            }
            emit_rle(&mut out, run_ch, run_len);
        }

        out.push('-'); // Graphics New Line
    }

    out.push_str("\x1b\\"); // String Terminator
    out
}

#[inline]
fn emit_rle(out: &mut String, ch: u8, count: usize) {
    if count >= 4 {
        write!(out, "!{}{}", count, ch as char).unwrap();
    } else {
        for _ in 0..count {
            out.push(ch as char);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_rle_short_run_is_literal() {
        let mut s = String::new();
        emit_rle(&mut s, b'?', 3);
        assert_eq!(s, "???");
    }

    #[test]
    fn emit_rle_long_run_uses_rle() {
        let mut s = String::new();
        emit_rle(&mut s, b'?', 10);
        assert_eq!(s, "!10?");
    }

    #[test]
    fn encode_produces_dcs_header_and_terminator() {
        let lut = PaletteLut::grayscale();
        let data = vec![0u8; 320 * 200];
        let out = encode_doom_sixel(&data, &lut, 0, 320, 200, 320, 200, 40);
        assert!(out.contains("\x1bP7;0;"), "missing DCS header");
        assert!(out.ends_with("\x1b\\"), "missing ST terminator");
    }

    #[test]
    fn encode_defines_only_used_colors() {
        let lut = PaletteLut::grayscale();
        let data = vec![42u8; 320 * 200];
        let out = encode_doom_sixel(&data, &lut, 0, 320, 200, 320, 200, 40);
        assert!(out.contains("#42;2;"), "color 42 should be defined");
        assert!(!out.contains("#0;2;"), "color 0 should not be defined");
    }

    #[test]
    fn encode_upscaled_uniform_compresses_well() {
        let lut = PaletteLut::grayscale();
        let data = vec![7u8; 320 * 200];
        let out = encode_doom_sixel(&data, &lut, 0, 320, 200, 640, 400, 80);
        assert!(out.contains("#7;2;"));
        assert!(
            out.len() < 4096,
            "uniform upscaled image should compress well"
        );
    }

    #[test]
    fn col_ranges_cover_full_dst_width() {
        let src_w = 320usize;
        let dst_w = 1760usize;
        let mut ranges = vec![(0usize, 0usize); src_w];
        for (src_col, range) in ranges.iter_mut().enumerate() {
            *range = ((src_col * dst_w) / src_w, ((src_col + 1) * dst_w) / src_w);
        }
        assert_eq!(ranges[0].0, 0);
        assert_eq!(ranges[src_w - 1].1, dst_w);
        // No gaps: each range end == next range start.
        for (i, pair) in ranges.windows(2).enumerate() {
            assert_eq!(pair[0].1, pair[1].0, "gap at src col {i}");
        }
    }
}

#[cfg(test)]
mod tests_havoc {
    use super::*;
    use doom_renderer::framebuffer::Framebuffer;
    use doom_renderer::palette::PaletteLut;
    use ratatui::layout::Rect;

    #[test]
    fn havoc_encode_sixel_zero_area_panic() {
        let fb = Framebuffer::new();
        let lut = PaletteLut::grayscale();
        let area = Rect::new(0, 0, 0, 0); // Trigger division by zero
        let _ = encode_doom_sixel(
            fb.as_slice(),
            &lut,
            0,
            Framebuffer::width(),
            Framebuffer::height(),
            0,
            0,
            area.width,
        );
    }
}
