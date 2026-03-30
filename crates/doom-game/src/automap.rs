//! Automap game logic: state, visibility tracking, grid overlay, and thing markers.
//!
//! This module handles the game-side automap logic that supplements the
//! renderer's visual automap drawing. It provides:
//!
//! - [`AutomapState`]: Extended automap state with cheat reveal flags.
//! - Line visibility tracking (`seen_lines` in [`GameState`](crate::state::GameState)).
//! - Grid overlay computation.
//! - Thing marker classification and placement.
//!
//! # Drawing model
//! Functions here produce draw commands into a [`AutomapCanvas`] trait that
//! abstracts over the actual framebuffer. This keeps doom-game free of
//! renderer dependencies while enabling comprehensive testing.

use doom_map::Level;

// ---------------------------------------------------------------------------
// Screen constants (mirrors the renderer's 320x200 framebuffer)
// ---------------------------------------------------------------------------

/// Framebuffer width in pixels.
pub const SCREEN_W: i32 = 320;
/// Framebuffer height in pixels.
pub const SCREEN_H: i32 = 200;
const HALF_W: i32 = SCREEN_W / 2;
const HALF_H: i32 = SCREEN_H / 2;

// ---------------------------------------------------------------------------
// Palette colour indices (Doom palette)
// ---------------------------------------------------------------------------

/// Grid lines: dark gray.
pub const COLOR_GRID: u8 = 104;
/// One-sided wall (solid): red.
pub const COLOR_ONE_SIDED: u8 = 176;
/// Two-sided wall (no height change): brown.
pub const COLOR_TWO_SIDED: u8 = 64;
/// Two-sided wall with height change: yellow.
pub const COLOR_HEIGHT_CHANGE: u8 = 231;
/// Secret linedef (flag bit 5): purple.
pub const COLOR_SECRET: u8 = 252;
/// Unseen line (drawn when show_all_lines is active for previously unseen): gray.
pub const COLOR_UNSEEN: u8 = 96;
/// Player marker: white.
pub const COLOR_PLAYER_MARKER: u8 = 4;
/// Monster marker: red.
pub const COLOR_MONSTER: u8 = 176;
/// Item marker: green.
pub const COLOR_ITEM: u8 = 112;
/// Key marker: yellow.
pub const COLOR_KEY: u8 = 231;
/// Background.
pub const COLOR_BACKGROUND: u8 = 0;
/// Player arrow.
pub const COLOR_PLAYER_ARROW: u8 = 119;

/// Linedef flag bit 5 -- secret wall.
const FLAG_SECRET: u16 = 0x0020;

/// Grid spacing in map units.
pub const GRID_SPACING: i32 = 128;

/// Marker half-size in screen pixels.
const MARKER_SIZE: i32 = 2;

// ---------------------------------------------------------------------------
// AutomapState
// ---------------------------------------------------------------------------

/// Extended automap state with cheat-reveal flags.
///
/// This extends the basic zoom/pan/follow state with flags for the IDDT cheat
/// (show all lines, show all things).
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
    /// When true, draw ALL linedefs regardless of visibility.
    pub show_all_lines: bool,
    /// When true, draw markers for ALL things including hidden ones.
    pub show_all_things: bool,
}

/// Maximum zoom (pixels per map unit).
pub const ZOOM_MAX: f32 = 4.0;
/// Minimum zoom (pixels per map unit).
pub const ZOOM_MIN: f32 = 0.1;
/// Zoom multiplier for each zoom-in step.
pub const ZOOM_FACTOR: f32 = 1.2;

impl AutomapState {
    /// Create a new `AutomapState` with sensible defaults.
    ///
    /// Starts inactive, zoom 0.5, follow-player enabled, center at origin,
    /// cheat reveal flags off.
    pub fn new() -> Self {
        Self {
            active: false,
            zoom: 0.5,
            center_x: 0.0,
            center_y: 0.0,
            follow_player: true,
            show_all_lines: false,
            show_all_things: false,
        }
    }

    /// Toggle the automap on/off.
    pub fn toggle(&mut self) {
        self.active = !self.active;
    }

    /// Zoom in by multiplying zoom by [`ZOOM_FACTOR`], capped at [`ZOOM_MAX`].
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_game::AutomapState;
    ///
    /// let mut automap = AutomapState::new();
    /// let initial_zoom = automap.zoom;
    /// automap.zoom_in();
    /// assert!(automap.zoom > initial_zoom);
    /// ```
    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * ZOOM_FACTOR).min(ZOOM_MAX);
    }

    /// Zoom out by dividing zoom by [`ZOOM_FACTOR`], floored at [`ZOOM_MIN`].
    ///
    /// ## Examples
    ///
    /// ```rust
    /// use doom_game::AutomapState;
    ///
    /// let mut automap = AutomapState::new();
    /// let initial_zoom = automap.zoom;
    /// automap.zoom_out();
    /// assert!(automap.zoom < initial_zoom);
    /// ```
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
// AutomapCanvas trait
// ---------------------------------------------------------------------------

/// Abstraction for a pixel target (framebuffer or test buffer).
///
/// The automap drawing functions use this trait to set pixels, keeping
/// doom-game decoupled from the renderer's `Framebuffer` type.
pub trait AutomapCanvas {
    /// Set a pixel at `(x, y)` to the given palette index.
    ///
    /// Coordinates outside the valid range should be silently ignored.
    fn set_pixel(&mut self, x: i32, y: i32, color: u8);

    /// Get the pixel at `(x, y)`, or `None` if out of bounds.
    fn get_pixel(&self, x: i32, y: i32) -> Option<u8>;

    /// Fill the entire canvas with the given palette index.
    fn clear(&mut self, color: u8);

    /// Canvas width in pixels.
    fn width(&self) -> i32;

    /// Canvas height in pixels.
    fn height(&self) -> i32;
}

// ---------------------------------------------------------------------------
// TestCanvas: a simple test-only canvas
// ---------------------------------------------------------------------------

/// Simple pixel buffer for testing automap drawing.
#[derive(Clone)]
pub struct TestCanvas {
    /// Pixel data (row-major, width * height).
    pub data: Vec<u8>,
    /// Width in pixels.
    pub w: i32,
    /// Height in pixels.
    pub h: i32,
}

impl TestCanvas {
    /// Create a new canvas of the given dimensions, filled with zeros.
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            data: vec![0u8; (w * h) as usize],
            w,
            h,
        }
    }

    /// Count non-zero pixels.
    pub fn count_nonzero(&self) -> usize {
        self.data.iter().filter(|&&b| b != 0).count()
    }

    /// Count pixels of a specific color.
    pub fn count_color(&self, color: u8) -> usize {
        self.data.iter().filter(|&&b| b == color).count()
    }
}

