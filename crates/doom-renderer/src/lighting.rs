//! Colormap-based distance shading for the Doom software renderer.
//!
//! Doom uses a 32-level colormap table (loaded from the `COLORMAP` WAD lump)
//! to simulate lighting.  Index 0 is full-bright and index 31 is the darkest.
//! This module provides the math that converts a sector's light level (0-255)
//! and a pixel's distance from the camera into a colormap index, plus helper
//! functions to apply the resulting colormap to framebuffer pixels, columns,
//! and spans.
//!
//! # `LightParams`
//!
//! The primary high-level interface.  Construct one per sector (or per wall
//! seg) and call `get_colormap` / `get_wall_colormap` / `get_flat_colormap`
//! to obtain the correct 256-byte colormap row from a `ColormapCache`.
//!
//! # Standalone helpers
//!
//! Lower-level functions (`light_to_colormap_index`, `compute_wall_light`,
//! etc.) are also public for callers that want to manage colormap indices
//! manually.

use crate::colormap::ColormapCache;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Focal length in pixels (half the 320-wide screen).
pub const FOCAL_LEN: f32 = 160.0;

/// Maximum valid colormap index (0 = bright, 31 = dark).
pub const MAX_COLORMAP_INDEX: u8 = 31;

/// Minimum distance used in light calculations to avoid division-by-zero
/// and extreme near-field brightness.
pub const MIN_DISTANCE: f32 = 1.0;

/// Assumed screen width for angular-falloff calculations.
pub const SCREEN_W: usize = 320;

/// Half the screen width (float).
pub const HALF_W: f32 = 160.0;

// ---------------------------------------------------------------------------
// Standalone functions
// ---------------------------------------------------------------------------

/// Convert a sector light level (0-255) to a base colormap index (0-31).
///
/// `0` means full-bright, `31` means pitch-dark.  The conversion simply
/// inverts and scales: brighter sectors yield lower indices.
#[inline]
pub fn light_to_colormap_index(light_level: u8) -> u8 {
    (255u8.wrapping_sub(light_level)) >> 3
}

/// Compute a distance-attenuated colormap index for a wall column.
///
/// Closer walls are brighter (lower index), farther walls are darker.
/// The `base_cm_index` is the sector's base index from
/// [`light_to_colormap_index`].
#[inline]
pub fn compute_wall_light(base_cm_index: u8, distance: f32) -> u8 {
    let d = if distance < MIN_DISTANCE {
        MIN_DISTANCE
    } else {
        distance
    };
    let scale = FOCAL_LEN / d;
    let offset = (scale as u8) >> 4;
    let result = base_cm_index.saturating_sub(offset);
    result.min(MAX_COLORMAP_INDEX)
}

/// Compute a distance-attenuated colormap index for a floor/ceiling span.
///
/// Uses the same formula as wall light.
#[inline]
pub fn compute_flat_light(base_cm_index: u8, distance: f32) -> u8 {
    compute_wall_light(base_cm_index, distance)
}

/// Compute wall lighting with an angular penalty for screen-edge columns.
///
/// Columns near the horizontal centre of the screen receive the base
/// brightness, while columns near the edges are penalised (darker).  This
/// simulates the angular falloff inherent in perspective projection.
#[inline]
pub fn compute_wall_light_with_falloff(base_cm_index: u8, distance: f32, screen_x: usize) -> u8 {
    let base_lit = compute_wall_light(base_cm_index, distance);
    // Angular penalty: how far this column is from screen centre, scaled
    // into 0..~10 extra darkness levels at the very edge.
    let dx = (screen_x as f32 - HALF_W).abs();
    let penalty = (dx / HALF_W * 10.0) as u8;
    (base_lit + penalty).min(MAX_COLORMAP_INDEX)
}

/// Look up a single pixel through a 256-byte colormap.
#[inline]
pub fn shade_pixel(colormap: &[u8; 256], raw_pixel: u8) -> u8 {
    colormap[raw_pixel as usize]
}

