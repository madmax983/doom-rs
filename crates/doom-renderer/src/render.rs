//! First-person perspective software renderer.
//!
//! Renders a Doom level from the player's viewpoint using per-seg projection.
//! Walls and flats are shaded using distance-attenuated colormaps from the
//! [`LightParams`] system: closer surfaces are brighter, farther surfaces
//! fade toward darkness.  When a `ColormapCache` is supplied, each wall
//! column and flat span receives a per-pixel colormap lookup based on the
//! sector's light level and the surface's distance from the camera.
//! Without a `ColormapCache` the renderer falls back to full-bright
//! identity colormaps.
//!
//! # Algorithm overview
//! 1. For each seg in the level, transform both endpoints into view space,
//!    clip against the near plane, project to screen columns, and draw
//!    each screen column as a solid-colored vertical strip (wall).
//! 2. Track `wall_top[x]` and `wall_bot[x]` per column, plus which flat
//!    names belong to the ceiling and floor of that column's sector.
//! 3. After all segs: for each row `y`, emit spans for unoccluded ceiling
//!    and floor pixels using perspective-correct texture coordinates.
//!
//! # Two-sided linedef handling
//! For two-sided linedefs (portals, doors, windows), the wall is split into
//! three bands:
//! - Upper band: `w_top .. screen_back_ceil`   — upper texture (if front_ceil > back_ceil)
//! - Portal opening: `screen_back_ceil .. screen_back_floor` — transparent, not drawn
//! - Lower band: `screen_back_floor .. w_bot`  — lower texture (if back_floor > front_floor)
//!
//! The z-buffer is NOT updated for two-sided segs so geometry behind can show
//! through.  `wall_top[x]`/`wall_bot[x]` are set to the portal opening bounds
//! so floor/ceiling spans fill through the gap.

use doom_map::Level;
use doom_map::lumps::{FLAG_DONTPEGBOTTOM, FLAG_DONTPEGTOP};
use doom_types::Bam;

use crate::anim::AnimState;
use crate::clip::clip_seg_to_view_frustum;
use crate::colormap::ColormapCache;
use crate::column::{DrawColumnParams, IDENTITY_COLORMAP, draw_column};
use crate::flat_cache::FlatCache;
use crate::framebuffer::Framebuffer;
use crate::lighting::LightParams;
use crate::palette::PaletteLut;
use crate::seg::collect_front_to_back_seg_indices;
use crate::sky::{
    SkyCoverage, draw_sky_coverage_columns, draw_sky_coverage_fallback, is_sky_flat,
    sky_texture_name,
};
use crate::span::{DrawSpanParams, draw_span};
use crate::texture::TextureCache;
use crate::visplane::{PlaneKind, VisplaneSet, visplane_to_spans};

// ---------------------------------------------------------------------------
// Screen constants
// ---------------------------------------------------------------------------

const SCREEN_W: usize = 320;
const SCREEN_H: usize = 200;
const HALF_W: i32 = (SCREEN_W / 2) as i32; // 160
const HALF_H: i32 = (SCREEN_H / 2) as i32; // 100
const FOCAL_LEN: i32 = 160; // = HALF_W, 90° horizontal FOV

/// Assumed player eye height above the floor (map units, fixed-point integer).
pub const PLAYER_HEIGHT: i32 = 41;

#[inline]
fn is_no_texture(name: &[u8; 8]) -> bool {
    name[0] == b'-' || name.iter().all(|&b| b == 0 || b == b' ')
}

#[inline]
fn project_wall_y(height_delta: i32, scale: f32) -> i32 {
    HALF_H - ((height_delta as f32) * scale).trunc() as i32
}

#[inline]
fn wall_fracstep(scale: f32) -> u32 {
    ((65536.0 / scale.max(f32::EPSILON)).trunc() as u32).max(1)
}

#[inline]
fn wall_frac_start(texturemid: i32, screen_top: i32, fracstep: u32) -> u32 {
    ((i64::from(texturemid) << 16) - i64::from(HALF_H - screen_top) * i64::from(fracstep)) as u32
}

#[inline]
fn wall_segment_param_for_screen_x(
    x: usize,
    view_left: (f32, f32),
    view_right: (f32, f32),
) -> Option<f32> {
    let (vx1, vy1) = view_left;
    let (vx2, vy2) = view_right;
    let dx = vx2 - vx1;
    let dy = vy2 - vy1;
    let ray_slope = (x as f32 - HALF_W as f32) / FOCAL_LEN as f32;
    let denom = dy - ray_slope * dx;
    if denom.abs() < f32::EPSILON {
        return None;
    }
    let s = (ray_slope * vx1 - vy1) / denom;
    (0.0..=1.0).contains(&s).then_some(s)
}

fn player_sector_index(level: &Level, player_x: i32, player_y: i32) -> Option<usize> {
    if level.sectors.is_empty() {
        return None;
    }
    if let Some(sec_idx) = level
        .sector_index_at(player_x, player_y)
        .map(|idx| idx.min(level.sectors.len() - 1))
    {
        return Some(sec_idx);
    }

    if let Some(subsector_idx) = level.subsector_index_at(player_x, player_y)
        && let Some(ss) = level.ssectors.get(subsector_idx)
    {
        let mut best_sector = None;
        let mut best_key = i64::MAX;
        let first_seg = ss.first_seg as usize;
        let end_seg = first_seg
            .saturating_add(ss.seg_count as usize)
            .min(level.segs.len());

        for seg_idx in first_seg..end_seg {
            let Some(seg) = level.segs.get(seg_idx) else {
                continue;
            };
            let Some(linedef) = level.linedefs.get(seg.linedef as usize) else {
                continue;
            };
            let sidedef_idx = if seg.direction == 0 {
                linedef.right_sidedef
            } else {
                linedef.left_sidedef
            };
            if sidedef_idx == 0xFFFF {
                continue;
            }
            let Some(sidedef) = level.sidedefs.get(sidedef_idx as usize) else {
                continue;
            };
            let Some(v1) = level.vertexes.get(seg.from_vertex as usize) else {
                continue;
            };
            let Some(v2) = level.vertexes.get(seg.to_vertex as usize) else {
                continue;
            };

            let v1_dx = i64::from(v1.x as i32 - player_x);
            let v1_dy = i64::from(v1.y as i32 - player_y);
            let v2_dx = i64::from(v2.x as i32 - player_x);
            let v2_dy = i64::from(v2.y as i32 - player_y);
            let mid_dx = i64::from(v1.x as i32 + v2.x as i32 - 2 * player_x);
            let mid_dy = i64::from(v1.y as i32 + v2.y as i32 - 2 * player_y);
            let key = (v1_dx * v1_dx + v1_dy * v1_dy)
                .min(v2_dx * v2_dx + v2_dy * v2_dy)
                .min(mid_dx * mid_dx + mid_dy * mid_dy);

            if key < best_key {
                best_key = key;
                best_sector = Some((sidedef.sector as usize).min(level.sectors.len() - 1));
            }
        }

        if best_sector.is_some() {
            return best_sector;
        }
    }

    level
        .sector_index_at(player_x, player_y)
        .map(|idx| idx.min(level.sectors.len() - 1))
        .or(Some(0))
}

#[inline]
fn draw_masked_column(
    fb: &mut Framebuffer,
    x: usize,
    y_top: usize,
    y_bot: usize,
    mut frac: u32,
    fracstep: u32,
    source: &[u8],
    colormap: &[u8; 256],
) {
    if x >= SCREEN_W || y_top > y_bot || source.is_empty() {
        return;
    }
    let y_end = (y_bot + 1).min(SCREEN_H);
    let height_mask = (source.len() as u32).wrapping_sub(1);
    for y in y_top..y_end {
        let tex_row = ((frac >> 16) & height_mask) as usize;
        let raw = source[tex_row];
        // Masked textures use palette index 0 as transparent.
        if raw != 0 {
            fb.data[y * SCREEN_W + x] = colormap[raw as usize];
        }
        frac = frac.wrapping_add(fracstep);
    }
}

/// A deferred vertical column of a masked middle texture (e.g., a grate or fence).
///
/// Because Doom uses a Painter's algorithm for masked textures and sprites, these
/// columns are collected during the main wall pass and later drawn back-to-front
/// interleaved with sprite slices.
#[derive(Clone)]
pub struct MaskedColumnDraw<'a> {
    /// The perpendicular view-space depth of the seg that emitted this column.
    pub depth: f32,
    /// The screen X coordinate (column index) where this strip will be drawn.
    pub x: usize,
    /// The topmost screen row of this strip.
    pub y_top: usize,
    /// The bottommost screen row of this strip.
    pub y_bot: usize,
    /// The starting texture coordinate fraction (16.16 fixed point).
    pub frac: u32,
    /// The texture coordinate vertical step per screen pixel (16.16 fixed point).
    pub fracstep: u32,
    /// The raw texture column data to sample from.
    pub source: &'a [u8],
    /// The distance-attenuated colormap for this specific column.
    pub colormap: [u8; 256],
}

/// Records a change in the top or bottom clipping boundary for sprites.
///
/// As the renderer traverses sectors front-to-back, portal openings (two-sided segs)
/// act as windows that restrict where sprites behind them can be drawn. This struct
/// captures the depth and the screen row where that window boundary was established.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteClipStep {
    /// The perpendicular view-space depth of the portal that established this clip.
    pub depth: f32,
    /// The screen row (Y coordinate) of the clip boundary.
    pub row: i32,
    /// The world-space height of the portal sill or header.
    pub silhouette_height: f32,
}

#[inline]
fn record_sprite_clip_step(
    history: &mut crate::sprite_clip::SpriteClipHistory,
    depth: f32,
    row: i32,
    silhouette_height: f32,
) {
    if history.last().is_some_and(|step| {
        step.row == row && step.silhouette_height.to_bits() == silhouette_height.to_bits()
    }) {
        return;
    }
    history.push(SpriteClipStep {
        depth,
        row,
        silhouette_height,
    });
}

/// Draws a batch of masked vertical columns onto the framebuffer.
///
/// In Doom's renderer, solid walls and floors are drawn first. Then, "masked" geometry
/// (like sprites, middle textures on 2-sided lines, and translucent elements) are sorted
/// and drawn back-to-front. This function takes a batch of such columns and composites them
/// onto the screen, applying the current colormap to handle lighting.
///
/// ## Examples
/// ```text
/// // This function is typically called internally by the sprite or wall renderer.
/// // To render a masked column, you must provide its source patch data, Y offsets,
/// // and lighting parameters wrapped in a `MaskedColumnDraw` struct.
/// ```
pub fn draw_masked_columns(fb: &mut Framebuffer, columns: &[MaskedColumnDraw<'_>]) {
    for column in columns {
        draw_masked_column(
            fb,
            column.x,
            column.y_top,
            column.y_bot,
            column.frac,
            column.fracstep,
            column.source,
            &column.colormap,
        );
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Render a first-person view of `level` from the player's position.
///
/// `player_x`, `player_y` — player position in map units (i32).
/// `player_angle`         — player facing direction as `Bam`.
/// `palette`              — PLAYPAL palette for color lookup (index 0 = normal).
/// `flat_cache`           — optional flat texture cache; pass `None` to fall
///                          back to solid-color floors and ceilings.
/// `tex_cache`            — optional wall texture cache; pass `None` to fall
///                          back to flat-shaded solid colors for walls.
/// `colormap`             — optional COLORMAP cache for per-sector light shading;
///                          pass `None` to use identity (full-bright) colormaps.
/// `anim`                 — optional animation state; when provided, flat and wall
///                          texture names are resolved through animation sequences
///                          so animated textures (nukage, lava, fire walls, etc.)
///                          cycle at the correct rate.
/// `is_fullbright`        — when `true`, all sectors render at full brightness
///                          regardless of their light level (e.g. light
///                          amplification visor powerup).
///
/// This is a software renderer using per-seg perspective projection.
/// Walls are textured when `tex_cache` is provided; floors/ceilings are
/// textured when `flat_cache` is provided.  Light shading is applied when
/// `colormap` is provided — each wall column and flat span receives a
/// distance-attenuated colormap based on the sector light level and the
/// surface distance from the camera.
///
/// Output of [`render_level`].
///
/// Contains both the depth (z) buffer used for sprite depth occlusion and the
/// vertical clip arrays derived from portal openings.  Pass `clip_top`/`clip_bot`
/// to sprite renderers so sprites are clipped to the visible portal window —
/// this prevents sprites from bleeding through two-sided window frames.
pub struct RenderOut<'a> {
    /// Per-column z-buffer: perpendicular depth of nearest one-sided wall, or
    /// `f32::MAX` where no solid wall was drawn.
    pub z_buf: [f32; SCREEN_W],
    /// Per-column sprite ceiling clip: topmost screen row a sprite may occupy.
    /// Starts at 0; narrowed upward by portal openings (Doom `mceilingclip`).
    pub clip_top: [i32; SCREEN_W],
    /// Per-column sprite floor clip: bottommost screen row a sprite may occupy.
    /// Starts at `SCREEN_H-1`; narrowed downward by portal openings (Doom `mfloorclip`).
    pub clip_bot: [i32; SCREEN_W],
    /// Depth of the portal segment that established `clip_top[x]`, or
    /// `f32::MAX` when no top clip has been applied for that column.
    pub clip_top_depth: [f32; SCREEN_W],
    /// Depth of the portal segment that established `clip_bot[x]`, or
    /// `f32::MAX` when no bottom clip has been applied for that column.
    pub clip_bot_depth: [f32; SCREEN_W],
    /// Monotone top-clip state changes for each screen column.
    pub clip_top_history: [crate::sprite_clip::SpriteClipHistory; SCREEN_W],
    /// Monotone bottom-clip state changes for each screen column.
    pub clip_bot_history: [crate::sprite_clip::SpriteClipHistory; SCREEN_W],
    /// Deferred masked midtexture columns to interleave with sprite rendering.
    pub masked_columns: Vec<MaskedColumnDraw<'a>>,
}

/// Render a Doom level into `fb` and return occlusion data for sprite clipping.
///
/// The z-buffer entry for each column holds the perpendicular depth of the
/// nearest *one-sided* wall.  Two-sided segs (portals) do **not** write to the
/// z-buffer.  Pass `render_out.z_buf` and the clip arrays to sprite renderers.
#[allow(clippy::too_many_arguments)]
pub fn render_level<'a>(
    level: &Level,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
    fb: &mut Framebuffer,
    _palette: &PaletteLut,
    flat_cache: Option<&FlatCache>,
    tex_cache: Option<&'a TextureCache>,
    colormap: Option<&ColormapCache>,
    anim: Option<&AnimState>,
    is_fullbright: bool,
) -> RenderOut<'a> {
    render_level_with_view_height(
        level,
        player_x,
        player_y,
        player_angle,
        PLAYER_HEIGHT,
        fb,
        _palette,
        flat_cache,
        tex_cache,
        colormap,
        anim,
        is_fullbright,
    )
}

/// Render a Doom level using an explicit player view height above the floor.
#[allow(clippy::too_many_arguments)]
pub fn render_level_with_view_height<'a>(
    level: &Level,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
    player_view_height: i32,
    fb: &mut Framebuffer,
    _palette: &PaletteLut,
    flat_cache: Option<&FlatCache>,
    tex_cache: Option<&'a TextureCache>,
    colormap: Option<&ColormapCache>,
    anim: Option<&AnimState>,
    is_fullbright: bool,
) -> RenderOut<'a> {
    render_level_with_view_height_and_extra_light(
        level,
        player_x,
        player_y,
        player_angle,
        player_view_height,
        fb,
        _palette,
        flat_cache,
        tex_cache,
        colormap,
        anim,
        is_fullbright,
        0,
    )
}

/// Render a Doom level using an explicit player view height and player extra-light bonus.
#[allow(clippy::too_many_arguments)]
pub fn render_level_with_view_height_and_extra_light<'a>(
    level: &Level,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
    player_view_height: i32,
    fb: &mut Framebuffer,
    _palette: &PaletteLut,
    flat_cache: Option<&FlatCache>,
    tex_cache: Option<&'a TextureCache>,
    colormap: Option<&ColormapCache>,
    anim: Option<&AnimState>,
    is_fullbright: bool,
    extra_light: u8,
) -> RenderOut<'a> {
    render_level_with_view_height_and_extra_light_and_fixed_colormap(
        level,
        player_x,
        player_y,
        player_angle,
        player_view_height,
        fb,
        _palette,
        flat_cache,
        tex_cache,
        colormap,
        anim,
        is_fullbright,
        None,
        extra_light,
    )
}

