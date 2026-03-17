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
//! The automap supports three modes:
//! - **Legacy**: Call [`draw_automap`] directly with player coords (auto-fit zoom).
//! - **Stateful**: Create an [`AutomapState`], call [`draw_automap_ex`] for
//!   interactive zoom/pan/follow.
//! - **Full integration**: Use [`RendererAutomapCanvas`] with
//!   [`doom_game::draw_automap_full`] to leverage game-side visibility tracking,
//!   grid overlay, and thing markers via the [`AutomapCanvas`] trait bridge.
//!
//! # Automap colour palette
//! The [`automap_colors`] module exposes classic Doom automap palette indices
//! as public constants.

use doom_game::AutomapCanvas;
use doom_game::AutomapState;
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
// Palette indices -- Doom automap colour scheme (internal, for legacy API)
// ---------------------------------------------------------------------------

/// One-sided wall (solid, no left sidedef).
const COLOR_ONE_SIDED: u8 = 176; // red
/// Two-sided line, no height difference between sectors.
const COLOR_TWO_SIDED: u8 = 64; // brown
/// Two-sided line with a floor or ceiling height change.
const COLOR_HEIGHT_CHANGE: u8 = 231; // yellow
/// Secret line (linedef flag bit 5).
const COLOR_SECRET: u8 = 252; // purple
/// Player arrow marker.
const COLOR_PLAYER: u8 = 119; // green (classic automap player arrow)
/// Background fill.
const COLOR_BACKGROUND: u8 = 0; // black

/// Linedef flag bit 5 -- secret wall.
const FLAG_SECRET: u16 = 0x0020;

// Padding fraction applied to each side of the computed map bounds (legacy mode).
const PADDING_FRAC: f32 = 0.05;

// ---------------------------------------------------------------------------
// Public automap colour constants
// ---------------------------------------------------------------------------

/// Classic Doom automap palette indices (PLAYPAL colour table).
///
/// These constants map to the original Doom automap colours and can be used
/// by external code that needs to query or override automap drawing colours.
pub mod automap_colors {
    /// Background: black.
    pub const BACKGROUND: u8 = 0;
    /// One-sided (solid) wall: red.
    pub const WALL: u8 = 176;
    /// Two-sided wall with floor/ceiling change: brown/dark tan.
    pub const TWO_SIDED: u8 = 64;
    /// Floor height change across two-sided line: yellow-ish.
    pub const FLOOR_CHANGE: u8 = 231;
    /// Ceiling-only change across two-sided line: dark brown.
    pub const CEIL_CHANGE: u8 = 163;
    /// Secret sector linedef: bright purple/yellow.
    pub const SECRET: u8 = 252;
    /// Not yet seen but mapped (cheat-revealed): gray.
    pub const UNSEEN: u8 = 96;
    /// Player arrow marker: green.
    pub const PLAYER: u8 = 119;
    /// Monster marker: red.
    pub const MONSTER: u8 = 176;
    /// Item marker: yellow.
    pub const ITEM: u8 = 231;
    /// Blue keycard / skull key marker.
    pub const KEY_BLUE: u8 = 200;
    /// Red keycard / skull key marker.
    pub const KEY_RED: u8 = 176;
    /// Yellow keycard / skull key marker.
    pub const KEY_YELLOW: u8 = 231;
    /// Grid overlay: dark gray.
    pub const GRID: u8 = 104;
    /// Crosshair / player position indicator: green.
    pub const CROSSHAIR: u8 = 112;
}

// ---------------------------------------------------------------------------
// RendererAutomapCanvas -- bridges AutomapCanvas trait to Framebuffer
// ---------------------------------------------------------------------------

/// Automap canvas implementation that draws onto a [`Framebuffer`].
///
/// Implements [`doom_game::AutomapCanvas`] so that `doom_game::draw_automap_full`
/// can render directly into the renderer's framebuffer without the game crate
/// knowing about `Framebuffer`.
pub struct RendererAutomapCanvas<'a> {
    /// Mutable reference to the target framebuffer.
    fb: &'a mut Framebuffer,
}

impl<'a> RendererAutomapCanvas<'a> {
    /// Wrap a mutable `Framebuffer` reference as an `AutomapCanvas`.
    pub fn new(fb: &'a mut Framebuffer) -> Self {
        Self { fb }
    }
}

impl AutomapCanvas for RendererAutomapCanvas<'_> {
    fn set_pixel(&mut self, x: i32, y: i32, color: u8) {
        if (0..SCREEN_W).contains(&x) && (0..SCREEN_H).contains(&y) {
            self.fb.set_pixel(x as usize, y as usize, color);
        }
    }

    fn get_pixel(&self, x: i32, y: i32) -> Option<u8> {
        if (0..SCREEN_W).contains(&x) && (0..SCREEN_H).contains(&y) {
            self.fb.get_pixel(x as usize, y as usize)
        } else {
            None
        }
    }

    fn clear(&mut self, color: u8) {
        self.fb.clear(color);
    }

    fn width(&self) -> i32 {
        SCREEN_W
    }

    fn height(&self) -> i32 {
        SCREEN_H
    }
}

// ---------------------------------------------------------------------------
// render_automap -- full integration entry point
// ---------------------------------------------------------------------------

/// Render the automap overlay onto the framebuffer using the full game-side
/// automap logic (visibility tracking, grid, thing markers, cheat flags).
///
/// This delegates to [`doom_game::draw_automap_full`] through the
/// [`RendererAutomapCanvas`] bridge.
///
/// # Parameters
/// - `fb`: target framebuffer (320x200).
/// - `level`: current level geometry.
/// - `player_x`, `player_y`: player position in map units.
/// - `player_angle`: player facing angle (BAM).
/// - `zoom`: pixels per map unit.
/// - `show_all_lines`: IDDT cheat flag for revealing all linedefs.
/// - `show_all_things`: IDDT cheat flag for revealing all things.
/// - `seen_lines`: per-linedef visibility (from `GameState`).
pub fn render_automap(
    fb: &mut Framebuffer,
    level: &Level,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
    zoom: f32,
    show_all_lines: bool,
    show_all_things: bool,
    seen_lines: &[bool],
) {
    // Build automap state from parameters.
    let state = doom_game::AutomapState {
        active: true,
        zoom,
        center_x: player_x as f32,
        center_y: player_y as f32,
        follow_player: true,
        show_all_lines,
        show_all_things,
    };

    // Convert BAM angle to radians.
    let angle_rad = (player_angle.0 as f64) * core::f64::consts::TAU / (u32::MAX as f64 + 1.0);

    let mut canvas = RendererAutomapCanvas::new(fb);
    doom_game::draw_automap_full(
        &mut canvas,
        level,
        &state,
        seen_lines,
        player_x,
        player_y,
        angle_rad,
    );
}