impl AutomapCanvas for TestCanvas {
    fn set_pixel(&mut self, x: i32, y: i32, color: u8) {
        if x >= 0 && x < self.w && y >= 0 && y < self.h {
            self.data[(y * self.w + x) as usize] = color;
        }
    }

    fn get_pixel(&self, x: i32, y: i32) -> Option<u8> {
        if x >= 0 && x < self.w && y >= 0 && y < self.h {
            Some(self.data[(y * self.w + x) as usize])
        } else {
            None
        }
    }

    fn clear(&mut self, color: u8) {
        self.data.fill(color);
    }

    fn width(&self) -> i32 {
        self.w
    }

    fn height(&self) -> i32 {
        self.h
    }
}

// ---------------------------------------------------------------------------
// Thing classification
// ---------------------------------------------------------------------------

/// Category of a map thing for automap marker color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThingCategory {
    /// Player starts (types 1-4).
    Player,
    /// Monsters (demons, imps, etc.).
    Monster,
    /// Pickup items (health, ammo, weapons, armor, etc.).
    Item,
    /// Keys (all six key types).
    Key,
    /// Decorations, teleport destinations, and other non-interactive things.
    Other,
}

/// Classify a DoomEd thing type number into a marker category.
///
/// Uses the standard Doom thing type ranges:
/// - 1-4: Player starts
/// - 5, 6: Keys (blue/yellow card)
/// - 13: Red skull key
/// - 38-40: Skull keys (red/yellow/blue)
/// - 7-9, 16, 58, 64-66, 68-69, 71, 84: Monsters
/// - 3001-3006, 65-69: Monsters
/// - 2001-2049: Pickups/items
/// - Health (2011-2014), Ammo (2007-2048), Armor (2015-2019), Weapons (2001-2006)
/// - Powerups (2022-2026), Backpack (8), etc.
pub fn classify_thing(doomed_type: u16) -> ThingCategory {
    match doomed_type {
        // Player starts
        1..=4 | 11 => ThingCategory::Player,

        // Keys (cards + skulls)
        5 | 6 | 13 | 38 | 39 | 40 => ThingCategory::Key,

        // Monsters
        7        // Spiderdemon
        | 9      // Shotgun Guy
        | 16     // Cyberdemon
        | 58     // Spectre
        | 64     // Arch-Vile
        | 65     // Heavy Weapon Dude / Chaingunner
        | 66     // Revenant
        | 67     // Mancubus
        | 68     // Arachnotron
        | 69     // Hell Knight
        | 71     // Pain Elemental
        | 84     // SS Nazi
        | 3001   // Imp
        | 3002   // Demon (Pinky)
        | 3003   // Baron of Hell
        | 3004   // Zombieman
        | 3005   // Cacodemon
        | 3006   // Lost Soul
        => ThingCategory::Monster,

        // Pickups: weapons, ammo, health, armor, powerups
        8        // Backpack
        | 2001   // Shotgun
        | 2002   // Chaingun
        | 2003   // Rocket Launcher
        | 2004   // Plasma Rifle
        | 2005   // Chainsaw
        | 2006   // BFG 9000
        | 2007   // Clip
        | 2008   // Shotgun shells
        | 2010   // Rocket
        | 2011   // Stimpack
        | 2012   // Medikit
        | 2013   // Soul Sphere
        | 2014   // Health bonus
        | 2015   // Armor bonus
        | 2018   // Green armor
        | 2019   // Blue armor
        | 2022   // Invulnerability
        | 2023   // Berserk
        | 2024   // Invisibility
        | 2025   // Radiation suit
        | 2026   // Computer map
        | 2035   // Barrel (exploding)
        | 2045   // Light amplification
        | 2046   // Box of rockets
        | 2047   // Cell
        | 2048   // Box of ammo
        | 2049   // Box of shells
        | 17     // Cell pack
        => ThingCategory::Item,

        // Everything else (decorations, teleport dests, etc.)
        _ => ThingCategory::Other,
    }
}

/// Return the automap marker color for a thing category.
pub fn thing_marker_color(category: ThingCategory) -> u8 {
    match category {
        ThingCategory::Player => COLOR_PLAYER_MARKER,
        ThingCategory::Monster => COLOR_MONSTER,
        ThingCategory::Item => COLOR_ITEM,
        ThingCategory::Key => COLOR_KEY,
        ThingCategory::Other => COLOR_GRID, // dim gray for decorations
    }
}

// ---------------------------------------------------------------------------
// Coordinate transform
// ---------------------------------------------------------------------------

/// Convert world coordinates to screen coordinates.
///
/// Doom Y increases upward; screen Y increases downward. The transform
/// flips Y so north remains up on the automap.
#[inline]
pub fn world_to_screen(wx: f32, wy: f32, center_x: f32, center_y: f32, zoom: f32) -> (i32, i32) {
    let sx = HALF_W + ((wx - center_x) * zoom) as i32;
    let sy = HALF_H - ((wy - center_y) * zoom) as i32;
    (sx, sy)
}

// ---------------------------------------------------------------------------
// Bresenham line drawing
// ---------------------------------------------------------------------------