/// Apply a colormap to a vertical column of pixels in a framebuffer.
///
/// Processes pixels at column `x` from row `y_top` to `y_bot` (inclusive).
/// Out-of-range coordinates are silently clamped / skipped so callers need
/// not bounds-check beforehand.
pub fn shade_column(
    fb_data: &mut [u8],
    fb_width: usize,
    x: usize,
    y_top: usize,
    y_bot: usize,
    colormap: &[u8; 256],
) {
    if fb_width == 0 || x >= fb_width {
        return;
    }
    let fb_height = if fb_width == 0 {
        0
    } else {
        fb_data.len() / fb_width
    };
    let top = y_top.min(fb_height.saturating_sub(1));
    let bot = y_bot.min(fb_height.saturating_sub(1));
    if top > bot {
        return;
    }
    for y in top..=bot {
        let idx = y * fb_width + x;
        if idx < fb_data.len() {
            fb_data[idx] = colormap[fb_data[idx] as usize];
        }
    }
}

/// Apply a colormap to a horizontal span of pixels in a framebuffer.
///
/// Processes row `y` from column `x1` to `x2` (inclusive).  Out-of-range
/// coordinates are silently clamped / skipped.
pub fn shade_span(
    fb_data: &mut [u8],
    fb_width: usize,
    y: usize,
    x1: usize,
    x2: usize,
    colormap: &[u8; 256],
) {
    if fb_width == 0 {
        return;
    }
    let fb_height = fb_data.len() / fb_width;
    if y >= fb_height {
        return;
    }
    let left = x1.min(fb_width.saturating_sub(1));
    let right = x2.min(fb_width.saturating_sub(1));
    if left > right {
        return;
    }
    let row_start = y * fb_width;
    for col in left..=right {
        let idx = row_start + col;
        if idx < fb_data.len() {
            fb_data[idx] = colormap[fb_data[idx] as usize];
        }
    }
}

// ---------------------------------------------------------------------------
// LightParams
// ---------------------------------------------------------------------------

/// Pre-computed lighting parameters for a sector or wall segment.
///
/// Construct once per sector and reuse for every pixel drawn from that
/// sector.  The struct caches the base colormap index and the fullbright
/// flag so the hot path does not recompute them.
#[derive(Clone, Copy, Debug)]
pub struct LightParams {
    base: u8,
    fullbright: bool,
}

impl Default for LightParams {
    /// Default is full-bright (colormap index 0, no distance attenuation).
    fn default() -> Self {
        Self {
            base: 0,
            fullbright: true,
        }
    }
}

impl LightParams {
    /// Create lighting parameters from a sector light level.
    ///
    /// If `is_fullbright` is `true` (or `sector_light` is 255), every pixel
    /// will use colormap index 0 regardless of distance.
    #[inline]
    pub fn new(sector_light: u8, is_fullbright: bool) -> Self {
        let fb = is_fullbright || sector_light == 255;
        Self {
            base: if fb {
                0
            } else {
                light_to_colormap_index(sector_light)
            },
            fullbright: fb,
        }
    }

    /// The base colormap index for this sector (before distance falloff).
    #[inline]
    pub fn base_index(&self) -> u8 {
        self.base
    }

    /// Whether this sector is fullbright (no distance attenuation).
    #[inline]
    pub fn is_fullbright(&self) -> bool {
        self.fullbright
    }

    /// Compute the colormap index for a given distance (general purpose).
    ///
    /// Returns `0` for fullbright sectors, otherwise applies distance
    /// falloff via [`compute_wall_light`].
    #[inline]
    pub fn colormap_for_distance(&self, distance: f32) -> usize {
        if self.fullbright {
            0
        } else {
            compute_wall_light(self.base, distance) as usize
        }
    }

    /// Colormap index for a wall column at `distance` and screen column `screen_x`.
    #[inline]
    pub fn colormap_for_wall(&self, distance: f32, screen_x: usize) -> usize {
        if self.fullbright {
            0
        } else {
            compute_wall_light_with_falloff(self.base, distance, screen_x) as usize
        }
    }

    /// Colormap index for a floor/ceiling span at `distance`.
    #[inline]
    pub fn colormap_for_flat(&self, distance: f32) -> usize {
        if self.fullbright {
            0
        } else {
            compute_flat_light(self.base, distance) as usize
        }
    }