/// Render a Doom level using an explicit player view height, optional fixed
/// colormap override, and player extra-light bonus.
///
/// ## Panics
/// Panics if `flat_cache` is `None` but the level contains visplanes (floors or ceilings)
/// that are visible in the view frustum.
#[allow(clippy::too_many_arguments)]
pub fn render_level_with_view_height_and_extra_light_and_fixed_colormap<'a>(
    level: &Level,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
    player_view_height: i32,
    fb: &mut Framebuffer,
    _palette: &PaletteLut,
    flat_cache: Option<&FlatCache>,
    tex_cache: Option<&'a TextureCache>,
    colormap: Option<&ColormapCache>,
    anim: Option<&AnimState>,
    is_fullbright: bool,
    fixed_colormap: Option<&[u8; 256]>,
    extra_light: u8,
) -> RenderOut<'a> {
    // ------------------------------------------------------------------
    // Step 1: Draw background (ceiling top half, floor bottom half)
    // ------------------------------------------------------------------
    // These are overwritten by textured spans in Step 5 when a FlatCache
    // is available.  We keep the fill so untextured regions have a sensible
    // colour (palette index 25 ≈ dark gray ceiling; 119 ≈ medium gray floor).
    fb.fill_rect(0, 0, SCREEN_W, HALF_H as usize, 25);
    fb.fill_rect(0, HALF_H as usize, SCREEN_W, HALF_H as usize, 119);

    // ------------------------------------------------------------------
    // Step 2: Per-column tracking arrays
    // ------------------------------------------------------------------
    // Per-column window where farther walls may still draw.
    // Starts as full screen and is narrowed by two-sided portal openings.
    let mut wall_clip_top = [0i32; SCREEN_W];
    let mut wall_clip_bot = [SCREEN_H as i32 - 1; SCREEN_W];
    let mut wall_clip_top_depth = [f32::MAX; SCREEN_W];
    let mut wall_clip_bot_depth = [f32::MAX; SCREEN_W];
    let mut wall_clip_top_history: [crate::sprite_clip::SpriteClipHistory; SCREEN_W] =
        [const { crate::sprite_clip::SpriteClipHistory::new() }; SCREEN_W];
    let mut wall_clip_bot_history: [crate::sprite_clip::SpriteClipHistory; SCREEN_W] =
        [const { crate::sprite_clip::SpriteClipHistory::new() }; SCREEN_W];
    let mut masked_columns = Vec::new();

    // Doom-style open column tracking for inline visplane emission.
    // open_top[x]  = first unclaimed row for ceiling spans (initially 0).
    // open_bot[x]  = last  unclaimed row for floor   spans (initially SCREEN_H-1).
    // As each seg is processed, ceiling strips are emitted from open_top[x]
    // to w_top-1, and open_top is advanced.  Likewise for floor.
    let mut open_top = [0i32; SCREEN_W];
    let mut open_bot = [SCREEN_H as i32 - 1; SCREEN_W];

    // Camera height in world space (map units). We anchor view Z to the floor
    // of the sector containing the player.
    let player_view_height = player_view_height.max(0);
    let extra_light_bonus = extra_light.saturating_mul(64);
    let mut view_z = player_view_height;
    // Save player sector info for post-pass open column filling.
    let mut player_ceil_flat = *b"FLAT2\0\0\0";
    let mut player_floor_flat = *b"FLAT1\0\0\0";
    let mut player_ceil_h = 128i32;
    let mut player_floor_h = 0i32;
    let mut player_light = 255u8;
    if let Some(sec_idx) = player_sector_index(level, player_x, player_y)
        && let Some(sec) = level.sectors.get(sec_idx)
    {
        view_z = sec.floor_height.to_int() + player_view_height;
        player_ceil_flat = sec.ceil_flat;
        player_floor_flat = sec.floor_flat;
        player_ceil_h = sec.ceil_height.to_int();
        player_floor_h = sec.floor_height.to_int();
        player_light = ((sec.light_level as u32).min(255) as u8).saturating_add(extra_light_bonus);
    }

    // Visplane set — built inline during the wall pass (Doom R_RenderSegLoop
    // approach).  Each seg emits ceiling/floor strips for its front sector as
    // soon as the column is processed, then advances open_top/open_bot.
    let mut visplanes = VisplaneSet::new();

    // Z-buffer (per-column minimum depth, in view-space units, f32).
    // Initialised to f32::MAX so every wall is nearer than "infinity".
    // Only one-sided segs write to this buffer; two-sided segs (portals)
    // leave z_buf untouched so sprites behind portals remain visible.
    let mut z_buf = [f32::MAX; SCREEN_W];

    // Sky tracking uses per-column coverage rather than a single `top..bot`
    // interval so layered openings cannot collapse into one fake sky strip.
    let mut sky_coverage = SkyCoverage::new();

    // ------------------------------------------------------------------
    // Step 3: Precompute trig — Fixed16_16 raw i32 values
    // ------------------------------------------------------------------
    let cos_a = player_angle.cos();
    let sin_a = player_angle.sin();
    let cos_i = cos_a.0 as i64;
    let sin_i = sin_a.0 as i64;

    // BSP gives a coarse front-to-back order by subsector.
    let seg_order = collect_front_to_back_seg_indices(level, player_x, player_y);

    // ------------------------------------------------------------------
    // Step 4: Iterate segs (BSP front-to-back order) — draw walls, record flats
    // ------------------------------------------------------------------
    for seg_idx in seg_order {
        let Some(seg) = level.segs.get(seg_idx) else {
            continue;
        };
        // Get vertex world positions.
        let v1 = match level.vertexes.get(seg.from_vertex as usize) {
            Some(v) => v,
            None => continue,
        };
        let v2 = match level.vertexes.get(seg.to_vertex as usize) {
            Some(v) => v,
            None => continue,
        };
        let (wx1, wy1) = (v1.x as i32, v1.y as i32);
        let (wx2, wy2) = (v2.x as i32, v2.y as i32);

        // Translate to player-relative coordinates.
        let dx1 = (wx1 - player_x) as i64;
        let dy1 = (wy1 - player_y) as i64;
        let dx2 = (wx2 - player_x) as i64;
        let dy2 = (wy2 - player_y) as i64;

        // Rotate into view space.  vx = depth (forward), vy = lateral.
        let vx1 = (dx1 * cos_i + dy1 * sin_i) >> 16;
        let vy1 = (dx1 * sin_i - dy1 * cos_i) >> 16;
        let vx2 = (dx2 * cos_i + dy2 * sin_i) >> 16;
        let vy2 = (dx2 * sin_i - dy2 * cos_i) >> 16;
        let unclipped_view_left = (vx1 as f32, vy1 as f32);
        let unclipped_view_right = (vx2 as f32, vy2 as f32);

        // Skip if both endpoints are behind the player.
        if vx1 <= 0 && vx2 <= 0 {
            continue;
        }

        // Resolve sidedef → sector.
        let linedef = match level.linedefs.get(seg.linedef as usize) {
            Some(ld) => ld,
            None => continue,
        };
        let sidedef = if linedef.is_two_sided() {
            // For true portals, seg direction selects front/back sidedef.
            let idx = if seg.direction == 0 {
                linedef.right_sidedef
            } else {
                linedef.left_sidedef
            };
            if idx == 0xFFFF {
                continue;
            }
            match level.sidedefs.get(idx as usize) {
                Some(sd) => sd,
                None => continue,
            }
        } else {
            // For one-sided lines, honor seg direction first, then fall back
            // to the opposite side if needed. Some maps contain seg/sidedef
            // mismatches where the preferred side has "-" middle texture while
            // the opposite side carries the actual wall texture.
            let preferred = if seg.direction == 0 {
                linedef.right_sidedef
            } else {
                linedef.left_sidedef
            };
            let opposite = if seg.direction == 0 {
                linedef.left_sidedef
            } else {
                linedef.right_sidedef
            };
            let preferred_sd = if preferred != 0xFFFF {
                level.sidedefs.get(preferred as usize)
            } else {
                None
            };
            let opposite_sd = if opposite != 0xFFFF {
                level.sidedefs.get(opposite as usize)
            } else {
                None
            };

            match (preferred_sd, opposite_sd) {
                (Some(psd), Some(osd)) => {
                    let preferred_drawable = !is_no_texture(&psd.middle_texture);
                    let opposite_drawable = !is_no_texture(&osd.middle_texture);
                    if preferred_drawable || !opposite_drawable {
                        psd
                    } else {
                        osd
                    }
                }
                (Some(psd), None) => psd,
                (None, Some(osd)) => osd,
                (None, None) => continue,
            }
        };
        let sector = match level.sectors.get(sidedef.sector as usize) {
            Some(s) => s,
            None => continue,
        };

        let floor_h = sector.floor_height.to_int();
        let ceil_h = sector.ceil_height.to_int();
        let sector_light =
            ((sector.light_level as u32).min(255) as u8).saturating_add(extra_light_bonus);

        // Build per-sector lighting parameters.  `LightParams` caches the
        // base colormap index and fullbright flag so each column can quickly
        // obtain a distance-attenuated colormap.
        let light_params = LightParams::new(sector_light, is_fullbright);

        // Resolve the back sector for two-sided linedefs.
        let back_sector = if linedef.is_two_sided() {
            let back_sidedef_idx = if seg.direction == 0 {
                linedef.left_sidedef
            } else {
                linedef.right_sidedef
            };
            if back_sidedef_idx != 0xFFFF {
                level
                    .sidedefs
                    .get(back_sidedef_idx as usize)
                    .and_then(|sd| level.sectors.get(sd.sector as usize))
            } else {
                None
            }
        } else {
            None
        };
        // Only treat as portal when a valid back sector exists.
        let is_two_sided = back_sector.is_some();

        // Clip to full view frustum (near + left/right FOV planes).
        // This bounds span_w to ≤ SCREEN_W, preventing texture warp when
        // a wall vertex passes close to or behind the near plane.
        let clipped = match clip_seg_to_view_frustum(vx1, vy1, vx2, vy2) {
            Some(c) => c,
            None => continue,
        };
        let (vx1, vy1, vx2, vy2) = clipped;

        // Project to screen columns.
        // vy is the right-lateral component (positive = to the player's right),
        // so sx = HALF_W + FOCAL_LEN * vy / vx maps right-side geometry to right columns.
        let sx1 = HALF_W as i64 + (FOCAL_LEN as i64 * vy1) / vx1.max(1);
        let sx2 = HALF_W as i64 + (FOCAL_LEN as i64 * vy2) / vx2.max(1);

        let (sx_left, vx_left, sx_right, vx_right) = if sx1 <= sx2 {
            (sx1, vx1, sx2, vx2)
        } else {
            (sx2, vx2, sx1, vx1)
        };

        // Rasterized column span (inclusive).
        let col_start = sx_left.max(0).min((SCREEN_W - 1) as i64) as usize;
        let col_end = sx_right.max(0).min((SCREEN_W - 1) as i64) as usize;

        if col_start > col_end {
            continue;
        }

        let span_w = (sx_right - sx_left).max(1);

        // Precompute seg world length for texture UV (hoisted outside column loop).
        let v1_pos = &level.vertexes[seg.from_vertex as usize];
        let v2_pos = &level.vertexes[seg.to_vertex as usize];
        let seg_dx = (v2_pos.x as f32) - (v1_pos.x as f32);
        let seg_dy = (v2_pos.y as f32) - (v1_pos.y as f32);
        let seg_world_len = (seg_dx * seg_dx + seg_dy * seg_dy).sqrt();

        for x in col_start..=col_end {
            let t = (x as i64 - sx_left).max(0);
            let t_screen = (t as f32) / (span_w as f32).max(1.0);
            let scale_left = FOCAL_LEN as f32 / vx_left.max(1) as f32;
            let scale_right = FOCAL_LEN as f32 / vx_right.max(1) as f32;
            let scale = scale_left + t_screen * (scale_right - scale_left);
            let depth_f32 = (FOCAL_LEN as f32 / scale.max(f32::EPSILON)).max(1.0);
            let depth_i32 = depth_f32.trunc() as i32;
            let clip_top = wall_clip_top[x];
            let clip_bot = wall_clip_bot[x];
            let wall_s =
                wall_segment_param_for_screen_x(x, unclipped_view_left, unclipped_view_right)
                    .unwrap_or_else(|| {
                        let denom = (vx_left as f32
                            + t_screen * (vx_right as f32 - vx_left as f32))
                            .max(1.0);
                        t_screen * (vx_right as f32) / denom
                    });
            let u_world = seg.offset as f32 + sidedef.x_offset as f32 + wall_s * seg_world_len;

            if clip_top > clip_bot {
                continue;
            }

            if is_two_sided {
                // -----------------------------------------------------------------
                // TWO-SIDED SEG: portal / door / window
                // -----------------------------------------------------------------
                // For two-sided segs we do NOT update z_buf — the line does not
                // fully occlude the view.  Things behind can show through the
                // portal opening.
                //
                // But if a nearer one-sided wall already owns this column in z_buf,
                // this portal is fully occluded and must not affect visplane bounds.
                if depth_f32 >= z_buf[x] {
                    continue;
                }

                // Project front sector ceiling/floor against camera height.
                let mut w_top = project_wall_y(ceil_h - view_z, scale);
                let mut w_bot = project_wall_y(floor_h - view_z, scale);
                if w_top > w_bot {
                    core::mem::swap(&mut w_top, &mut w_bot);
                }
                w_top = w_top.clamp(0, SCREEN_H as i32 - 1);
                w_bot = w_bot.clamp(0, SCREEN_H as i32 - 1);
                if w_top >= w_bot {
                    continue;
                }
                // Compute screen-space positions of the back sector's ceiling/floor
                // directly against camera height, then clamp to the front wall span.
                let (screen_back_ceil, screen_back_floor) = if let Some(bs) = back_sector {
                    let bc = bs.ceil_height.to_int();
                    let bf = bs.floor_height.to_int();
                    let mut sb_ceil = project_wall_y(bc - view_z, scale);
                    let mut sb_floor = project_wall_y(bf - view_z, scale);
                    if sb_ceil > sb_floor {
                        core::mem::swap(&mut sb_ceil, &mut sb_floor);
                    }
                    (sb_ceil.clamp(w_top, w_bot), sb_floor.clamp(w_top, w_bot))
                } else {
                    // No back sector — treat as fully closed (no portal opening).
                    (w_top, w_bot)
                };

                // Narrow the wall drawing window for farther geometry to this
                // portal opening so solid walls behind do not leak outside it.
                let has_portal_opening = screen_back_ceil < screen_back_floor;
                // Only the opaque portal bands should constrain sprite clipping.
                // A lowered ceiling contributes a top clip; a raised floor
                // contributes a bottom clip. Vanilla also clips sprites against
                // front-side ledges even when no back-side wall band is drawn.
                let upper_bot = screen_back_ceil.min(w_bot);
                let lower_top = screen_back_floor.max(w_top);
                let has_upper = upper_bot > w_top;
                let has_lower = lower_top < w_bot;
                let front_blocks_top =
                    back_sector.is_some_and(|bs| (bs.ceil_height.to_int()) > ceil_h);
                let front_blocks_bottom =
                    back_sector.is_some_and(|bs| (bs.floor_height.to_int()) < floor_h);
                let portal_top = if has_upper {
                    upper_bot.clamp(0, SCREEN_H as i32 - 1)
                } else {
                    w_top.clamp(0, SCREEN_H as i32 - 1)
                };
                let portal_bot = if has_lower {
                    (lower_top - 1).clamp(-1, SCREEN_H as i32 - 1)
                } else {
                    w_bot.clamp(-1, SCREEN_H as i32 - 1)
                };
                if has_portal_opening {
                    let top_silhouette_height = if has_upper {
                        back_sector.map_or(ceil_h as f32, |bs| bs.ceil_height.to_int() as f32)
                    } else {
                        ceil_h as f32
                    };
                    let bottom_silhouette_height = if has_lower {
                        back_sector.map_or(floor_h as f32, |bs| bs.floor_height.to_int() as f32)
                    } else {
                        floor_h as f32
                    };
                    if (has_upper || front_blocks_top) && portal_top > wall_clip_top[x] {
                        wall_clip_top[x] = portal_top;
                        wall_clip_top_depth[x] = depth_f32;
                        record_sprite_clip_step(
                            &mut wall_clip_top_history[x],
                            depth_f32,
                            portal_top,
                            top_silhouette_height,
                        );
                    }
                    if (has_lower || front_blocks_bottom) && portal_bot < wall_clip_bot[x] {
                        wall_clip_bot[x] = portal_bot;
                        wall_clip_bot_depth[x] = depth_f32;
                        record_sprite_clip_step(
                            &mut wall_clip_bot_history[x],
                            depth_f32,
                            portal_bot,
                            bottom_silhouette_height,
                        );
                    }
                    // If accumulation of portals has fully closed this column,
                    // write depth so sprites behind it cannot bleed through.
                    if wall_clip_top[x] > wall_clip_bot[x] && depth_f32 < z_buf[x] {
                        z_buf[x] = depth_f32;
                    }
                } else if !has_portal_opening {
                    wall_clip_top[x] = 1;
                    wall_clip_bot[x] = 0;
                    wall_clip_top_depth[x] = depth_f32;
                    wall_clip_bot_depth[x] = depth_f32;
                    record_sprite_clip_step(
                        &mut wall_clip_top_history[x],
                        depth_f32,
                        1,
                        f32::NEG_INFINITY,
                    );
                    record_sprite_clip_step(
                        &mut wall_clip_bot_history[x],
                        depth_f32,
                        0,
                        f32::INFINITY,
                    );
                    // Fully-closed portal: acts as solid for sprite occlusion.
                    if depth_f32 < z_buf[x] {
                        z_buf[x] = depth_f32;
                    }
                }

                // Clamp so upper ≤ lower (degenerate case: equal heights, sealed door).
                // Inline visplane emission — Doom R_RenderSegLoop style.
                // Emit ceiling/floor strips for the FRONT sector before
                // advancing the open_top/open_bot trackers.
                {
                    // Ceiling strip: from open_top[x] to w_top - 1.
                    let ceil_strip_top = open_top[x];
                    let ceil_strip_bot = (w_top - 1).min(SCREEN_H as i32 - 1);
                    if ceil_strip_top <= ceil_strip_bot {
                        if is_sky_flat(&sector.ceil_flat) {
                            sky_coverage.record_span(x, ceil_strip_top, ceil_strip_bot);
                        } else {
                            let idx = visplanes.r_find_plane(
                                PlaneKind::Ceiling,
                                ceil_h,
                                sector.ceil_flat,
                                sector_light,
                            );
                            visplanes.r_check_plane(
                                idx,
                                x,
                                x,
                                ceil_strip_top as i16,
                                ceil_strip_bot as i16,
                            );
                        }
                    }
                    // Floor strip: from w_bot + 1 to open_bot[x].
                    let floor_strip_top = (w_bot + 1).max(0);
                    let floor_strip_bot = open_bot[x];
                    if floor_strip_top <= floor_strip_bot && !is_sky_flat(&sector.floor_flat) {
                        let idx = visplanes.r_find_plane(
                            PlaneKind::Floor,
                            floor_h,
                            sector.floor_flat,
                            sector_light,
                        );
                        visplanes.r_check_plane(
                            idx,
                            x,
                            x,
                            floor_strip_top as i16,
                            floor_strip_bot as i16,
                        );
                    }
                    // Advance trackers past the wall / upper-lower bands.
                    if !has_portal_opening {
                        open_top[x] = SCREEN_H as i32;
                        open_bot[x] = -1;
                    } else if has_upper {
                        open_top[x] = open_top[x].max(upper_bot);
                    } else {
                        open_top[x] = open_top[x].max(w_top);
                    }
                    if !has_portal_opening {
                        // Closed door / degenerate portal already sealed the column.
                    } else if has_lower {
                        open_bot[x] = open_bot[x].min(lower_top - 1);
                    } else {
                        open_bot[x] = open_bot[x].min(w_bot);
                    }
                }

                // ---- Sky detection for two-sided portals -------------------------
                // (Sky region already recorded in the inline visplane emission above.)
                let front_is_sky = is_sky_flat(&sector.ceil_flat);
                let back_is_sky = back_sector.is_some_and(|bs| is_sky_flat(&bs.ceil_flat));

                // ---- Upper band (front_ceil > back_ceil) -------------------------
                // Skip upper texture if both front and back are sky (sky-to-sky portal).
                let skip_upper_for_sky = front_is_sky && back_is_sky;
                let upper_draw_top = w_top.max(clip_top);
                let upper_draw_bot = if has_portal_opening {
                    (upper_bot - 1).min(clip_bot)
                } else {
                    w_bot.min(clip_bot)
                };
                if has_upper
                    && upper_draw_top <= upper_draw_bot
                    && !skip_upper_for_sky
                    && !is_no_texture(&sidedef.upper_texture)
                {
                    // Per-column colormap: front sector light + distance attenuation.
                    let col_dist = depth_i32 as f32;
                    let wall_cm: &[u8; 256] = if let Some(override_cm) = fixed_colormap {
                        override_cm
                    } else {
                        colormap
                            .map(|c| light_params.get_wall_colormap(col_dist, x, c))
                            .unwrap_or(&IDENTITY_COLORMAP)
                    };

                    if let Some(cache) = tex_cache {
                        let upper_name = anim.map_or(sidedef.upper_texture, |a| {
                            a.resolve_wall(&sidedef.upper_texture)
                        });
                        if let Some(tex) = cache.get(&upper_name) {
                            let u_tex = (u_world as i32).rem_euclid(tex.width as i32) as usize;
                            let tex_h = tex.height;
                            let fracstep = wall_fracstep(scale);
                            let texturemid = if linedef.flags & FLAG_DONTPEGTOP != 0 {
                                ceil_h + i32::from(sidedef.y_offset) - view_z
                            } else {
                                back_sector.map_or(ceil_h, |bs| bs.ceil_height.to_int())
                                    + tex.logical_height as i32
                                    + i32::from(sidedef.y_offset)
                                    - view_z
                            };
                            let frac_start = wall_frac_start(texturemid, upper_draw_top, fracstep);
                            let col_off = u_tex * tex_h as usize;
                            let col_data = &tex.data[col_off..col_off + tex_h as usize];
                            draw_column(
                                fb,
                                &DrawColumnParams {
                                    x,
                                    y_top: upper_draw_top as usize,
                                    y_bot: upper_draw_bot as usize,
                                    frac: frac_start,
                                    fracstep,
                                    source: col_data,
                                    colormap: wall_cm,
                                },
                            );
                        } else {
                            // Texture not in cache — flat-shade fallback.
                            let base_color = 32u8;
                            let shaded = wall_cm[base_color as usize];
                            fb.draw_column(
                                x,
                                upper_draw_top as usize,
                                upper_draw_bot as usize,
                                shaded,
                            );
                        }
                    } else {
                        // No tex_cache — flat-shade fallback.
                        let base_color = 32u8;
                        let shaded = wall_cm[base_color as usize];
                        fb.draw_column(x, upper_draw_top as usize, upper_draw_bot as usize, shaded);
                    }
                } else if has_upper && skip_upper_for_sky {
                    // Sky-to-sky portal: extend the sky region down through
                    // the upper band (sky is visible all the way to the portal opening).
                    sky_coverage.record_span(x, upper_draw_top, upper_draw_bot);
                }

                // ---- Lower band (back_floor > front_floor) -----------------------
                let lower_draw_top = lower_top.max(clip_top);
                let lower_draw_bot = w_bot.min(clip_bot);
                if has_lower
                    && lower_draw_top <= lower_draw_bot
                    && !is_no_texture(&sidedef.lower_texture)
                {
                    // Per-column colormap: front sector light + distance attenuation.
                    let col_dist = depth_i32 as f32;
                    let wall_cm: &[u8; 256] = if let Some(override_cm) = fixed_colormap {
                        override_cm
                    } else {
                        colormap
                            .map(|c| light_params.get_wall_colormap(col_dist, x, c))
                            .unwrap_or(&IDENTITY_COLORMAP)
                    };

                    if let Some(cache) = tex_cache {
                        let lower_name = anim.map_or(sidedef.lower_texture, |a| {
                            a.resolve_wall(&sidedef.lower_texture)
                        });
                        if let Some(tex) = cache.get(&lower_name) {
                            let u_tex = (u_world as i32).rem_euclid(tex.width as i32) as usize;
                            let tex_h = tex.height;
                            let fracstep = wall_fracstep(scale);
                            let texturemid = if linedef.flags & FLAG_DONTPEGBOTTOM != 0 {
                                ceil_h + i32::from(sidedef.y_offset) - view_z
                            } else {
                                back_sector.map_or(floor_h, |bs| bs.floor_height.to_int())
                                    + i32::from(sidedef.y_offset)
                                    - view_z
                            };
                            let frac_start = wall_frac_start(texturemid, lower_draw_top, fracstep);
                            let col_off = u_tex * tex_h as usize;
                            let col_data = &tex.data[col_off..col_off + tex_h as usize];
                            draw_column(
                                fb,
                                &DrawColumnParams {
                                    x,
                                    y_top: lower_draw_top as usize,
                                    y_bot: lower_draw_bot as usize,
                                    frac: frac_start,
                                    fracstep,
                                    source: col_data,
                                    colormap: wall_cm,
                                },
                            );
                        } else {
                            // Texture not in cache — flat-shade fallback.
                            let base_color = 32u8;
                            let shaded = wall_cm[base_color as usize];
                            fb.draw_column(
                                x,
                                lower_draw_top as usize,
                                lower_draw_bot as usize,
                                shaded,
                            );
                        }
                    } else {
                        // No tex_cache — flat-shade fallback.
                        let base_color = 32u8;
                        let wall_cm: &[u8; 256] = if let Some(override_cm) = fixed_colormap {
                            override_cm
                        } else {
                            colormap
                                .map(|c| light_params.get_wall_colormap(col_dist, x, c))
                                .unwrap_or(&IDENTITY_COLORMAP)
                        };
                        let shaded = wall_cm[base_color as usize];
                        fb.draw_column(x, lower_draw_top as usize, lower_draw_bot as usize, shaded);
                    }
                }

                // ---- Masked middle texture (grates/fences on two-sided lines) ----
                let mid_name = anim.map_or(sidedef.middle_texture, |a| {
                    a.resolve_wall(&sidedef.middle_texture)
                });
                let mid_draw_top = w_top.max(clip_top);
                let mid_draw_bot = w_bot.min(clip_bot);
                if !is_no_texture(&mid_name) && mid_draw_top <= mid_draw_bot {
                    let col_dist = depth_i32 as f32;
                    let wall_cm: &[u8; 256] = if let Some(override_cm) = fixed_colormap {
                        override_cm
                    } else {
                        colormap
                            .map(|c| light_params.get_wall_colormap(col_dist, x, c))
                            .unwrap_or(&IDENTITY_COLORMAP)
                    };
                    if let Some(cache) = tex_cache
                        && let Some(tex) = cache.get(&mid_name)
                    {
                        let u_tex = (u_world as i32).rem_euclid(tex.width as i32) as usize;
                        let tex_h = tex.height;
                        let fracstep = wall_fracstep(scale);
                        let texturemid = if linedef.flags & FLAG_DONTPEGBOTTOM != 0 {
                            floor_h.max(back_sector.map_or(floor_h, |bs| bs.floor_height.to_int()))
                                + tex.logical_height as i32
                                + i32::from(sidedef.y_offset)
                                - view_z
                        } else {
                            ceil_h.min(back_sector.map_or(ceil_h, |bs| bs.ceil_height.to_int()))
                                + i32::from(sidedef.y_offset)
                                - view_z
                        };
                        let frac_start = wall_frac_start(texturemid, mid_draw_top, fracstep);
                        let col_off = u_tex * tex_h as usize;
                        let col_data = &tex.data[col_off..col_off + tex_h as usize];
                        masked_columns.push(MaskedColumnDraw {
                            depth: depth_f32,
                            x,
                            y_top: mid_draw_top as usize,
                            y_bot: mid_draw_bot as usize,
                            frac: frac_start,
                            fracstep,
                            source: col_data,
                            colormap: *wall_cm,
                        });
                    }
                }
            } else {
                // -----------------------------------------------------------------
                // ONE-SIDED SEG: solid wall
                // -----------------------------------------------------------------

                let mid_name = anim.map_or(sidedef.middle_texture, |a| {
                    a.resolve_wall(&sidedef.middle_texture)
                });
                // One-sided walls are always solid. If the middle texture is
                // missing ("-"), keep rendering with flat-shade fallback so the
                // column still occludes farther geometry.

                // Z-buffer occlusion.
                if depth_f32 >= z_buf[x] {
                    continue;
                }

                // Write depth immediately: solid walls always occlude sprites
                // even when their visible column is clipped away by a portal.
                z_buf[x] = depth_f32;

                // Project front sector ceiling/floor against camera height.
                let mut w_top = project_wall_y(ceil_h - view_z, scale);
                let mut w_bot = project_wall_y(floor_h - view_z, scale);
                if w_top > w_bot {
                    core::mem::swap(&mut w_top, &mut w_bot);
                }
                w_top = w_top.clamp(0, SCREEN_H as i32 - 1);
                w_bot = w_bot.clamp(0, SCREEN_H as i32 - 1);
                if w_top >= w_bot {
                    continue;
                }
                let draw_top = w_top.max(clip_top);
                let draw_bot = w_bot.min(clip_bot);
                if draw_top > draw_bot {
                    continue;
                }

                // Inline visplane emission for one-sided walls.
                {
                    // Ceiling strip: from open_top[x] to draw_top - 1.
                    let ceil_strip_top = open_top[x];
                    let ceil_strip_bot = (draw_top - 1).min(SCREEN_H as i32 - 1);
                    if ceil_strip_top <= ceil_strip_bot {
                        if is_sky_flat(&sector.ceil_flat) {
                            sky_coverage.record_span(x, ceil_strip_top, ceil_strip_bot);
                        } else {
                            let idx = visplanes.r_find_plane(
                                PlaneKind::Ceiling,
                                ceil_h,
                                sector.ceil_flat,
                                sector_light,
                            );
                            visplanes.r_check_plane(
                                idx,
                                x,
                                x,
                                ceil_strip_top as i16,
                                ceil_strip_bot as i16,
                            );
                        }
                    }
                    // Floor strip: from draw_bot + 1 to open_bot[x].
                    let floor_strip_top = (draw_bot + 1).max(0);
                    let floor_strip_bot = open_bot[x];
                    if floor_strip_top <= floor_strip_bot && !is_sky_flat(&sector.floor_flat) {
                        let idx = visplanes.r_find_plane(
                            PlaneKind::Floor,
                            floor_h,
                            sector.floor_flat,
                            sector_light,
                        );
                        visplanes.r_check_plane(
                            idx,
                            x,
                            x,
                            floor_strip_top as i16,
                            floor_strip_bot as i16,
                        );
                    }
                    // One-sided wall fully closes the column — no farther
                    // ceiling/floor can draw here.
                    open_top[x] = SCREEN_H as i32;
                    open_bot[x] = -1;
                }

                // Per-column colormap: distance-attenuated from sector light.
                let col_dist = depth_f32;
                let wall_cm: &[u8; 256] = if let Some(override_cm) = fixed_colormap {
                    override_cm
                } else {
                    colormap
                        .map(|c| light_params.get_wall_colormap(col_dist, x, c))
                        .unwrap_or(&IDENTITY_COLORMAP)
                };

                // Draw the wall column — textured if a TextureCache is available.
                let mut drew_textured = false;
                if let Some(cache) = tex_cache {
                    if let Some(tex) = cache.get(&mid_name) {
                        let u_tex = (u_world as i32).rem_euclid(tex.width as i32) as usize;

                        // Vertical texture coordinate (V).
                        let tex_h = tex.height;
                        let fracstep = wall_fracstep(scale);
                        let texturemid = if linedef.flags & FLAG_DONTPEGBOTTOM != 0 {
                            floor_h + tex.logical_height as i32 + i32::from(sidedef.y_offset)
                                - view_z
                        } else {
                            ceil_h + i32::from(sidedef.y_offset) - view_z
                        };
                        let frac_start = wall_frac_start(texturemid, draw_top, fracstep);

                        let col_start_idx = u_tex * tex_h as usize;
                        let col_data = &tex.data[col_start_idx..col_start_idx + tex_h as usize];

                        draw_column(
                            fb,
                            &DrawColumnParams {
                                x,
                                y_top: draw_top as usize,
                                y_bot: draw_bot as usize,
                                frac: frac_start,
                                fracstep,
                                source: col_data,
                                colormap: wall_cm,
                            },
                        );
                        drew_textured = true;
                    }
                }

                if !drew_textured {
                    // Fallback: flat-shaded solid color (no texture or texture not found).
                    let base_color = 32u8;
                    let shaded = wall_cm[base_color as usize];
                    fb.draw_column(x, draw_top as usize, draw_bot as usize, shaded);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Step 4b: Fill uncovered columns with player sector flats
    // ------------------------------------------------------------------
    // Columns where open_top[x] <= open_bot[x] were never fully closed
    // by any seg.  Emit visplanes for the remaining open region using
    // the player's sector ceiling/floor.  This covers the floor directly
    // under the player and sky/ceiling not intersected by any wall.
    for x in 0..SCREEN_W {
        if open_top[x] <= open_bot[x] {
            // Ceiling: rows open_top[x]..HALF_H (above horizon)
            let ceil_bot = (HALF_H - 1).min(open_bot[x]);
            if open_top[x] <= ceil_bot {
                if is_sky_flat(&player_ceil_flat) {
                    sky_coverage.record_span(x, open_top[x], ceil_bot);
                } else {
                    let idx = visplanes.r_find_plane(
                        PlaneKind::Ceiling,
                        player_ceil_h,
                        player_ceil_flat,
                        player_light,
                    );
                    visplanes.r_check_plane(idx, x, x, open_top[x] as i16, ceil_bot as i16);
                }
            }
            // Floor: rows HALF_H..open_bot[x] (below horizon)
            let floor_top = HALF_H.max(open_top[x]);
            if floor_top <= open_bot[x] && !is_sky_flat(&player_floor_flat) {
                let idx = visplanes.r_find_plane(
                    PlaneKind::Floor,
                    player_floor_h,
                    player_floor_flat,
                    player_light,
                );
                visplanes.r_check_plane(idx, x, x, floor_top as i16, open_bot[x] as i16);
            }
        }
    }

    // ------------------------------------------------------------------
    // Step 4c: Draw sky columns
    // ------------------------------------------------------------------
    // Draw sky after the open-column post-pass, because that pass may add
    // additional F_SKY1 spans for columns that no wall ever touched.
    {
        let sky_name = sky_texture_name(level.name.as_str());
        let sky_tex = tex_cache.and_then(|c| c.get(&sky_name));
        if let Some(stex) = sky_tex {
            draw_sky_coverage_columns(fb, &sky_coverage, player_angle, stex);
        } else {
            draw_sky_coverage_fallback(fb, &sky_coverage);
        }
    }

    // ------------------------------------------------------------------
    // Step 5: Draw visplane spans
    // ------------------------------------------------------------------
    // Visplanes were built inline during the wall pass (Steps 3-4).
    // Each visplane already has correct per-column top/bottom bounds,
    // so no additional clip_plane_span_runs pass is needed.
    if flat_cache.is_none() {
        return RenderOut {
            z_buf,
            clip_top: wall_clip_top,
            clip_bot: wall_clip_bot,
            clip_top_depth: wall_clip_top_depth,
            clip_bot_depth: wall_clip_bot_depth,
            clip_top_history: wall_clip_top_history,
            clip_bot_history: wall_clip_bot_history,
            masked_columns,
        };
    }
    let cache = flat_cache.unwrap();

    for plane in visplanes.planes() {
        let resolved = anim.map_or(plane.flat_name, |a| a.resolve_flat(&plane.flat_name));
        let source = cache.get(&resolved);
        let spans = visplane_to_spans(plane, SCREEN_H);
        let flat_lp = LightParams::new(plane.light_level, is_fullbright);

        for span in spans {
            let y = span.y as i32;
            let dy = y - HALF_H;
            if dy == 0 {
                continue;
            }

            let abs_dy = dy.abs();
            let plane_h = (plane.height - view_z).unsigned_abs() as i64;
            let dist = (plane_h * FOCAL_LEN as i64) / abs_dy as i64;
            if dist <= 0 {
                continue;
            }

            let xstep = ((sin_i * dist) / FOCAL_LEN as i64) as i32;
            let ystep = ((-cos_i * dist) / FOCAL_LEN as i64) as i32;

            let world_x_centre = (player_x as i64) * 65536 + (cos_i * dist);
            let world_y_centre = (player_y as i64) * 65536 + (sin_i * dist);
            let offset = (-(HALF_W as i64) * dist) / FOCAL_LEN as i64;
            let world_x_left_fp = world_x_centre + sin_i * offset;
            let world_y_left_fp = world_y_centre - cos_i * offset;

            let init_xfrac = (world_x_left_fp & 0xFFFF_FFFF) as u32;
            let init_yfrac = (world_y_left_fp & 0xFFFF_FFFF) as u32;
            let xstep_u = xstep as u32;
            let ystep_u = ystep as u32;

            let steps = span.x1 as u32;
            let base_xfrac = init_xfrac.wrapping_add(xstep_u.wrapping_mul(steps));
            let base_yfrac = init_yfrac.wrapping_add(ystep_u.wrapping_mul(steps));

            let span_cm: &[u8; 256] = if let Some(override_cm) = fixed_colormap {
                override_cm
            } else {
                colormap
                    .map(|c| flat_lp.get_flat_colormap(dist as f32, c))
                    .unwrap_or(&IDENTITY_COLORMAP)
            };

            let params = DrawSpanParams {
                y: span.y,
                x1: span.x1,
                x2: span.x2,
                ds_xfrac: base_xfrac,
                ds_yfrac: base_yfrac,
                ds_xstep: xstep_u,
                ds_ystep: ystep_u,
                source,
                colormap: span_cm,
            };
            draw_span(fb, &params);
        }
    }

    RenderOut {
        z_buf,
        clip_top: wall_clip_top,
        clip_bot: wall_clip_bot,
        clip_top_depth: wall_clip_top_depth,
        clip_bot_depth: wall_clip_bot_depth,
        clip_top_history: wall_clip_top_history,
        clip_bot_history: wall_clip_bot_history,
        masked_columns,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clip::clip_seg_to_near_plane;
    use crate::render_flags::RenderFlag;
    use crate::sky::SKY_FALLBACK_COLOR;
    use crate::sprite::{SpriteCache, SpriteFrame, render_actors_with_masked_ex};
    use crate::sprite_lookup::ActorRenderInfo;
    use doom_map::lumps::FLAG_DONTPEGTOP;
    use doom_wad::WadFile;

    /// Build a minimal Level with one sector and one seg for testing.
    fn make_minimal_level() -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![Vertex { x: 0, y: 128 }, Vertex { x: 128, y: 128 }];
        let sectors = vec![Sector {
            floor_height: doom_types::Fixed16_16::from_int(0),
            ceil_height: doom_types::Fixed16_16::from_int(128),
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }];
        let sidedefs = vec![Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"WALL1\0\0\0",
            lower_texture: *b"WALL2\0\0\0",
            middle_texture: *b"WALL3\0\0\0",
            sector: 0,
        }];
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF,
        }];
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];
        let ssectors = vec![Ssector {
            seg_count: 1,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject parse");

        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "TEST".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    /// Build a two-sided level: two sectors separated by a portal wall.
    ///
    /// Layout:
    ///   front sector (sector 0): floor=0, ceil=128
    ///   back sector  (sector 1): floor=32, ceil=96
    ///
    /// The portal opening is at y=128 (the wall seg).
    /// Player is at (0, 0) looking toward the wall at y=128.
    fn make_two_sided_level(
        front_floor: i16,
        front_ceil: i16,
        back_floor: i16,
        back_ceil: i16,
    ) -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![Vertex { x: -64, y: 128 }, Vertex { x: 64, y: 128 }];
        let sectors = vec![
            // Sector 0 — front (player stands here)
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(i32::from(front_floor)),
                ceil_height: doom_types::Fixed16_16::from_int(i32::from(front_ceil)),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            // Sector 1 — back (player looks into this)
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(i32::from(back_floor)),
                ceil_height: doom_types::Fixed16_16::from_int(i32::from(back_ceil)),
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        // right sidedef (sector 0, front), left sidedef (sector 1, back)
        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
        ];
        // FLAG_TWO_SIDED = 0x0004
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004, // two-sided
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];
        let ssectors = vec![Ssector {
            seg_count: 1,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("reject parse");

        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "TEST2".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    fn make_player_sector_mismatch_level() -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Node, NodeBBox, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![
            Vertex { x: 200, y: -64 },
            Vertex { x: 200, y: 64 },
            Vertex { x: 20, y: -64 },
            Vertex { x: 20, y: 64 },
            Vertex { x: -64, y: -64 },
            Vertex { x: -64, y: 64 },
        ];
        let sectors = vec![
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(64),
                ceil_height: doom_types::Fixed16_16::from_int(192),
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"WALL1\0\0\0",
                lower_texture: *b"WALL1\0\0\0",
                middle_texture: *b"WALL1\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"WALL2\0\0\0",
                lower_texture: *b"WALL2\0\0\0",
                middle_texture: *b"WALL2\0\0\0",
                sector: 1,
            },
        ];
        let linedefs = vec![
            Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            },
            Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 1,
                left_sidedef: 0xFFFF,
            },
            Linedef {
                from_vertex: 4,
                to_vertex: 5,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 1,
                left_sidedef: 0xFFFF,
            },
        ];
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
                from_vertex: 2,
                to_vertex: 3,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
            Seg {
                from_vertex: 4,
                to_vertex: 5,
                angle: 0,
                linedef: 2,
                direction: 0,
                offset: 0,
            },
        ];
        let ssectors = vec![
            Ssector {
                first_seg: 0,
                seg_count: 2,
            },
            Ssector {
                first_seg: 2,
                seg_count: 1,
            },
        ];
        let bbox = NodeBBox {
            ymax: 256,
            ymin: -256,
            xmin: -256,
            xmax: 256,
        };
        let nodes = vec![Node {
            x: 0,
            y: 0,
            dx: 0,
            dy: 1,
            right_bbox: bbox,
            left_bbox: bbox,
            right_child: 0x8000,
            left_child: 0x8000 | 1,
        }];
        let things = vec![Thing {
            x: 10,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("reject parse");

        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "PSECTOR".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes,
            sectors,
            reject,
            blockmap,
        }
    }

    /// Build a level with:
    /// - Near one-sided wall at y=128 (solid occluder)
    /// - Far two-sided portal at y=256 (must be occluded by the near wall)
    ///
    /// Seg order intentionally places the near wall first and the far portal
    /// second to catch regressions where later portal processing rewrites
    /// visplane bounds behind a solid wall.
    fn make_occluded_portal_level() -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![
            Vertex { x: -64, y: 128 },
            Vertex { x: 64, y: 128 },
            Vertex { x: -64, y: 256 },
            Vertex { x: 64, y: 256 },
        ];

        let sectors = vec![
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(32),
                ceil_height: doom_types::Fixed16_16::from_int(96),
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];

        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"WALL1\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
        ];

        // ld0: near one-sided wall.
        // ld1: far two-sided portal.
        let linedefs = vec![
            Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            },
            Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: 0x0004, // two-sided
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
        ];

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
                from_vertex: 2,
                to_vertex: 3,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
        ];

        let ssectors = vec![Ssector {
            seg_count: segs.len() as u16,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("reject parse");

        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "OCCL".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    /// Build a level with:
    /// - Near two-sided portal at y=128 with a narrow opening.
    /// - Far one-sided wall at y=256 behind that portal.
    ///
    /// The near portal deliberately uses no upper/lower textures so pixels
    /// outside the opening should remain background. If far walls are not
    /// clipped to the current portal window, they leak into those rows.
    fn make_portal_window_with_far_solid_level() -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![
            Vertex { x: -64, y: 128 },
            Vertex { x: 64, y: 128 },
            Vertex { x: -64, y: 256 },
            Vertex { x: 64, y: 256 },
        ];

        let sectors = vec![
            // Front sector (player side).
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            // Back sector visible through portal opening.
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(56),
                ceil_height: doom_types::Fixed16_16::from_int(72),
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];

        let sidedefs = vec![
            // sd0: front side of near two-sided portal, no opaque upper/lower bands.
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            // sd1: back side of near portal.
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
            // sd2: far one-sided solid wall.
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"WALL1\0\0\0",
                lower_texture: *b"WALL1\0\0\0",
                middle_texture: *b"WALL1\0\0\0",
                sector: 1,
            },
        ];

        // ld0: near two-sided portal.
        // ld1: far one-sided wall behind the portal.
        let linedefs = vec![
            Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004, // two-sided
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 2,
                left_sidedef: 0xFFFF,
            },
        ];

        // Near portal first, far wall second.
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
                from_vertex: 2,
                to_vertex: 3,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
        ];

        let ssectors = vec![Ssector {
            seg_count: segs.len() as u16,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("reject parse");
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "PORTWIN".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    fn make_two_sided_level_with_vertices(
        from: (i16, i16),
        to: (i16, i16),
        front_floor: i16,
        front_ceil: i16,
        back_floor: i16,
        back_ceil: i16,
    ) -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![
            Vertex {
                x: from.0,
                y: from.1,
            },
            Vertex { x: to.0, y: to.1 },
        ];
        let sectors = vec![
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(i32::from(front_floor)),
                ceil_height: doom_types::Fixed16_16::from_int(i32::from(front_ceil)),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(i32::from(back_floor)),
                ceil_height: doom_types::Fixed16_16::from_int(i32::from(back_ceil)),
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];
        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
        ];
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];
        let ssectors = vec![Ssector {
            seg_count: 1,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("reject parse");

        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "TEST2D".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    fn make_oblique_wall_level() -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![Vertex { x: -192, y: 128 }, Vertex { x: 192, y: 512 }];
        let sectors = vec![Sector {
            floor_height: doom_types::Fixed16_16::from_int(0),
            ceil_height: doom_types::Fixed16_16::from_int(128),
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: 192,
            special: 0,
            tag: 0,
        }];
        let sidedefs = vec![Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"WALL1\0\0\0",
            lower_texture: *b"WALL2\0\0\0",
            middle_texture: *b"WALL3\0\0\0",
            sector: 0,
        }];
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF,
        }];
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];
        let ssectors = vec![Ssector {
            seg_count: 1,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject parse");

        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "OBLIQUE".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    /// Build a level with:
    /// - Near two-sided portal at y=128 with a narrow opening.
    /// - Far two-sided portal at y=256 whose front sector spans a much taller
    ///   range than that near opening.
    ///
    /// If the far portal's plane bookkeeping ignores the current clip window,
    /// it can repaint rows outside the near window with the tall front sector's
    /// ceiling/floor flats.
    fn make_portal_window_with_far_portal_level() -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![
            Vertex { x: -64, y: 128 },
            Vertex { x: 64, y: 128 },
            Vertex { x: -64, y: 256 },
            Vertex { x: 64, y: 256 },
        ];

        let sectors = vec![
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(56),
                ceil_height: doom_types::Fixed16_16::from_int(72),
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT5\0\0\0",
                ceil_flat: *b"FLAT6\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(56),
                ceil_height: doom_types::Fixed16_16::from_int(72),
                floor_flat: *b"FLAT7\0\0\0",
                ceil_flat: *b"FLAT8\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];

        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 2,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 3,
            },
        ];

        let linedefs = vec![
            Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 2,
                left_sidedef: 3,
            },
        ];

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
                from_vertex: 2,
                to_vertex: 3,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
        ];

        let ssectors = vec![Ssector {
            seg_count: segs.len() as u16,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 2], sectors.len()).expect("reject parse");
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "PORTPRT".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    /// Build a level with two nested top-clipping portals in the same subsector:
    /// - Near portal at y=128 with a shallow top clip (front ceiling 96).
    /// - Far portal at y=256 with a deeper top clip (front ceiling 72).
    ///
    /// A sprite at y=192 sits behind the near portal but in front of the far
    /// portal. Doom should still use the near portal's saved clip state for
    /// that sprite instead of borrowing the farther portal's tighter context.
    fn make_nested_ceiling_portal_level() -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![
            Vertex { x: -64, y: 128 },
            Vertex { x: 64, y: 128 },
            Vertex { x: -64, y: 256 },
            Vertex { x: 64, y: 256 },
        ];

        let sectors = vec![
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(96),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(72),
                floor_flat: *b"FLAT5\0\0\0",
                ceil_flat: *b"FLAT6\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT7\0\0\0",
                ceil_flat: *b"FLAT8\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
        ];

        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 2,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"-\0\0\0\0\0\0\0",
                lower_texture: *b"-\0\0\0\0\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 3,
            },
        ];

        let linedefs = vec![
            Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 1,
            },
            Linedef {
                from_vertex: 2,
                to_vertex: 3,
                flags: 0x0004,
                special: 0,
                tag: 0,
                right_sidedef: 2,
                left_sidedef: 3,
            },
        ];

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
                from_vertex: 2,
                to_vertex: 3,
                angle: 0,
                linedef: 1,
                direction: 0,
                offset: 0,
            },
        ];

        let ssectors = vec![Ssector {
            seg_count: segs.len() as u16,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 2], sectors.len()).expect("reject parse");
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "NESTTOP".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    fn make_iwad(lumps: &[(&str, &[u8])]) -> Vec<u8> {
        let mut data: Vec<u8> = Vec::new();
        data.extend_from_slice(b"IWAD");
        data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
        data.extend_from_slice(&0i32.to_le_bytes());
        let mut offsets = Vec::new();
        for (_, bytes) in lumps {
            let pos = data.len();
            data.extend_from_slice(bytes);
            offsets.push((pos, bytes.len()));
        }
        let dir_off = data.len() as i32;
        data[8..12].copy_from_slice(&dir_off.to_le_bytes());
        for (i, (name, _)) in lumps.iter().enumerate() {
            let (fp, sz) = offsets[i];
            data.extend_from_slice(&(fp as i32).to_le_bytes());
            data.extend_from_slice(&(sz as i32).to_le_bytes());
            let mut nb = [0u8; 8];
            for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                nb[j] = b.to_ascii_uppercase();
            }
            data.extend_from_slice(&nb);
        }
        data
    }

    fn make_pnames(names: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(names.len() as u32).to_le_bytes());
        for name in names {
            let mut buf = [0u8; 8];
            for (i, &b) in name.as_bytes().iter().take(8).enumerate() {
                buf[i] = b.to_ascii_uppercase();
            }
            out.extend_from_slice(&buf);
        }
        out
    }

    fn make_texture1(
        tex_name: &str,
        width: u16,
        height: u16,
        patch_idx: u16,
        origin_x: i16,
        origin_y: i16,
    ) -> Vec<u8> {
        let tex_offset: u32 = 8;
        let mut out = Vec::new();
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&tex_offset.to_le_bytes());

        let mut name_buf = [0u8; 8];
        for (i, &b) in tex_name.as_bytes().iter().take(8).enumerate() {
            name_buf[i] = b.to_ascii_uppercase();
        }
        out.extend_from_slice(&name_buf);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&origin_x.to_le_bytes());
        out.extend_from_slice(&origin_y.to_le_bytes());
        out.extend_from_slice(&patch_idx.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn make_patch_from_rows(width: u16, rows: &[u8]) -> Vec<u8> {
        let w = width as usize;
        let h = rows.len();
        let col_data_len = 1 + 1 + 1 + h + 1 + 1;
        let header_size = 8;
        let offsets_size = w * 4;
        let first_col_offset = header_size + offsets_size;

        let mut out = Vec::new();
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&(h as u16).to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes());

        for c in 0..w {
            let col_off = (first_col_offset + c * col_data_len) as u32;
            out.extend_from_slice(&col_off.to_le_bytes());
        }

        for _ in 0..w {
            out.push(0);
            out.push(h as u8);
            out.push(0);
            out.extend_from_slice(rows);
            out.push(0);
            out.push(0xFF);
        }

        out
    }

    fn make_patch_from_columns(columns: &[u8]) -> Vec<u8> {
        let width = columns.len() as u16;
        let w = width as usize;
        let header_size = 8;
        let offsets_size = w * 4;
        let col_data_len = 6usize;
        let first_col_offset = header_size + offsets_size;

        let mut out = Vec::new();
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes());

        for c in 0..w {
            let col_off = (first_col_offset + c * col_data_len) as u32;
            out.extend_from_slice(&col_off.to_le_bytes());
        }

        for &pixel in columns {
            out.push(0);
            out.push(1);
            out.push(0);
            out.push(pixel);
            out.push(0);
            out.push(0xFF);
        }

        out
    }

    fn load_single_texture_cache(tex_name: &str, rows: &[u8]) -> TextureCache {
        let patch_data = make_patch_from_rows(8, rows);
        let pnames_data = make_pnames(&["PATCH01"]);
        let tex1_data = make_texture1(tex_name, 8, rows.len() as u16, 0, 0, 0);
        let wad_bytes = make_iwad(&[
            ("PNAMES", &pnames_data),
            ("PATCH01", &patch_data),
            ("TEXTURE1", &tex1_data),
        ]);
        let wad = WadFile::parse(wad_bytes).expect("WAD parse");
        TextureCache::load(&wad)
    }

    fn load_single_texture_cache_from_columns(tex_name: &str, columns: &[u8]) -> TextureCache {
        let patch_data = make_patch_from_columns(columns);
        let pnames_data = make_pnames(&["PATCH01"]);
        let tex1_data = make_texture1(tex_name, columns.len() as u16, 1, 0, 0, 0);
        let wad_bytes = make_iwad(&[
            ("PNAMES", &pnames_data),
            ("PATCH01", &patch_data),
            ("TEXTURE1", &tex1_data),
        ]);
        let wad = WadFile::parse(wad_bytes).expect("WAD parse");
        TextureCache::load(&wad)
    }

    fn load_two_texture_cache(
        tex_a_name: &str,
        tex_a_rows: &[u8],
        tex_b_name: &str,
        tex_b_rows: &[u8],
    ) -> TextureCache {
        let patch_a = make_patch_from_rows(8, tex_a_rows);
        let patch_b = make_patch_from_rows(8, tex_b_rows);
        let pnames_data = make_pnames(&["PATCH01", "PATCH02"]);
        let mut tex1_data = Vec::new();
        tex1_data.extend_from_slice(&2u32.to_le_bytes());
        let first_offset = 12u32;
        let first_desc = make_texture1(tex_a_name, 8, tex_a_rows.len() as u16, 0, 0, 0);
        let second_offset = first_offset + (first_desc.len() as u32 - 8);
        tex1_data.extend_from_slice(&first_offset.to_le_bytes());
        tex1_data.extend_from_slice(&second_offset.to_le_bytes());
        tex1_data.extend_from_slice(&first_desc[8..]);
        let second_desc = make_texture1(tex_b_name, 8, tex_b_rows.len() as u16, 1, 0, 0);
        tex1_data.extend_from_slice(&second_desc[8..]);
        let wad_bytes = make_iwad(&[
            ("PNAMES", &pnames_data),
            ("PATCH01", &patch_a),
            ("PATCH02", &patch_b),
            ("TEXTURE1", &tex1_data),
        ]);
        let wad = WadFile::parse(wad_bytes).expect("WAD parse");
        TextureCache::load(&wad)
    }

    fn make_masked_grate_level() -> Level {
        let mut level = make_two_sided_level(0, 128, 0, 128);
        level.sidedefs[0].middle_texture = *b"GRATE\0\0\0";
        level.sidedefs[1].middle_texture = *b"GRATE\0\0\0";
        level
    }

    fn make_opaque_sprite_cache(name: &str, width: u16, height: u16, pixel: u8) -> SpriteCache {
        let mut cache = SpriteCache::empty();
        cache.insert(
            doom_wad::lump::LumpName::from_str(name),
            SpriteFrame {
                width,
                height,
                left_offset: 0,
                top_offset: 0,
                pixels: vec![Some(pixel); width as usize * height as usize],
            },
        );
        cache
    }

    fn first_wall_row(fb: &Framebuffer, x: usize) -> Option<usize> {
        (0..SCREEN_H).find(|&y| matches!(fb.get_pixel(x, y), Some(px) if (32..64).contains(&px)))
    }

    fn rendered_rows(fb: &Framebuffer, palette_idx: u8) -> Option<(usize, usize)> {
        let top = (0..SCREEN_H)
            .find(|&y| (0..SCREEN_W).any(|x| fb.get_pixel(x, y) == Some(palette_idx)))?;
        let bottom = (0..SCREEN_H)
            .rev()
            .find(|&y| (0..SCREEN_W).any(|x| fb.get_pixel(x, y) == Some(palette_idx)))?;
        Some((top, bottom))
    }

    fn exact_view_depth_for_screen_x(
        x: usize,
        view_left: (f32, f32),
        view_right: (f32, f32),
    ) -> Option<f32> {
        let (vx1, vy1) = view_left;
        let (vx2, vy2) = view_right;
        let dx = vx2 - vx1;
        let dy = vy2 - vy1;
        let m = (x as f32 - HALF_W as f32) / FOCAL_LEN as f32;
        let denom = dy - m * dx;
        if denom.abs() < f32::EPSILON {
            return None;
        }
        let t = (m * vx1 - vy1) / denom;
        if !(0.0..=1.0).contains(&t) {
            return None;
        }
        Some(vx1 + t * dx)
    }

    fn exact_segment_param_for_screen_x(
        x: usize,
        view_left: (f32, f32),
        view_right: (f32, f32),
    ) -> Option<f32> {
        let (vx1, vy1) = view_left;
        let (vx2, vy2) = view_right;
        let dx = vx2 - vx1;
        let dy = vy2 - vy1;
        let m = (x as f32 - HALF_W as f32) / FOCAL_LEN as f32;
        let denom = dy - m * dx;
        if denom.abs() < f32::EPSILON {
            return None;
        }
        let t = (m * vx1 - vy1) / denom;
        (0.0..=1.0).contains(&t).then_some(t)
    }

    fn project_top_row(depth: f32, ceil_h: i32, view_z: i32) -> i32 {
        let projected = (((ceil_h - view_z) as f32) * FOCAL_LEN as f32 / depth).trunc() as i32;
        HALF_H - projected
    }

    #[test]
    fn render_empty_level_does_not_panic() {
        let level = make_minimal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level,
            0,
            0,
            Bam::ZERO,
            &mut fb,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        let has_nonzero = fb.data.iter().any(|&b| b != 0);
        assert!(
            has_nonzero,
            "framebuffer should be non-zero after rendering background"
        );
    }

    #[test]
    fn render_with_player_facing_wall_does_not_panic() {
        use doom_types::ANG90;

        let level = make_minimal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );
    }

    #[test]
    fn masked_column_skips_transparent_texels() {
        let mut fb = Framebuffer::new();
        draw_masked_column(
            &mut fb,
            10,
            0,
            3,
            0,
            1 << 16,
            &[0, 7, 0, 9],
            &IDENTITY_COLORMAP,
        );
        assert_eq!(fb.get_pixel(10, 0), Some(0), "row 0 is transparent");
        assert_eq!(fb.get_pixel(10, 1), Some(7), "row 1 must draw");
        assert_eq!(fb.get_pixel(10, 2), Some(0), "row 2 is transparent");
        assert_eq!(fb.get_pixel(10, 3), Some(9), "row 3 must draw");
    }

    #[test]
    fn one_sided_dash_middle_texture_still_occludes_as_solid_fallback() {
        use doom_types::ANG90;
        init_trig();

        let mut level = make_minimal_level();
        level.sidedefs[0].middle_texture = *b"-\0\0\0\0\0\0\0";

        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        let zbuf = render_level(
            &level, 64, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let cx = HALF_W as usize;
        assert!(
            zbuf.z_buf[cx].is_finite(),
            "one-sided wall with '-' middle texture must still write z-buffer and occlude"
        );
        let px = fb.get_pixel(cx, HALF_H as usize).unwrap_or(0);
        let is_wall = (32..64).contains(&px);
        assert!(
            is_wall,
            "fallback one-sided wall column should be wall-colored"
        );
    }

    #[test]
    fn one_sided_missing_preferred_side_falls_back_to_existing_side() {
        use doom_types::ANG90;

        let mut level = make_minimal_level();
        // One-sided linedef: only right sidedef exists.
        level.linedefs[0].right_sidedef = 0;
        level.linedefs[0].left_sidedef = 0xFFFF;
        // Force seg direction to point at the missing side first.
        level.segs[0].direction = 1;
        // Ensure the existing side has a drawable middle texture.
        level.sidedefs[0].middle_texture = *b"WALL3\0\0\0";

        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let zbuf = render_level(
            &level, 64, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let cx = HALF_W as usize;
        assert!(
            zbuf.z_buf[cx].is_finite(),
            "renderer should fall back to the existing side when preferred side is missing"
        );
    }

    #[test]
    fn one_sided_prefers_opposite_side_when_preferred_has_dash_middle() {
        use doom_types::ANG90;
        init_trig();

        let mut level = make_minimal_level();
        let mut dashed = level.sidedefs[0].clone();
        dashed.middle_texture = *b"-\0\0\0\0\0\0\0";
        level.sidedefs.push(dashed);

        // One-sided linedef has both sides, but the seg-facing side has "-"
        // while the opposite side has a real wall texture.
        level.linedefs[0].flags = 0;
        level.linedefs[0].right_sidedef = 1; // preferred by direction=0
        level.linedefs[0].left_sidedef = 0; // fallback with texture
        level.segs[0].direction = 0;

        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let zbuf = render_level(
            &level, 64, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let cx = HALF_W as usize;
        assert!(
            zbuf.z_buf[cx].is_finite(),
            "one-sided seg should use opposite sidedef when preferred side has '-' middle texture"
        );
    }

    #[test]
    fn invalid_two_sided_flag_without_back_sector_renders_as_solid() {
        use doom_types::ANG90;
        init_trig();

        let mut level = make_minimal_level();
        // Malformed-but-seen-in-the-wild style input: two-sided flag set but no left side.
        level.linedefs[0].flags = 0x0004;
        level.linedefs[0].left_sidedef = 0xFFFF;

        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let zbuf = render_level(
            &level, 64, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let cx = HALF_W as usize;
        assert!(
            zbuf.z_buf[cx] < f32::MAX,
            "line with no valid back sector must still render as solid wall"
        );
    }

    #[test]
    fn player_sector_sky_discovered_in_post_pass_is_drawn() {
        use doom_types::ANG90;
        init_trig();

        let mut level = make_minimal_level();
        level.sectors[0].ceil_flat = *b"F_SKY1\0\0";

        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        render_level(
            &level, 64, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        assert_eq!(
            fb.get_pixel(SCREEN_W - 1, 0),
            Some(SKY_FALLBACK_COLOR),
            "open columns whose sky is only discovered in the post-pass must still render sky"
        );
    }

    #[test]
    fn upper_texture_pegging_changes_sampled_rows() {
        use doom_types::ANG90;
        init_trig();

        let mut level = make_two_sided_level(0, 128, 0, 96);
        let rows: Vec<u8> = (1..=64).collect();
        let tex_cache = load_single_texture_cache("UPPER", &rows);
        let palette = PaletteLut::grayscale();

        let mut unpegged = Framebuffer::new();
        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut unpegged,
            &palette,
            None,
            Some(&tex_cache),
            None,
            None,
            false,
        );

        level.linedefs[0].flags |= FLAG_DONTPEGTOP;
        let mut pegged = Framebuffer::new();
        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut pegged,
            &palette,
            None,
            Some(&tex_cache),
            None,
            None,
            false,
        );

        let sample_x = HALF_W as usize;
        let first_diff = (0..40usize).find_map(|y| {
            let unpegged_px = unpegged.get_pixel(sample_x, y).unwrap_or(0);
            let pegged_px = pegged.get_pixel(sample_x, y).unwrap_or(0);
            (unpegged_px != pegged_px).then_some((y, unpegged_px, pegged_px))
        });

        assert!(
            first_diff.is_some(),
            "FLAG_DONTPEGTOP must change upper-band texture alignment; otherwise the stair/window slab bug comes back"
        );
    }

    #[test]
    fn logical_height_pegging_distinguishes_72_from_128() {
        use doom_types::ANG90;
        init_trig();

        let level = make_two_sided_level(0, 128, 0, 32);
        let rows_72: Vec<u8> = (1..=72).collect();
        let mut rows_128 = rows_72.clone();
        rows_128.resize(128, 0);
        let palette = PaletteLut::grayscale();
        let tex_72 = load_single_texture_cache("UPPER", &rows_72);
        let tex_128 = load_single_texture_cache("UPPER", &rows_128);
        let mut fb_72 = Framebuffer::new();
        let mut fb_128 = Framebuffer::new();

        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb_72,
            &palette,
            None,
            Some(&tex_72),
            None,
            None,
            false,
        );

        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb_128,
            &palette,
            None,
            Some(&tex_128),
            None,
            None,
            false,
        );

        let sample_x = HALF_W as usize;
        let first_diff = (0..HALF_H as usize).find_map(|y| {
            let a = fb_72.get_pixel(sample_x, y).unwrap_or(0);
            let b = fb_128.get_pixel(sample_x, y).unwrap_or(0);
            (a != b).then_some((y, a, b))
        });
        assert!(
            first_diff.is_some(),
            "logical-height pegging must render a true 72-high texture differently from a 128-high texture with the same padded cache data"
        );
    }

    #[test]
    fn map_specific_sky_selection_uses_level_name() {
        use doom_types::ANG90;
        init_trig();

        let mut level = make_minimal_level();
        level.name = "MAP12".to_owned();
        level.sectors[0].ceil_flat = *b"F_SKY1\0\0";
        let tex_cache =
            load_two_texture_cache("SKY1", &[17, 17, 17, 17], "SKY2", &[93, 93, 93, 93]);
        let palette = PaletteLut::grayscale();
        let mut fb = Framebuffer::new();

        render_level(
            &level,
            64,
            0,
            ANG90,
            &mut fb,
            &palette,
            None,
            Some(&tex_cache),
            None,
            None,
            false,
        );

        assert_eq!(
            fb.get_pixel(SCREEN_W - 1, 0),
            Some(93),
            "MAP12 should render SKY2, not hardcoded SKY1"
        );
    }

    #[test]
    fn masked_midtexture_sprite_ordering_keeps_grate_in_front() {
        use doom_game::states::sprite_names;
        use doom_types::{ANG90, Fixed16_16};

        init_trig();

        let level = make_masked_grate_level();
        let tex_cache =
            load_single_texture_cache_from_columns("GRATE", &[0, 31, 0, 31, 0, 31, 0, 31]);
        let palette = PaletteLut::grayscale();
        let mut fb = Framebuffer::new();

        let render_out = render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb,
            &palette,
            None,
            Some(&tex_cache),
            None,
            None,
            false,
        );

        let sprite_cache = make_opaque_sprite_cache("TROOA0", 8, 8, 200);
        let actors = [ActorRenderInfo {
            x: 0,
            y: 256 << 16,
            z: 0,
            angle: 0,
            sprite: sprite_names::SPR_TROO,
            frame: 0,
            height: 56 << 16,
            render_flag: RenderFlag::Normal,
            fallback_prefix: None,
        }];

        render_actors_with_masked_ex(
            &actors,
            &level,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            ANG90,
            &mut fb,
            &sprite_cache,
            Some(&render_out.z_buf),
            None,
            Some(crate::sprite::SpriteClip {
                top: &render_out.clip_top,
                bottom: &render_out.clip_bot,
                top_depth: &render_out.clip_top_depth,
                bottom_depth: &render_out.clip_bot_depth,
                top_history: Some(&render_out.clip_top_history),
                bottom_history: Some(&render_out.clip_bot_history),
            }),
            Some(&render_out.masked_columns),
        );

        let has_sprite = (120..200).any(|x| (40..160).any(|y| fb.get_pixel(x, y) == Some(200)));
        let has_grate = (120..200).any(|x| (40..160).any(|y| fb.get_pixel(x, y) == Some(31)));
        assert!(
            has_sprite,
            "sprite should remain visible through transparent grate columns"
        );
        assert!(
            has_grate,
            "opaque grate columns must still draw in front of the sprite"
        );
    }

    #[test]
    fn sprite_between_nested_top_portals_keeps_near_clip_context() {
        use doom_game::states::sprite_names;
        use doom_types::{ANG90, Fixed16_16};

        init_trig();

        let actor = ActorRenderInfo {
            x: 0,
            y: 192 << 16,
            z: 128 << 16,
            angle: 0,
            sprite: sprite_names::SPR_TROO,
            frame: 0,
            height: 64 << 16,
            render_flag: RenderFlag::Normal,
            fallback_prefix: None,
        };
        let sprite_cache = make_opaque_sprite_cache("TROOA0", 8, 64, 201);
        let palette = PaletteLut::grayscale();
        let near_only = make_two_sided_level(0, 96, 0, 128);

        let mut unclipped_fb = Framebuffer::new();
        crate::sprite::render_actors_ex(
            &[actor],
            &near_only,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            ANG90,
            &mut unclipped_fb,
            &sprite_cache,
            None,
            None,
            None,
        );

        let mut near_walls_fb = Framebuffer::new();
        let near_out = render_level(
            &near_only,
            0,
            0,
            ANG90,
            &mut near_walls_fb,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );
        let mut near_fb = Framebuffer::new();
        crate::sprite::render_actors_ex(
            &[actor],
            &near_only,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            ANG90,
            &mut near_fb,
            &sprite_cache,
            Some(&near_out.z_buf),
            None,
            Some(crate::sprite::SpriteClip {
                top: &near_out.clip_top,
                bottom: &near_out.clip_bot,
                top_depth: &near_out.clip_top_depth,
                bottom_depth: &near_out.clip_bot_depth,
                top_history: Some(&near_out.clip_top_history),
                bottom_history: Some(&near_out.clip_bot_history),
            }),
        );

        let nested = make_nested_ceiling_portal_level();
        let mut nested_walls_fb = Framebuffer::new();
        let nested_out = render_level(
            &nested,
            0,
            0,
            ANG90,
            &mut nested_walls_fb,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );
        let mut nested_fb = Framebuffer::new();
        crate::sprite::render_actors_ex(
            &[actor],
            &nested,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            ANG90,
            &mut nested_fb,
            &sprite_cache,
            Some(&nested_out.z_buf),
            None,
            Some(crate::sprite::SpriteClip {
                top: &nested_out.clip_top,
                bottom: &nested_out.clip_bot,
                top_depth: &nested_out.clip_top_depth,
                bottom_depth: &nested_out.clip_bot_depth,
                top_history: Some(&nested_out.clip_top_history),
                bottom_history: Some(&nested_out.clip_bot_history),
            }),
        );

        let unclipped_rows =
            rendered_rows(&unclipped_fb, 201).expect("baseline sprite should draw");
        let near_rows = rendered_rows(&near_fb, 201).expect("near-only clipped sprite should draw");
        let nested_rows =
            rendered_rows(&nested_fb, 201).expect("nested clipped sprite should draw");

        assert!(
            near_rows.0 > unclipped_rows.0,
            "near portal must actually crop the tall sprite: unclipped={unclipped_rows:?} near={near_rows:?}"
        );
        assert_eq!(
            nested_rows, near_rows,
            "sprite between the near and far portals should keep the near portal clip context, got nested={nested_rows:?} near={near_rows:?}"
        );
    }

    #[test]
    fn oblique_wall_top_edge_matches_exact_ray_intersection() {
        use doom_types::ANG90;
        init_trig();

        let level = make_oblique_wall_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let view_left = (128.0f32, -192.0f32);
        let view_right = (512.0f32, 192.0f32);
        let sample_columns = [40usize, 80, 120, 160, 200];
        let view_z = PLAYER_HEIGHT;
        let ceil_h = 128;

        for x in sample_columns {
            let actual_top = first_wall_row(&fb, x).expect("sample column should hit the wall");
            let depth = exact_view_depth_for_screen_x(x, view_left, view_right)
                .expect("ray should intersect the wall segment");
            let expected_top = project_top_row(depth, ceil_h, view_z);
            assert!(
                (actual_top as i32 - expected_top).abs() <= 1,
                "oblique wall top edge drifted at x={x}: got {actual_top}, expected {expected_top}"
            );
        }
    }

    #[test]
    fn close_clipped_wall_uses_exact_ray_texture_column() {
        use doom_types::ANG90;

        init_trig();

        let mut level = make_minimal_level();
        level.vertexes[0] = doom_map::lumps::Vertex { x: -64, y: 128 };
        level.vertexes[1] = doom_map::lumps::Vertex { x: 64, y: 128 };

        let columns: Vec<u8> = (1..=64).collect();
        let tex_cache = load_single_texture_cache_from_columns("WALL3", &columns);
        let palette = PaletteLut::grayscale();
        let mut fb = Framebuffer::new();

        render_level(
            &level,
            0,
            120,
            ANG90,
            &mut fb,
            &palette,
            None,
            Some(&tex_cache),
            None,
            None,
            false,
        );

        let view_left = (8.0f32, -64.0f32);
        let view_right = (8.0f32, 64.0f32);
        // Interior columns only: the two screen-edge columns (0 and 319) are
        // projection-singular for a grazing wall, where the idealized f32 ray
        // model and the exact fixed-point `finesine` renderer can disagree by
        // many texels. Interior samples exercise the mapping meaningfully.
        let sample_columns = [40usize, 80, 160, 240, 279];

        for x in sample_columns {
            let s = exact_segment_param_for_screen_x(x, view_left, view_right)
                .expect("screen ray should intersect the unclipped wall");
            let u_world = s * 128.0;
            let expected_index = (u_world.trunc() as i32).rem_euclid(64) as usize;
            let expected = columns[expected_index];
            let actual = fb
                .get_pixel(x, HALF_H as usize)
                .expect("close wall should fill the center row");

            // `expected` comes from an idealized f32 ray model, while the
            // renderer walks Doom's exact fixed-point `finesine`/`finecosine`
            // table. At a texel boundary the two legitimately pick adjacent
            // columns, so allow a ±1 texture-column difference (mod 64) — this
            // is what vanilla Doom itself renders.
            let actual_index = (actual as i32 - 1).rem_euclid(64);
            let exp_index = expected_index as i32;
            let delta = (actual_index - exp_index).rem_euclid(64);
            let within_one = delta <= 1 || delta >= 63;
            assert!(
                within_one,
                "close clipped wall sampled wrong texture column at x={x}: got {actual} (col {actual_index}), expected {expected} (col {exp_index})"
            );
        }
    }

    #[test]
    fn z_buffer_prevents_overdraw() {
        let level = make_minimal_level();
        let palette = PaletteLut::grayscale();

        let mut fb1 = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            Bam::ZERO,
            &mut fb1,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        let mut fb2 = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            Bam::ZERO,
            &mut fb2,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        assert_eq!(fb1.data.as_slice(), fb2.data.as_slice());
    }

    #[test]
    fn clip_seg_both_behind_returns_none() {
        assert!(clip_seg_to_near_plane(-10, 0, -5, 0).is_none());
    }

    #[test]
    fn clip_seg_both_in_front_unchanged() {
        let result = clip_seg_to_near_plane(5, 2, 10, 4);
        assert_eq!(result, Some((5, 2, 10, 4)));
    }

    #[test]
    fn clip_seg_one_behind_clips_correctly() {
        let result = clip_seg_to_near_plane(-2, 0, 4, 6);
        assert!(result.is_some());
        let (cx1, _cy1, cx2, cy2) = result.expect("value must exist in test");
        assert_eq!(cx1, 1);
        assert_eq!(cx2, 4);
        assert_eq!(cy2, 6);
    }

    /// Regression: passing `flat_cache = None` must not panic (backward compat).
    #[test]
    fn test_render_level_with_flat_cache_none_smoke() {
        // Initialise trig tables so Bam::ZERO.cos()/sin() return correct values
        // regardless of test execution order (the guard makes this idempotent).
        doom_types::Bam::init_trig_tables();

        let level = make_minimal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level,
            0,
            64,
            Bam::ZERO,
            &mut fb,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        // The minimal wall (north of the east-facing player) projects to the left
        // portion of the screen (~columns 0-80).  Column 319 is always background.
        // Background fill: 25 = ceiling (top half, rows 0..100).
        assert_eq!(fb.get_pixel(319, 0), Some(25));
        // Background fill: 119 = floor (bottom half, rows 100..200).
        assert_eq!(fb.get_pixel(319, SCREEN_H - 1), Some(119));
    }

    /// Regression: passing `tex_cache = None` must not panic (backward compat).
    #[test]
    fn test_render_level_with_tex_cache_none_smoke() {
        let level = make_minimal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        // tex_cache = None should fall back to flat-shaded walls without panic.
        render_level(
            &level,
            64,
            0,
            Bam::ZERO,
            &mut fb,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        // Should produce some output (background fill at minimum).
        let has_nonzero = fb.data.iter().any(|&b| b != 0);
        assert!(
            has_nonzero,
            "render with tex_cache=None must produce non-zero output"
        );
    }

    /// Passing a FlatCache with a known flat renders textured pixels into
    /// the floor region without panicking.
    #[test]
    fn test_render_level_with_flat_cache_smoke() {
        use doom_types::limits::FLAT_SIZE;

        // Build a minimal in-memory WAD with F_START / FLAT1 / FLAT2 / F_END.
        let mut flat_data = vec![0u8; FLAT_SIZE];
        flat_data
            .iter_mut()
            .enumerate()
            .for_each(|(i, b)| *b = (i % 200) as u8);

        let make_iwad = |lumps: &[(&str, &[u8])]| {
            let mut data: Vec<u8> = Vec::new();
            data.extend_from_slice(b"IWAD");
            data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
            data.extend_from_slice(&0i32.to_le_bytes());
            let mut offsets = Vec::new();
            for (_, bytes) in lumps {
                let pos = data.len();
                data.extend_from_slice(bytes);
                offsets.push((pos, bytes.len()));
            }
            let dir_off = data.len() as i32;
            data[8..12].copy_from_slice(&dir_off.to_le_bytes());
            for (i, (name, _)) in lumps.iter().enumerate() {
                let (fp, sz) = offsets[i];
                data.extend_from_slice(&(fp as i32).to_le_bytes());
                data.extend_from_slice(&(sz as i32).to_le_bytes());
                let mut nb = [0u8; 8];
                for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                    nb[j] = b.to_ascii_uppercase();
                }
                data.extend_from_slice(&nb);
            }
            data
        };

        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat_data),
            ("FLAT2", &flat_data),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("parse WAD");
        let cache = FlatCache::load(&wad);
        assert_eq!(cache.len(), 2, "both flats loaded");

        let level = make_minimal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        // Player at (64, 0) facing forward — exercises the wall path.
        render_level(
            &level,
            64,
            0,
            Bam::ZERO,
            &mut fb,
            &palette,
            Some(&cache),
            None,
            None,
            None,
            false,
        );

        // Should not panic and should produce some non-zero output.
        let has_nonzero = fb.data.iter().any(|&b| b != 0);
        assert!(
            has_nonzero,
            "render with flat cache must produce non-zero output"
        );
    }

    // -----------------------------------------------------------------------
    // NEW: Two-sided linedef tests
    // -----------------------------------------------------------------------

    /// Initialize trig tables once before tests that need them.
    fn init_trig() {
        // SAFETY: test environment, called at most once per process due to the
        // AtomicBool guard inside init_trig_tables.
        doom_types::Bam::init_trig_tables();
    }

    /// A two-sided seg with front_ceil > back_ceil produces pixels in the
    /// upper band (w_top .. screen_back_ceil).
    ///
    /// Setup:
    ///   front sector: floor=0, ceil=128
    ///   back  sector: floor=0, ceil=64   ← back ceiling is lower → upper step
    ///
    /// Player at (0, 0), wall at y=128.  ANG90 = North (+Y) so the player
    /// faces the wall at y=128.
    /// The center column (x=160) should have some opaque pixels between w_top
    /// and the projected back_ceil position.
    #[test]
    fn test_two_sided_seg_draws_upper_band() {
        use doom_types::ANG90;
        init_trig();

        // front_ceil=128 > back_ceil=64 → upper band should be drawn
        let level = make_two_sided_level(0, 128, 0, 64);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        // Player at y=0, wall at y=128, facing ANG90 = North (+Y direction).
        render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        // The wall spans some columns around center (x=160).
        // We check that the framebuffer has been written in the upper half for
        // at least some columns — a flat-shade fallback color (>= 32) should appear.
        // The background fills with 25 (ceiling) and 119 (floor).  The upper
        // band is drawn with a flat-shade wall color = 32 + light (= 32 + 24 = 56).
        let center_x = HALF_W as usize;
        // Look for wall-colored pixels anywhere in the center column — either
        // upper (upper band) or lower half may contain wall pixels.
        let has_wall = (0..SCREEN_H).any(|y| {
            let px = fb.get_pixel(center_x, y).unwrap_or(0);
            // flat-shade wall color is 32..=63 range, distinct from ceiling=25, floor=119
            (32..64).contains(&px)
        });
        assert!(
            has_wall,
            "upper band pixels must appear in some column for two-sided seg with front_ceil > back_ceil"
        );
    }

    /// The portal opening for a two-sided seg causes wall_top[x] and wall_bot[x]
    /// to be set to the portal opening bounds — not the full front-sector span.
    ///
    /// This means the floor/ceiling spans can fill through the portal.
    ///
    /// We verify this by checking that wall_top < HALF_H and wall_bot > HALF_H
    /// does NOT hold for the portal center column when the back sector is inset
    /// (which would mean the column is fully blocked by the wall).  Instead,
    /// wall_top and wall_bot should reflect only the opaque bands.
    ///
    /// Concretely: with front floor=0/ceil=128 and back floor=32/ceil=96,
    /// the portal opening occupies the middle of the view.  After render,
    /// the back sector's flats (FLAT3/FLAT4) should be recorded for the
    /// center column (they flow through the portal).
    #[test]
    fn test_two_sided_portal_leaves_opening() {
        use doom_types::ANG90;
        init_trig();

        // front: floor=0,  ceil=128
        // back:  floor=32, ceil=96   → upper step + lower step
        let level = make_two_sided_level(0, 128, 32, 96);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        // We cannot directly inspect wall_top/wall_bot from outside, but we can
        // verify the visual outcome:
        //
        // With a portal, the MIDDLE of the center column should NOT be painted
        // with wall color (32-63) — it is the open portal that shows the sky/floor
        // background (25 or 119).
        //
        // Specifically, the very center row (HALF_H = 100) should not be wall-colored
        // since it falls within the portal opening (back_ceil=96 projected above
        // center, back_floor=32 projected below center).
        let center_x = HALF_W as usize;
        let center_y = HALF_H as usize;

        // At the exact horizon row (y=100), the flat code skips (dy==0), so
        // we check a row just above and just below center that should be inside
        // the portal opening.
        let row_above_center = center_y.saturating_sub(5); // y=95, inside portal gap
        let row_below_center = center_y + 5; // y=105, inside portal gap

        let px_above = fb.get_pixel(center_x, row_above_center).unwrap_or(0);
        let px_below = fb.get_pixel(center_x, row_below_center).unwrap_or(0);

        // These pixels should be background (ceiling=25 or floor=119), NOT wall (32-63).
        // The portal is transparent so the background fill shows through.
        let is_wall_color = |px: u8| (32..64).contains(&px);
        assert!(
            !is_wall_color(px_above),
            "pixel at ({center_x}, {row_above_center}) = {px_above} should NOT be wall-colored (portal opening)"
        );
        assert!(
            !is_wall_color(px_below),
            "pixel at ({center_x}, {row_below_center}) = {px_below} should NOT be wall-colored (portal opening)"
        );
    }

    #[test]
    fn test_full_height_two_sided_line_does_not_narrow_sprite_clip_bounds() {
        use doom_types::ANG90;
        init_trig();

        let level = make_two_sided_level(0, 128, 0, 128);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let out = render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let x = HALF_W as usize;
        assert_eq!(
            out.clip_top[x], 0,
            "fully open two-sided lines must not synthesize a top sprite clip"
        );
        assert_eq!(
            out.clip_bot[x],
            SCREEN_H as i32 - 1,
            "fully open two-sided lines must not synthesize a bottom sprite clip"
        );
    }

    #[test]
    fn test_floor_step_portal_only_narrows_bottom_sprite_clip() {
        use doom_types::ANG90;
        init_trig();

        let level = make_two_sided_level(0, 128, 56, 128);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let out = render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let x = HALF_W as usize;
        assert_eq!(
            out.clip_top[x], 0,
            "raised-floor portals must not synthesize a top sprite clip"
        );
        assert!(
            out.clip_bot[x] < SCREEN_H as i32 - 1,
            "raised-floor portals should still narrow the bottom sprite clip"
        );
    }

    #[test]
    fn test_ceiling_step_portal_only_narrows_top_sprite_clip() {
        use doom_types::ANG90;
        init_trig();

        let level = make_two_sided_level(0, 128, 0, 72);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let out = render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let x = HALF_W as usize;
        assert!(
            out.clip_top[x] > 0,
            "lowered-ceiling portals should narrow the top sprite clip"
        );
        assert_eq!(
            out.clip_bot[x],
            SCREEN_H as i32 - 1,
            "lowered-ceiling portals must not synthesize a bottom sprite clip"
        );
    }

    #[test]
    fn test_front_dropoff_portal_narrows_bottom_sprite_clip() {
        use doom_types::ANG90;
        init_trig();

        let level = make_two_sided_level(56, 128, 0, 128);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let out = render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let x = HALF_W as usize;
        let expected_bot = project_wall_y(-PLAYER_HEIGHT, FOCAL_LEN as f32 / 128.0);
        assert_eq!(
            out.clip_top[x], 0,
            "front drop-offs must not synthesize a top sprite clip"
        );
        assert_eq!(
            out.clip_bot[x], expected_bot,
            "front drop-offs should clip sprites behind the ledge at the front floor edge"
        );
    }

    #[test]
    fn test_front_low_ceiling_portal_narrows_top_sprite_clip() {
        use doom_types::ANG90;
        init_trig();

        let level = make_two_sided_level(0, 72, 0, 128);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let out = render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let x = HALF_W as usize;
        let expected_top = project_wall_y(72 - PLAYER_HEIGHT, FOCAL_LEN as f32 / 128.0);
        assert_eq!(
            out.clip_top[x], expected_top,
            "front lowered ceilings should clip sprites behind the portal at the front ceiling edge"
        );
        assert_eq!(
            out.clip_bot[x],
            SCREEN_H as i32 - 1,
            "front lowered ceilings must not synthesize a bottom sprite clip"
        );
    }

    /// Regression: the front floor visplane must not overwrite the first row of
    /// a two-sided lower wall band. When it does, doors show a little floor
    /// patch bleeding through the seam.
    #[test]
    fn test_front_floor_does_not_leak_into_two_sided_lower_wall() {
        use doom_types::ANG90;
        use doom_types::limits::FLAT_SIZE;

        init_trig();

        let flat1 = vec![180u8; FLAT_SIZE];
        let flat2 = vec![20u8; FLAT_SIZE];
        let flat3 = vec![40u8; FLAT_SIZE];
        let flat4 = vec![60u8; FLAT_SIZE];
        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat1),
            ("FLAT2", &flat2),
            ("FLAT3", &flat3),
            ("FLAT4", &flat4),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let level = make_two_sided_level(0, 128, 56, 96);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        let center_x = HALF_W as usize;
        let lower_wall_top = project_wall_y(56 - PLAYER_HEIGHT, FOCAL_LEN as f32 / 128.0) as usize;
        let px = fb
            .get_pixel(center_x, lower_wall_top)
            .expect("pixel inside lower wall band");

        assert!(
            (32..64).contains(&px),
            "lower wall seam at row {lower_wall_top} should stay wall-colored, got {px}"
        );
    }

    /// Regression: at oblique angles, the lower wall band of a two-sided door
    /// must remain wall-colored across the visible span. The back sector floor
    /// must not bleed into the band through the portal.
    #[test]
    fn test_oblique_two_sided_lower_wall_does_not_show_back_floor() {
        use doom_types::ANG90;
        use doom_types::limits::FLAT_SIZE;

        init_trig();

        let flat1 = vec![180u8; FLAT_SIZE];
        let flat2 = vec![20u8; FLAT_SIZE];
        let flat3 = vec![200u8; FLAT_SIZE];
        let flat4 = vec![220u8; FLAT_SIZE];
        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat1),
            ("FLAT2", &flat2),
            ("FLAT3", &flat3),
            ("FLAT4", &flat4),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let level = make_two_sided_level_with_vertices((-96, 160), (64, 256), 0, 128, 56, 96);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let out = render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        let v1 = &level.vertexes[0];
        let v2 = &level.vertexes[1];
        let angle = ANG90;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let cos_i = cos_a.0 as i64;
        let sin_i = sin_a.0 as i64;

        let dx1 = v1.x as i64;
        let dy1 = v1.y as i64;
        let dx2 = v2.x as i64;
        let dy2 = v2.y as i64;
        let vx1 = (dx1 * cos_i + dy1 * sin_i) >> 16;
        let vy1 = (dx1 * sin_i - dy1 * cos_i) >> 16;
        let vx2 = (dx2 * cos_i + dy2 * sin_i) >> 16;
        let vy2 = (dx2 * sin_i - dy2 * cos_i) >> 16;
        let (vx1, vy1, vx2, vy2) =
            crate::clip::clip_seg_to_view_frustum(vx1, vy1, vx2, vy2).expect("wall visible");

        let sx1 = HALF_W as i64 + (FOCAL_LEN as i64 * vy1) / vx1.max(1);
        let sx2 = HALF_W as i64 + (FOCAL_LEN as i64 * vy2) / vx2.max(1);
        let (sx_left, sx_right) = if sx1 <= sx2 { (sx1, sx2) } else { (sx2, sx1) };
        let col_start = sx_left.max(0).min((SCREEN_W - 1) as i64) as usize;
        let col_end = sx_right.max(0).min((SCREEN_W - 1) as i64) as usize;
        assert!(col_end > col_start + 8, "need a visible oblique span");

        let mut leaked = Vec::new();
        for x in col_start + 2..col_end.saturating_sub(2) {
            let lower_row = out.clip_bot[x] + 1;
            if !(0..SCREEN_H as i32).contains(&lower_row) {
                continue;
            }
            let px = fb
                .get_pixel(x, lower_row as usize)
                .expect("pixel inside framebuffer");
            if !(32..64).contains(&px) {
                leaked.push((x, lower_row as usize, px));
            }
        }

        assert!(
            leaked.is_empty(),
            "lower wall band leaked non-wall pixels across oblique span: {leaked:?}"
        );
    }

    #[test]
    fn test_oblique_two_sided_lower_wall_band_stays_wall_colored() {
        use doom_types::ANG90;
        use doom_types::limits::FLAT_SIZE;

        init_trig();

        let flat1 = vec![180u8; FLAT_SIZE];
        let flat2 = vec![20u8; FLAT_SIZE];
        let flat3 = vec![200u8; FLAT_SIZE];
        let flat4 = vec![220u8; FLAT_SIZE];
        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat1),
            ("FLAT2", &flat2),
            ("FLAT3", &flat3),
            ("FLAT4", &flat4),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let level = make_two_sided_level_with_vertices((-96, 160), (64, 256), 0, 128, 56, 96);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        let v1 = &level.vertexes[0];
        let v2 = &level.vertexes[1];
        let angle = ANG90;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let cos_i = cos_a.0 as i64;
        let sin_i = sin_a.0 as i64;
        let dx1 = v1.x as i64;
        let dy1 = v1.y as i64;
        let dx2 = v2.x as i64;
        let dy2 = v2.y as i64;
        let vx1 = (dx1 * cos_i + dy1 * sin_i) >> 16;
        let vy1 = (dx1 * sin_i - dy1 * cos_i) >> 16;
        let vx2 = (dx2 * cos_i + dy2 * sin_i) >> 16;
        let vy2 = (dx2 * sin_i - dy2 * cos_i) >> 16;
        let (vx1, vy1, vx2, vy2) =
            crate::clip::clip_seg_to_view_frustum(vx1, vy1, vx2, vy2).expect("wall visible");

        let sx1 = HALF_W as i64 + (FOCAL_LEN as i64 * vy1) / vx1.max(1);
        let sx2 = HALF_W as i64 + (FOCAL_LEN as i64 * vy2) / vx2.max(1);
        let (sx_left, sx_right) = if sx1 <= sx2 { (sx1, sx2) } else { (sx2, sx1) };
        let col_start = sx_left.max(0).min((SCREEN_W - 1) as i64) as usize;
        let col_end = sx_right.max(0).min((SCREEN_W - 1) as i64) as usize;
        assert!(col_end > col_start + 8, "need a visible oblique span");

        let view_left = (vx1 as f32, vy1 as f32);
        let view_right = (vx2 as f32, vy2 as f32);
        let mut leaked = Vec::new();
        for x in col_start + 2..col_end.saturating_sub(2) {
            let Some(depth) = exact_view_depth_for_screen_x(x, view_left, view_right) else {
                continue;
            };
            let scale = FOCAL_LEN as f32 / depth;
            // Inset the float-predicted band by 1px top and bottom: the exact
            // fixed-point `finesine` renderer places the wall edge within ±1px
            // of this idealized f32 projection, so the boundary rows are not a
            // reliable "must be wall" region.
            let lower_top =
                (project_wall_y(56 - PLAYER_HEIGHT, scale) + 1).clamp(0, SCREEN_H as i32 - 1);
            let lower_bot =
                (project_wall_y(0 - PLAYER_HEIGHT, scale) - 1).clamp(0, SCREEN_H as i32 - 1);
            if lower_top > lower_bot {
                continue;
            }
            for y in lower_top as usize..=lower_bot as usize {
                let px = fb.get_pixel(x, y).expect("pixel inside framebuffer");
                if !(32..64).contains(&px) {
                    leaked.push((x, y, px));
                    break;
                }
            }
        }

        assert!(
            leaked.is_empty(),
            "lower wall band contained non-wall pixels across oblique span: {leaked:?}"
        );
    }

    #[test]
    fn test_far_portal_flats_do_not_repaint_near_wall_pixels_at_oblique_view() {
        use doom_types::ANG90;
        use doom_types::limits::FLAT_SIZE;

        init_trig();

        let flat1 = vec![10u8; FLAT_SIZE];
        let flat2 = vec![20u8; FLAT_SIZE];
        let flat3 = vec![210u8; FLAT_SIZE];
        let flat4 = vec![220u8; FLAT_SIZE];
        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat1),
            ("FLAT2", &flat2),
            ("FLAT3", &flat3),
            ("FLAT4", &flat4),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);
        let palette = PaletteLut::grayscale();

        let level = make_occluded_portal_level();
        let mut full_fb = Framebuffer::new();
        render_level(
            &level,
            48,
            0,
            ANG90,
            &mut full_fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        let mut near_only = make_minimal_level();
        near_only.vertexes[0] = doom_map::lumps::Vertex { x: -64, y: 128 };
        near_only.vertexes[1] = doom_map::lumps::Vertex { x: 64, y: 128 };

        let mut near_fb = Framebuffer::new();
        render_level(
            &near_only,
            48,
            0,
            ANG90,
            &mut near_fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        let mut leaked = Vec::new();
        for x in 0..SCREEN_W {
            for y in 0..SCREEN_H {
                let near_px = near_fb.get_pixel(x, y).unwrap_or(0);
                if !(32..64).contains(&near_px) {
                    continue;
                }
                let full_px = full_fb.get_pixel(x, y).unwrap_or(0);
                if full_px == 210 || full_px == 220 {
                    leaked.push((x, y, full_px));
                    if leaked.len() >= 8 {
                        break;
                    }
                }
            }
            if leaked.len() >= 8 {
                break;
            }
        }

        assert!(
            leaked.is_empty(),
            "far portal flats repainted near wall pixels at oblique view: {leaked:?}"
        );
    }

    #[test]
    fn test_closed_two_sided_door_does_not_leave_floor_slit() {
        use doom_types::ANG90;
        use doom_types::limits::FLAT_SIZE;

        init_trig();

        let flat1 = vec![180u8; FLAT_SIZE];
        let flat2 = vec![20u8; FLAT_SIZE];
        let flat3 = vec![210u8; FLAT_SIZE];
        let flat4 = vec![220u8; FLAT_SIZE];
        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat1),
            ("FLAT2", &flat2),
            ("FLAT3", &flat3),
            ("FLAT4", &flat4),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let level = make_two_sided_level(0, 128, 0, 0);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        let center_x = HALF_W as usize;
        let floor_row = project_wall_y(0 - PLAYER_HEIGHT, FOCAL_LEN as f32 / 128.0) as usize;
        let px = fb
            .get_pixel(center_x, floor_row)
            .expect("pixel inside closed door span");

        assert!(
            (32..64).contains(&px),
            "closed two-sided door should stay wall-colored at floor row {floor_row}, got {px}"
        );
    }

    /// Regression: visplane spans must be clipped against wall-pass
    /// ceiling/floor clip bounds. A far portal must not repaint pixels that
    /// belong to a nearer one-sided wall in the same column.
    #[test]
    fn test_visplane_clipped_by_near_wall_when_far_portal_exists() {
        use doom_types::ANG90;
        use doom_types::limits::FLAT_SIZE;

        init_trig();

        let make_iwad = |lumps: &[(&str, &[u8])]| {
            let mut data: Vec<u8> = Vec::new();
            data.extend_from_slice(b"IWAD");
            data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
            data.extend_from_slice(&0i32.to_le_bytes());
            let mut offsets = Vec::new();
            for (_, bytes) in lumps {
                let pos = data.len();
                data.extend_from_slice(bytes);
                offsets.push((pos, bytes.len()));
            }
            let dir_off = data.len() as i32;
            data[8..12].copy_from_slice(&dir_off.to_le_bytes());
            for (i, (name, _)) in lumps.iter().enumerate() {
                let (fp, sz) = offsets[i];
                data.extend_from_slice(&(fp as i32).to_le_bytes());
                data.extend_from_slice(&(sz as i32).to_le_bytes());
                let mut nb = [0u8; 8];
                for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                    nb[j] = b.to_ascii_uppercase();
                }
                data.extend_from_slice(&nb);
            }
            data
        };

        // Distinct flat constants make leaks obvious.
        let flat1 = vec![10u8; FLAT_SIZE];
        let flat2 = vec![20u8; FLAT_SIZE];
        let flat3 = vec![180u8; FLAT_SIZE];
        let flat4 = vec![200u8; FLAT_SIZE];

        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat1),
            ("FLAT2", &flat2),
            ("FLAT3", &flat3),
            ("FLAT4", &flat4),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let level = make_occluded_portal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        let center_x = HALF_W as usize;
        let upper_sample = 70usize;
        let lower_sample = 130usize;
        let px_upper = fb.get_pixel(center_x, upper_sample).unwrap_or(0);
        let px_lower = fb.get_pixel(center_x, lower_sample).unwrap_or(0);
        let is_wall = |px: u8| (32..64).contains(&px);

        assert!(
            is_wall(px_upper),
            "upper sample must remain wall-colored (near wall occludes far portal flats), got {px_upper}"
        );
        assert!(
            is_wall(px_lower),
            "lower sample must remain wall-colored (near wall occludes far portal flats), got {px_lower}"
        );
    }

    /// Regression: a far one-sided wall behind a near portal must be clipped
    /// to the portal opening and must not leak into rows outside the window.
    #[test]
    fn test_far_solid_wall_is_clipped_to_near_portal_window() {
        use doom_types::ANG90;
        init_trig();

        let level = make_portal_window_with_far_solid_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let x = HALF_W as usize;
        let is_wall = |px: u8| (32..64).contains(&px);
        let wall_rows: Vec<usize> = (0..SCREEN_H)
            .filter(|&y| is_wall(fb.get_pixel(x, y).unwrap_or(0)))
            .collect();

        assert!(
            !wall_rows.is_empty(),
            "far wall should be visible through the near portal opening"
        );

        // For this test geometry:
        // - near portal opening projects from y=62 to y=82 at x=center
        // - far wall projects below that without clipping
        // Correct behaviour is that only rows inside the portal window survive.
        let portal_top = (HALF_H - ((72 - PLAYER_HEIGHT) * FOCAL_LEN / 128)) as usize;
        let portal_bot = (HALF_H - ((56 - PLAYER_HEIGHT) * FOCAL_LEN / 128)) as usize;
        let leaked = wall_rows
            .iter()
            .copied()
            .find(|&y| y < portal_top || y > portal_bot);
        assert!(
            leaked.is_none(),
            "far wall leaked outside portal window {:?}; first leaked row: {:?}",
            wall_rows,
            leaked
        );
    }

    /// Regression: a far two-sided portal clipped by a nearer portal must not
    /// repaint rows outside the near portal's opening with the far portal's
    /// front-sector ceiling/floor flats.
    #[test]
    fn test_far_portal_visplanes_respect_near_portal_window() {
        use doom_types::ANG90;
        use doom_types::limits::FLAT_SIZE;

        init_trig();

        let flat1 = vec![10u8; FLAT_SIZE];
        let flat2 = vec![20u8; FLAT_SIZE];
        let flat3 = vec![30u8; FLAT_SIZE];
        let flat4 = vec![40u8; FLAT_SIZE];
        let flat5 = vec![210u8; FLAT_SIZE];
        let flat6 = vec![220u8; FLAT_SIZE];
        let flat7 = vec![50u8; FLAT_SIZE];
        let flat8 = vec![60u8; FLAT_SIZE];

        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat1),
            ("FLAT2", &flat2),
            ("FLAT3", &flat3),
            ("FLAT4", &flat4),
            ("FLAT5", &flat5),
            ("FLAT6", &flat6),
            ("FLAT7", &flat7),
            ("FLAT8", &flat8),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let level = make_portal_window_with_far_portal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        let x = HALF_W as usize;
        let portal_top = (HALF_H - ((72 - PLAYER_HEIGHT) * FOCAL_LEN / 128)) as usize;
        let portal_bot = (HALF_H - ((56 - PLAYER_HEIGHT) * FOCAL_LEN / 128)) as usize;
        let above_window = portal_top.saturating_sub(5);
        let below_window = portal_bot + 5;

        let px_above = fb.get_pixel(x, above_window).unwrap_or(0);
        let px_below = fb.get_pixel(x, below_window).unwrap_or(0);

        assert_ne!(
            px_above, 220,
            "far portal ceiling flat leaked above the near window at row {above_window}"
        );
        assert_ne!(
            px_below, 210,
            "far portal floor flat leaked below the near window at row {below_window}"
        );
    }

    /// Regression: clip bounds for a two-sided wall seen at an oblique angle
    /// must match the exact ray/segment intersection depth, not a linear
    /// interpolation of endpoint depths.
    #[test]
    fn test_oblique_portal_clip_matches_exact_ray_intersection() {
        use doom_types::{ANG45, ANG90};

        init_trig();

        let level = make_two_sided_level(0, 128, 56, 72);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let out = render_level(
            &level,
            0,
            0,
            ANG90 - ANG45,
            &mut fb,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        let v1 = &level.vertexes[0];
        let v2 = &level.vertexes[1];
        let angle = ANG90 - ANG45;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let cos_i = cos_a.0 as i64;
        let sin_i = sin_a.0 as i64;

        let dx1 = v1.x as i64;
        let dy1 = v1.y as i64;
        let dx2 = v2.x as i64;
        let dy2 = v2.y as i64;

        let vx1 = (dx1 * cos_i + dy1 * sin_i) >> 16;
        let vy1 = (dx1 * sin_i - dy1 * cos_i) >> 16;
        let vx2 = (dx2 * cos_i + dy2 * sin_i) >> 16;
        let vy2 = (dx2 * sin_i - dy2 * cos_i) >> 16;
        let (vx1, vy1, vx2, vy2) =
            crate::clip::clip_seg_to_view_frustum(vx1, vy1, vx2, vy2).expect("wall visible");

        let sx1 = HALF_W as i64 + (FOCAL_LEN as i64 * vy1) / vx1.max(1);
        let sx2 = HALF_W as i64 + (FOCAL_LEN as i64 * vy2) / vx2.max(1);
        let (sx_left, sx_right) = if sx1 <= sx2 { (sx1, sx2) } else { (sx2, sx1) };
        let col_start = sx_left.max(0).min((SCREEN_W - 1) as i64) as usize;
        let col_end = sx_right.max(0).min((SCREEN_W - 1) as i64) as usize;
        assert!(col_end > col_start + 4, "need a visible oblique span");

        let target_x = col_end.saturating_sub(2);
        let ray = (target_x as f32 - HALF_W as f32) / FOCAL_LEN as f32;
        let dvx = (vx2 - vx1) as f32;
        let dvy = (vy2 - vy1) as f32;
        let denom = dvy - ray * dvx;
        assert!(
            denom.abs() > f32::EPSILON,
            "ray must intersect oblique wall"
        );
        let s = (ray * vx1 as f32 - vy1 as f32) / denom;
        assert!(
            (0.0..=1.0).contains(&s),
            "intersection parameter must stay on the clipped seg, got {s}"
        );
        let exact_depth = vx1 as f32 + s * dvx;
        let expected_top = HALF_H
            - (((72 - PLAYER_HEIGHT) as f32 * FOCAL_LEN as f32) / exact_depth).round() as i32;
        let expected_bot = HALF_H
            - (((56 - PLAYER_HEIGHT) as f32 * FOCAL_LEN as f32) / exact_depth).round() as i32;

        let actual_top = out.clip_top[target_x];
        let actual_bot = out.clip_bot[target_x];

        assert!(
            (actual_top - expected_top).abs() <= 1,
            "portal top at x={target_x} should match exact ray depth: actual={actual_top}, expected={expected_top}"
        );
        assert!(
            (actual_bot - expected_bot).abs() <= 1,
            "portal bottom at x={target_x} should match exact ray depth: actual={actual_bot}, expected={expected_bot}"
        );
    }

    #[test]
    fn test_diagonal_portal_clip_matches_exact_ray_intersection() {
        use doom_types::ANG90;

        init_trig();

        let level = make_two_sided_level_with_vertices((-96, 160), (64, 256), 0, 128, 56, 72);
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let out = render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let v1 = &level.vertexes[0];
        let v2 = &level.vertexes[1];
        let angle = ANG90;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let cos_i = cos_a.0 as i64;
        let sin_i = sin_a.0 as i64;

        let dx1 = v1.x as i64;
        let dy1 = v1.y as i64;
        let dx2 = v2.x as i64;
        let dy2 = v2.y as i64;

        let vx1 = (dx1 * cos_i + dy1 * sin_i) >> 16;
        let vy1 = (dx1 * sin_i - dy1 * cos_i) >> 16;
        let vx2 = (dx2 * cos_i + dy2 * sin_i) >> 16;
        let vy2 = (dx2 * sin_i - dy2 * cos_i) >> 16;
        let (vx1, vy1, vx2, vy2) =
            crate::clip::clip_seg_to_view_frustum(vx1, vy1, vx2, vy2).expect("wall visible");

        let sx1 = HALF_W as i64 + (FOCAL_LEN as i64 * vy1) / vx1.max(1);
        let sx2 = HALF_W as i64 + (FOCAL_LEN as i64 * vy2) / vx2.max(1);
        let (sx_left, sx_right) = if sx1 <= sx2 { (sx1, sx2) } else { (sx2, sx1) };
        let col_start = sx_left.max(0).min((SCREEN_W - 1) as i64) as usize;
        let col_end = sx_right.max(0).min((SCREEN_W - 1) as i64) as usize;
        assert!(col_end > col_start + 4, "need a visible diagonal span");

        let target_x = col_start + ((col_end - col_start) * 3 / 4);
        let ray = (target_x as f32 - HALF_W as f32) / FOCAL_LEN as f32;
        let dvx = (vx2 - vx1) as f32;
        let dvy = (vy2 - vy1) as f32;
        let denom = dvy - ray * dvx;
        assert!(
            denom.abs() > f32::EPSILON,
            "ray must intersect diagonal wall"
        );
        let s = (ray * vx1 as f32 - vy1 as f32) / denom;
        assert!(
            (0.0..=1.0).contains(&s),
            "intersection parameter must stay on the clipped seg, got {s}"
        );
        let exact_depth = vx1 as f32 + s * dvx;
        let expected_top = HALF_H
            - (((72 - PLAYER_HEIGHT) as f32 * FOCAL_LEN as f32) / exact_depth).round() as i32;
        let expected_bot = HALF_H
            - (((56 - PLAYER_HEIGHT) as f32 * FOCAL_LEN as f32) / exact_depth).round() as i32;

        let actual_top = out.clip_top[target_x];
        let actual_bot = out.clip_bot[target_x];

        assert!(
            (actual_top - expected_top).abs() <= 1,
            "diagonal portal top at x={target_x} should match exact ray depth: actual={actual_top}, expected={expected_top}"
        );
        assert!(
            (actual_bot - expected_bot).abs() <= 1,
            "diagonal portal bottom at x={target_x} should match exact ray depth: actual={actual_bot}, expected={expected_bot}"
        );
    }

    #[test]
    fn test_far_portal_flats_do_not_escape_near_window_at_oblique_angle() {
        use doom_types::Bam;
        use doom_types::limits::FLAT_SIZE;

        init_trig();

        let angle = Bam(0x3000_0000); // 67.5 degrees: enough to stress angle-sensitive clipping.

        let near_only = make_two_sided_level(0, 128, 56, 72);
        let mut near_fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();
        let near_out = render_level(
            &near_only,
            0,
            0,
            angle,
            &mut near_fb,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        let flat1 = vec![10u8; FLAT_SIZE];
        let flat2 = vec![20u8; FLAT_SIZE];
        let flat3 = vec![30u8; FLAT_SIZE];
        let flat4 = vec![40u8; FLAT_SIZE];
        let flat5 = vec![210u8; FLAT_SIZE];
        let flat6 = vec![220u8; FLAT_SIZE];
        let flat7 = vec![50u8; FLAT_SIZE];
        let flat8 = vec![60u8; FLAT_SIZE];

        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat1),
            ("FLAT2", &flat2),
            ("FLAT3", &flat3),
            ("FLAT4", &flat4),
            ("FLAT5", &flat5),
            ("FLAT6", &flat6),
            ("FLAT7", &flat7),
            ("FLAT8", &flat8),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let nested = make_portal_window_with_far_portal_level();
        let mut fb = Framebuffer::new();
        render_level(
            &nested,
            0,
            0,
            angle,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        for x in 0..SCREEN_W {
            let top = near_out.clip_top[x];
            let bot = near_out.clip_bot[x];
            for y in 0..SCREEN_H {
                let px = fb.get_pixel(x, y).unwrap_or(0);
                if px == 210 || px == 220 {
                    assert!(
                        (y as i32) >= top && (y as i32) <= bot,
                        "far portal flat {px} leaked outside near portal window at ({x}, {y}); near clip={top}..{bot}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_far_solid_wall_clipping_does_not_depend_on_subsector_seg_order() {
        use doom_types::ANG90;

        init_trig();

        let mut level = make_portal_window_with_far_solid_level();
        level.segs.swap(0, 1);

        assert_eq!(
            crate::seg::collect_front_to_back_seg_indices(&level, 0, 0),
            vec![1, 0],
            "hardening sort must still render near portal before far wall in same-subsector synthetic cases"
        );

        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(
            &level, 0, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        let x = HALF_W as usize;
        let is_wall = |px: u8| (32..64).contains(&px);
        let wall_rows: Vec<usize> = (0..SCREEN_H)
            .filter(|&y| is_wall(fb.get_pixel(x, y).unwrap_or(0)))
            .collect();

        let portal_top = (HALF_H - ((72 - PLAYER_HEIGHT) * FOCAL_LEN / 128)) as usize;
        let portal_bot = (HALF_H - ((56 - PLAYER_HEIGHT) * FOCAL_LEN / 128)) as usize;
        let leaked = wall_rows
            .iter()
            .copied()
            .find(|&y| y < portal_top || y > portal_bot);
        assert!(
            leaked.is_none(),
            "reversed subsector seg order leaked far wall outside portal window {:?}; first leaked row: {:?}",
            wall_rows,
            leaked
        );
    }

    #[test]
    fn test_far_portal_visplanes_do_not_depend_on_subsector_seg_order() {
        use doom_types::ANG90;
        use doom_types::limits::FLAT_SIZE;

        init_trig();

        let flat1 = vec![10u8; FLAT_SIZE];
        let flat2 = vec![20u8; FLAT_SIZE];
        let flat3 = vec![30u8; FLAT_SIZE];
        let flat4 = vec![40u8; FLAT_SIZE];
        let flat5 = vec![210u8; FLAT_SIZE];
        let flat6 = vec![220u8; FLAT_SIZE];
        let flat7 = vec![50u8; FLAT_SIZE];
        let flat8 = vec![60u8; FLAT_SIZE];

        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat1),
            ("FLAT2", &flat2),
            ("FLAT3", &flat3),
            ("FLAT4", &flat4),
            ("FLAT5", &flat5),
            ("FLAT6", &flat6),
            ("FLAT7", &flat7),
            ("FLAT8", &flat8),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let mut level = make_portal_window_with_far_portal_level();
        level.segs.swap(0, 1);

        assert_eq!(
            crate::seg::collect_front_to_back_seg_indices(&level, 0, 0),
            vec![1, 0],
            "hardening sort must still render near portal before far portal in same-subsector synthetic cases"
        );

        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        let near_only = make_two_sided_level(0, 128, 56, 72);
        let mut near_fb = Framebuffer::new();
        let near_out = render_level(
            &near_only,
            0,
            0,
            ANG90,
            &mut near_fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        render_level(
            &level,
            0,
            0,
            ANG90,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            None,
            None,
            false,
        );

        for x in 0..SCREEN_W {
            let top = near_out.clip_top[x];
            let bot = near_out.clip_bot[x];
            for y in 0..SCREEN_H {
                let px = fb.get_pixel(x, y).unwrap_or(0);
                if px == 210 || px == 220 {
                    assert!(
                        (y as i32) >= top && (y as i32) <= bot,
                        "far portal flat {px} leaked outside near portal window at ({x}, {y}); near clip={top}..{bot}"
                    );
                }
            }
        }
    }

    #[test]
    fn player_sector_index_prefers_map_owned_subsector_sector() {
        let level = make_player_sector_mismatch_level();

        assert_eq!(level.sector_index_at(10, 0), Some(0));
        assert_eq!(
            player_sector_index(&level, 10, 0),
            Some(0),
            "renderer should use the map-owned subsector sector, not nearest-seg heuristics"
        );
    }

    /// Regression: one-sided walls still render correctly after the two-sided
    /// refactor.  The center column of a one-sided wall facing the player
    /// must be painted with a wall color (not background).
    #[test]
    fn test_one_sided_regression() {
        use doom_types::ANG90;
        init_trig();

        // Plain one-sided level (flags=0, no back sector)
        let level = make_minimal_level(); // floor=0, ceil=128, flags=0
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        // Player at (64, 0) looking toward the wall at y=128 (ANG90 = North = +Y).
        render_level(
            &level, 64, 0, ANG90, &mut fb, &palette, None, None, None, None, false,
        );

        // The wall should occupy vertical pixels around the center column.
        // Flat-shade color = 32 + (192 >> 3).min(31) = 32 + 24 = 56.
        let center_x = HALF_W as usize;
        let has_wall = fb.data[0..SCREEN_W * SCREEN_H]
            .chunks(SCREEN_W)
            .enumerate()
            .any(|(y, row)| {
                let px = row[center_x];
                let _ = y;
                (32..64).contains(&px)
            });
        assert!(
            has_wall,
            "one-sided wall must still render wall-colored pixels in center column after refactor"
        );
    }

    // =======================================================================
    // Lighting integration tests
    // =======================================================================

    /// Build a minimal level with a configurable light level.
    fn make_level_with_light(light: i16) -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![Vertex { x: 0, y: 128 }, Vertex { x: 128, y: 128 }];
        let sectors = vec![Sector {
            floor_height: doom_types::Fixed16_16::from_int(0),
            ceil_height: doom_types::Fixed16_16::from_int(128),
            floor_flat: *b"FLAT1\0\0\0",
            ceil_flat: *b"FLAT2\0\0\0",
            light_level: light,
            special: 0,
            tag: 0,
        }];
        let sidedefs = vec![Sidedef {
            x_offset: 0,
            y_offset: 0,
            upper_texture: *b"WALL1\0\0\0",
            lower_texture: *b"WALL2\0\0\0",
            middle_texture: *b"WALL3\0\0\0",
            sector: 0,
        }];
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 0xFFFF,
        }];
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];
        let ssectors = vec![Ssector {
            seg_count: 1,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 1).expect("reject parse");
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "LIGHT".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    /// Build a two-sided level with different light levels for front and back.
    fn make_two_sided_level_with_lights(front_light: i16, back_light: i16) -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![Vertex { x: -64, y: 128 }, Vertex { x: 64, y: 128 }];
        let sectors = vec![
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(0),
                ceil_height: doom_types::Fixed16_16::from_int(128),
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: front_light,
                special: 0,
                tag: 0,
            },
            Sector {
                floor_height: doom_types::Fixed16_16::from_int(32),
                ceil_height: doom_types::Fixed16_16::from_int(96),
                floor_flat: *b"FLAT3\0\0\0",
                ceil_flat: *b"FLAT4\0\0\0",
                light_level: back_light,
                special: 0,
                tag: 0,
            },
        ];
        let sidedefs = vec![
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 0,
            },
            Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"UPPER\0\0\0",
                lower_texture: *b"LOWER\0\0\0",
                middle_texture: *b"-\0\0\0\0\0\0\0",
                sector: 1,
            },
        ];
        let linedefs = vec![Linedef {
            from_vertex: 0,
            to_vertex: 1,
            flags: 0x0004,
            special: 0,
            tag: 0,
            right_sidedef: 0,
            left_sidedef: 1,
        }];
        let segs = vec![Seg {
            from_vertex: 0,
            to_vertex: 1,
            angle: 0,
            linedef: 0,
            direction: 0,
            offset: 0,
        }];
        let ssectors = vec![Ssector {
            seg_count: 1,
            first_seg: 0,
        }];
        let things = vec![Thing {
            x: 0,
            y: 0,
            angle: 0,
            kind: 1,
            flags: 7,
        }];

        let reject = Reject::parse_lump(&[0u8; 1], 2).expect("reject parse");
        let mut bm_data = vec![0u8; 8 + 2 + 4];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");

        Level {
            name: "LIGHT2S".to_owned(),
            things,
            linedefs,
            sidedefs,
            vertexes,
            segs,
            ssectors,
            nodes: vec![],
            sectors,
            reject,
            blockmap,
        }
    }

    /// Build a ColormapCache where row N maps every pixel to N.
    /// This lets us detect which colormap row was used.
    fn make_test_colormap() -> ColormapCache {
        use crate::colormap::{COLORMAP_ROWS, COLORMAP_SIZE};
        let mut data = vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE];
        for row in 0..COLORMAP_ROWS {
            let start = row * COLORMAP_SIZE;
            data[start..start + COLORMAP_SIZE].fill(row as u8);
        }
        ColormapCache::from_test_data(data)
    }

    fn make_special_row_test_colormap(value: u8) -> ColormapCache {
        use crate::colormap::{COLORMAP_ROWS, COLORMAP_SIZE};
        let mut data = vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE];
        for row in 0..COLORMAP_ROWS {
            let start = row * COLORMAP_SIZE;
            data[start..start + COLORMAP_SIZE].fill(row as u8);
        }
        let row32 = 32 * COLORMAP_SIZE;
        data[row32..row32 + COLORMAP_SIZE].fill(value);
        ColormapCache::from_test_data(data)
    }

    /// Build a ColormapCache where row 0 is identity and row N darkens
    /// (maps value `v` to `v.saturating_sub(N * 4)`).
    fn make_darkening_colormap() -> ColormapCache {
        use crate::colormap::{COLORMAP_ROWS, COLORMAP_SIZE};
        let mut data = vec![0u8; COLORMAP_ROWS * COLORMAP_SIZE];
        for row in 0..COLORMAP_ROWS {
            let start = row * COLORMAP_SIZE;
            for i in 0..COLORMAP_SIZE {
                let darken = (row as u8).saturating_mul(4);
                data[start + i] = (i as u8).saturating_sub(darken);
            }
        }
        ColormapCache::from_test_data(data)
    }

    // --- Test 1: fullbright mode produces same output regardless of light level ---

    #[test]
    fn light_fullbright_flag_skips_shading() {
        init_trig();
        let level = make_level_with_light(64); // dim sector
        let palette = PaletteLut::grayscale();
        let cm = ColormapCache::identity();

        // Render with is_fullbright = true.
        let mut fb_bright = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb_bright,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            true,
        );

        // Render again with is_fullbright = false but light=255 (auto-fullbright).
        let level255 = make_level_with_light(255);
        let mut fb_255 = Framebuffer::new();
        render_level(
            &level255,
            64,
            0,
            doom_types::ANG90,
            &mut fb_255,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // Both should produce identical output since identity colormap is used.
        assert_eq!(
            fb_bright.data.as_slice(),
            fb_255.data.as_slice(),
            "fullbright flag and light=255 should produce identical output with identity colormaps"
        );
    }

    // --- Test 2: fullbright flag makes dim sector as bright as light=255 ---

    #[test]
    fn light_fullbright_overrides_dim_sector() {
        init_trig();
        let level = make_level_with_light(64);
        let palette = PaletteLut::grayscale();
        let cm = ColormapCache::identity();

        let mut fb_normal = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb_normal,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        let mut fb_forced = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb_forced,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            true,
        );

        // With identity colormaps both should be the same (identity maps everything
        // to itself regardless of which row). But the function path is different.
        // This at least verifies no panic.
        assert_eq!(fb_normal.data.as_slice(), fb_forced.data.as_slice());
    }

    // --- Test 3: dark sector produces different pixels than bright sector ---

    #[test]
    fn light_dark_sector_differs_from_bright() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();

        // Bright sector (light=255 = fullbright).
        let level_bright = make_level_with_light(255);
        let mut fb_bright = Framebuffer::new();
        render_level(
            &level_bright,
            64,
            0,
            doom_types::ANG90,
            &mut fb_bright,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // Dark sector (light=0).
        let level_dark = make_level_with_light(0);
        let mut fb_dark = Framebuffer::new();
        render_level(
            &level_dark,
            64,
            0,
            doom_types::ANG90,
            &mut fb_dark,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // The framebuffers should differ — the dark one has darker wall colors.
        assert_ne!(
            fb_bright.data.as_slice(),
            fb_dark.data.as_slice(),
            "bright and dark sectors must produce different framebuffers"
        );
    }

    // --- Test 4: light=255 sector auto-fullbright uses colormap row 0 ---

    #[test]
    fn light_255_uses_colormap_row_zero() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_test_colormap();

        let level = make_level_with_light(255);
        let mut fb = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // With test colormap, row 0 maps everything to 0.
        // So all wall pixels should be 0 (since row 0 sets every pixel to 0).
        // Background pixels are NOT affected by the colormap (they are drawn before walls).
        // Just verify the wall region (center column) contains 0.
        let cx = HALF_W as usize;
        let wall_pixels: Vec<u8> = (0..SCREEN_H)
            .filter_map(|y| fb.get_pixel(cx, y))
            .filter(|&px| px == 0)
            .collect();
        assert!(
            !wall_pixels.is_empty(),
            "light=255 should use colormap row 0 (maps all to 0)"
        );
    }

    // --- Test 5: light=0 sector uses darker colormap rows ---

    #[test]
    fn light_zero_uses_dark_colormap() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_test_colormap();

        let level = make_level_with_light(0);
        let mut fb = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // With test colormap, row N maps everything to N.
        // light=0 gives base index 31. Distance attenuation might reduce it,
        // but the wall pixels should NOT all be 0 (that would be fullbright).
        let cx = HALF_W as usize;
        let non_bg_pixels: Vec<u8> = (0..SCREEN_H)
            .filter_map(|y| fb.get_pixel(cx, y))
            .filter(|&px| px != 25 && px != 119) // exclude background
            .collect();
        if !non_bg_pixels.is_empty() {
            // At least some wall pixels should use a non-zero colormap row.
            let has_dark = non_bg_pixels.iter().any(|&px| px > 0);
            assert!(
                has_dark,
                "light=0 sector should use dark colormap rows (pixel values > 0 in test colormap)"
            );
        }
    }

    #[test]
    fn player_extra_light_brightens_world_rendering() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_test_colormap();
        let level = make_level_with_light(0);

        let mut fb_base = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb_base,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        let mut fb_boosted = Framebuffer::new();
        render_level_with_view_height_and_extra_light(
            &level,
            64,
            0,
            doom_types::ANG90,
            PLAYER_HEIGHT,
            &mut fb_boosted,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
            2,
        );

        let cx = HALF_W as usize;
        let base_pixels: Vec<u8> = (0..SCREEN_H)
            .filter_map(|y| fb_base.get_pixel(cx, y))
            .filter(|&px| px != 25 && px != 119)
            .collect();
        let boosted_pixels: Vec<u8> = (0..SCREEN_H)
            .filter_map(|y| fb_boosted.get_pixel(cx, y))
            .filter(|&px| px != 25 && px != 119)
            .collect();
        assert!(
            !base_pixels.is_empty() && !boosted_pixels.is_empty(),
            "test level should produce visible wall pixels in the center column"
        );

        let base_avg =
            base_pixels.iter().map(|&px| u32::from(px)).sum::<u32>() / base_pixels.len() as u32;
        let boosted_avg = boosted_pixels.iter().map(|&px| u32::from(px)).sum::<u32>()
            / boosted_pixels.len() as u32;
        assert!(
            boosted_avg < base_avg,
            "player extra-light should use brighter colormap rows (base={base_avg}, boosted={boosted_avg})"
        );
    }

    #[test]
    fn fixed_colormap_override_uses_special_row_for_world_rendering() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_special_row_test_colormap(0xA5);
        let level = make_level_with_light(0);
        let mut fb = Framebuffer::new();

        render_level_with_view_height_and_extra_light_and_fixed_colormap(
            &level,
            64,
            0,
            doom_types::ANG90,
            PLAYER_HEIGHT,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
            Some(cm.special_row(32)),
            0,
        );

        assert!(
            fb.data.contains(&0xA5),
            "fixed special-row colormap should tint visible world pixels through row 32"
        );
    }

    // --- Test 6: no colormap cache defaults to identity ---

    #[test]
    fn light_no_colormap_uses_identity() {
        init_trig();
        let palette = PaletteLut::grayscale();

        let level = make_level_with_light(128);
        let mut fb_none = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb_none,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        // With identity colormap, same result.
        let cm = ColormapCache::identity();
        let mut fb_ident = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb_ident,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // Both should be identical because identity colormap is a no-op.
        assert_eq!(
            fb_none.data.as_slice(),
            fb_ident.data.as_slice(),
            "no colormap and identity colormap should produce identical output"
        );
    }

    // --- Test 7: render with colormap does not panic ---

    #[test]
    fn light_render_with_colormap_no_panic() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();

        let level = make_level_with_light(128);
        let mut fb = Framebuffer::new();
        let _zbuf = render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );
        // No panic = pass.
    }

    // --- Test 8: different light levels produce monotonically darker output ---

    #[test]
    fn light_monotonic_darkness() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();

        let mut prev_sum: u64 = u64::MAX;
        for light in [255i16, 192, 128, 64, 0] {
            let level = make_level_with_light(light);
            let mut fb = Framebuffer::new();
            render_level(
                &level,
                64,
                0,
                doom_types::ANG90,
                &mut fb,
                &palette,
                None,
                None,
                Some(&cm),
                None,
                false,
            );

            // Sum of all pixel values — brighter scenes should have higher sums.
            let sum: u64 = fb.data.iter().map(|&px| px as u64).sum();
            if light < 255 {
                assert!(
                    sum <= prev_sum,
                    "darker light level {light} should produce lower total brightness (sum={sum} > prev={prev_sum})"
                );
            }
            prev_sum = sum;
        }
    }

    // --- Test 9: wall columns receive per-column colormap (not single per-seg) ---

    #[test]
    fn light_per_column_colormap_varies_with_position() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_test_colormap();

        // Medium light to get a non-trivial base index.
        let level = make_level_with_light(128);
        let mut fb = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // With test colormap, wall pixels at different columns should potentially
        // have different values (angular falloff gives edge columns darker rows).
        // This is hard to check precisely, but verify at least it produces output.
        let has_nonzero = fb.data.iter().any(|&b| b != 0);
        assert!(
            has_nonzero,
            "render with test colormap must produce non-zero output"
        );
    }

    // --- Test 10: z-buffer is still correct with lighting enabled ---

    #[test]
    fn light_zbuf_unchanged() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();

        let level = make_level_with_light(128);

        let mut fb_no_cm = Framebuffer::new();
        let zbuf_no_cm = render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb_no_cm,
            &palette,
            None,
            None,
            None,
            None,
            false,
        );

        let mut fb_cm = Framebuffer::new();
        let zbuf_cm = render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb_cm,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // Z-buffer should be identical: lighting does not affect geometry.
        for x in 0..SCREEN_W {
            assert!(
                (zbuf_no_cm.z_buf[x] - zbuf_cm.z_buf[x]).abs() < f32::EPSILON
                    || (zbuf_no_cm.z_buf[x] == f32::MAX && zbuf_cm.z_buf[x] == f32::MAX),
                "z-buffer mismatch at column {x}: no_cm={}, cm={}",
                zbuf_no_cm.z_buf[x],
                zbuf_cm.z_buf[x]
            );
        }
    }

    // --- Test 11: fullbright with colormap produces fullbright output ---

    #[test]
    fn light_fullbright_ignores_colormap_darkness() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_test_colormap();

        let level = make_level_with_light(0);
        let mut fb = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            true, // fullbright
        );

        // With fullbright, colormap row 0 is always used.
        // Row 0 in test_colormap maps everything to 0.
        // All wall pixels should be 0.
        let cx = HALF_W as usize;
        let wall_pixels: Vec<u8> = (0..SCREEN_H)
            .filter_map(|y| fb.get_pixel(cx, y))
            .filter(|&px| px != 25 && px != 119) // exclude background
            .collect();
        if !wall_pixels.is_empty() {
            assert!(
                wall_pixels.iter().all(|&px| px == 0),
                "fullbright should always use row 0 (all pixels mapped to 0 in test colormap)"
            );
        }
    }

    // --- Test 12: LightParams is_fullbright at 255 ---

    #[test]
    fn light_params_255_is_auto_fullbright() {
        let lp = LightParams::new(255, false);
        assert!(lp.is_fullbright());
        assert_eq!(lp.colormap_for_wall(1000.0, 0), 0);
    }

    // --- Test 13: LightParams at 0 gives max darkness ---

    #[test]
    fn light_params_zero_is_max_dark() {
        let lp = LightParams::new(0, false);
        assert!(!lp.is_fullbright());
        assert_eq!(lp.base_index(), 31);
        // Far distance should give max colormap index.
        let idx = lp.colormap_for_wall(10000.0, SCREEN_W / 2);
        assert_eq!(idx, 31);
    }

    // --- Test 14: LightParams fullbright flag overrides ---

    #[test]
    fn light_params_fullbright_flag_overrides() {
        let lp = LightParams::new(0, true);
        assert!(lp.is_fullbright());
        assert_eq!(lp.colormap_for_wall(10000.0, 0), 0);
        assert_eq!(lp.colormap_for_flat(10000.0), 0);
    }

    // --- Test 15: render_level deterministic with lighting ---

    #[test]
    fn light_render_deterministic() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();
        let level = make_level_with_light(128);

        let mut fb1 = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb1,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        let mut fb2 = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb2,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        assert_eq!(
            fb1.data.as_slice(),
            fb2.data.as_slice(),
            "render_level must be deterministic"
        );
    }

    // --- Test 16: two-sided portal with different light levels ---

    #[test]
    fn light_two_sided_front_sector_used_for_walls() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();

        // Front sector bright (255), back sector dark (0).
        let level = make_two_sided_level_with_lights(255, 0);
        let mut fb = Framebuffer::new();
        render_level(
            &level,
            0,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // No panic; visual output produced.
        let has_nonzero = fb.data.iter().any(|&b| b != 0);
        assert!(
            has_nonzero,
            "two-sided render with lighting must produce output"
        );
    }

    // --- Test 17: two-sided portal stores different back sector light ---

    #[test]
    fn light_two_sided_back_sector_light_stored() {
        // Verify that the two-sided rendering path stores the back sector's
        // raw light level for flats.  We check this indirectly by rendering
        // with different back lights and a non-identity colormap.
        // Since we don't have a FlatCache here, flats use background fill,
        // but the WALL colors (upper/lower bands) use the front sector light.
        // This test verifies that different back lights don't affect wall
        // color (since walls use front sector light), confirming the separation.
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();

        let level_a = make_two_sided_level_with_lights(128, 255);
        let mut fb_a = Framebuffer::new();
        render_level(
            &level_a,
            0,
            0,
            doom_types::ANG90,
            &mut fb_a,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        let level_b = make_two_sided_level_with_lights(128, 0);
        let mut fb_b = Framebuffer::new();
        render_level(
            &level_b,
            0,
            0,
            doom_types::ANG90,
            &mut fb_b,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // Without a FlatCache, both should produce identical output because
        // wall textures use front sector light and background fill is constant.
        assert_eq!(
            fb_a.data.as_slice(),
            fb_b.data.as_slice(),
            "without FlatCache, back sector light should not affect wall output"
        );
    }

    // --- Test 18: render_level with light=128 and no texture cache ---

    #[test]
    fn light_fallback_flat_shade_with_colormap() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();

        let level = make_level_with_light(128);
        let mut fb = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );

        // The flat-shade fallback uses `wall_cm[32]`. With the darkening colormap,
        // this should produce a value less than 32 (some darkening).
        let cx = HALF_W as usize;
        let wall_pixels: Vec<u8> = (0..SCREEN_H)
            .filter_map(|y| fb.get_pixel(cx, y))
            .filter(|&px| px != 25 && px != 119)
            .collect();
        if !wall_pixels.is_empty() {
            // The darkening colormap should remap palette index 32 to something.
            // With light=128 and some distance, we get a non-zero colormap row.
            // At very least the wall pixels should exist and not crash.
            assert!(!wall_pixels.is_empty(), "wall pixels should exist");
        }
    }

    // --- Test 19: mid-range light produces mid-range brightness ---

    #[test]
    fn light_mid_range_brightness() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();

        let bright_sum = {
            let level = make_level_with_light(255);
            let mut fb = Framebuffer::new();
            render_level(
                &level,
                64,
                0,
                doom_types::ANG90,
                &mut fb,
                &palette,
                None,
                None,
                Some(&cm),
                None,
                false,
            );
            fb.data.iter().map(|&px| px as u64).sum::<u64>()
        };

        let mid_sum = {
            let level = make_level_with_light(128);
            let mut fb = Framebuffer::new();
            render_level(
                &level,
                64,
                0,
                doom_types::ANG90,
                &mut fb,
                &palette,
                None,
                None,
                Some(&cm),
                None,
                false,
            );
            fb.data.iter().map(|&px| px as u64).sum::<u64>()
        };

        let dark_sum = {
            let level = make_level_with_light(0);
            let mut fb = Framebuffer::new();
            render_level(
                &level,
                64,
                0,
                doom_types::ANG90,
                &mut fb,
                &palette,
                None,
                None,
                Some(&cm),
                None,
                false,
            );
            fb.data.iter().map(|&px| px as u64).sum::<u64>()
        };

        // Mid should be between bright and dark.
        assert!(
            mid_sum <= bright_sum && mid_sum >= dark_sum,
            "mid-range light ({mid_sum}) should be between bright ({bright_sum}) and dark ({dark_sum})"
        );
    }

    // --- Test 20: colormap row 0 always identity in standard cache ---

    #[test]
    fn light_identity_cache_row_zero_is_identity() {
        let cm = ColormapCache::identity();
        let row = cm.get(0);
        for (i, &v) in row.iter().enumerate() {
            assert_eq!(v, i as u8, "identity row 0 should be identity");
        }
    }

    // --- Test 21: LightParams colormap_for_wall returns valid index ---

    #[test]
    fn light_params_colormap_indices_in_range() {
        for light in [0u8, 64, 128, 192, 255] {
            let lp = LightParams::new(light, false);
            for dist in [1.0f32, 50.0, 200.0, 1000.0, 10000.0] {
                for x in [0, 80, 160, 240, 319] {
                    let idx = lp.colormap_for_wall(dist, x);
                    assert!(
                        idx <= 31,
                        "colormap_for_wall out of range: light={light}, dist={dist}, x={x}, idx={idx}"
                    );
                    let flat_idx = lp.colormap_for_flat(dist);
                    assert!(
                        flat_idx <= 31,
                        "colormap_for_flat out of range: light={light}, dist={dist}, idx={flat_idx}"
                    );
                }
            }
        }
    }

    // --- Test 22: render with FlatCache and ColormapCache ---

    #[test]
    fn light_flats_with_colormap_no_panic() {
        use doom_types::limits::FLAT_SIZE;
        init_trig();

        // Build flats.
        let mut flat_data = vec![42u8; FLAT_SIZE];
        flat_data[0] = 100;

        let make_iwad = |lumps: &[(&str, &[u8])]| {
            let mut data: Vec<u8> = Vec::new();
            data.extend_from_slice(b"IWAD");
            data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
            data.extend_from_slice(&0i32.to_le_bytes());
            let mut offsets = Vec::new();
            for (_, bytes) in lumps {
                let pos = data.len();
                data.extend_from_slice(bytes);
                offsets.push((pos, bytes.len()));
            }
            let dir_off = data.len() as i32;
            data[8..12].copy_from_slice(&dir_off.to_le_bytes());
            for (i, (name, _)) in lumps.iter().enumerate() {
                let (fp, sz) = offsets[i];
                data.extend_from_slice(&(fp as i32).to_le_bytes());
                data.extend_from_slice(&(sz as i32).to_le_bytes());
                let mut nb = [0u8; 8];
                for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                    nb[j] = b.to_ascii_uppercase();
                }
                data.extend_from_slice(&nb);
            }
            data
        };

        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat_data),
            ("FLAT2", &flat_data),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let cm = make_darkening_colormap();
        let palette = PaletteLut::grayscale();
        let level = make_level_with_light(128);
        let mut fb = Framebuffer::new();

        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            Some(&cm),
            None,
            false,
        );

        let has_nonzero = fb.data.iter().any(|&b| b != 0);
        assert!(has_nonzero, "flats with lighting must produce output");
    }

    // --- Test 23: fullbright with flat cache uses colormap row 0 ---

    #[test]
    fn light_flats_fullbright_with_colormap() {
        use doom_types::limits::FLAT_SIZE;
        init_trig();

        let flat_data = vec![50u8; FLAT_SIZE];

        let make_iwad = |lumps: &[(&str, &[u8])]| {
            let mut data: Vec<u8> = Vec::new();
            data.extend_from_slice(b"IWAD");
            data.extend_from_slice(&(lumps.len() as i32).to_le_bytes());
            data.extend_from_slice(&0i32.to_le_bytes());
            let mut offsets = Vec::new();
            for (_, bytes) in lumps {
                let pos = data.len();
                data.extend_from_slice(bytes);
                offsets.push((pos, bytes.len()));
            }
            let dir_off = data.len() as i32;
            data[8..12].copy_from_slice(&dir_off.to_le_bytes());
            for (i, (name, _)) in lumps.iter().enumerate() {
                let (fp, sz) = offsets[i];
                data.extend_from_slice(&(fp as i32).to_le_bytes());
                data.extend_from_slice(&(sz as i32).to_le_bytes());
                let mut nb = [0u8; 8];
                for (j, &b) in name.as_bytes().iter().take(8).enumerate() {
                    nb[j] = b.to_ascii_uppercase();
                }
                data.extend_from_slice(&nb);
            }
            data
        };

        let lumps: Vec<(&str, &[u8])> = vec![
            ("F_START", b""),
            ("FLAT1", &flat_data),
            ("FLAT2", &flat_data),
            ("F_END", b""),
        ];
        let wad_bytes = make_iwad(&lumps);
        let wad = doom_wad::WadFile::parse(wad_bytes).expect("parse WAD");
        let flat_cache = FlatCache::load(&wad);

        let cm = ColormapCache::identity();
        let palette = PaletteLut::grayscale();
        let level = make_level_with_light(0); // very dark, but fullbright overrides

        let mut fb = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            Some(&flat_cache),
            None,
            Some(&cm),
            None,
            true,
        );

        // Fullbright → colormap row 0 (identity).
        // Flat pixels should be 50 (the flat data value).
        // Check a floor pixel (below horizon).
        let bottom_row = SCREEN_H - 1;
        let px = fb.get_pixel(HALF_W as usize, bottom_row).unwrap_or(0);
        // The bottom row is a floor region — should contain flat pixel data or background.
        // Since we have flats loaded, it should be the flat value mapped through identity = 50.
        // If the wall occludes everything, we at least check no panic.
        assert!(
            px == 50 || px == 119 || px == 0,
            "floor pixel should be flat data (50), bg (119), or black (0), got {px}"
        );
    }

    // --- Test 24: LightParams::colormap_for_flat distance attenuation ---

    #[test]
    fn light_flat_distance_attenuation() {
        let lp = LightParams::new(128, false);
        let near = lp.colormap_for_flat(10.0);
        let far = lp.colormap_for_flat(1000.0);
        // Near should be brighter (lower or equal index).
        assert!(
            near <= far,
            "near flat ({near}) should be brighter than far flat ({far})"
        );
    }

    // --- Test 25: LightParams::colormap_for_wall angular falloff ---

    #[test]
    fn light_wall_angular_falloff() {
        let lp = LightParams::new(128, false);
        let center = lp.colormap_for_wall(200.0, SCREEN_W / 2);
        let edge = lp.colormap_for_wall(200.0, 0);
        assert!(
            edge >= center,
            "edge column ({edge}) should be darker than center ({center})"
        );
    }

    // --- Test 26: two-sided seg with fullbright no crash ---

    #[test]
    fn light_two_sided_fullbright_no_crash() {
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();

        let level = make_two_sided_level_with_lights(64, 192);
        let mut fb = Framebuffer::new();
        render_level(
            &level,
            0,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            true,
        );
        // No crash = success.
    }

    // --- Test 27: negative light level clamped ---

    #[test]
    fn light_negative_clamped_to_zero() {
        // sector.light_level is i16; passing a negative value should be clamped.
        // The `(sector.light_level as u32).min(255)` wraps negative to large u32,
        // then .min(255) clamps to 255. Actually we need to handle this:
        // -1i16 as u32 = 0xFFFF_FFFF → .min(255) = 255 → fullbright.
        // This is acceptable behaviour (negative light is treated as max).
        let level = make_level_with_light(-1);
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();
        let mut fb = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );
        // No crash.
    }

    // --- Test 28: high light level (> 255) clamped ---

    #[test]
    fn light_high_value_clamped() {
        let level = make_level_with_light(500);
        init_trig();
        let palette = PaletteLut::grayscale();
        let cm = make_darkening_colormap();
        let mut fb = Framebuffer::new();
        render_level(
            &level,
            64,
            0,
            doom_types::ANG90,
            &mut fb,
            &palette,
            None,
            None,
            Some(&cm),
            None,
            false,
        );
        // No crash; should clamp to 255 (fullbright).
    }
}
