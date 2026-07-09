//! Nearest-neighbour integer scaler with vanilla-Doom 4:3 aspect correction.
//!
//! Doom renders a 320×200 framebuffer, but the original VGA presented those
//! pixels on a 4:3 display: the 200 scanlines are stretched vertically to 240
//! (a factor of 1.2) so pixels are non-square. This module reproduces that at
//! present time by scaling the 320×200 ARGB source to `(320·S) × (240·S)` for an
//! integer scale factor `S`, sampling the nearest source pixel.
//!
//! The function is deliberately independent of `winit`/`softbuffer` so it runs
//! (and is unit-tested) under a plain `cargo test`.

/// Source framebuffer width (Doom's canonical render target).
pub const SRC_W: usize = 320;
/// Source framebuffer height (Doom's canonical render target).
pub const SRC_H: usize = 200;
/// Aspect-corrected (stretched) height: 200 → 240 (×1.2).
pub const STRETCH_H: usize = 240;

/// Compute the integer scale factor that best fits a `320×200`→`4:3` image into
/// a window of the given inner size.
///
/// `S = max(1, min(win_w / 320, win_h / 240))`, so the aspect-corrected image
/// (`320·S × 240·S`) never exceeds the window and is at least `1×`.
#[must_use]
pub fn fit_scale(win_w: u32, win_h: u32) -> usize {
    let by_w = (win_w as usize) / SRC_W;
    let by_h = (win_h as usize) / STRETCH_H;
    by_w.min(by_h).max(1)
}

/// Nearest-neighbour scale a 320×200 ARGB buffer to `(320·S) × (240·S)`.
///
/// Output pixel `(x, y)` samples source pixel
/// `src_x = x / S`, `src_y = (y · 200) / (240 · S)`, reproducing the 1.2×
/// vertical stretch. `scale` is clamped to at least 1.
///
/// # Panics
///
/// Panics if `src.len() != SRC_W * SRC_H` (320×200 = 64000).
#[must_use]
pub fn scale_nearest_4_3(src: &[u32], scale: usize) -> Vec<u32> {
    assert_eq!(
        src.len(),
        SRC_W * SRC_H,
        "scaler source must be exactly 320×200"
    );
    let s = scale.max(1);
    let out_w = SRC_W * s;
    let out_h = STRETCH_H * s;
    let mut out = vec![0u32; out_w * out_h];
    for y in 0..out_h {
        // src_y = (y * 200) / (240 * s)
        let src_y = (y * SRC_H) / (STRETCH_H * s);
        let src_row = src_y * SRC_W;
        let dst_row = y * out_w;
        for x in 0..out_w {
            let src_x = x / s;
            out[dst_row + x] = src[src_row + src_x];
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_src() -> Vec<u32> {
        // Encode each source pixel as (y << 16) | x so we can verify sampling.
        let mut v = vec![0u32; SRC_W * SRC_H];
        for y in 0..SRC_H {
            for x in 0..SRC_W {
                v[y * SRC_W + x] = ((y as u32) << 16) | x as u32;
            }
        }
        v
    }

    #[test]
    fn scale1_dimensions_are_320x240() {
        let out = scale_nearest_4_3(&make_src(), 1);
        assert_eq!(out.len(), SRC_W * STRETCH_H);
    }

    #[test]
    fn scale1_row_mapping() {
        let src = make_src();
        let out = scale_nearest_4_3(&src, 1);
        let sample = |x: usize, y: usize| out[y * SRC_W + x];
        let src_x = |v: u32| (v & 0xFFFF) as usize;
        let src_y = |v: u32| (v >> 16) as usize;

        // Corners + interior rows: y in [0,240) maps to (y*200)/240.
        assert_eq!(src_y(sample(0, 0)), 0);
        assert_eq!(src_x(sample(0, 0)), 0);
        assert_eq!(src_y(sample(0, 239)), 199); // (239*200)/240 = 199
        assert_eq!(src_y(sample(0, 120)), 100); // (120*200)/240 = 100
        // Column mapping is 1:1 at S=1.
        assert_eq!(src_x(sample(319, 0)), 319);
        assert_eq!(src_x(sample(100, 0)), 100);
        // Interior pixel round-trip.
        assert_eq!(src_x(sample(200, 120)), 200);
        assert_eq!(src_y(sample(200, 120)), 100);
    }

    #[test]
    fn scale2_dimensions_and_mapping() {
        let src = make_src();
        let s = 2;
        let out = scale_nearest_4_3(&src, s);
        let out_w = SRC_W * s;
        let out_h = STRETCH_H * s;
        assert_eq!(out.len(), out_w * out_h);
        let sample = |x: usize, y: usize| out[y * out_w + x];
        let src_x = |v: u32| (v & 0xFFFF) as usize;
        let src_y = |v: u32| (v >> 16) as usize;
        // x/2 column mapping.
        assert_eq!(src_x(sample(0, 0)), 0);
        assert_eq!(src_x(sample(1, 0)), 0);
        assert_eq!(src_x(sample(2, 0)), 1);
        assert_eq!(src_x(sample(3, 0)), 1);
        // Last row maps to source row 199.
        assert_eq!(src_y(sample(0, out_h - 1)), 199);
        // Top-left is origin.
        assert_eq!(src_y(sample(0, 0)), 0);
    }

    #[test]
    fn zero_scale_is_clamped_to_one() {
        let out = scale_nearest_4_3(&make_src(), 0);
        assert_eq!(out.len(), SRC_W * STRETCH_H);
    }

    #[test]
    fn fit_scale_picks_integer_factor() {
        assert_eq!(fit_scale(960, 720), 3); // 960/320=3, 720/240=3
        assert_eq!(fit_scale(640, 480), 2);
        assert_eq!(fit_scale(320, 240), 1);
        assert_eq!(fit_scale(100, 100), 1); // clamped up
        // Limited by the tighter dimension.
        assert_eq!(fit_scale(1280, 480), 2); // w allows 4, h allows 2
    }
}
