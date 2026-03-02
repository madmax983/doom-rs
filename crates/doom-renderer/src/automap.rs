//! 2D overhead automap renderer.
//!
//! Draws all linedefs from a `Level` scaled to fit the 320x200 framebuffer,
//! colour-coded by linedef type, with a directional player arrow marker.
//!
//! # Coordinate system
//! Doom Y increases upward; screen Y increases downward.  The world->screen
//! transform flips Y so north remains up on the automap.
//!
//! # Usage
//! The automap supports two modes:
//! - **Legacy**: Call [`draw_automap`] directly with player coords (auto-fit zoom).
//! - **Stateful**: Create an [`AutomapState`], call [`draw_automap_ex`] for
//!   interactive zoom/pan/follow.

use doom_map::Level;
use doom_types::Bam;

use crate::framebuffer::Framebuffer;
use crate::palette::PaletteLut;

// ---------------------------------------------------------------------------
// Screen constants (mirrors FB_WIDTH / FB_HEIGHT)
// ---------------------------------------------------------------------------

const SCREEN_W: i32 = 320;
const SCREEN_H: i32 = 200;
const HALF_W: i32 = SCREEN_W / 2; // 160
const HALF_H: i32 = SCREEN_H / 2; // 100

// ---------------------------------------------------------------------------
// Palette indices — Doom automap colour scheme
// ---------------------------------------------------------------------------

/// One-sided wall (solid, no left sidedef).
const COLOR_ONE_SIDED: u8 = 176; // red
/// Two-sided line, no height difference between sectors.
const COLOR_TWO_SIDED: u8 = 64; // brown
/// Two-sided line with a floor or ceiling height change.
const COLOR_HEIGHT_CHANGE: u8 = 231; // yellow
/// Secret line (linedef flag bit 5).
const COLOR_SECRET: u8 = 252; // purple
/// Unseen line (future visibility tracking — not yet wired).
#[allow(dead_code)]
const COLOR_UNSEEN: u8 = 96; // gray
/// Player arrow marker.
const COLOR_PLAYER: u8 = 119; // green (classic automap player arrow)
/// Background fill.
const COLOR_BACKGROUND: u8 = 0; // black

/// Linedef flag bit 5 — secret wall.
const FLAG_SECRET: u16 = 0x0020;

// Padding fraction applied to each side of the computed map bounds (legacy mode).
const PADDING_FRAC: f32 = 0.05;

// ---------------------------------------------------------------------------
// AutomapState
// ---------------------------------------------------------------------------

/// Interactive state for the automap overlay.
///
/// Controls zoom level, center position, and whether the map follows the
/// player automatically.
#[derive(Debug, Clone)]
pub struct AutomapState {
    /// Whether the automap is currently visible.
    pub active: bool,
    /// Zoom level: pixels per map unit (higher = more zoomed in).
    pub zoom: f32,
    /// Center X in map-space coordinates.
    pub center_x: f32,
    /// Center Y in map-space coordinates.
    pub center_y: f32,
    /// When true, center tracks the player position each frame.
    pub follow_player: bool,
}

/// Maximum zoom (pixels per map unit).
const ZOOM_MAX: f32 = 4.0;
/// Minimum zoom (pixels per map unit).
const ZOOM_MIN: f32 = 0.1;
/// Zoom multiplier for each zoom-in step.
const ZOOM_FACTOR: f32 = 1.2;

impl AutomapState {
    /// Create a new `AutomapState` with sensible defaults.
    ///
    /// Starts inactive, zoom 0.5, follow-player enabled, center at origin.
    pub fn new() -> Self {
        Self {
            active: false,
            zoom: 0.5,
            center_x: 0.0,
            center_y: 0.0,
            follow_player: true,
        }
    }