/// Integer Bresenham line drawing.
///
/// Pixels outside canvas bounds are silently skipped (no panic).
pub fn draw_line(canvas: &mut dyn AutomapCanvas, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) {
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx: i32 = if x0 < x1 { 1 } else { -1 };
    let sy: i32 = if y0 < y1 { 1 } else { -1 };

    let mut x = x0;
    let mut y = y0;
    let mut err = dx - dy;

    loop {
        canvas.set_pixel(x, y, color);

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
// Thing marker drawing
// ---------------------------------------------------------------------------

/// Draw a small diamond marker at a screen position.
///
/// The diamond is `MARKER_SIZE` pixels in each cardinal direction from center.
/// Drawn as four lines forming a diamond shape.
pub fn draw_thing_marker(canvas: &mut dyn AutomapCanvas, sx: i32, sy: i32, color: u8) {
    // Diamond: four lines connecting N-E-S-W points.
    //   top: (sx, sy - MARKER_SIZE)
    //   right: (sx + MARKER_SIZE, sy)
    //   bottom: (sx, sy + MARKER_SIZE)
    //   left: (sx - MARKER_SIZE, sy)
    let top = (sx, sy - MARKER_SIZE);
    let right = (sx + MARKER_SIZE, sy);
    let bottom = (sx, sy + MARKER_SIZE);
    let left = (sx - MARKER_SIZE, sy);

    draw_line(canvas, top.0, top.1, right.0, right.1, color);
    draw_line(canvas, right.0, right.1, bottom.0, bottom.1, color);
    draw_line(canvas, bottom.0, bottom.1, left.0, left.1, color);
    draw_line(canvas, left.0, left.1, top.0, top.1, color);
}

// ---------------------------------------------------------------------------
// Grid overlay
// ---------------------------------------------------------------------------

/// Draw a grid of lines at [`GRID_SPACING`] map-unit intervals.
///
/// The grid is drawn in [`COLOR_GRID`] (dark gray) and should be called
/// BEFORE drawing map lines so that lines appear on top.
pub fn draw_grid(canvas: &mut dyn AutomapCanvas, center_x: f32, center_y: f32, zoom: f32) {
    if zoom <= 0.0 {
        return;
    }

    let w = canvas.width();
    let h = canvas.height();
    let half_w = w / 2;
    let half_h = h / 2;

    let spacing = GRID_SPACING as f32;

    // Compute the world-coordinate range visible on screen.
    let world_left = center_x - (half_w as f32 / zoom);
    let world_right = center_x + (half_w as f32 / zoom);
    let world_bottom = center_y - (half_h as f32 / zoom);
    let world_top = center_y + (half_h as f32 / zoom);

    // Vertical grid lines (constant X).
    let first_x = (world_left / spacing).floor() as i32 * GRID_SPACING;
    let last_x = (world_right / spacing).ceil() as i32 * GRID_SPACING;

    let mut gx = first_x;
    while gx <= last_x {
        let (sx, _) = world_to_screen(gx as f32, 0.0, center_x, center_y, zoom);
        if sx >= 0 && sx < w {
            draw_line(canvas, sx, 0, sx, h - 1, COLOR_GRID);
        }
        gx += GRID_SPACING;
    }

    // Horizontal grid lines (constant Y).
    let first_y = (world_bottom / spacing).floor() as i32 * GRID_SPACING;
    let last_y = (world_top / spacing).ceil() as i32 * GRID_SPACING;

    let mut gy = first_y;
    while gy <= last_y {
        let (_, sy) = world_to_screen(0.0, gy as f32, center_x, center_y, zoom);
        if sy >= 0 && sy < h {
            draw_line(canvas, 0, sy, w - 1, sy, COLOR_GRID);
        }
        gy += GRID_SPACING;
    }
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
    if ld.flags & FLAG_SECRET != 0 {
        return COLOR_SECRET;
    }

    if !ld.is_two_sided() {
        return COLOR_ONE_SIDED;
    }

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
// Line visibility
// ---------------------------------------------------------------------------

/// Initialize the `seen_lines` vector to `false` for every linedef.
pub fn init_seen_lines(level: &Level) -> Vec<bool> {
    vec![false; level.linedefs.len()]
}

/// Mark all linedefs adjacent to a subsector as seen.
///
/// Walks the segs belonging to the subsector and marks each seg's parent
/// linedef in `seen_lines`.
///
/// # Safety
/// This function does NOT panic on out-of-bounds indices; it silently
/// skips invalid references (defensive against corrupt map data).
pub fn mark_subsector_lines_seen(seen_lines: &mut [bool], subsector_idx: usize, level: &Level) {
    let Some(ss) = level.ssectors.get(subsector_idx) else {
        return;
    };

    let first = ss.first_seg as usize;
    let count = ss.seg_count as usize;

    for seg_idx in first..first.saturating_add(count) {
        let Some(seg) = level.segs.get(seg_idx) else {
            continue;
        };
        let ld_idx = seg.linedef as usize;
        if let Some(seen) = seen_lines.get_mut(ld_idx) {
            *seen = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Full automap draw
// ---------------------------------------------------------------------------

/// Render the complete automap into `canvas`.
///
/// Draw order (back to front):
/// 1. Background clear
/// 2. Grid overlay
/// 3. Map linedefs (filtered by visibility or show_all_lines)
/// 4. Thing markers (filtered by show_all_things)
/// 5. Player arrow
///
/// # Parameters
/// - `canvas`: pixel target
/// - `level`: the current level geometry
/// - `state`: automap state (zoom, center, cheat flags)
/// - `seen_lines`: per-linedef visibility (from GameState)
/// - `player_x`, `player_y`: player position in map units
/// - `player_angle_rad`: player facing angle in radians
/// - `player_thing_indices`: indices of things to always show (player starts)
pub fn draw_automap_full(
    canvas: &mut dyn AutomapCanvas,
    level: &Level,
    state: &AutomapState,
    seen_lines: &[bool],
    player_x: i32,
    player_y: i32,
    player_angle_rad: f64,
) {
    // 1. Background clear.
    canvas.clear(COLOR_BACKGROUND);

    if level.vertexes.is_empty() {
        return;
    }

    let cx = state.center_x;
    let cy = state.center_y;
    let zoom = state.zoom;

    // 2. Grid overlay (behind everything else).
    draw_grid(canvas, cx, cy, zoom);

    // 3. Map linedefs.
    for (i, ld) in level.linedefs.iter().enumerate() {
        let visible = state.show_all_lines || seen_lines.get(i).copied().unwrap_or(false);
        if !visible {
            continue;
        }

        // Choose color: unseen lines revealed by cheat are gray.
        let is_actually_seen = seen_lines.get(i).copied().unwrap_or(false);
        let color = if state.show_all_lines && !is_actually_seen {
            COLOR_UNSEEN
        } else {
            line_color(ld, level)
        };

        let Some(v1) = level.vertexes.get(ld.from_vertex as usize) else {
            continue;
        };
        let Some(v2) = level.vertexes.get(ld.to_vertex as usize) else {
            continue;
        };

        let (x0, y0) = world_to_screen(v1.x as f32, v1.y as f32, cx, cy, zoom);
        let (x1, y1) = world_to_screen(v2.x as f32, v2.y as f32, cx, cy, zoom);

        draw_line(canvas, x0, y0, x1, y1, color);
    }

    // 4. Thing markers.
    draw_things(canvas, level, state, cx, cy, zoom);

    // 5. Player arrow.
    let (px, py) = world_to_screen(player_x as f32, player_y as f32, cx, cy, zoom);
    draw_player_arrow(canvas, px, py, player_angle_rad);
}

/// Draw thing markers on the automap.
///
/// When `show_all_things` is true, ALL things are drawn.
/// Otherwise, only player starts are drawn (they are always visible).
fn draw_things(
    canvas: &mut dyn AutomapCanvas,
    level: &Level,
    state: &AutomapState,
    center_x: f32,
    center_y: f32,
    zoom: f32,
) {
    for thing in &level.things {
        let cat = classify_thing(thing.kind);

        // Without the cheat, only draw player starts.
        if !state.show_all_things && cat != ThingCategory::Player {
            continue;
        }

        let color = thing_marker_color(cat);
        let (sx, sy) = world_to_screen(thing.x as f32, thing.y as f32, center_x, center_y, zoom);
        draw_thing_marker(canvas, sx, sy, color);
    }
}

// ---------------------------------------------------------------------------
// Player arrow
// ---------------------------------------------------------------------------

/// Draw a directional arrow at the player's screen position.
///
/// The arrow is approximately 8 pixels long, pointing in `angle_rad`.
/// Three lines form the arrowhead shape:
///   - A shaft from tail to tip
///   - Two barbs angled 135 degrees from the shaft
fn draw_player_arrow(canvas: &mut dyn AutomapCanvas, px: i32, py: i32, angle_rad: f64) {
    let cos_a = angle_rad.cos() as f32;
    let sin_a = angle_rad.sin() as f32;

    let shaft_len: f32 = 8.0;
    let barb_len: f32 = 4.0;

    // Tip of the arrow (ahead of the player position).
    // Screen Y is flipped, so sin component is subtracted.
    let tip_x = px as f32 + cos_a * shaft_len;
    let tip_y = py as f32 - sin_a * shaft_len;

    // Tail of the arrow (behind the player position).
    let tail_x = px as f32 - cos_a * shaft_len;
    let tail_y = py as f32 + sin_a * shaft_len;

    // Barb angles: 135 degrees from the forward direction (each side).
    let barb_angle_offset = core::f64::consts::FRAC_PI_4 * 3.0;

    let left_barb_angle = angle_rad + barb_angle_offset;
    let right_barb_angle = angle_rad - barb_angle_offset;

    let left_x = tip_x + (left_barb_angle.cos() as f32) * barb_len;
    let left_y = tip_y - (left_barb_angle.sin() as f32) * barb_len;

    let right_x = tip_x + (right_barb_angle.cos() as f32) * barb_len;
    let right_y = tip_y - (right_barb_angle.sin() as f32) * barb_len;

    // Draw shaft: tail -> tip
    draw_line(
        canvas,
        tail_x as i32,
        tail_y as i32,
        tip_x as i32,
        tip_y as i32,
        COLOR_PLAYER_ARROW,
    );
    // Draw left barb: tip -> left
    draw_line(
        canvas,
        tip_x as i32,
        tip_y as i32,
        left_x as i32,
        left_y as i32,
        COLOR_PLAYER_ARROW,
    );
    // Draw right barb: tip -> right
    draw_line(
        canvas,
        tip_x as i32,
        tip_y as i32,
        right_x as i32,
        right_y as i32,
        COLOR_PLAYER_ARROW,
    );
}

// ---------------------------------------------------------------------------
// GameState integration
// ---------------------------------------------------------------------------

/// Mark all linedefs adjacent to the given subsector as seen.
///
/// Called from the BSP traversal during rendering to track which parts
/// of the map the player has visited.
pub fn mark_lines_seen(seen_lines: &mut Vec<bool>, subsector_idx: usize, level: &Level) {
    // Ensure seen_lines is large enough.
    if seen_lines.len() < level.linedefs.len() {
        seen_lines.resize(level.linedefs.len(), false);
    }
    mark_subsector_lines_seen(seen_lines, subsector_idx, level);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use doom_map::Level;
    use doom_map::{
        Blockmap, FLAG_TWO_SIDED, Linedef as LdRaw, Reject, Sector, Seg, Sidedef as SdRaw, Ssector,
        Thing as ThingRaw, Vertex as VxRaw,
    };

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn make_sector(floor: i16, ceil: i16) -> Sector {
        Sector {
            floor_height: floor,
            ceil_height: ceil,
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }
    }

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

    fn make_thing(x: i16, y: i16, kind: u16) -> ThingRaw {
        ThingRaw {
            x,
            y,
            angle: 0,
            kind,
            flags: 7,
        }
    }

    fn make_level_full(
        vertexes: Vec<VxRaw>,
        linedefs: Vec<LdRaw>,
        sidedefs: Vec<SdRaw>,
        sectors: Vec<Sector>,
        segs: Vec<Seg>,
        ssectors: Vec<Ssector>,
        things: Vec<ThingRaw>,
    ) -> Level {
        let n_sectors = sectors.len().max(1);

        let final_sectors = if sectors.is_empty() {
            vec![make_sector(0, 128)]
        } else {
            sectors
        };

        let ssectors = if ssectors.is_empty() {
            vec![Ssector {
                seg_count: 0,
                first_seg: 0,
            }]
        } else {
            ssectors
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
            segs,
            ssectors,
            nodes: vec![],
            sectors: final_sectors,
            reject,
            blockmap,
        }
    }

    fn make_simple_level(vertexes: Vec<VxRaw>, linedefs: Vec<LdRaw>) -> Level {
        make_level_full(vertexes, linedefs, vec![], vec![], vec![], vec![], vec![])
    }

    fn make_default_state() -> AutomapState {
        let mut s = AutomapState::new();
        s.active = true;
        s.zoom = 1.0;
        s.center_x = 0.0;
        s.center_y = 0.0;
        s
    }

    // -----------------------------------------------------------------------
    // Test 1: AutomapState defaults
    // -----------------------------------------------------------------------

    #[test]
    fn automap_state_new_defaults() {
        let state = AutomapState::new();
        assert!(!state.active);
        assert!((state.zoom - 0.5).abs() < f32::EPSILON);
        assert!((state.center_x).abs() < f32::EPSILON);
        assert!((state.center_y).abs() < f32::EPSILON);
        assert!(state.follow_player);
        assert!(!state.show_all_lines);
        assert!(!state.show_all_things);
    }

    // -----------------------------------------------------------------------
    // Test 2: Default trait matches new()
    // -----------------------------------------------------------------------

    #[test]
    fn automap_state_default_matches_new() {
        let a = AutomapState::new();
        let b = AutomapState::default();
        assert_eq!(a.active, b.active);
        assert_eq!(a.show_all_lines, b.show_all_lines);
        assert_eq!(a.show_all_things, b.show_all_things);
        assert!((a.zoom - b.zoom).abs() < f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Test 3: Toggle
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
    // Test 4: Zoom in
    // -----------------------------------------------------------------------

    #[test]
    fn zoom_in_increases() {
        let mut state = AutomapState::new();
        let initial = state.zoom;
        state.zoom_in();
        assert!(state.zoom > initial);
    }

    // -----------------------------------------------------------------------
    // Test 5: Zoom out
    // -----------------------------------------------------------------------

    #[test]
    fn zoom_out_decreases() {
        let mut state = AutomapState::new();
        let initial = state.zoom;
        state.zoom_out();
        assert!(state.zoom < initial);
    }

    // -----------------------------------------------------------------------
    // Test 6: Zoom caps
    // -----------------------------------------------------------------------

    #[test]
    fn zoom_caps_at_limits() {
        let mut state = AutomapState::new();
        for _ in 0..100 {
            state.zoom_in();
        }
        assert!(state.zoom <= ZOOM_MAX + f32::EPSILON);

        for _ in 0..200 {
            state.zoom_out();
        }
        assert!(state.zoom >= ZOOM_MIN - f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Test 7: Update center follows player
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
    // Test 8: Update center ignores when follow disabled
    // -----------------------------------------------------------------------

    #[test]
    fn update_center_ignores_when_disabled() {
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
        let mut canvas = TestCanvas::new(64, 64);
        draw_line(&mut canvas, 10, 20, 50, 20, 176);
        for x in 10..=50 {
            assert_eq!(canvas.get_pixel(x, 20), Some(176));
        }
        assert_eq!(canvas.get_pixel(9, 20), Some(0));
        assert_eq!(canvas.get_pixel(51, 20), Some(0));
    }

    // -----------------------------------------------------------------------
    // Test 10: draw_line vertical
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_vertical() {
        let mut canvas = TestCanvas::new(64, 64);
        draw_line(&mut canvas, 30, 10, 30, 50, 231);
        for y in 10..=50 {
            assert_eq!(canvas.get_pixel(30, y), Some(231));
        }
    }

    // -----------------------------------------------------------------------
    // Test 11: draw_line clips safely
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_clips_to_bounds() {
        let mut canvas = TestCanvas::new(64, 64);
        // Should not panic.
        draw_line(&mut canvas, -100, -100, 200, 200, 255);
        // The diagonal should hit (0,0).
        assert_eq!(canvas.get_pixel(0, 0), Some(255));
    }

    // -----------------------------------------------------------------------
    // Test 12: draw_line single point
    // -----------------------------------------------------------------------

    #[test]
    fn draw_line_single_point() {
        let mut canvas = TestCanvas::new(64, 64);
        draw_line(&mut canvas, 10, 10, 10, 10, 42);
        assert_eq!(canvas.get_pixel(10, 10), Some(42));
    }

    // -----------------------------------------------------------------------
    // Test 13: Thing classification - player
    // -----------------------------------------------------------------------

    #[test]
    fn classify_player_starts() {
        assert_eq!(classify_thing(1), ThingCategory::Player);
        assert_eq!(classify_thing(2), ThingCategory::Player);
        assert_eq!(classify_thing(3), ThingCategory::Player);
        assert_eq!(classify_thing(4), ThingCategory::Player);
        assert_eq!(classify_thing(11), ThingCategory::Player); // deathmatch start
    }

    // -----------------------------------------------------------------------
    // Test 14: Thing classification - monsters
    // -----------------------------------------------------------------------

    #[test]
    fn classify_monsters() {
        assert_eq!(classify_thing(3001), ThingCategory::Monster); // Imp
        assert_eq!(classify_thing(3002), ThingCategory::Monster); // Demon
        assert_eq!(classify_thing(3003), ThingCategory::Monster); // Baron
        assert_eq!(classify_thing(3004), ThingCategory::Monster); // Zombieman
        assert_eq!(classify_thing(3005), ThingCategory::Monster); // Cacodemon
        assert_eq!(classify_thing(7), ThingCategory::Monster); // Spiderdemon
        assert_eq!(classify_thing(16), ThingCategory::Monster); // Cyberdemon
    }

    // -----------------------------------------------------------------------
    // Test 15: Thing classification - keys
    // -----------------------------------------------------------------------

    #[test]
    fn classify_keys() {
        assert_eq!(classify_thing(5), ThingCategory::Key); // Blue key card
        assert_eq!(classify_thing(6), ThingCategory::Key); // Yellow key card
        assert_eq!(classify_thing(13), ThingCategory::Key); // Red skull key
        assert_eq!(classify_thing(38), ThingCategory::Key); // Red skull
        assert_eq!(classify_thing(39), ThingCategory::Key); // Yellow skull
        assert_eq!(classify_thing(40), ThingCategory::Key); // Blue skull
    }

    // -----------------------------------------------------------------------
    // Test 16: Thing classification - items
    // -----------------------------------------------------------------------

    #[test]
    fn classify_items() {
        assert_eq!(classify_thing(2001), ThingCategory::Item); // Shotgun
        assert_eq!(classify_thing(2011), ThingCategory::Item); // Stimpack
        assert_eq!(classify_thing(2012), ThingCategory::Item); // Medikit
        assert_eq!(classify_thing(2013), ThingCategory::Item); // Soul Sphere
        assert_eq!(classify_thing(2019), ThingCategory::Item); // Blue armor
        assert_eq!(classify_thing(8), ThingCategory::Item); // Backpack
    }

    // -----------------------------------------------------------------------
    // Test 17: Thing classification - other
    // -----------------------------------------------------------------------

    #[test]
    fn classify_decorations_as_other() {
        // Decorations, teleport destinations, etc.
        assert_eq!(classify_thing(14), ThingCategory::Other); // Teleport dest
        assert_eq!(classify_thing(10), ThingCategory::Other); // Bloody mess
        assert_eq!(classify_thing(15), ThingCategory::Other); // Dead player
        assert_eq!(classify_thing(9999), ThingCategory::Other); // Unknown type
    }

    // -----------------------------------------------------------------------
    // Test 18: Thing marker colors
    // -----------------------------------------------------------------------

    #[test]
    fn thing_marker_colors_correct() {
        assert_eq!(
            thing_marker_color(ThingCategory::Player),
            COLOR_PLAYER_MARKER
        );
        assert_eq!(thing_marker_color(ThingCategory::Monster), COLOR_MONSTER);
        assert_eq!(thing_marker_color(ThingCategory::Item), COLOR_ITEM);
        assert_eq!(thing_marker_color(ThingCategory::Key), COLOR_KEY);
        assert_eq!(thing_marker_color(ThingCategory::Other), COLOR_GRID);
    }

    // -----------------------------------------------------------------------
    // Test 19: draw_thing_marker draws diamond pixels
    // -----------------------------------------------------------------------

    #[test]
    fn draw_thing_marker_draws_pixels() {
        let mut canvas = TestCanvas::new(32, 32);
        draw_thing_marker(&mut canvas, 16, 16, COLOR_MONSTER);

        // The cardinal points of the diamond should have pixels.
        assert_eq!(canvas.get_pixel(16, 16 - MARKER_SIZE), Some(COLOR_MONSTER)); // top
        assert_eq!(canvas.get_pixel(16 + MARKER_SIZE, 16), Some(COLOR_MONSTER)); // right
        assert_eq!(canvas.get_pixel(16, 16 + MARKER_SIZE), Some(COLOR_MONSTER)); // bottom
        assert_eq!(canvas.get_pixel(16 - MARKER_SIZE, 16), Some(COLOR_MONSTER)); // left
    }

    // -----------------------------------------------------------------------
    // Test 20: draw_thing_marker clips safely
    // -----------------------------------------------------------------------

    #[test]
    fn draw_thing_marker_clips_at_edge() {
        let mut canvas = TestCanvas::new(8, 8);
        // Place marker at corner -- should not panic.
        draw_thing_marker(&mut canvas, 0, 0, COLOR_KEY);
        // At least the center pixel should be set (or nearby).
        // The diamond from (0,0) has points at (0,-2), (2,0), (0,2), (-2,0).
        // Only (2,0) and (0,2) are in-bounds.
        assert_eq!(canvas.get_pixel(2, 0), Some(COLOR_KEY));
        assert_eq!(canvas.get_pixel(0, 2), Some(COLOR_KEY));
    }

    // -----------------------------------------------------------------------
    // Test 21: Grid draws lines
    // -----------------------------------------------------------------------

    #[test]
    fn grid_draws_lines() {
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        // Center at origin, zoom=1.0 => grid lines at 128-unit intervals.
        draw_grid(&mut canvas, 0.0, 0.0, 1.0);

        let grid_pixel_count = canvas.count_color(COLOR_GRID);
        assert!(
            grid_pixel_count > 0,
            "grid should draw at least some pixels"
        );
    }

    // -----------------------------------------------------------------------
    // Test 22: Grid with zero zoom draws nothing
    // -----------------------------------------------------------------------

    #[test]
    fn grid_zero_zoom_draws_nothing() {
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        draw_grid(&mut canvas, 0.0, 0.0, 0.0);
        assert_eq!(canvas.count_nonzero(), 0);
    }

    // -----------------------------------------------------------------------
    // Test 23: init_seen_lines correct size
    // -----------------------------------------------------------------------

    #[test]
    fn init_seen_lines_size() {
        let level = make_simple_level(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }],
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
        let seen = init_seen_lines(&level);
        assert_eq!(seen.len(), 1);
        assert!(!seen[0]);
    }

    // -----------------------------------------------------------------------
    // Test 24: init_seen_lines all false
    // -----------------------------------------------------------------------

    #[test]
    fn init_seen_lines_all_false() {
        let linedefs: Vec<LdRaw> = (0..10)
            .map(|_| LdRaw {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0xFFFF,
                left_sidedef: 0xFFFF,
            })
            .collect();
        let level = make_simple_level(vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }], linedefs);
        let seen = init_seen_lines(&level);
        assert_eq!(seen.len(), 10);
        assert!(seen.iter().all(|&b| !b));
    }

    // -----------------------------------------------------------------------
    // Test 25: mark_subsector_lines_seen marks correct lines
    // -----------------------------------------------------------------------

    #[test]
    fn mark_subsector_lines_seen_marks_correct() {
        // Create 3 linedefs, 2 segs in ssector 0 referencing linedefs 0 and 2.
        let segs = vec![
            Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            },
            Seg {
                from_vertex: 1,
                to_vertex: 2,
                angle: 0,
                linedef: 2,
                direction: 0,
                offset: 0,
            },
        ];
        let ssectors = vec![Ssector {
            seg_count: 2,
            first_seg: 0,
        }];
        let linedefs: Vec<LdRaw> = (0..3)
            .map(|_| LdRaw {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0xFFFF,
                left_sidedef: 0xFFFF,
            })
            .collect();
        let level = make_level_full(
            vec![
                VxRaw { x: 0, y: 0 },
                VxRaw { x: 100, y: 0 },
                VxRaw { x: 100, y: 100 },
            ],
            linedefs,
            vec![],
            vec![],
            segs,
            ssectors,
            vec![],
        );

        let mut seen = init_seen_lines(&level);
        mark_subsector_lines_seen(&mut seen, 0, &level);

        assert!(seen[0], "linedef 0 should be marked seen");
        assert!(!seen[1], "linedef 1 should NOT be marked seen");
        assert!(seen[2], "linedef 2 should be marked seen");
    }

    // -----------------------------------------------------------------------
    // Test 26: mark_subsector_lines_seen with invalid subsector index
    // -----------------------------------------------------------------------

    #[test]
    fn mark_subsector_invalid_index_no_panic() {
        let level = make_simple_level(vec![VxRaw { x: 0, y: 0 }], vec![]);
        let mut seen = vec![];
        // Should not panic.
        mark_subsector_lines_seen(&mut seen, 999, &level);
    }

    // -----------------------------------------------------------------------
    // Test 27: mark_lines_seen resizes vector
    // -----------------------------------------------------------------------

    #[test]
    fn mark_lines_seen_resizes() {
        let level = make_simple_level(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }],
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

        let mut seen = vec![]; // starts empty
        mark_lines_seen(&mut seen, 0, &level);
        assert_eq!(seen.len(), 1, "should resize to linedef count");
    }

    // -----------------------------------------------------------------------
    // Test 28: line_color one-sided
    // -----------------------------------------------------------------------

    #[test]
    fn line_color_one_sided() {
        let level = make_simple_level(vec![], vec![]);
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

    // -----------------------------------------------------------------------
    // Test 29: line_color two-sided same height
    // -----------------------------------------------------------------------

    #[test]
    fn line_color_two_sided_same_height() {
        let sectors = vec![make_sector(0, 128), make_sector(0, 128)];
        let sidedefs = vec![make_sidedef(0), make_sidedef(1)];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }],
            vec![],
            sidedefs,
            sectors,
            vec![],
            vec![],
            vec![],
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
    // Test 30: line_color two-sided height change
    // -----------------------------------------------------------------------

    #[test]
    fn line_color_two_sided_height_change() {
        let sectors = vec![make_sector(0, 128), make_sector(24, 128)];
        let sidedefs = vec![make_sidedef(0), make_sidedef(1)];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }],
            vec![],
            sidedefs,
            sectors,
            vec![],
            vec![],
            vec![],
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
    // Test 31: line_color secret
    // -----------------------------------------------------------------------

    #[test]
    fn line_color_secret_takes_priority() {
        let level = make_simple_level(vec![], vec![]);
        let ld = LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: FLAG_SECRET | FLAG_TWO_SIDED,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF,
        };
        assert_eq!(line_color(&ld, &level), COLOR_SECRET);
    }

    // -----------------------------------------------------------------------
    // Test 32: world_to_screen at center
    // -----------------------------------------------------------------------

    #[test]
    fn world_to_screen_center() {
        let (sx, sy) = world_to_screen(0.0, 0.0, 0.0, 0.0, 1.0);
        assert_eq!(sx, HALF_W);
        assert_eq!(sy, HALF_H);
    }

    // -----------------------------------------------------------------------
    // Test 33: world_to_screen offset
    // -----------------------------------------------------------------------

    #[test]
    fn world_to_screen_offset() {
        // 100 units right of center at zoom 1.0 => 100 pixels right.
        let (sx, sy) = world_to_screen(100.0, 0.0, 0.0, 0.0, 1.0);
        assert_eq!(sx, HALF_W + 100);
        assert_eq!(sy, HALF_H);

        // 100 units up => 100 pixels up (screen Y decreases).
        let (sx, sy) = world_to_screen(0.0, 100.0, 0.0, 0.0, 1.0);
        assert_eq!(sx, HALF_W);
        assert_eq!(sy, HALF_H - 100);
    }

    // -----------------------------------------------------------------------
    // Test 34: world_to_screen with zoom
    // -----------------------------------------------------------------------

    #[test]
    fn world_to_screen_with_zoom() {
        // 100 units right at zoom 2.0 => 200 pixels right.
        let (sx, _) = world_to_screen(100.0, 0.0, 0.0, 0.0, 2.0);
        assert_eq!(sx, HALF_W + 200);
    }

    // -----------------------------------------------------------------------
    // Test 35: draw_automap_full on empty level
    // -----------------------------------------------------------------------

    #[test]
    fn draw_automap_full_empty_level() {
        let level = make_simple_level(vec![], vec![]);
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let state = make_default_state();
        let seen = vec![];

        draw_automap_full(&mut canvas, &level, &state, &seen, 0, 0, 0.0);

        // With no vertexes, only background should be drawn.
        assert!(canvas.data.iter().all(|&b| b == 0));
    }

    // -----------------------------------------------------------------------
    // Test 36: draw_automap_full draws seen lines
    // -----------------------------------------------------------------------

    #[test]
    fn draw_automap_full_draws_seen_lines() {
        let vertexes = vec![VxRaw { x: -50, y: 0 }, VxRaw { x: 50, y: 0 }];
        let linedefs = vec![LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0xFFFF,
            left_sidedef: 0xFFFF,
        }];
        let level = make_simple_level(vertexes, linedefs);
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let state = make_default_state();
        let seen = vec![true]; // line 0 is seen

        draw_automap_full(&mut canvas, &level, &state, &seen, 0, 0, 0.0);

        // Should have drawn the line (non-grid non-zero pixels).
        let non_grid_non_zero = canvas
            .data
            .iter()
            .filter(|&&b| b != 0 && b != COLOR_GRID)
            .count();
        assert!(non_grid_non_zero > 0, "seen line should produce pixels");
    }

    // -----------------------------------------------------------------------
    // Test 37: draw_automap_full hides unseen lines
    // -----------------------------------------------------------------------

    #[test]
    fn draw_automap_full_hides_unseen_lines() {
        let vertexes = vec![VxRaw { x: -50, y: 0 }, VxRaw { x: 50, y: 0 }];
        let linedefs = vec![LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0xFFFF,
            left_sidedef: 0xFFFF,
        }];
        let level = make_simple_level(vertexes, linedefs);
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let state = make_default_state();
        let seen = vec![false]; // line 0 is NOT seen

        draw_automap_full(&mut canvas, &level, &state, &seen, 0, 0, 0.0);

        // Should NOT have drawn the linedef (only grid + player arrow).
        // Check that no red (COLOR_ONE_SIDED) pixels exist.
        assert_eq!(
            canvas.count_color(COLOR_ONE_SIDED),
            0,
            "unseen line should not be drawn"
        );
    }

    // -----------------------------------------------------------------------
    // Test 38: show_all_lines reveals unseen lines in gray
    // -----------------------------------------------------------------------

    #[test]
    fn show_all_lines_reveals_unseen_in_gray() {
        let vertexes = vec![VxRaw { x: -50, y: 0 }, VxRaw { x: 50, y: 0 }];
        let linedefs = vec![LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0xFFFF,
            left_sidedef: 0xFFFF,
        }];
        let level = make_simple_level(vertexes, linedefs);
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let mut state = make_default_state();
        state.show_all_lines = true;
        let seen = vec![false]; // line 0 NOT seen by player

        draw_automap_full(&mut canvas, &level, &state, &seen, 0, 0, 0.0);

        // The unseen line should be drawn in gray.
        assert!(
            canvas.count_color(COLOR_UNSEEN) > 0,
            "show_all_lines should reveal unseen lines in gray"
        );
    }

    // -----------------------------------------------------------------------
    // Test 39: show_all_lines + seen line uses normal color
    // -----------------------------------------------------------------------

    #[test]
    fn show_all_lines_seen_uses_normal_color() {
        let vertexes = vec![VxRaw { x: -50, y: 0 }, VxRaw { x: 50, y: 0 }];
        let linedefs = vec![LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0xFFFF,
            left_sidedef: 0xFFFF,
        }];
        let level = make_simple_level(vertexes, linedefs);
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let mut state = make_default_state();
        state.show_all_lines = true;
        let seen = vec![true]; // line IS seen

        draw_automap_full(&mut canvas, &level, &state, &seen, 0, 0, 0.0);

        // Seen line should use normal color (red for one-sided), not gray.
        assert!(
            canvas.count_color(COLOR_ONE_SIDED) > 0,
            "seen line with show_all_lines should use normal color"
        );
        assert_eq!(
            canvas.count_color(COLOR_UNSEEN),
            0,
            "seen line should not be gray"
        );
    }

    // -----------------------------------------------------------------------
    // Test 40: show_all_things draws thing markers
    // -----------------------------------------------------------------------

    #[test]
    fn show_all_things_draws_markers() {
        let things = vec![
            make_thing(0, 0, 3001), // Imp (monster)
        ];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            things,
        );
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let mut state = make_default_state();
        state.show_all_things = true;

        draw_automap_full(&mut canvas, &level, &state, &[], 0, 0, 0.0);

        // Monster marker should have been drawn at center.
        assert!(
            canvas.count_color(COLOR_MONSTER) > 0,
            "show_all_things should draw monster marker"
        );
    }

    // -----------------------------------------------------------------------
    // Test 41: without show_all_things, monsters are hidden
    // -----------------------------------------------------------------------

    #[test]
    fn no_show_all_things_hides_monsters() {
        let things = vec![
            make_thing(0, 0, 3001), // Imp
        ];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 0 }],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            things,
        );
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let state = make_default_state(); // show_all_things=false

        draw_automap_full(&mut canvas, &level, &state, &[], 0, 0, 0.0);

        // Monster marker should NOT have been drawn.
        assert_eq!(
            canvas.count_color(COLOR_MONSTER),
            0,
            "monsters should be hidden without show_all_things"
        );
    }

    // -----------------------------------------------------------------------
    // Test 42: player starts always shown
    // -----------------------------------------------------------------------

    #[test]
    fn player_starts_always_shown() {
        let things = vec![
            make_thing(50, 50, 1), // Player 1 start
        ];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }, VxRaw { x: 100, y: 100 }],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            things,
        );
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let state = make_default_state(); // show_all_things=false

        draw_automap_full(&mut canvas, &level, &state, &[], 50, 50, 0.0);

        // Player start marker should be drawn even without show_all_things.
        assert!(
            canvas.count_color(COLOR_PLAYER_MARKER) > 0,
            "player starts should always be drawn"
        );
    }

    // -----------------------------------------------------------------------
    // Test 43: show_all_things draws keys with correct color
    // -----------------------------------------------------------------------

    #[test]
    fn show_all_things_key_color() {
        let things = vec![
            make_thing(0, 0, 5), // Blue keycard
        ];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            things,
        );
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let mut state = make_default_state();
        state.show_all_things = true;

        draw_automap_full(&mut canvas, &level, &state, &[], 0, 0, 0.0);

        assert!(
            canvas.count_color(COLOR_KEY) > 0,
            "key things should be drawn in yellow"
        );
    }

    // -----------------------------------------------------------------------
    // Test 44: show_all_things draws items with correct color
    // -----------------------------------------------------------------------

    #[test]
    fn show_all_things_item_color() {
        let things = vec![
            make_thing(0, 0, 2012), // Medikit
        ];
        let level = make_level_full(
            vec![VxRaw { x: 0, y: 0 }],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            things,
        );
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let mut state = make_default_state();
        state.show_all_things = true;

        draw_automap_full(&mut canvas, &level, &state, &[], 0, 0, 0.0);

        assert!(
            canvas.count_color(COLOR_ITEM) > 0,
            "item things should be drawn in green"
        );
    }

    // -----------------------------------------------------------------------
    // Test 45: player arrow draws pixels
    // -----------------------------------------------------------------------

    #[test]
    fn player_arrow_draws_pixels() {
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        draw_player_arrow(&mut canvas, 160, 100, 0.0);
        assert!(
            canvas.count_color(COLOR_PLAYER_ARROW) > 0,
            "player arrow should draw visible pixels"
        );
    }

    // -----------------------------------------------------------------------
    // Test 46: Grid is behind map lines (draw order)
    // -----------------------------------------------------------------------

    #[test]
    fn grid_drawn_behind_lines() {
        // A horizontal line at y=0 with zoom=1.0 centered at origin.
        // The grid has lines at y=0 (screen y=100). The linedef also crosses y=100.
        // The linedef color (red) should overwrite grid color at overlap points.
        let vertexes = vec![VxRaw { x: -50, y: 0 }, VxRaw { x: 50, y: 0 }];
        let linedefs = vec![LdRaw {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0xFFFF,
            left_sidedef: 0xFFFF,
        }];
        let level = make_simple_level(vertexes, linedefs);
        let mut canvas = TestCanvas::new(SCREEN_W, SCREEN_H);
        let state = make_default_state();
        let seen = vec![true];

        draw_automap_full(&mut canvas, &level, &state, &seen, 0, 0, 0.0);

        // At screen center (160, 100), the linedef passes through.
        // If grid and linedef overlap, linedef color should be on top.
        // Check that the center pixel row has red pixels.
        let center_y = HALF_H;
        let has_linedef_color_at_center =
            (0..SCREEN_W).any(|x| canvas.get_pixel(x, center_y) == Some(COLOR_ONE_SIDED));
        assert!(
            has_linedef_color_at_center,
            "linedef should overwrite grid at overlap points"
        );
    }

    // -----------------------------------------------------------------------
    // Test 47: TestCanvas dimensions
    // -----------------------------------------------------------------------

    #[test]
    fn test_canvas_dimensions() {
        let canvas = TestCanvas::new(100, 50);
        assert_eq!(canvas.width(), 100);
        assert_eq!(canvas.height(), 50);
        assert_eq!(canvas.data.len(), 5000);
    }

    // -----------------------------------------------------------------------
    // Test 48: TestCanvas clear
    // -----------------------------------------------------------------------

    #[test]
    fn test_canvas_clear() {
        let mut canvas = TestCanvas::new(10, 10);
        canvas.set_pixel(5, 5, 42);
        assert_eq!(canvas.get_pixel(5, 5), Some(42));
        canvas.clear(0);
        assert_eq!(canvas.get_pixel(5, 5), Some(0));
    }

    // -----------------------------------------------------------------------
    // Test 49: TestCanvas out-of-bounds get_pixel
    // -----------------------------------------------------------------------

    #[test]
    fn test_canvas_out_of_bounds() {
        let canvas = TestCanvas::new(10, 10);
        assert_eq!(canvas.get_pixel(10, 10), None);
        assert_eq!(canvas.get_pixel(-1, -1), None);
    }

    // -----------------------------------------------------------------------
    // Test 50: TestCanvas count helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_canvas_count_helpers() {
        let mut canvas = TestCanvas::new(10, 10);
        assert_eq!(canvas.count_nonzero(), 0);
        canvas.set_pixel(0, 0, 42);
        canvas.set_pixel(1, 0, 42);
        canvas.set_pixel(2, 0, 99);
        assert_eq!(canvas.count_nonzero(), 3);
        assert_eq!(canvas.count_color(42), 2);
        assert_eq!(canvas.count_color(99), 1);
    }

    // -----------------------------------------------------------------------
    // Test 51: Multiple subsectors mark different lines
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_subsectors_mark_different_lines() {
        let segs = vec![
            // ssector 0: seg refs linedef 0
            Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            },
            // ssector 1: seg refs linedef 1
            Seg {
                from_vertex: 1,
                to_vertex: 2,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
        ];
        let ssectors = vec![
            Ssector {
                seg_count: 1,
                first_seg: 0,
            },
            Ssector {
                seg_count: 1,
                first_seg: 1,
            },
        ];
        let linedefs: Vec<LdRaw> = (0..2)
            .map(|_| LdRaw {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0xFFFF,
                left_sidedef: 0xFFFF,
            })
            .collect();
        let level = make_level_full(
            vec![
                VxRaw { x: 0, y: 0 },
                VxRaw { x: 100, y: 0 },
                VxRaw { x: 100, y: 100 },
            ],
            linedefs,
            vec![],
            vec![],
            segs,
            ssectors,
            vec![],
        );

        let mut seen = init_seen_lines(&level);

        // Visit ssector 0 only.
        mark_subsector_lines_seen(&mut seen, 0, &level);
        assert!(seen[0]);
        assert!(!seen[1]);

        // Visit ssector 1.
        mark_subsector_lines_seen(&mut seen, 1, &level);
        assert!(seen[0]);
        assert!(seen[1]);
    }
}