// ---------------------------------------------------------------------------
// Grid drawing (renderer-side, for standalone use)
// ---------------------------------------------------------------------------

/// Draw a grid overlay onto the framebuffer at the given map center and zoom.
///
/// Grid lines are spaced at 128 map-unit intervals (the classic Doom grid).
/// Uses [`automap_colors::GRID`] colour.
///
/// This is a standalone renderer-side helper; [`render_automap`] already draws
/// a grid via the game-side logic.
pub fn draw_grid_on_fb(fb: &mut Framebuffer, center_x: f32, center_y: f32, zoom: f32) {
    let mut canvas = RendererAutomapCanvas::new(fb);
    doom_game::draw_grid(&mut canvas, center_x, center_y, zoom);
}

/// Draw a directional player arrow onto the framebuffer at a screen position.
///
/// The arrow is approximately 8 pixels long, pointing in `player_angle`.
/// Uses [`automap_colors::PLAYER`] colour.
pub fn draw_player_arrow_on_fb(fb: &mut Framebuffer, sx: i32, sy: i32, player_angle: Bam) {
    let angle_rad = (player_angle.0 as f64) * core::f64::consts::TAU / (u32::MAX as f64 + 1.0);

    let cos_a = angle_rad.cos() as f32;
    let sin_a = angle_rad.sin() as f32;

    let shaft_len: f32 = 8.0;
    let barb_len: f32 = 4.0;

    let tip_x = sx as f32 + cos_a * shaft_len;
    let tip_y = sy as f32 - sin_a * shaft_len;
    let tail_x = sx as f32 - cos_a * shaft_len;
    let tail_y = sy as f32 + sin_a * shaft_len;

    let barb_angle_offset = core::f64::consts::FRAC_PI_4 * 3.0;
    let left_barb_angle = angle_rad + barb_angle_offset;
    let right_barb_angle = angle_rad - barb_angle_offset;

    let left_x = tip_x + (left_barb_angle.cos() as f32) * barb_len;
    let left_y = tip_y - (left_barb_angle.sin() as f32) * barb_len;
    let right_x = tip_x + (right_barb_angle.cos() as f32) * barb_len;
    let right_y = tip_y - (right_barb_angle.sin() as f32) * barb_len;

    draw_line_fb(
        fb,
        tail_x as i32,
        tail_y as i32,
        tip_x as i32,
        tip_y as i32,
        COLOR_PLAYER,
    );
    draw_line_fb(
        fb,
        tip_x as i32,
        tip_y as i32,
        left_x as i32,
        left_y as i32,
        COLOR_PLAYER,
    );
    draw_line_fb(
        fb,
        tip_x as i32,
        tip_y as i32,
        right_x as i32,
        right_y as i32,
        COLOR_PLAYER,
    );
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Public API -- legacy (auto-fit) entry point
// ---------------------------------------------------------------------------

/// Render a 2D overhead automap of `level` into `fb` (legacy auto-fit mode).
///
/// This is the backward-compatible entry point that auto-computes zoom and
/// center from the level bounds.
///
/// * `player_x`, `player_y` -- player position in Doom map units.
/// * `player_angle` -- player facing angle (used for the directional arrow).
/// * `_palette` -- unused for now (automap uses fixed palette indices).
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
    let mut state = AutomapState::new();
    state.center_x = (min_x as f32 + max_x as f32) / 2.0;
    state.center_y = (min_y as f32 + max_y as f32) / 2.0;
    state.zoom = scale;

    // Use the internal draw with this computed scale/center.
    draw_automap_internal(fb, level, &state, player_x, player_y, player_angle);
}

// ---------------------------------------------------------------------------
// Public API -- stateful entry point
// ---------------------------------------------------------------------------

/// Render a 2D overhead automap using the interactive [`AutomapState`].
///
/// This is the preferred entry point for interactive automap usage with
/// zoom/pan controls.
///
/// * `player_x`, `player_y` -- player position in Doom map units.
/// * `player_angle` -- player facing angle (used for the directional arrow).
pub fn draw_automap_ex(
    fb: &mut Framebuffer,
    level: &Level,
    state: &AutomapState,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
) {
    fb.clear(COLOR_BACKGROUND);

    draw_automap_internal(fb, level, state, player_x, player_y, player_angle);
}

// ---------------------------------------------------------------------------
// Internal rendering
// ---------------------------------------------------------------------------

/// Core automap rendering: draws linedefs + player arrow.
fn draw_automap_internal(
    fb: &mut Framebuffer,
    level: &Level,
    state: &AutomapState,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
) {
    if level.vertexes.is_empty() {
        return;
    }

    let center_x = state.center_x;
    let center_y = state.center_y;
    let zoom = state.zoom;
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

        draw_line_fb(fb, x0, y0, x1, y1, color);
    }

    // Draw player arrow.
    let (px, py) = world_to_screen(player_x as f32, player_y as f32);
    draw_player_arrow_internal(fb, px, py, player_angle);
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
// Player arrow (internal, for legacy API)
// ---------------------------------------------------------------------------