    /// Toggle the automap on/off.
    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Zoom in by multiplying zoom by [`ZOOM_FACTOR`], capped at [`ZOOM_MAX`].
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * ZOOM_FACTOR).min(ZOOM_MAX);
    }

    /// Zoom out by dividing zoom by [`ZOOM_FACTOR`], floored at [`ZOOM_MIN`].
    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / ZOOM_FACTOR).max(ZOOM_MIN);
    }

    /// Update the center to follow the player position (if `follow_player` is enabled).
    ///
    /// `player_x` and `player_y` are in map units (not fixed-point).
    pub fn update_center(&mut self, player_x: i32, player_y: i32) {
        if self.follow_player {
            self.center_x = player_x as f32;
            self.center_y = player_y as f32;
        }
    }
}

impl Default for AutomapState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Public API — legacy (auto-fit) entry point
// ---------------------------------------------------------------------------

/// Render a 2D overhead automap of `level` into `fb` (legacy auto-fit mode).
///
/// This is the backward-compatible entry point that auto-computes zoom and
/// center from the level bounds.
///
/// * `player_x`, `player_y` — player position in Doom map units.
/// * `player_angle` — player facing angle (used for the directional arrow).
/// * `_palette` — unused for now (automap uses fixed palette indices).
pub fn draw_automap(
    level: &Level,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
    fb: &mut Framebuffer,
    _palette: &PaletteLut,
) {
    // Fill background.
    fb.clear(COLOR_BACKGROUND);

    // Compute map bounds from vertexes.
    let Some((min_x, max_x, min_y, max_y)) = map_bounds(level) else {
        return;
    };

    let map_w = (max_x - min_x) as f32;
    let map_h = (max_y - min_y) as f32;

    if map_w < 1.0 || map_h < 1.0 {
        return;
    }

    let padded_w = map_w * (1.0 + 2.0 * PADDING_FRAC);
    let padded_h = map_h * (1.0 + 2.0 * PADDING_FRAC);

    let scale = (SCREEN_W as f32 / padded_w).min(SCREEN_H as f32 / padded_h);

    // Build an ephemeral AutomapState centered on the map.
    let center_x = (min_x as f32 + max_x as f32) / 2.0;
    let center_y = (min_y as f32 + max_y as f32) / 2.0;

    // Use the internal draw with this computed scale/center.
    draw_automap_internal(
        fb,
        level,
        center_x,
        center_y,
        scale,
        player_x,
        player_y,
        player_angle,
    );
}

// ---------------------------------------------------------------------------
// Public API — stateful entry point
// ---------------------------------------------------------------------------

/// Render a 2D overhead automap using the interactive [`AutomapState`].
///
/// This is the preferred entry point for interactive automap usage with
/// zoom/pan controls.
///
/// * `player_x`, `player_y` — player position in Doom map units.
/// * `player_angle` — player facing angle (used for the directional arrow).
pub fn draw_automap_ex(
    fb: &mut Framebuffer,
    level: &Level,
    state: &AutomapState,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
) {
    fb.clear(COLOR_BACKGROUND);

    draw_automap_internal(
        fb,
        level,
        state.center_x,
        state.center_y,
        state.zoom,
        player_x,
        player_y,
        player_angle,
    );
}

// ---------------------------------------------------------------------------
// Internal rendering
// ---------------------------------------------------------------------------

/// Core automap rendering: draws linedefs + player arrow.
fn draw_automap_internal(
    fb: &mut Framebuffer,
    level: &Level,
    center_x: f32,
    center_y: f32,
    zoom: f32,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
) {
    if level.vertexes.is_empty() {
        return;
    }

    // World -> screen coordinate transform.
    let world_to_screen = |wx: f32, wy: f32| -> (i32, i32) {
        let sx = HALF_W + ((wx - center_x) * zoom) as i32;
        let sy = HALF_H - ((wy - center_y) * zoom) as i32;
        (sx, sy)
    };

    // Draw all linedefs.
    for ld in &level.linedefs {
        let color = line_color(ld, level);

        let Some(v1) = level.vertexes.get(ld.from_vertex as usize) else {
            continue;
        };
        let Some(v2) = level.vertexes.get(ld.to_vertex as usize) else {
            continue;
        };

        let (x0, y0) = world_to_screen(v1.x as f32, v1.y as f32);
        let (x1, y1) = world_to_screen(v2.x as f32, v2.y as f32);

        draw_line(fb, x0, y0, x1, y1, color);
    }

    // Draw player arrow.
    let (px, py) = world_to_screen(player_x as f32, player_y as f32);
    draw_player_arrow(fb, px, py, player_angle);
}