    /// Obtain the 256-byte colormap row for a general distance lookup.
    #[inline]
    pub fn get_colormap<'a>(&self, distance: f32, cache: &'a ColormapCache) -> &'a [u8; 256] {
        let idx = self.colormap_for_distance(distance);
        cache.get(idx as u8)
    }

    /// Obtain the 256-byte colormap row for a wall column.
    #[inline]
    pub fn get_wall_colormap<'a>(
        &self,
        distance: f32,
        screen_x: usize,
        cache: &'a ColormapCache,
    ) -> &'a [u8; 256] {
        let idx = self.colormap_for_wall(distance, screen_x);
        cache.get(idx as u8)
    }

    /// Obtain the 256-byte colormap row for a floor/ceiling span.
    #[inline]
    pub fn get_flat_colormap<'a>(&self, distance: f32, cache: &'a ColormapCache) -> &'a [u8; 256] {
        let idx = self.colormap_for_flat(distance);
        cache.get(idx as u8)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colormap::{COLORMAP_ROWS, COLORMAP_SIZE, ColormapCache};

    /// Build a `ColormapCache` where row `n` has every byte set to `n`.
    fn test_cache() -> ColormapCache {
        let mut data = vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE];
        for row in 0..COLORMAP_ROWS {
            let start = row * COLORMAP_SIZE;
            data[start..start + COLORMAP_SIZE].fill(row as u8);
        }
        ColormapCache::from_test_data(data)
    }

    // -----------------------------------------------------------------------
    // light_to_colormap_index
    // -----------------------------------------------------------------------

    #[test]
    fn light_to_colormap_index_full_bright() {
        assert_eq!(light_to_colormap_index(255), 0);
    }

    #[test]
    fn light_to_colormap_index_full_dark() {
        assert_eq!(light_to_colormap_index(0), 31);
    }

    #[test]
    fn light_to_colormap_index_mid() {
        // 128 -> (255 - 128) >> 3 = 127 >> 3 = 15
        assert_eq!(light_to_colormap_index(128), 15);
    }

    #[test]
    fn light_to_colormap_index_just_below_max() {
        // 247 -> (255 - 247) >> 3 = 8 >> 3 = 1
        assert_eq!(light_to_colormap_index(247), 1);
    }

    #[test]
    fn light_to_colormap_index_monotonic() {
        // Higher light levels should yield lower (brighter) indices.
        let mut prev = light_to_colormap_index(0);
        for light in 1..=255u8 {
            let cur = light_to_colormap_index(light);
            assert!(
                cur <= prev,
                "index should decrease (or stay) as light increases"
            );
            prev = cur;
        }
    }

    // -----------------------------------------------------------------------
    // compute_wall_light
    // -----------------------------------------------------------------------

    #[test]
    fn wall_light_closer_is_brighter() {
        let base = 20u8;
        let near = compute_wall_light(base, 50.0);
        let far = compute_wall_light(base, 500.0);
        assert!(near <= far, "closer should be brighter (lower index)");
    }

    #[test]
    fn wall_light_very_close() {
        // At distance 1.0: scale = 160 / 1 = 160, offset = 160 >> 4 = 10
        // result = 20 - 10 = 10
        assert_eq!(compute_wall_light(20, 1.0), 10);
    }

    #[test]
    fn wall_light_never_exceeds_max() {
        let result = compute_wall_light(31, 10000.0);
        assert!(result <= MAX_COLORMAP_INDEX);
    }

    #[test]
    fn wall_light_clamped_near_zero_distance() {
        // Distance < 1.0 should be clamped to 1.0 internally.
        let a = compute_wall_light(20, 0.5);
        let b = compute_wall_light(20, 1.0);
        assert_eq!(a, b, "distances below MIN_DISTANCE should be clamped");
    }

    #[test]
    fn wall_light_base_zero_stays_zero() {
        // If the sector is already full-bright (base 0), distance cannot
        // make it even brighter — saturating_sub clamps at 0.
        assert_eq!(compute_wall_light(0, 1.0), 0);
        assert_eq!(compute_wall_light(0, 1000.0), 0);
    }

    // -----------------------------------------------------------------------
    // compute_flat_light
    // -----------------------------------------------------------------------

    #[test]
    fn flat_light_matches_wall_light() {
        for d in [1.0, 50.0, 200.0, 1000.0] {
            assert_eq!(
                compute_flat_light(15, d),
                compute_wall_light(15, d),
                "flat and wall should be identical at distance {d}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // compute_wall_light_with_falloff
    // -----------------------------------------------------------------------

    #[test]
    fn falloff_center_equals_base_wall_light() {
        // Centre column (x == 160) should have no angular penalty.
        let base = compute_wall_light(20, 200.0);
        let with_falloff = compute_wall_light_with_falloff(20, 200.0, SCREEN_W / 2);
        assert_eq!(with_falloff, base);
    }

    #[test]
    fn falloff_edge_is_darker_than_center() {
        let center = compute_wall_light_with_falloff(20, 200.0, SCREEN_W / 2);
        let edge = compute_wall_light_with_falloff(20, 200.0, 0);
        assert!(edge >= center, "edges should be >= (darker) than center");
    }

    #[test]
    fn falloff_never_exceeds_max() {
        let result = compute_wall_light_with_falloff(31, 10000.0, 0);
        assert!(result <= MAX_COLORMAP_INDEX);
    }

    #[test]
    fn falloff_symmetric() {
        let left = compute_wall_light_with_falloff(20, 200.0, 10);
        let right = compute_wall_light_with_falloff(20, 200.0, SCREEN_W - 10);
        assert_eq!(left, right, "symmetrically placed columns should match");
    }

    // -----------------------------------------------------------------------
    // shade_pixel
    // -----------------------------------------------------------------------

    #[test]
    fn shade_pixel_identity() {
        let mut map = [0u8; 256];
        for i in 0..256 {
            map[i] = i as u8;
        }
        for i in 0..=255u8 {
            assert_eq!(shade_pixel(&map, i), i);
        }
    }

    #[test]
    fn shade_pixel_remap() {
        let mut map = [0u8; 256];
        map[42] = 99;
        assert_eq!(shade_pixel(&map, 42), 99);
    }

    // -----------------------------------------------------------------------
    // shade_column
    // -----------------------------------------------------------------------

    #[test]
    fn shade_column_applies_colormap() {
        let width = 4;
        let height = 4;
        let mut fb = vec![10u8; width * height];
        let mut map = [0u8; 256];
        map[10] = 77;

        shade_column(&mut fb, width, 1, 0, 3, &map);

        // Column 1 should all be remapped to 77.
        for y in 0..height {
            assert_eq!(fb[y * width + 1], 77, "row {y} col 1");
        }
        // Column 0 should be untouched.
        for y in 0..height {
            assert_eq!(fb[y * width + 0], 10, "row {y} col 0 untouched");
        }
    }

    #[test]
    fn shade_column_out_of_bounds_x() {
        let mut fb = vec![5u8; 16];
        let map = [0u8; 256];
        // x >= fb_width should be a no-op.
        shade_column(&mut fb, 4, 10, 0, 3, &map);
        assert!(fb.iter().all(|&b| b == 5), "out-of-bounds x is a no-op");
    }

    #[test]
    fn shade_column_empty_range() {
        let mut fb = vec![5u8; 16];
        let map = [0u8; 256];
        // y_top > y_bot (after clamping) should be a no-op.
        shade_column(&mut fb, 4, 0, 3, 1, &map);
        // The clamping logic turns this into top=3, bot=1 which is top > bot => no-op.
        // Actually both get clamped to valid range, so top=3, bot=1 means top > bot => skip.
        assert!(fb.iter().all(|&b| b == 5));
    }

    // -----------------------------------------------------------------------
    // shade_span
    // -----------------------------------------------------------------------

    #[test]
    fn shade_span_applies_colormap() {
        let width = 4;
        let height = 4;
        let mut fb = vec![20u8; width * height];
        let mut map = [0u8; 256];
        map[20] = 88;

        shade_span(&mut fb, width, 2, 0, 3, &map);

        // Row 2 should all be remapped.
        for x in 0..width {
            assert_eq!(fb[2 * width + x], 88, "row 2 col {x}");
        }
        // Row 0 untouched.
        for x in 0..width {
            assert_eq!(fb[0 * width + x], 20, "row 0 col {x} untouched");
        }
    }

    #[test]
    fn shade_span_out_of_bounds_y() {
        let mut fb = vec![5u8; 16];
        let map = [0u8; 256];
        shade_span(&mut fb, 4, 100, 0, 3, &map);
        assert!(fb.iter().all(|&b| b == 5), "out-of-bounds y is a no-op");
    }

    // -----------------------------------------------------------------------
    // LightParams — construction
    // -----------------------------------------------------------------------

    #[test]
    fn light_params_default_is_fullbright() {
        let lp = LightParams::default();
        assert!(lp.is_fullbright());
        assert_eq!(lp.base_index(), 0);
    }

    #[test]
    fn light_params_new_fullbright_flag() {
        let lp = LightParams::new(100, true);
        assert!(lp.is_fullbright());
        assert_eq!(lp.base_index(), 0);
    }

    #[test]
    fn light_params_new_auto_fullbright_at_255() {
        let lp = LightParams::new(255, false);
        assert!(lp.is_fullbright());
        assert_eq!(lp.base_index(), 0);
    }

    #[test]
    fn light_params_new_normal() {
        let lp = LightParams::new(128, false);
        assert!(!lp.is_fullbright());
        assert_eq!(lp.base_index(), light_to_colormap_index(128));
    }

    // -----------------------------------------------------------------------
    // LightParams — index methods
    // -----------------------------------------------------------------------

    #[test]
    fn colormap_for_distance_fullbright() {
        let lp = LightParams::new(255, false);
        assert_eq!(lp.colormap_for_distance(500.0), 0);
    }

    #[test]
    fn colormap_for_distance_normal() {
        let lp = LightParams::new(128, false);
        let idx = lp.colormap_for_distance(200.0);
        assert!(idx <= MAX_COLORMAP_INDEX as usize);
    }

    #[test]
    fn colormap_for_wall_has_falloff() {
        let lp = LightParams::new(128, false);
        let center = lp.colormap_for_wall(200.0, SCREEN_W / 2);
        let edge = lp.colormap_for_wall(200.0, 0);
        assert!(edge >= center);
    }

    #[test]
    fn colormap_for_flat_equals_distance() {
        let lp = LightParams::new(128, false);
        assert_eq!(lp.colormap_for_flat(300.0), lp.colormap_for_distance(300.0));
    }

    // -----------------------------------------------------------------------
    // LightParams — cache lookups
    // -----------------------------------------------------------------------

    #[test]
    fn get_colormap_returns_correct_row() {
        let cache = test_cache();
        let lp = LightParams::new(128, false);
        let idx = lp.colormap_for_distance(200.0);
        let row = lp.get_colormap(200.0, &cache);
        // In our test cache, row N has all bytes = N.
        assert_eq!(row[0], idx as u8);
    }

    #[test]
    fn get_wall_colormap_returns_correct_row() {
        let cache = test_cache();
        let lp = LightParams::new(128, false);
        let idx = lp.colormap_for_wall(200.0, 100);
        let row = lp.get_wall_colormap(200.0, 100, &cache);
        assert_eq!(row[0], idx as u8);
    }

    #[test]
    fn get_flat_colormap_returns_correct_row() {
        let cache = test_cache();
        let lp = LightParams::new(128, false);
        let idx = lp.colormap_for_flat(200.0);
        let row = lp.get_flat_colormap(200.0, &cache);
        assert_eq!(row[0], idx as u8);
    }

    #[test]
    fn get_colormap_fullbright_returns_row_zero() {
        let cache = test_cache();
        let lp = LightParams::default();
        let row = lp.get_colormap(500.0, &cache);
        // Row 0 in test cache: all bytes = 0.
        assert_eq!(row[0], 0);
    }

    // -----------------------------------------------------------------------
    // Edge cases / boundary
    // -----------------------------------------------------------------------

    #[test]
    fn shade_column_single_pixel() {
        let mut fb = vec![7u8; 4];
        let mut map = [0u8; 256];
        map[7] = 42;
        shade_column(&mut fb, 2, 0, 0, 0, &map);
        assert_eq!(fb[0], 42);
        assert_eq!(fb[1], 7); // untouched
    }

    #[test]
    fn shade_span_single_pixel() {
        let mut fb = vec![7u8; 4];
        let mut map = [0u8; 256];
        map[7] = 42;
        shade_span(&mut fb, 2, 0, 0, 0, &map);
        assert_eq!(fb[0], 42);
        assert_eq!(fb[1], 7); // untouched
    }

    #[test]
    fn shade_column_zero_width_is_noop() {
        let mut fb = vec![5u8; 4];
        let map = [0u8; 256];
        shade_column(&mut fb, 0, 0, 0, 3, &map);
        assert!(fb.iter().all(|&b| b == 5));
    }

    #[test]
    fn shade_span_zero_width_is_noop() {
        let mut fb = vec![5u8; 4];
        let map = [0u8; 256];
        shade_span(&mut fb, 0, 0, 0, 3, &map);
        assert!(fb.iter().all(|&b| b == 5));
    }
}