/// Draw a directional arrow at the player's screen position.
///
/// The arrow is approximately 8 pixels long, pointing in `player_angle`.
/// Three lines form the arrowhead shape:
///   - A shaft from tail to tip
///   - Two barbs angled 135 degrees from the shaft
fn draw_player_arrow_internal(fb: &mut Framebuffer, px: i32, py: i32, player_angle: Bam) {
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
    draw_line_fb(
        fb,
        tail_x as i32,
        tail_y as i32,
        tip_x as i32,
        tip_y as i32,
        COLOR_PLAYER,
    );
    // Draw left barb: tip -> left
    draw_line_fb(
        fb,
        tip_x as i32,
        tip_y as i32,
        left_x as i32,
        left_y as i32,
        COLOR_PLAYER,
    );
    // Draw right barb: tip -> right
    draw_line_fb(
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
// Bresenham line rasteriser (framebuffer-direct)
// ---------------------------------------------------------------------------

/// Integer Bresenham line drawing directly onto a `Framebuffer`.
///
/// Pixels outside `[0, 319] x [0, 199]` are silently skipped (no panic).
pub fn draw_line_fb(fb: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };

    let mut x = x0;
    let mut y = y0;
    let mut err = dx - dy;

    loop {
        // Only plot pixels within screen bounds.
        if (0..SCREEN_W).contains(&x) && (0..SCREEN_H).contains(&y) {
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

// Backward-compat alias used in old tests.
#[cfg(test)]
#[allow(dead_code)]
fn draw_line(fb: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
    draw_line_fb(fb, x0, y0, x1, y1, color);
}

// ---------------------------------------------------------------------------
// Coordinate transform helpers (public, for external use)
// ---------------------------------------------------------------------------

/// Convert map coordinates to screen coordinates using the given center and zoom.
///
/// Doom Y increases upward; screen Y increases downward. The transform
/// flips Y so north remains up on the automap.
pub fn map_to_screen(
    map_x: f32,
    map_y: f32,
    center_x: f32,
    center_y: f32,
    zoom: f32,
) -> (i32, i32) {
    let sx = HALF_W + ((map_x - center_x) * zoom) as i32;
    let sy = HALF_H - ((map_y - center_y) * zoom) as i32;
    (sx, sy)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use super::*;
    use doom_game::AutomapState;
    use doom_map::Level;
    use doom_map::lumps::{
        Blockmap, Linedef as LdRaw, Reject, Sector, Sidedef as SdRaw, Ssector, Thing as ThingRaw,
        Vertex as VxRaw,
    };

    // -----------------------------------------------------------------------
    // Minimal Level builder
    // -----------------------------------------------------------------------

    /// Build a `Level` with caller-supplied vertexes, linedefs, sidedefs, and
    /// sectors.  Provides sensible defaults for BSP data.
    #[allow(dead_code)]
    fn make_level_full(
        vertexes: Vec<VxRaw>,
        linedefs: Vec<LdRaw>,
        sidedefs: Vec<SdRaw>,
        sectors: Vec<Sector>,
    ) -> Level {
        make_level_full_with_things(vertexes, linedefs, sidedefs, sectors, vec![])
    }

    #[allow(dead_code)]
    fn make_level_full_with_things(
        vertexes: Vec<VxRaw>,
        linedefs: Vec<LdRaw>,
        sidedefs: Vec<SdRaw>,
        sectors: Vec<Sector>,
        things: Vec<ThingRaw>,
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

        let reject_size = (n_sectors * n_sectors).div_ceil(8);
        let reject = Reject::parse_lump(&vec![0u8; reject_size], n_sectors).expect("reject parse");

        Level {
            name: "TEST".to_owned(),
            things,
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
    #[allow(dead_code)]
    fn make_level(vertexes: Vec<VxRaw>, linedefs: Vec<LdRaw>) -> Level {
        make_level_full(vertexes, linedefs, vec![], vec![])
    }

    /// Helper to create a sidedef pointing at a given sector.
    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn make_thing(x: i16, y: i16, kind: u16) -> ThingRaw {
        ThingRaw {
            x,
            y,
            angle: 0,
            kind,
            flags: 0,
        }
    }

    // =======================================================================
    // ---------------------------------------------------------------------------

    /// Automap canvas implementation that draws onto a [`Framebuffer`].
    ///
    /// Implements [`doom_game::AutomapCanvas`] so that `doom_game::draw_automap_full`
    /// can render directly into the renderer's framebuffer without the game crate
    /// knowing about `Framebuffer`.
    pub struct RendererAutomapCanvas<'a> {
        /// Mutable reference to the target framebuffer.
        fb: &'a mut Framebuffer,
    }

    impl<'a> RendererAutomapCanvas<'a> {
        /// Wrap a mutable `Framebuffer` reference as an `AutomapCanvas`.
        pub fn new(fb: &'a mut Framebuffer) -> Self {
            Self { fb }
        }
    }

    impl AutomapCanvas for RendererAutomapCanvas<'_> {
        fn set_pixel(&mut self, x: i32, y: i32, color: u8) {
            if (0..SCREEN_W).contains(&x) && (0..SCREEN_H).contains(&y) {
                self.fb.set_pixel(x as usize, y as usize, color);
            }
        }

        fn get_pixel(&self, x: i32, y: i32) -> Option<u8> {
            if (0..SCREEN_W).contains(&x) && (0..SCREEN_H).contains(&y) {
                self.fb.get_pixel(x as usize, y as usize)
            } else {
                None
            }
        }

        fn clear(&mut self, color: u8) {
            self.fb.clear(color);
        }

        fn width(&self) -> i32 {
            SCREEN_W
        }

        fn height(&self) -> i32 {
            SCREEN_H
        }
    }

    // ---------------------------------------------------------------------------
    // render_automap -- full integration entry point
    // ---------------------------------------------------------------------------

    /// Render the automap overlay onto the framebuffer using the full game-side
    /// automap logic (visibility tracking, grid, thing markers, cheat flags).
    ///
    /// This delegates to [`doom_game::draw_automap_full`] through the
    /// [`RendererAutomapCanvas`] bridge.
    ///
    /// # Parameters
    /// - `fb`: target framebuffer (320x200).
    /// - `level`: current level geometry.
    /// - `player_x`, `player_y`: player position in map units.
    /// - `player_angle`: player facing angle (BAM).
    /// - `zoom`: pixels per map unit.
    /// - `show_all_lines`: IDDT cheat flag for revealing all linedefs.
    /// - `show_all_things`: IDDT cheat flag for revealing all things.
    /// - `seen_lines`: per-linedef visibility (from `GameState`).
    pub fn render_automap(
        fb: &mut Framebuffer,
        level: &Level,
        player_x: i32,
        player_y: i32,
        player_angle: Bam,
        zoom: f32,
        show_all_lines: bool,
        show_all_things: bool,
        seen_lines: &[bool],
    ) {
        // Build automap state from parameters.
        let state = doom_game::AutomapState {
            active: true,
            zoom,
            center_x: player_x as f32,
            center_y: player_y as f32,
            follow_player: true,
            show_all_lines,
            show_all_things,
        };

        // Convert BAM angle to radians.
        let angle_rad = (player_angle.0 as f64) * core::f64::consts::TAU / (u32::MAX as f64 + 1.0);

        let mut canvas = RendererAutomapCanvas::new(fb);
        doom_game::draw_automap_full(
            &mut canvas,
            level,
            &state,
            seen_lines,
            player_x,
            player_y,
            angle_rad,
        );
    }

    // ---------------------------------------------------------------------------
    // Grid drawing (renderer-side, for standalone use)
    // ---------------------------------------------------------------------------

    /// Draw a grid overlay onto the framebuffer at the given map center and zoom.
    ///
    /// Grid lines are spaced at 128 map-unit intervals (the classic Doom grid).
    /// Uses [`automap_colors::GRID`] colour.
    ///
    /// This is a standalone renderer-side helper; [`render_automap`] already draws
    /// a grid via the game-side logic.
    pub fn draw_grid_on_fb(fb: &mut Framebuffer, center_x: f32, center_y: f32, zoom: f32) {
        let mut canvas = RendererAutomapCanvas::new(fb);
        doom_game::draw_grid(&mut canvas, center_x, center_y, zoom);
    }

    /// Draw a directional player arrow onto the framebuffer at a screen position.
    ///
    /// The arrow is approximately 8 pixels long, pointing in `player_angle`.
    /// Uses [`automap_colors::PLAYER`] colour.
    pub fn draw_player_arrow_on_fb(fb: &mut Framebuffer, sx: i32, sy: i32, player_angle: Bam) {
        let angle_rad = (player_angle.0 as f64) * core::f64::consts::TAU / (u32::MAX as f64 + 1.0);

        let cos_a = angle_rad.cos() as f32;
        let sin_a = angle_rad.sin() as f32;

        let shaft_len: f32 = 8.0;
        let barb_len: f32 = 4.0;

        let tip_x = sx as f32 + cos_a * shaft_len;
        let tip_y = sy as f32 - sin_a * shaft_len;
        let tail_x = sx as f32 - cos_a * shaft_len;
        let tail_y = sy as f32 + sin_a * shaft_len;

        let barb_angle_offset = core::f64::consts::FRAC_PI_4 * 3.0;
        let left_barb_angle = angle_rad + barb_angle_offset;
        let right_barb_angle = angle_rad - barb_angle_offset;

        let left_x = tip_x + (left_barb_angle.cos() as f32) * barb_len;
        let left_y = tip_y - (left_barb_angle.sin() as f32) * barb_len;
        let right_x = tip_x + (right_barb_angle.cos() as f32) * barb_len;
        let right_y = tip_y - (right_barb_angle.sin() as f32) * barb_len;

        draw_line_fb(
            fb,
            tail_x as i32,
            tail_y as i32,
            tip_x as i32,
            tip_y as i32,
            COLOR_PLAYER,
        );
        draw_line_fb(
            fb,
            tip_x as i32,
            tip_y as i32,
            left_x as i32,
            left_y as i32,
            COLOR_PLAYER,
        );
        draw_line_fb(
            fb,
            tip_x as i32,
            tip_y as i32,
            right_x as i32,
            right_y as i32,
            COLOR_PLAYER,
        );
    }

    // ---------------------------------------------------------------------------
    // ---------------------------------------------------------------------------

    // ---------------------------------------------------------------------------
    // Public API -- legacy (auto-fit) entry point
    // ---------------------------------------------------------------------------

    /// Render a 2D overhead automap of `level` into `fb` (legacy auto-fit mode).
    ///
    /// This is the backward-compatible entry point that auto-computes zoom and
    /// center from the level bounds.
    ///
    /// * `player_x`, `player_y` -- player position in Doom map units.
    /// * `player_angle` -- player facing angle (used for the directional arrow).
    /// * `_palette` -- unused for now (automap uses fixed palette indices).
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
        let mut state = AutomapState::new();
        state.center_x = (min_x as f32 + max_x as f32) / 2.0;
        state.center_y = (min_y as f32 + max_y as f32) / 2.0;
        state.zoom = scale;

        // Use the internal draw with this computed scale/center.
        draw_automap_internal(fb, level, &state, player_x, player_y, player_angle);
    }

    // ---------------------------------------------------------------------------
    // Public API -- stateful entry point
    // ---------------------------------------------------------------------------

    /// Render a 2D overhead automap using the interactive [`AutomapState`].
    ///
    /// This is the preferred entry point for interactive automap usage with
    /// zoom/pan controls.
    ///
    /// * `player_x`, `player_y` -- player position in Doom map units.
    /// * `player_angle` -- player facing angle (used for the directional arrow).
    pub fn draw_automap_ex(
        fb: &mut Framebuffer,
        level: &Level,
        state: &AutomapState,
        player_x: i32,
        player_y: i32,
        player_angle: Bam,
    ) {
        fb.clear(COLOR_BACKGROUND);

        draw_automap_internal(fb, level, state, player_x, player_y, player_angle);
    }

    // ---------------------------------------------------------------------------
    // Internal rendering
    // ---------------------------------------------------------------------------

    /// Core automap rendering: draws linedefs + player arrow.
    fn draw_automap_internal(
        fb: &mut Framebuffer,
        level: &Level,
        state: &AutomapState,
        player_x: i32,
        player_y: i32,
        player_angle: Bam,
    ) {
        if level.vertexes.is_empty() {
            return;
        }

        let center_x = state.center_x;
        let center_y = state.center_y;
        let zoom = state.zoom;
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

            draw_line_fb(fb, x0, y0, x1, y1, color);
        }

        // Draw player arrow.
        let (px, py) = world_to_screen(player_x as f32, player_y as f32);
        draw_player_arrow_internal(fb, px, py, player_angle);
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
    // Player arrow (internal, for legacy API)
    // ---------------------------------------------------------------------------

    /// Draw a directional arrow at the player's screen position.
    ///
    /// The arrow is approximately 8 pixels long, pointing in `player_angle`.
    /// Three lines form the arrowhead shape:
    ///   - A shaft from tail to tip
    ///   - Two barbs angled 135 degrees from the shaft
    fn draw_player_arrow_internal(fb: &mut Framebuffer, px: i32, py: i32, player_angle: Bam) {
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
        draw_line_fb(
            fb,
            tail_x as i32,
            tail_y as i32,
            tip_x as i32,
            tip_y as i32,
            COLOR_PLAYER,
        );
        // Draw left barb: tip -> left
        draw_line_fb(
            fb,
            tip_x as i32,
            tip_y as i32,
            left_x as i32,
            left_y as i32,
            COLOR_PLAYER,
        );
        // Draw right barb: tip -> right
        draw_line_fb(
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
    // Bresenham line rasteriser (framebuffer-direct)
    // ---------------------------------------------------------------------------

    /// Integer Bresenham line drawing directly onto a `Framebuffer`.
    ///
    /// Pixels outside `[0, 319] x [0, 199]` are silently skipped (no panic).
    pub fn draw_line_fb(fb: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
        let dx = (x1 - x0).abs();
        let dy = (y1 - y0).abs();
        let sx: i32 = if x0 < x1 { 1 } else { -1 };
        let sy: i32 = if y0 < y1 { 1 } else { -1 };

        let mut x = x0;
        let mut y = y0;
        let mut err = dx - dy;

        loop {
            // Only plot pixels within screen bounds.
            if (0..SCREEN_W).contains(&x) && (0..SCREEN_H).contains(&y) {
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

    // Backward-compat alias used in old tests.
    #[cfg(test)]
    #[allow(dead_code)]
    fn draw_line(fb: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
        draw_line_fb(fb, x0, y0, x1, y1, color);
    }

    // ---------------------------------------------------------------------------
    // Coordinate transform helpers (public, for external use)
    // ---------------------------------------------------------------------------

    /// Convert map coordinates to screen coordinates using the given center and zoom.
    ///
    /// Doom Y increases upward; screen Y increases downward. The transform
    /// flips Y so north remains up on the automap.
    pub fn map_to_screen(
        map_x: f32,
        map_y: f32,
        center_x: f32,
        center_y: f32,
        zoom: f32,
    ) -> (i32, i32) {
        let sx = HALF_W + ((map_x - center_x) * zoom) as i32;
        let sy = HALF_H - ((map_y - center_y) * zoom) as i32;
        (sx, sy)
    }

    // ---------------------------------------------------------------------------
    // Tests
    // ---------------------------------------------------------------------------

    #[cfg(test)]
    #[allow(clippy::module_inception)]
    mod tests {
        use super::*;
        use doom_game::AutomapState;
        use doom_map::Level;
        use doom_map::lumps::{
            Blockmap, FLAG_TWO_SIDED, Linedef as LdRaw, Reject, Sector, Sidedef as SdRaw, Ssector,
            Thing as ThingRaw, Vertex as VxRaw,
        };

        // -----------------------------------------------------------------------
        // Minimal Level builder
        // -----------------------------------------------------------------------

        /// Build a `Level` with caller-supplied vertexes, linedefs, sidedefs, and
        /// sectors.  Provides sensible defaults for BSP data.
        #[allow(dead_code)]
        fn make_level_full(
            vertexes: Vec<VxRaw>,
            linedefs: Vec<LdRaw>,
            sidedefs: Vec<SdRaw>,
            sectors: Vec<Sector>,
        ) -> Level {
            make_level_full_with_things(vertexes, linedefs, sidedefs, sectors, vec![])
        }

        #[allow(dead_code)]
        fn make_level_full_with_things(
            vertexes: Vec<VxRaw>,
            linedefs: Vec<LdRaw>,
            sidedefs: Vec<SdRaw>,
            sectors: Vec<Sector>,
            things: Vec<ThingRaw>,
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

            let reject_size = (n_sectors * n_sectors).div_ceil(8);
            let reject =
                Reject::parse_lump(&vec![0u8; reject_size], n_sectors).expect("reject parse");

            Level {
                name: "TEST".to_owned(),
                things,
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
        #[allow(dead_code)]
        fn make_level(vertexes: Vec<VxRaw>, linedefs: Vec<LdRaw>) -> Level {
            make_level_full(vertexes, linedefs, vec![], vec![])
        }

        /// Helper to create a sidedef pointing at a given sector.
        #[allow(dead_code)]
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

        #[allow(dead_code)]
        fn make_thing(x: i16, y: i16, kind: u16) -> ThingRaw {
            ThingRaw {
                x,
                y,
                angle: 0,
                kind,
                flags: 0,
            }
        }

        // =======================================================================
        // Tests 1-8: AutomapState (renderer-side)
        // =======================================================================

        #[test]
        fn t01_automap_state_new_defaults() {
            let state = AutomapState::new();
            assert!(!state.active);
            assert!((state.zoom - 0.5).abs() < f32::EPSILON);
            assert!((state.center_x - 0.0).abs() < f32::EPSILON);
            assert!((state.center_y - 0.0).abs() < f32::EPSILON);
            assert!(state.follow_player);
        }

        #[test]
        fn t02_toggle_flips_active() {
            let mut state = AutomapState::new();
            assert!(!state.active);
            state.toggle();
            assert!(state.active);
            state.toggle();
            assert!(!state.active);
        }

        #[test]
        fn t03_zoom_in_increases_zoom() {
            let mut state = AutomapState::new();
            let initial = state.zoom;
            state.zoom_in();
            assert!(state.zoom > initial);
        }

        #[test]
        fn t04_zoom_out_decreases_zoom() {
            let mut state = AutomapState::new();
            let initial = state.zoom;
            state.zoom_out();
            assert!(state.zoom < initial);
        }

        #[test]
        fn t07_update_center_follows_player() {
            let mut state = AutomapState::new();
            state.follow_player = true;
            state.update_center(100, 200);
            assert!((state.center_x - 100.0).abs() < f32::EPSILON);
            assert!((state.center_y - 200.0).abs() < f32::EPSILON);
        }

        #[test]
        fn t08_update_center_ignores_when_follow_disabled() {
            let mut state = AutomapState::new();
            state.follow_player = false;
            state.center_x = 42.0;
            state.center_y = 99.0;
            state.update_center(100, 200);
            assert!((state.center_x - 42.0).abs() < f32::EPSILON);
            assert!((state.center_y - 99.0).abs() < f32::EPSILON);
        }

        // =======================================================================
        // Tests 9-14: Bresenham line drawing
        // =======================================================================

        #[test]
        fn t09_draw_line_horizontal() {
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

        #[test]
        fn t10_draw_line_vertical() {
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

        #[test]
        fn t11_draw_line_diagonal() {
            let mut fb = Framebuffer::new();
            draw_line(&mut fb, 0, 0, 10, 10, 200);
            for i in 0..=10 {
                assert_eq!(
                    fb.get_pixel(i, i),
                    Some(200),
                    "pixel ({i}, {i}) should be 200"
                );
            }
        }

        #[test]
        fn t12_draw_line_clips_to_bounds() {
            let mut fb = Framebuffer::new();
            // Coordinates far outside -- should not panic.
            draw_line(&mut fb, -5000, -5000, 5000, 5000, 255);
            // Verify some edge pixels that should be set (the diagonal
            // crosses [0,0] to [319,199]).
            assert_eq!(fb.get_pixel(0, 0), Some(255));
        }

        #[test]
        fn t13_draw_line_single_point() {
            let mut fb = Framebuffer::new();
            draw_line(&mut fb, 50, 50, 50, 50, 42);
            assert_eq!(fb.get_pixel(50, 50), Some(42));
        }

        #[test]
        fn t14_draw_line_from_origin_to_far_corner_stays_in_bounds() {
            let mut fb = Framebuffer::new();
            // Line from (0,0) to (319,199) -- should hit many pixels, none OOB.
            draw_line(&mut fb, 0, 0, 319, 199, 77);
            assert_eq!(fb.get_pixel(0, 0), Some(77));
            assert_eq!(fb.get_pixel(319, 199), Some(77));
            // No panic means no OOB writes.
        }

        // =======================================================================
        // Tests 15-18: Line colour classification
        // =======================================================================

        #[test]
        fn t15_line_color_one_sided() {
            let level = make_level(vec![], vec![]);
            let ld = LdRaw {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            };
            assert_eq!(line_color(&ld, &level), COLOR_ONE_SIDED);
        }

        #[test]
        fn t16_line_color_two_sided_same_height() {
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

        #[test]
        fn t17_line_color_two_sided_height_change() {
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
                    floor_height: 24,
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

        #[test]
        fn t18_line_color_secret_overrides_two_sided() {
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
                flags: FLAG_TWO_SIDED | FLAG_SECRET,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            };
            assert_eq!(line_color(&ld, &level), COLOR_SECRET);
        }

        // =======================================================================
        // Tests 19-24: Legacy draw_automap / draw_automap_ex
        // =======================================================================

        #[test]
        fn t19_draw_automap_empty_level_does_not_panic() {
            let level = make_level(vec![], vec![]);
            let mut fb = Framebuffer::new();
            let palette = PaletteLut::grayscale();
            draw_automap(&level, 0, 0, Bam(0), &mut fb, &palette);
            assert!(fb.data.iter().all(|&b| b == 0));
        }

        #[test]
        fn t20_draw_automap_ex_empty_level_does_not_panic() {
            let level = make_level(vec![], vec![]);
            let mut fb = Framebuffer::new();
            let state = AutomapState::new();
            draw_automap_ex(&mut fb, &level, &state, 0, 0, Bam(0));
            assert!(fb.data.iter().all(|&b| b == 0));
        }

        #[test]
        fn t21_draw_automap_single_linedef_marks_pixels() {
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
            assert!(has_non_black);
        }

        #[test]
        fn t22_draw_automap_ex_zoomed_draws_pixels() {
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

            let has_non_black = fb.data.iter().any(|&b| b != 0);
            assert!(has_non_black, "zoomed automap should draw visible pixels");
        }

        #[test]
        fn t23_player_arrow_draws_pixels() {
            let mut fb = Framebuffer::new();
            draw_player_arrow_internal(&mut fb, 160, 100, Bam(0));
            let has_non_black = fb.data.iter().any(|&b| b != 0);
            assert!(has_non_black, "player arrow should draw visible pixels");
        }

        #[test]
        fn t24_automap_state_default_matches_new() {
            let from_new = AutomapState::new();
            let from_default = AutomapState::default();
            assert_eq!(from_new.active, from_default.active);
            assert!((from_new.zoom - from_default.zoom).abs() < f32::EPSILON);
            assert!((from_new.center_x - from_default.center_x).abs() < f32::EPSILON);
            assert!((from_new.center_y - from_default.center_y).abs() < f32::EPSILON);
            assert_eq!(from_new.follow_player, from_default.follow_player);
        }

        // =======================================================================
        // Tests 25-30: automap_colors constants
        // =======================================================================

        #[test]
        fn t26_wall_and_two_sided_have_different_colors() {
            assert_ne!(automap_colors::WALL, automap_colors::TWO_SIDED);
        }

        #[test]
        fn t27_secret_color_is_distinct() {
            assert_ne!(automap_colors::SECRET, automap_colors::WALL);
            assert_ne!(automap_colors::SECRET, automap_colors::TWO_SIDED);
        }

        #[test]
        fn t28_background_is_black() {
            assert_eq!(automap_colors::BACKGROUND, 0);
        }

        #[test]
        fn t29_grid_color_is_dim() {
            let grid = automap_colors::GRID;
            let crosshair = automap_colors::CROSSHAIR;
            assert_ne!(grid, automap_colors::BACKGROUND);
            assert_ne!(grid, crosshair, "grid should remain visually distinct");
        }

        #[test]
        fn t30_player_color_is_not_background() {
            assert_ne!(automap_colors::PLAYER, automap_colors::BACKGROUND);
        }

        // =======================================================================
        // Tests 31-36: RendererAutomapCanvas trait implementation
        // =======================================================================

        #[test]
        fn t31_canvas_set_pixel_and_get_pixel() {
            let mut fb = Framebuffer::new();
            {
                let mut canvas = RendererAutomapCanvas::new(&mut fb);
                canvas.set_pixel(10, 20, 42);
                assert_eq!(canvas.get_pixel(10, 20), Some(42));
            }
            // Verify it wrote through to the framebuffer.
            assert_eq!(fb.get_pixel(10, 20), Some(42));
        }

        #[test]
        fn t32_canvas_out_of_bounds_set_does_not_panic() {
            let mut fb = Framebuffer::new();
            let mut canvas = RendererAutomapCanvas::new(&mut fb);
            // Negative coordinates.
            canvas.set_pixel(-1, -1, 255);
            // Beyond screen bounds.
            canvas.set_pixel(320, 200, 255);
            canvas.set_pixel(10000, 10000, 255);
            // Nothing should have changed.
            assert!(fb.data.iter().all(|&b| b == 0));
        }

        #[test]
        fn t33_canvas_out_of_bounds_get_returns_none() {
            let fb = Framebuffer::new();
            // We need a mutable ref for the canvas, but we just test get_pixel
            // via a raw framebuffer check -- let's use a separate approach.
            let mut fb2 = Framebuffer::new();
            let canvas = RendererAutomapCanvas::new(&mut fb2);
            assert_eq!(canvas.get_pixel(-1, 0), None);
            assert_eq!(canvas.get_pixel(0, -1), None);
            assert_eq!(canvas.get_pixel(320, 0), None);
            assert_eq!(canvas.get_pixel(0, 200), None);
            // Ensure fb is used (suppress warning).
            let _ = fb;
        }

        #[test]
        fn t34_canvas_clear_fills_entire_buffer() {
            let mut fb = Framebuffer::new();
            {
                let mut canvas = RendererAutomapCanvas::new(&mut fb);
                canvas.clear(77);
            }
            assert!(fb.data.iter().all(|&b| b == 77));
        }

        #[test]
        fn t35_canvas_width_and_height() {
            let mut fb = Framebuffer::new();
            let canvas = RendererAutomapCanvas::new(&mut fb);
            assert_eq!(canvas.width(), 320);
            assert_eq!(canvas.height(), 200);
        }

        #[test]
        fn t36_canvas_draw_line_produces_pixels() {
            let mut fb = Framebuffer::new();
            {
                let mut canvas = RendererAutomapCanvas::new(&mut fb);
                doom_game::draw_line(&mut canvas, 10, 10, 50, 10, 123);
            }
            // Check that the horizontal line was drawn.
            for x in 10..=50 {
                assert_eq!(
                    fb.get_pixel(x, 10),
                    Some(123),
                    "pixel ({x}, 10) should be 123"
                );
            }
        }

        // =======================================================================
        // Tests 37-42: render_automap (full integration)
        // =======================================================================

        #[test]
        fn t37_render_automap_empty_level_does_not_panic() {
            let level = make_level(vec![], vec![]);
            let mut fb = Framebuffer::new();
            render_automap(&mut fb, &level, 0, 0, Bam(0), 0.5, false, false, &[]);
            // Background should be black (cleared).
            assert!(fb.data.iter().all(|&b| b == 0));
        }

        #[test]
        fn t38_render_automap_single_linedef_seen() {
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
            let seen = vec![true]; // line 0 is seen

            render_automap(&mut fb, &level, 100, 0, Bam(0), 1.0, false, false, &seen);

            let has_non_black = fb.data.iter().any(|&b| b != 0);
            assert!(has_non_black, "seen linedef should produce visible pixels");
        }

        #[test]
        fn t39_render_automap_hidden_line_not_drawn_without_cheat() {
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
            let seen = vec![false]; // line 0 is NOT seen

            render_automap(&mut fb, &level, 100, 0, Bam(0), 1.0, false, false, &seen);

            // The player arrow and grid will still draw some pixels, but
            // we count non-grid/non-arrow pixels specifically by checking
            // for the one-sided line color.
            let one_sided_count = fb.data.iter().filter(|&&b| b == COLOR_ONE_SIDED).count();
            assert_eq!(
                one_sided_count, 0,
                "hidden line should not produce one-sided wall colour pixels"
            );
        }

        #[test]
        fn t40_render_automap_show_all_lines_reveals_hidden() {
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
            let seen = vec![false]; // line 0 is NOT seen

            // But show_all_lines is true.
            render_automap(&mut fb, &level, 100, 0, Bam(0), 1.0, true, false, &seen);

            // Should draw the line in UNSEEN colour (gray).
            let unseen_count = fb
                .data
                .iter()
                .filter(|&&b| b == doom_game::automap::COLOR_UNSEEN)
                .count();
            assert!(
                unseen_count > 0,
                "show_all_lines should reveal hidden lines in unseen colour"
            );
        }

        #[test]
        fn t41_render_automap_clears_framebuffer() {
            let level = make_level(vec![], vec![]);
            let mut fb = Framebuffer::new();
            fb.clear(99); // Fill with non-zero.

            render_automap(&mut fb, &level, 0, 0, Bam(0), 0.5, false, false, &[]);

            // After render_automap, fb should be cleared to background.
            assert!(fb.data.iter().all(|&b| b == 0));
        }

        #[test]
        fn t42_render_automap_player_arrow_does_not_panic() {
            let level = make_level(
                vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 100 }],
                vec![LdRaw {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0xFFFF,
                    left_sidedef: 0xFFFF,
                }],
            );
            let mut fb = Framebuffer::new();

            // Player at various angles.
            for angle in [0u32, u32::MAX / 4, u32::MAX / 2, u32::MAX] {
                render_automap(
                    &mut fb,
                    &level,
                    50,
                    50,
                    Bam(angle),
                    1.0,
                    true,
                    false,
                    &[true],
                );
            }
            // No panic is success.
        }

        // =======================================================================
        // Tests 43-47: Coordinate transform
        // =======================================================================

        #[test]
        fn t43_map_to_screen_origin_returns_center() {
            let (sx, sy) = map_to_screen(0.0, 0.0, 0.0, 0.0, 1.0);
            assert_eq!(sx, HALF_W); // 160
            assert_eq!(sy, HALF_H); // 100
        }

        #[test]
        fn t44_map_to_screen_y_is_flipped() {
            // Positive Y in map space should map to SMALLER screen Y (upward).
            let (_, sy_pos) = map_to_screen(0.0, 100.0, 0.0, 0.0, 1.0);
            let (_, sy_zero) = map_to_screen(0.0, 0.0, 0.0, 0.0, 1.0);
            assert!(
                sy_pos < sy_zero,
                "positive map Y should map to smaller screen Y"
            );
        }

        #[test]
        fn t45_map_to_screen_scale_affects_output() {
            let (sx_low, _) = map_to_screen(100.0, 0.0, 0.0, 0.0, 0.5);
            let (sx_high, _) = map_to_screen(100.0, 0.0, 0.0, 0.0, 2.0);
            // Higher zoom should push the point further from center.
            let dist_low = (sx_low - HALF_W).abs();
            let dist_high = (sx_high - HALF_W).abs();
            assert!(
                dist_high > dist_low,
                "higher zoom should increase screen distance from center"
            );
        }

        #[test]
        fn t46_map_to_screen_different_center_offsets() {
            let (sx1, _) = map_to_screen(100.0, 0.0, 0.0, 0.0, 1.0);
            let (sx2, _) = map_to_screen(100.0, 0.0, 100.0, 0.0, 1.0);
            // When center is at 100, the point at 100 should be at screen center.
            assert_eq!(sx2, HALF_W);
            assert_ne!(sx1, sx2);
        }

        #[test]
        fn t47_map_to_screen_negative_coords() {
            let (sx, sy) = map_to_screen(-100.0, -100.0, 0.0, 0.0, 1.0);
            // Negative X -> left of center, negative Y -> below center.
            assert!(sx < HALF_W);
            assert!(sy > HALF_H);
        }

        // =======================================================================
        // Tests 48-52: Grid and player arrow on framebuffer
        // =======================================================================

        #[test]
        fn t48_draw_grid_on_fb_does_not_panic() {
            let mut fb = Framebuffer::new();
            draw_grid_on_fb(&mut fb, 0.0, 0.0, 0.5);
            // Should have drawn some grid pixels.
            let grid_count = fb
                .data
                .iter()
                .filter(|&&b| b == automap_colors::GRID)
                .count();
            assert!(grid_count > 0, "grid should produce some pixels");
        }

        #[test]
        fn t49_draw_grid_on_fb_zero_zoom_does_not_panic() {
            let mut fb = Framebuffer::new();
            draw_grid_on_fb(&mut fb, 0.0, 0.0, 0.0);
            // Zero zoom should be a no-op (doom_game::draw_grid bails on zoom <= 0).
            assert!(fb.data.iter().all(|&b| b == 0));
        }

        #[test]
        fn t50_draw_player_arrow_on_fb_produces_pixels() {
            let mut fb = Framebuffer::new();
            draw_player_arrow_on_fb(&mut fb, 160, 100, Bam(0));
            let has_non_black = fb.data.iter().any(|&b| b != 0);
            assert!(has_non_black, "player arrow should draw visible pixels");
        }

        #[test]
        fn t51_draw_player_arrow_on_fb_at_various_angles() {
            for angle_frac in [0u32, 1, 2, 3, 4, 5, 6, 7] {
                let mut fb = Framebuffer::new();
                let angle = Bam(angle_frac * (u32::MAX / 8));
                draw_player_arrow_on_fb(&mut fb, 160, 100, angle);
                let has_non_black = fb.data.iter().any(|&b| b != 0);
                assert!(
                    has_non_black,
                    "player arrow at angle fraction {angle_frac}/8 should draw pixels"
                );
            }
        }

        #[test]
        fn t52_draw_player_arrow_on_fb_out_of_bounds_does_not_panic() {
            let mut fb = Framebuffer::new();
            // Arrow at far-off-screen position.
            draw_player_arrow_on_fb(&mut fb, -100, -100, Bam(0));
            draw_player_arrow_on_fb(&mut fb, 500, 500, Bam(0));
            // No panic is success.
        }

        // =======================================================================
        // Tests 53-56: render_automap with things
        // =======================================================================

        #[test]
        fn t53_render_automap_show_all_things_draws_markers() {
            let vertexes = vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 200, y: 200 }];
            let linedefs = vec![LdRaw {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0xFFFF,
                left_sidedef: 0xFFFF,
            }];
            let things = vec![
                make_thing(100, 100, 3001), // Imp (monster)
                make_thing(50, 50, 2011),   // Stimpack (item)
            ];
            let level = make_level_full_with_things(vertexes, linedefs, vec![], vec![], things);
            let mut fb = Framebuffer::new();
            let seen = vec![true];

            render_automap(&mut fb, &level, 100, 100, Bam(0), 1.0, false, true, &seen);

            let has_non_black = fb.data.iter().any(|&b| b != 0);
            assert!(has_non_black, "things should produce visible markers");
        }

        #[test]
        fn t54_render_automap_without_show_things_hides_monsters() {
            let vertexes = vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 200, y: 200 }];
            let linedefs = vec![LdRaw {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0xFFFF,
                left_sidedef: 0xFFFF,
            }];
            let things = vec![
                make_thing(100, 100, 3001), // Imp (monster)
            ];
            let level = make_level_full_with_things(vertexes, linedefs, vec![], vec![], things);

            // With show_all_things = true
            let mut fb_with = Framebuffer::new();
            render_automap(
                &mut fb_with,
                &level,
                100,
                100,
                Bam(0),
                1.0,
                true,
                true,
                &[true],
            );

            // Without show_all_things
            let mut fb_without = Framebuffer::new();
            render_automap(
                &mut fb_without,
                &level,
                100,
                100,
                Bam(0),
                1.0,
                true,
                false,
                &[true],
            );

            // The version with things should have more non-zero pixels
            // (monster markers add pixels).
            let count_with = fb_with.data.iter().filter(|&&b| b != 0).count();
            let count_without = fb_without.data.iter().filter(|&&b| b != 0).count();
            assert!(
                count_with > count_without,
                "show_all_things should draw more pixels ({count_with} vs {count_without})"
            );
        }

        #[test]
        fn t55_render_automap_player_starts_always_visible() {
            let vertexes = vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 200, y: 200 }];
            let linedefs = vec![LdRaw {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0xFFFF,
                left_sidedef: 0xFFFF,
            }];
            let things = vec![
                make_thing(100, 100, 1), // Player 1 start
            ];
            let level = make_level_full_with_things(vertexes, linedefs, vec![], vec![], things);
            let mut fb = Framebuffer::new();

            // show_all_things = false, but player starts should still draw.
            render_automap(&mut fb, &level, 100, 100, Bam(0), 1.0, true, false, &[true]);

            // The player marker colour (from doom_game) should appear.
            let player_marker_count = fb
                .data
                .iter()
                .filter(|&&b| b == doom_game::automap::COLOR_PLAYER_MARKER)
                .count();
            assert!(
                player_marker_count > 0,
                "player start markers should always be visible"
            );
        }

        #[test]
        fn t56_render_automap_seen_lines_correctly_filters() {
            let vertexes = vec![
                VxRaw { x: 0, y: 0 },
                VxRaw { x: 200, y: 0 },
                VxRaw { x: 0, y: 200 },
            ];
            let linedefs = vec![
                LdRaw {
                    from_vertex: 0,
                    to_vertex: 1,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0xFFFF,
                    left_sidedef: 0xFFFF,
                },
                LdRaw {
                    from_vertex: 0,
                    to_vertex: 2,
                    flags: 0,
                    special: 0,
                    tag: 0,
                    right_sidedef: 0xFFFF,
                    left_sidedef: 0xFFFF,
                },
            ];
            let level = make_level(vertexes, linedefs);

            // Only first line is seen.
            let mut fb1 = Framebuffer::new();
            render_automap(
                &mut fb1,
                &level,
                100,
                100,
                Bam(0),
                1.0,
                false,
                false,
                &[true, false],
            );
            let count1 = fb1.data.iter().filter(|&&b| b == COLOR_ONE_SIDED).count();

            // Both lines are seen.
            let mut fb2 = Framebuffer::new();
            render_automap(
                &mut fb2,
                &level,
                100,
                100,
                Bam(0),
                1.0,
                false,
                false,
                &[true, true],
            );
            let count2 = fb2.data.iter().filter(|&&b| b == COLOR_ONE_SIDED).count();

            assert!(
                count2 > count1,
                "seeing more lines should produce more wall pixels ({count2} vs {count1})"
            );
        }

        // =======================================================================
        // Test 57: draw_line_fb public API
        // =======================================================================

        #[test]
        fn t57_draw_line_fb_public_api() {
            let mut fb = Framebuffer::new();
            draw_line_fb(&mut fb, 0, 0, 20, 0, 55);
            for x in 0..=20 {
                assert_eq!(fb.get_pixel(x, 0), Some(55));
            }
        }

        // =======================================================================
        // Test 58: render_automap grid is drawn
        // =======================================================================

        #[test]
        fn t58_render_automap_draws_grid() {
            let vertexes = vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 1000, y: 1000 }];
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

            render_automap(&mut fb, &level, 500, 500, Bam(0), 0.5, true, false, &[true]);

            // Grid uses doom_game::COLOR_GRID.
            let grid_count = fb
                .data
                .iter()
                .filter(|&&b| b == doom_game::automap::COLOR_GRID)
                .count();
            assert!(grid_count > 0, "render_automap should draw grid lines");
        }
    }
}