// ---------------------------------------------------------------------------
// Line colour classification
// ---------------------------------------------------------------------------

/// Choose an automap colour for a linedef based on its properties.
///
/// Colour priority:
/// 1. **Secret** (flag bit 5): purple
/// 2. **One-sided** (no left sidedef): red
/// 3. **Two-sided with height change** (floor or ceiling differs): yellow
/// 4. **Two-sided normal**: brown
pub fn line_color(ld: &doom_map::Linedef, level: &Level) -> u8 {
    // Secret lines get a distinct colour.
    if ld.flags & FLAG_SECRET != 0 {
        return COLOR_SECRET;
    }

    if !ld.is_two_sided() {
        return COLOR_ONE_SIDED;
    }

    // Two-sided: check for height differences between front/back sectors.
    if has_height_change(ld, level) {
        COLOR_HEIGHT_CHANGE
    } else {
        COLOR_TWO_SIDED
    }
}

/// Returns `true` if the front and back sectors of a two-sided linedef have
/// different floor or ceiling heights.
fn has_height_change(ld: &doom_map::Linedef, level: &Level) -> bool {
    let right_sd = match level.sidedefs.get(ld.right_sidedef as usize) {
        Some(sd) => sd,
        None => return false,
    };
    let left_sd = match level.sidedefs.get(ld.left_sidedef as usize) {
        Some(sd) => sd,
        None => return false,
    };

    let right_sector = match level.sectors.get(right_sd.sector as usize) {
        Some(s) => s,
        None => return false,
    };
    let left_sector = match level.sectors.get(left_sd.sector as usize) {
        Some(s) => s,
        None => return false,
    };

    right_sector.floor_height != left_sector.floor_height
        || right_sector.ceil_height != left_sector.ceil_height
}

// ---------------------------------------------------------------------------
// Player arrow
// ---------------------------------------------------------------------------

/// Draw a directional arrow at the player's screen position.
///
/// The arrow is approximately 8 pixels long, pointing in `player_angle`.
/// Three lines form the arrowhead shape:
///   - A shaft from tail to tip
///   - Two barbs angled 135 degrees from the shaft
fn draw_player_arrow(fb: &mut Framebuffer, px: i32, py: i32, player_angle: Bam) {
    // Convert BAM angle to radians (f32 is fine for renderer-only math).
    let angle_rad = (player_angle.0 as f64) * core::f64::consts::TAU / (u32::MAX as f64 + 1.0);
    let cos_a = angle_rad.cos() as f32;
    let sin_a = angle_rad.sin() as f32;

    // Arrow dimensions (in screen pixels).
    let shaft_len: f32 = 8.0;
    let barb_len: f32 = 4.0;

    // Tip of the arrow (ahead of the player position).
    // Note: screen Y is flipped, so sin component is subtracted.
    let tip_x = px as f32 + cos_a * shaft_len;
    let tip_y = py as f32 - sin_a * shaft_len;

    // Tail of the arrow (behind the player position).
    let tail_x = px as f32 - cos_a * shaft_len;
    let tail_y = py as f32 + sin_a * shaft_len;

    // Barb angles: 135 degrees from the forward direction (each side).
    let barb_angle_offset = core::f64::consts::FRAC_PI_4 * 3.0; // 135 degrees

    let left_barb_angle = angle_rad + barb_angle_offset;
    let right_barb_angle = angle_rad - barb_angle_offset;

    let left_x = tip_x + (left_barb_angle.cos() as f32) * barb_len;
    let left_y = tip_y - (left_barb_angle.sin() as f32) * barb_len;

    let right_x = tip_x + (right_barb_angle.cos() as f32) * barb_len;
    let right_y = tip_y - (right_barb_angle.sin() as f32) * barb_len;

    // Draw shaft: tail -> tip
    draw_line(
        fb,
        tail_x as i32,
        tail_y as i32,
        tip_x as i32,
        tip_y as i32,
        COLOR_PLAYER,
    );
    // Draw left barb: tip -> left
    draw_line(
        fb,
        tip_x as i32,
        tip_y as i32,
        left_x as i32,
        left_y as i32,
        COLOR_PLAYER,
    );
    // Draw right barb: tip -> right
    draw_line(
        fb,
        tip_x as i32,
        tip_y as i32,
        right_x as i32,
        right_y as i32,
        COLOR_PLAYER,
    );
}

// ---------------------------------------------------------------------------
// Map bounds
// ---------------------------------------------------------------------------

/// Compute min/max world-coordinate bounds of all vertexes.
///
/// Returns `None` if the level has no vertexes.
fn map_bounds(level: &Level) -> Option<(i32, i32, i32, i32)> {
    let mut iter = level.vertexes.iter();
    let first = iter.next()?;

    let mut min_x = i32::from(first.x);
    let mut max_x = i32::from(first.x);
    let mut min_y = i32::from(first.y);
    let mut max_y = i32::from(first.y);

    for v in iter {
        let vx = i32::from(v.x);
        let vy = i32::from(v.y);
        if vx < min_x {
            min_x = vx;
        }
        if vx > max_x {
            max_x = vx;
        }
        if vy < min_y {
            min_y = vy;
        }
        if vy > max_y {
            max_y = vy;
        }
    }

    Some((min_x, max_x, min_y, max_y))
}

// ---------------------------------------------------------------------------
// Bresenham line rasteriser
// ---------------------------------------------------------------------------

/// Integer Bresenham line drawing.
///
/// Pixels outside `[0, 319] x [0, 199]` are silently skipped (no panic).
fn draw_line(fb: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };

    let mut x = x0;
    let mut y = y0;
    let mut err = dx - dy;

    loop {
        // Only plot pixels within screen bounds.
        if x >= 0 && x < SCREEN_W && y >= 0 && y < SCREEN_H {
            fb.set_pixel(x as usize, y as usize, color);
        }

        if x == x1 && y == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_map::Level;
    use doom_map::lumps::{
        Blockmap, FLAG_TWO_SIDED, Linedef as LdRaw, Reject, Sector, Sidedef as SdRaw, Ssector,
        Vertex as VxRaw,
    };

    // -----------------------------------------------------------------------
    // Minimal Level builder
    // -----------------------------------------------------------------------

    /// Build a `Level` with caller-supplied vertexes, linedefs, sidedefs, and
    /// sectors.  Provides sensible defaults for BSP data.
    fn make_level_full(
        vertexes: Vec<VxRaw>,
        linedefs: Vec<LdRaw>,
        sidedefs: Vec<SdRaw>,
        sectors: Vec<Sector>,
    ) -> Level {
        let n_sectors = sectors.len().max(1);

        // Ensure at least one sector for reject table sizing.
        let final_sectors = if sectors.is_empty() {
            vec![Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            }]
        } else {
            sectors
        };

        let ssector = Ssector {
            seg_count: 0,
            first_seg: 0,
        };

        let mut bm_bytes = vec![0u8; 8 + 2 + 4];
        bm_bytes[0..2].copy_from_slice(&0i16.to_le_bytes());
        bm_bytes[2..4].copy_from_slice(&0i16.to_le_bytes());
        bm_bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_bytes[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_bytes[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_bytes[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_bytes[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_bytes).expect("blockmap parse");

        let reject_size = (n_sectors * n_sectors + 7) / 8;
        let reject = Reject::parse_lump(&vec![0u8; reject_size], n_sectors).expect("reject parse");

        Level {
            name: "TEST".to_owned(),
            things: vec![],
            linedefs,
            sidedefs,
            vertexes,
            segs: vec![],
            ssectors: vec![ssector],
            nodes: vec![],
            sectors: final_sectors,
            reject,
            blockmap,
        }
    }

    /// Build a simple level with only vertexes and linedefs (no sidedefs/sectors).
    fn make_level(vertexes: Vec<VxRaw>, linedefs: Vec<LdRaw>) -> Level {
        make_level_full(vertexes, linedefs, vec![], vec![])
    }

    /// Helper to create a sidedef pointing at a given sector.
    fn make_sidedef(sector: u16) -> SdRaw {
        SdRaw {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"--------",
            lower_texture: *b"--------",
            middle_texture: *b"--------",
            sector,
        }
    }

    // -----------------------------------------------------------------------
    // Test 1: AutomapState::new() defaults
    // -----------------------------------------------------------------------

    #[test]
    fn automap_state_new_defaults() {
        let state = AutomapState::new();
        assert!(!state.active);
        assert!((state.zoom - 0.5).abs() < f32::EPSILON);
        assert!((state.center_x - 0.0).abs() < f32::EPSILON);
        assert!((state.center_y - 0.0).abs() < f32::EPSILON);
        assert!(state.follow_player);
    }

    // -----------------------------------------------------------------------
    // Test 2: toggle flips active
    // -----------------------------------------------------------------------

    #[test]
    fn toggle_flips_active() {
        let mut state = AutomapState::new();
        assert!(!state.active);
        state.toggle();
        assert!(state.active);
        state.toggle();
        assert!(!state.active);
    }

    // -----------------------------------------------------------------------
    // Test 3: zoom_in increases zoom
    // -----------------------------------------------------------------------

    #[test]
    fn zoom_in_increases_zoom() {
        let mut state = AutomapState::new();
        let initial = state.zoom;
        state.zoom_in();
        assert!(state.zoom > initial);
    }

    // -----------------------------------------------------------------------
    // Test 4: zoom_out decreases zoom
    // -----------------------------------------------------------------------

    #[test]
    fn zoom_out_decreases_zoom() {
        let mut state = AutomapState::new();
        let initial = state.zoom;
        state.zoom_out();
        assert!(state.zoom < initial);
    }

    // -----------------------------------------------------------------------
    // Test 5: zoom_in caps at ZOOM_MAX
    // -----------------------------------------------------------------------

    #[test]
    fn zoom_in_caps_at_max() {
        let mut state = AutomapState::new();
        // Zoom in many times to exceed the cap.
        for _ in 0..100 {
            state.zoom_in();
        }
        assert!(
            state.zoom <= ZOOM_MAX,
            "zoom {} should be <= {ZOOM_MAX}",
            state.zoom
        );
        assert!(
            (state.zoom - ZOOM_MAX).abs() < f32::EPSILON,
            "zoom should be at max after many zoom_in calls"
        );
    }

    // -----------------------------------------------------------------------
    // Test 6: zoom_out floors at ZOOM_MIN
    // -----------------------------------------------------------------------

    #[test]
    fn zoom_out_floors_at_min() {
        let mut state = AutomapState::new();
        for _ in 0..100 {
            state.zoom_out();
        }
        assert!(
            state.zoom >= ZOOM_MIN,
            "zoom {} should be >= {ZOOM_MIN}",
            state.zoom
        );
        assert!(
            (state.zoom - ZOOM_MIN).abs() < f32::EPSILON,
            "zoom should be at min after many zoom_out calls"
        );
    }

    // -----------------------------------------------------------------------
    // Test 7: update_center follows player when follow=true
    // -----------------------------------------------------------------------

    #[test]
    fn update_center_follows_player() {
        let mut state = AutomapState::new();
        state.follow_player = true;
        state.update_center(100, 200);
        assert!((state.center_x - 100.0).abs() < f32::EPSILON);
        assert!((state.center_y - 200.0).abs() < f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Test 8: update_center ignores when follow=false
    // -----------------------------------------------------------------------

    #[test]
    fn update_center_ignores_when_follow_disabled() {
        let mut state = AutomapState::new();
        state.follow_player = false;
        state.center_x = 42.0;
        state.center_y = 99.0;
        state.update_center(100, 200);
        assert!((state.center_x - 42.0).abs() < f32::EPSILON);
        assert!((state.center_y - 99.0).abs() < f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Test 9: draw_line horizontal
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_horizontal() {
        let mut fb = Framebuffer::new();
        draw_line(&mut fb, 10, 50, 50, 50, 176);

        for x in 10..=50 {
            assert_eq!(
                fb.get_pixel(x, 50),
                Some(176),
                "pixel ({x}, 50) should be 176"
            );
        }
        assert_eq!(fb.get_pixel(9, 50), Some(0));
        assert_eq!(fb.get_pixel(51, 50), Some(0));
    }

    // -----------------------------------------------------------------------
    // Test 10: draw_line vertical
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_vertical() {
        let mut fb = Framebuffer::new();
        draw_line(&mut fb, 100, 20, 100, 80, 231);

        for y in 20..=80 {
            assert_eq!(
                fb.get_pixel(100, y),
                Some(231),
                "pixel (100, {y}) should be 231"
            );
        }
        assert_eq!(fb.get_pixel(100, 19), Some(0));
        assert_eq!(fb.get_pixel(100, 81), Some(0));
    }

    // -----------------------------------------------------------------------
    // Test 11: draw_line clips to bounds (no panic)
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_clips_to_bounds() {
        let mut fb = Framebuffer::new();
        // Coordinates far outside — should not panic.
        draw_line(&mut fb, -5000, -5000, 5000, 5000, 255);
        // Verify some edge pixels that should be set (the diagonal
        // crosses [0,0] to [319,199]).
        assert_eq!(fb.get_pixel(0, 0), Some(255));
    }

    // -----------------------------------------------------------------------
    // Test 12: draw_automap on empty level doesn't panic
    // -----------------------------------------------------------------------

    #[test]
    fn draw_automap_empty_level_does_not_panic() {
        let level = make_level(vec![], vec![]);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        draw_automap(&level, 0, 0, Bam(0), &mut fb, &palette);
        // Background must be all black since there are no vertexes.
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    // -----------------------------------------------------------------------
    // Test 13: line_color returns correct color for one-sided wall
    // -----------------------------------------------------------------------

    #[test]
    fn line_color_one_sided() {
        let level = make_level(vec![], vec![]);
        let ld = LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0, // NOT two-sided
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF, // no left sidedef
        };
        assert_eq!(line_color(&ld, &level), COLOR_ONE_SIDED);
    }

    // -----------------------------------------------------------------------
    // Test 14: line_color returns correct color for two-sided wall (no height change)
    // -----------------------------------------------------------------------

    #[test]
    fn line_color_two_sided_same_height() {
        // Two sectors with identical floor/ceiling.
        let sectors = vec![
            Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        let sidedefs = vec![make_sidedef(0), make_sidedef(1)];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }],
            vec![],
            sidedefs,
            sectors,
        );

        let ld = LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: FLAG_TWO_SIDED,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        };
        assert_eq!(line_color(&ld, &level), COLOR_TWO_SIDED);
    }

    // -----------------------------------------------------------------------
    // Test 15: line_color returns yellow for two-sided wall with height change
    // -----------------------------------------------------------------------

    #[test]
    fn line_color_two_sided_height_change() {
        let sectors = vec![
            Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: 24, // different floor height
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        let sidedefs = vec![make_sidedef(0), make_sidedef(1)];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }],
            vec![],
            sidedefs,
            sectors,
        );

        let ld = LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: FLAG_TWO_SIDED,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        };
        assert_eq!(line_color(&ld, &level), COLOR_HEIGHT_CHANGE);
    }

    // -----------------------------------------------------------------------
    // Test 16: line_color returns purple for secret lines
    // -----------------------------------------------------------------------

    #[test]
    fn line_color_secret() {
        let level = make_level(vec![], vec![]);
        let ld = LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: FLAG_SECRET, // bit 5 set
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF,
        };
        assert_eq!(line_color(&ld, &level), COLOR_SECRET);
    }

    // -----------------------------------------------------------------------
    // Test 17: draw_automap_ex on empty level doesn't panic
    // -----------------------------------------------------------------------

    #[test]
    fn draw_automap_ex_empty_level_does_not_panic() {
        let level = make_level(vec![], vec![]);
        let mut fb = Framebuffer::new();
        let state = AutomapState::new();
        draw_automap_ex(&mut fb, &level, &state, 0, 0, Bam(0));
        assert!(fb.data.iter().all(|&b| b == 0));
    }

    // -----------------------------------------------------------------------
    // Test 18: draw_automap single linedef produces pixels
    // -----------------------------------------------------------------------

    #[test]
    fn draw_automap_single_linedef_marks_pixels() {
        let vertexes = vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 100 }];
        let linedefs = vec![LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0xFFFF,
            left_sidedef: 0xFFFF,
        }];
        let level = make_level(vertexes, linedefs);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        draw_automap(&level, 50, 50, Bam(0), &mut fb, &palette);

        let has_non_black = fb.data.iter().any(|&b| b != 0);
        assert!(
            has_non_black,
            "expected at least one non-black pixel after drawing a linedef"
        );
    }

    // -----------------------------------------------------------------------
    // Test 19: draw_line single point (degenerate line)
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_single_point() {
        let mut fb = Framebuffer::new();
        draw_line(&mut fb, 50, 50, 50, 50, 42);
        assert_eq!(fb.get_pixel(50, 50), Some(42));
    }

    // -----------------------------------------------------------------------
    // Test 20: draw_automap_ex with zoomed state draws pixels
    // -----------------------------------------------------------------------

    #[test]
    fn draw_automap_ex_zoomed_draws_pixels() {
        let vertexes = vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 200, y: 0 }];
        let linedefs = vec![LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0xFFFF,
            left_sidedef: 0xFFFF,
        }];
        let level = make_level(vertexes, linedefs);
        let mut fb = Framebuffer::new();
        let mut state = AutomapState::new();
        state.active = true;
        state.zoom = 1.0;
        state.center_x = 100.0;
        state.center_y = 0.0;

        draw_automap_ex(&mut fb, &level, &state, 100, 0, Bam(0));

        // A 200 map-unit horizontal line centered at 100,0 with zoom=1.0
        // should span roughly from screen x=60 to x=260 at y=100.
        let has_non_black = fb.data.iter().any(|&b| b != 0);
        assert!(has_non_black, "zoomed automap should draw visible pixels");
    }

    // -----------------------------------------------------------------------
    // Test 21: player arrow draws pixels
    // -----------------------------------------------------------------------

    #[test]
    fn player_arrow_draws_pixels() {
        let mut fb = Framebuffer::new();
        draw_player_arrow(&mut fb, 160, 100, Bam(0));
        // The arrow should have drawn some pixels near the center.
        let has_non_black = fb.data.iter().any(|&b| b != 0);
        assert!(has_non_black, "player arrow should draw visible pixels");
    }

    // -----------------------------------------------------------------------
    // Test 22: draw_line diagonal
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_diagonal() {
        let mut fb = Framebuffer::new();
        draw_line(&mut fb, 0, 0, 10, 10, 200);
        // Bresenham diagonal should hit (0,0), (1,1), ... (10,10)
        for i in 0..=10 {
            assert_eq!(
                fb.get_pixel(i, i),
                Some(200),
                "pixel ({i}, {i}) should be 200"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test 23: line_color secret takes priority over two-sided
    // -----------------------------------------------------------------------

    #[test]
    fn line_color_secret_overrides_two_sided() {
        let sectors = vec![
            Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        let sidedefs = vec![make_sidedef(0), make_sidedef(1)];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }],
            vec![],
            sidedefs,
            sectors,
        );

        let ld = LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: FLAG_TWO_SIDED | FLAG_SECRET, // both flags
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        };
        // Secret should take priority.
        assert_eq!(line_color(&ld, &level), COLOR_SECRET);
    }

    // -----------------------------------------------------------------------
    // Test 24: AutomapState Default matches new()
    // -----------------------------------------------------------------------

    #[test]
    fn automap_state_default_matches_new() {
        let from_new = AutomapState::new();
        let from_default = AutomapState::default();
        assert_eq!(from_new.active, from_default.active);
        assert!((from_new.zoom - from_default.zoom).abs() < f32::EPSILON);
        assert!((from_new.center_x - from_default.center_x).abs() < f32::EPSILON);
        assert!((from_new.center_y - from_default.center_y).abs() < f32::EPSILON);
        assert_eq!(from_new.follow_player, from_default.follow_player);
    }
}
