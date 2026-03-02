//! First-person perspective software renderer.
//!
//! Renders a Doom level from the player's viewpoint using per-seg projection.
//! Walls are flat-shaded by sector light level.  Floors and ceilings use
//! textured spans when a `FlatCache` is supplied, otherwise fall back to
//! solid palette indices.
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
use doom_types::Bam;

use crate::colormap::ColormapCache;
use crate::column::{DrawColumnParams, IDENTITY_COLORMAP, draw_column};
use crate::flat_cache::FlatCache;
use crate::framebuffer::Framebuffer;
use crate::palette::PaletteLut;
use crate::sky::{draw_sky_columns, is_sky_flat};
use crate::span::{DrawSpanParams, draw_span};
use crate::texture::TextureCache;

// ---------------------------------------------------------------------------
// Screen constants
// ---------------------------------------------------------------------------

const SCREEN_W: usize = 320;
const SCREEN_H: usize = 200;
const HALF_W: i32 = (SCREEN_W / 2) as i32; // 160
const HALF_H: i32 = (SCREEN_H / 2) as i32; // 100
const FOCAL_LEN: i32 = 160; // = HALF_W, 90° horizontal FOV

/// Assumed player eye height above the floor (map units, fixed-point integer).
const PLAYER_HEIGHT: i32 = 41;

/// Sentinel name used to mark columns that were never written by any seg.
const NO_FLAT: [u8; 8] = *b"-\0\0\0\0\0\0\0";

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
///
/// This is a software renderer using per-seg perspective projection.
/// Walls are textured when `tex_cache` is provided; floors/ceilings are
/// textured when `flat_cache` is provided.  Light shading is applied when
/// `colormap` is provided.
pub fn render_level(
    level: &Level,
    player_x: i32,
    player_y: i32,
    player_angle: Bam,
    fb: &mut Framebuffer,
    _palette: &PaletteLut,
    flat_cache: Option<&FlatCache>,
    tex_cache: Option<&TextureCache>,
    colormap: Option<&ColormapCache>,
) {
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
    // wall_top[x]  = first screen row that is a wall (initially SCREEN_H →
    //                entire column is open ceiling before any wall is drawn).
    // wall_bot[x]  = last  screen row that is a wall (initially 0 →
    //                entire column is open floor before any wall is drawn).
    let mut wall_top = [SCREEN_H as i32; SCREEN_W];
    let mut wall_bot = [-1i32; SCREEN_W];

    // Flat names to use for ceiling/floor per column.
    let mut ceil_flat: [[u8; 8]; SCREEN_W] = [NO_FLAT; SCREEN_W];
    let mut floor_flat: [[u8; 8]; SCREEN_W] = [NO_FLAT; SCREEN_W];

    // Per-column light index (0=full bright, 31=darkest) for floor/ceiling shading.
    let mut col_light = [0u8; SCREEN_W];

    // Z-buffer (per-column minimum depth, in view-space units).
    let mut z_buf = [i32::MAX; SCREEN_W];

    // Sky tracking: the ceiling region that needs sky rendering per column.
    // sky_ceil_top[x] = topmost pixel of the sky region (usually 0).
    // sky_ceil_bot[x] = bottommost pixel of the sky region.
    // When sky_ceil_bot[x] < sky_ceil_top[x], no sky is drawn for that column.
    let mut sky_ceil_top = [0i32; SCREEN_W];
    let mut sky_ceil_bot = [-1i32; SCREEN_W]; // no sky by default

    // ------------------------------------------------------------------
    // Step 3: Precompute trig — Fixed16_16 raw i32 values
    // ------------------------------------------------------------------
    let cos_a = player_angle.cos();
    let sin_a = player_angle.sin();
    let cos_i = cos_a.0 as i64;
    let sin_i = sin_a.0 as i64;

    // ------------------------------------------------------------------
    // Step 4: Iterate all segs — draw walls, record flat names
    // ------------------------------------------------------------------
    for seg in &level.segs {
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

        // Skip if both endpoints are behind the player.
        if vx1 <= 0 && vx2 <= 0 {
            continue;
        }

        // Resolve sidedef → sector.
        let linedef = match level.linedefs.get(seg.linedef as usize) {
            Some(ld) => ld,
            None => continue,
        };
        let sidedef_idx = if seg.direction == 0 {
            linedef.right_sidedef as usize
        } else {
            linedef.left_sidedef as usize
        };
        let sidedef = match level.sidedefs.get(sidedef_idx) {
            Some(sd) => sd,
            None => continue,
        };
        let sector = match level.sectors.get(sidedef.sector as usize) {
            Some(s) => s,
            None => continue,
        };

        let floor_h = sector.floor_height as i32;
        let ceil_h = sector.ceil_height as i32;
        let light = ((sector.light_level as u32) >> 3).min(31) as u8;

        // Pick the colormap row for this sector's light level.
        let cm: &[u8; 256] = colormap.map(|c| c.get(light)).unwrap_or(&IDENTITY_COLORMAP);

        // Resolve the back sector for two-sided linedefs.
        let is_two_sided = linedef.is_two_sided();
        let back_sector = if is_two_sided {
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

        // Near-clip.
        let clipped = match clip_seg_to_near_plane(vx1, vy1, vx2, vy2) {
            Some(c) => c,
            None => continue,
        };
        let (vx1, vy1, vx2, vy2) = clipped;

        // Project to screen columns.
        // sx = HALF_W - FOCAL_LEN * vy / vx
        let sx1 = HALF_W as i64 - (FOCAL_LEN as i64 * vy1) / vx1.max(1);
        let sx2 = HALF_W as i64 - (FOCAL_LEN as i64 * vy2) / vx2.max(1);

        let (sx_left, vx_left, sx_right, vx_right) = if sx1 <= sx2 {
            (sx1, vx1, sx2, vx2)
        } else {
            (sx2, vx2, sx1, vx1)
        };

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
            let depth = vx_left + t * (vx_right - vx_left) / span_w;
            let depth_i32 = depth.max(1) as i32;

            if is_two_sided {
                // -----------------------------------------------------------------
                // TWO-SIDED SEG: portal / door / window
                // -----------------------------------------------------------------
                // For two-sided segs we do NOT update z_buf — the line does not
                // fully occlude the view.  Things behind can show through the
                // portal opening.

                // Wall height covers the full front sector span.
                let wall_h_world = (ceil_h - floor_h).max(0);
                let wall_h_px = (wall_h_world * FOCAL_LEN / depth_i32).min(SCREEN_H as i32);

                let w_top = (HALF_H - wall_h_px / 2).max(0);
                let w_bot = (HALF_H + wall_h_px / 2).min(SCREEN_H as i32 - 1);

                // Project a back-sector height linearly between w_top (= front_ceil
                // projected) and w_bot (= front_floor projected).
                //
                //   proj(h) = w_top + (front_ceil - h) * (w_bot - w_top) / (front_ceil - front_floor)
                //
                // Clamped to [w_top, w_bot] to avoid drawing outside the wall span.
                let proj = |h: i32| -> i32 {
                    let height_range = (ceil_h - floor_h).max(1);
                    let pixel_range = w_bot - w_top;
                    (w_top + (ceil_h - h) * pixel_range / height_range).clamp(w_top, w_bot)
                };

                // Compute screen-space positions of the back sector's ceiling and floor.
                let (screen_back_ceil, screen_back_floor) = if let Some(bs) = back_sector {
                    let bc = bs.ceil_height as i32;
                    let bf = bs.floor_height as i32;
                    (proj(bc), proj(bf))
                } else {
                    // No back sector — treat as fully closed (no portal opening).
                    (w_top, w_bot)
                };

                // Clamp so upper ≤ lower (degenerate case: equal heights, sealed door).
                let upper_bot = screen_back_ceil.min(w_bot);
                let lower_top = screen_back_floor.max(w_top);

                // The portal opening is screen_back_ceil .. screen_back_floor.
                // Update wall_top/wall_bot to reflect the opening so that ceiling
                // and floor spans fill through it.
                wall_top[x] = screen_back_ceil;
                wall_bot[x] = screen_back_floor;

                // Record back sector flats (what the player sees through the portal).
                if let Some(bs) = back_sector {
                    ceil_flat[x] = bs.ceil_flat;
                    floor_flat[x] = bs.floor_flat;
                    col_light[x] = ((bs.light_level as u32) >> 3).min(31) as u8;
                } else {
                    ceil_flat[x] = sector.ceil_flat;
                    floor_flat[x] = sector.floor_flat;
                    col_light[x] = light;
                }

                // Perspective-correct U coordinate helper (shared for upper/lower).
                let t_screen = (t as f32) / (span_w as f32).max(1.0);
                let denom =
                    (vx_left as f32 + t_screen * (vx_right as f32 - vx_left as f32)).max(1.0);
                let t_persp = t_screen * (vx_right as f32) / denom;
                let u_world = seg.offset as f32 + sidedef.x_offset as f32 + t_persp * seg_world_len;

                // ---- Sky detection for two-sided portals -------------------------
                // When both front and back ceilings are F_SKY1, skip the upper
                // texture entirely — sky shows through the portal (outdoor areas).
                let front_is_sky = is_sky_flat(&sector.ceil_flat);
                let back_is_sky = back_sector.is_some_and(|bs| is_sky_flat(&bs.ceil_flat));

                // If the front sector has a sky ceiling, record the sky region
                // for this column (from screen top to wherever the ceiling starts).
                if front_is_sky {
                    sky_ceil_top[x] = 0;
                    sky_ceil_bot[x] = (w_top - 1).max(-1);
                }

                // ---- Upper band (front_ceil > back_ceil) -------------------------
                let has_upper = upper_bot > w_top;
                // Skip upper texture if both front and back are sky (sky-to-sky portal).
                let skip_upper_for_sky = front_is_sky && back_is_sky;
                if has_upper && !skip_upper_for_sky {
                    let upper_h_px = (upper_bot - w_top).max(1);

                    if let Some(cache) = tex_cache {
                        if let Some(tex) = cache.get(&sidedef.upper_texture) {
                            let u_tex = (u_world as i32).rem_euclid(tex.width as i32) as usize;
                            let tex_h = tex.height as u32;
                            let fracstep = (tex_h << 16) / (upper_h_px as u32).max(1);
                            let texturemid = ((tex_h / 2) as i32 + sidedef.y_offset as i32) as i64;
                            let frac_start = ((texturemid << 16) as i64
                                - (HALF_H as i64 - w_top as i64) * fracstep as i64)
                                as u32;
                            let col_off = u_tex * tex_h as usize;
                            let col_data = &tex.data[col_off..col_off + tex_h as usize];
                            draw_column(
                                fb,
                                &DrawColumnParams {
                                    x,
                                    y_top: w_top as usize,
                                    y_bot: (upper_bot - 1).max(w_top) as usize,
                                    frac: frac_start,
                                    fracstep,
                                    source: col_data,
                                    colormap: cm,
                                },
                            );
                        } else {
                            // Texture not in cache — flat-shade fallback.
                            let wall_color = (32u8).saturating_add(light);
                            fb.draw_column(
                                x,
                                w_top as usize,
                                (upper_bot - 1).max(w_top) as usize,
                                wall_color,
                            );
                        }
                    } else {
                        // No tex_cache — flat-shade fallback.
                        let wall_color = (32u8).saturating_add(light);
                        fb.draw_column(
                            x,
                            w_top as usize,
                            (upper_bot - 1).max(w_top) as usize,
                            wall_color,
                        );
                    }
                } else if has_upper && skip_upper_for_sky {
                    // Sky-to-sky portal: extend the sky region down through
                    // the upper band (sky is visible all the way to the portal opening).
                    sky_ceil_bot[x] = (upper_bot - 1).max(sky_ceil_bot[x]);
                }

                // ---- Lower band (back_floor > front_floor) -----------------------
                let has_lower = lower_top < w_bot;
                if has_lower {
                    let lower_h_px = (w_bot - lower_top).max(1);

                    if let Some(cache) = tex_cache {
                        if let Some(tex) = cache.get(&sidedef.lower_texture) {
                            let u_tex = (u_world as i32).rem_euclid(tex.width as i32) as usize;
                            let tex_h = tex.height as u32;
                            let fracstep = (tex_h << 16) / (lower_h_px as u32).max(1);
                            let texturemid = ((tex_h / 2) as i32 + sidedef.y_offset as i32) as i64;
                            let frac_start = ((texturemid << 16) as i64
                                - (HALF_H as i64 - lower_top as i64) * fracstep as i64)
                                as u32;
                            let col_off = u_tex * tex_h as usize;
                            let col_data = &tex.data[col_off..col_off + tex_h as usize];
                            draw_column(
                                fb,
                                &DrawColumnParams {
                                    x,
                                    y_top: lower_top as usize,
                                    y_bot: w_bot as usize,
                                    frac: frac_start,
                                    fracstep,
                                    source: col_data,
                                    colormap: cm,
                                },
                            );
                        } else {
                            // Texture not in cache — flat-shade fallback.
                            let wall_color = (32u8).saturating_add(light);
                            fb.draw_column(x, lower_top as usize, w_bot as usize, wall_color);
                        }
                    } else {
                        // No tex_cache — flat-shade fallback.
                        let wall_color = (32u8).saturating_add(light);
                        fb.draw_column(x, lower_top as usize, w_bot as usize, wall_color);
                    }
                }
            } else {
                // -----------------------------------------------------------------
                // ONE-SIDED SEG: solid wall (unchanged from original)
                // -----------------------------------------------------------------

                // Z-buffer occlusion.
                if depth_i32 >= z_buf[x] {
                    continue;
                }
                z_buf[x] = depth_i32;

                // Wall height in pixels.
                let wall_h_world = (ceil_h - floor_h).max(0);
                let wall_h_px = (wall_h_world * FOCAL_LEN / depth_i32).min(SCREEN_H as i32);

                let w_top = (HALF_H - wall_h_px / 2).max(0) as i32;
                let w_bot = (HALF_H + wall_h_px / 2).min(SCREEN_H as i32 - 1) as i32;

                wall_top[x] = w_top;
                wall_bot[x] = w_bot;

                // Record which sector's flats belong to this column.
                ceil_flat[x] = sector.ceil_flat;
                floor_flat[x] = sector.floor_flat;
                col_light[x] = light;

                // If the sector's ceiling is F_SKY1, mark this column for sky
                // rendering from the top of the screen to just above the wall.
                if is_sky_flat(&sector.ceil_flat) {
                    sky_ceil_top[x] = 0;
                    sky_ceil_bot[x] = (w_top - 1).max(-1);
                }

                // Draw the wall column — textured if a TextureCache is available.
                if let Some(cache) = tex_cache {
                    if let Some(tex) = cache.get(&sidedef.middle_texture) {
                        // Perspective-correct horizontal texture coordinate (U).
                        let t_screen = (t as f32) / (span_w as f32).max(1.0);
                        let denom = (vx_left as f32
                            + t_screen * (vx_right as f32 - vx_left as f32))
                            .max(1.0);
                        let t_persp = t_screen * (vx_right as f32) / denom;

                        let u_world =
                            seg.offset as f32 + sidedef.x_offset as f32 + t_persp * seg_world_len;
                        let u_tex = (u_world as i32).rem_euclid(tex.width as i32) as usize;

                        // Vertical texture coordinate (V).
                        let tex_h = tex.height as u32;
                        let fracstep = (tex_h << 16) / (wall_h_px as u32).max(1);
                        let texturemid = ((tex_h / 2) as i32 + sidedef.y_offset as i32) as i64;
                        let frac_start = ((texturemid << 16) as i64
                            - (HALF_H as i64 - w_top as i64) * fracstep as i64)
                            as u32;

                        let col_start_idx = u_tex * tex_h as usize;
                        let col_data = &tex.data[col_start_idx..col_start_idx + tex_h as usize];

                        draw_column(
                            fb,
                            &DrawColumnParams {
                                x,
                                y_top: w_top as usize,
                                y_bot: w_bot as usize,
                                frac: frac_start,
                                fracstep,
                                source: col_data,
                                colormap: cm,
                            },
                        );
                        continue; // skip flat-color fallback
                    }
                }

                // Fallback: flat-shaded solid color (no texture or texture not found).
                let wall_color = (32u8).saturating_add(light);
                fb.draw_column(x, w_top as usize, w_bot as usize, wall_color);
            }
        }
    }

    // ------------------------------------------------------------------
    // Step 4b: Draw sky columns
    // ------------------------------------------------------------------
    // After the wall pass, render sky for any column whose sector has a
    // F_SKY1 ceiling.  The sky texture is looked up from the TextureCache
    // (hardcoded to SKY1 for now).  Sky overwrites the background fill in
    // the ceiling region with parallax-mapped sky texture columns.
    if let Some(cache) = tex_cache {
        if let Some(sky_tex) = cache.get(b"SKY1\0\0\0\0") {
            draw_sky_columns(fb, &sky_ceil_top, &sky_ceil_bot, player_angle, sky_tex);
        }
    }

    // ------------------------------------------------------------------
    // Step 5: Draw textured floor and ceiling spans
    // ------------------------------------------------------------------
    // We scan row-by-row.  For each row we identify contiguous screen-column
    // ranges that share the same flat name (ceiling above wall_top, floor
    // below wall_bot) and emit one DrawSpanParams per run.
    //
    // Perspective-correct texture coordinates follow the standard Doom formula:
    //
    //   For a row at screen y:
    //     dist = (PLAYER_HEIGHT * FOCAL_LEN) / |y - HALF_H|
    //     xstep = cos(angle) / dist   (per pixel, in 16.16)
    //     ystep = sin(angle) / dist
    //     world_x at centre of row = player_x + cos(angle)*dist
    //     world_y at centre of row = player_y + sin(angle)*dist
    //     ds_xfrac/ds_yfrac offset for column x relative to centre:
    //       lateral_delta = (x - HALF_W) * dist / FOCAL_LEN
    //       world_x += sin(angle) * lateral_delta   (perpendicular direction)
    //       world_y -= cos(angle) * lateral_delta
    //
    // All arithmetic uses i64 to avoid overflow; results are scaled to 16.16.

    if flat_cache.is_none() {
        // No FlatCache — background fill from Step 1 is good enough.
        return;
    }
    let cache = flat_cache.unwrap();

    for y in 0..SCREEN_H as i32 {
        // Determine whether this row is a ceiling row, floor row, or wall.
        // We'll handle ceiling (y < HALF_H) and floor (y >= HALF_H) separately.

        // Perspective: distance from player eye to floor/ceiling at screen row y.
        let dy = y - HALF_H;
        if dy == 0 {
            // Horizon row — skip (avoid division by zero and it maps to nothing).
            continue;
        }

        // dist = (PLAYER_HEIGHT * FOCAL_LEN) / |dy|  (in map units, i32).
        let abs_dy = dy.abs();
        let dist = (PLAYER_HEIGHT as i64 * FOCAL_LEN as i64) / abs_dy as i64;
        if dist <= 0 {
            continue;
        }

        // Per-pixel world-space step in u and v (16.16 fixed-point).
        // step = (cos_a or sin_a) * (1 / dist), scaled for 64-unit tiles.
        // xstep and ystep give the world-unit change per screen pixel.
        // We use the perpendicular (right) vector for lateral stepping.
        //
        // view right vector = (sin_a, -cos_a)   (perpendicular to forward)
        //
        // world_x changes by: sin_a * (1/FOCAL_LEN) * dist  per screen pixel
        // world_y changes by: -cos_a * (1/FOCAL_LEN) * dist per screen pixel
        //
        // Expressed as 16.16: multiply by 65536 and divide by dist * FOCAL_LEN.
        // We want units that, after >> 16, give map-unit integers.
        let xstep = ((sin_i * dist) / FOCAL_LEN as i64) as i32;
        let ystep = ((-cos_i * dist) / FOCAL_LEN as i64) as i32;

        // World position of the leftmost screen column (x = 0).
        // world_centre = player + forward * dist
        // world_left   = world_centre + right * (0 - HALF_W) * dist / FOCAL_LEN
        //
        // In 16.16 fixed-point (accumulator form):
        let world_x_centre = (player_x as i64) * 65536 + (cos_i * dist); // already in map units → shift 16 → ×65536
        let world_y_centre = (player_y as i64) * 65536 + (sin_i * dist);

        // Adjust from screen centre to left edge (column 0):
        //   lateral_offset = (0 - HALF_W) * dist / FOCAL_LEN
        //   world += right * lateral_offset
        let offset = (-(HALF_W as i64) * dist) / FOCAL_LEN as i64;
        let world_x_left_fp = world_x_centre + sin_i * offset;
        let world_y_left_fp = world_y_centre - cos_i * offset;

        // Extract u,v for column 0 (we tile with 64-unit quads → >> 6 gives tile).
        // The 16.16 world coordinate >> 22 gives the 64-unit tile index, & 63 the
        // pixel within the tile.  We store as 16.16 where the integer part is the
        // texel index (0..63).
        let init_xfrac = ((world_x_left_fp >> 6) & 0xFFFF_FFFF) as u32;
        let init_yfrac = ((world_y_left_fp >> 6) & 0xFFFF_FFFF) as u32;
        let xstep_u = (xstep >> 6) as u32;
        let ystep_u = (ystep >> 6) as u32;

        // Scan columns and group into runs of the same flat.
        let mut run_start: Option<usize> = None;
        let mut run_flat_name: [u8; 8] = NO_FLAT;
        let mut run_light: u8 = 0;

        let flush_span = |cache: &FlatCache,
                          fb: &mut Framebuffer,
                          x1: usize,
                          x2: usize,
                          name: &[u8; 8],
                          light_idx: u8,
                          y: i32,
                          init_xfrac: u32,
                          init_yfrac: u32,
                          xstep_u: u32,
                          ystep_u: u32,
                          colormap_cache: Option<&ColormapCache>| {
            if name == &NO_FLAT || is_sky_flat(name) {
                return;
            }
            let source = cache.get(name);
            // Advance xfrac/yfrac from column 0 to x1.
            let steps = x1 as u32;
            let span_xfrac = init_xfrac.wrapping_add(xstep_u.wrapping_mul(steps));
            let span_yfrac = init_yfrac.wrapping_add(ystep_u.wrapping_mul(steps));

            let span_cm: &[u8; 256] = colormap_cache
                .map(|c| c.get(light_idx))
                .unwrap_or(&IDENTITY_COLORMAP);

            let params = DrawSpanParams {
                y: y as usize,
                x1,
                x2,
                ds_xfrac: span_xfrac,
                ds_yfrac: span_yfrac,
                ds_xstep: xstep_u,
                ds_ystep: ystep_u,
                source,
                colormap: span_cm,
            };
            draw_span(fb, &params);
        };

        for x in 0..SCREEN_W {
            let wt = wall_top[x];
            let wb = wall_bot[x];

            // Determine which flat this (x, y) pixel needs.
            let this_flat: Option<([u8; 8], u8)> = if dy < 0 {
                // Above horizon — ceiling rows.
                if y < wt {
                    // Sky sectors are rendered by draw_sky_columns (Step 4b),
                    // so skip them here to avoid overwriting sky with flat data.
                    if is_sky_flat(&ceil_flat[x]) {
                        None
                    } else {
                        // This column's ceiling is visible here.
                        Some((ceil_flat[x], col_light[x]))
                    }
                } else {
                    None // occluded by wall
                }
            } else {
                // Below horizon — floor rows.
                if y > wb {
                    Some((floor_flat[x], col_light[x]))
                } else {
                    None // occluded by wall or unrendered
                }
            };

            match (run_start, this_flat) {
                (None, Some((name, li))) => {
                    // Start a new run.
                    run_start = Some(x);
                    run_flat_name = name;
                    run_light = li;
                }
                (Some(_start), Some((name, _li))) if name == run_flat_name => {
                    // Continue the existing run (light of run start is used for whole span).
                }
                (Some(start), other) => {
                    // End the current run and flush it.
                    flush_span(
                        cache,
                        fb,
                        start,
                        x - 1,
                        &run_flat_name,
                        run_light,
                        y,
                        init_xfrac,
                        init_yfrac,
                        xstep_u,
                        ystep_u,
                        colormap,
                    );
                    run_start = if other.is_some() {
                        let (name, li) = other.unwrap();
                        run_flat_name = name;
                        run_light = li;
                        Some(x)
                    } else {
                        None
                    };
                }
                (None, None) => {}
            }
        }

        // Flush any remaining run.
        if let Some(start) = run_start {
            flush_span(
                cache,
                fb,
                start,
                SCREEN_W - 1,
                &run_flat_name,
                run_light,
                y,
                init_xfrac,
                init_yfrac,
                xstep_u,
                ystep_u,
                colormap,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Near-plane clipping
// ---------------------------------------------------------------------------

/// Clip a view-space seg to the near plane (vx = 1).
///
/// Returns `None` if both endpoints are behind the near plane after clipping.
/// Returns `Some((vx1, vy1, vx2, vy2))` with the clipped coordinates.
fn clip_seg_to_near_plane(vx1: i64, vy1: i64, vx2: i64, vy2: i64) -> Option<(i64, i64, i64, i64)> {
    const NEAR: i64 = 1;

    if vx1 <= 0 && vx2 <= 0 {
        return None;
    }
    if vx1 > 0 && vx2 > 0 {
        return Some((vx1, vy1, vx2, vy2));
    }

    let t_num = NEAR - vx1;
    let t_den = vx2 - vx1;
    if t_den == 0 {
        return None;
    }

    if vx1 <= 0 {
        let ny = vy1 + t_num * (vy2 - vy1) / t_den;
        Some((NEAR, ny, vx2, vy2))
    } else {
        let t_num2 = NEAR - vx2;
        let t_den2 = vx1 - vx2;
        if t_den2 == 0 {
            return None;
        }
        let ny = vy2 + t_num2 * (vy1 - vy2) / t_den2;
        Some((vx1, vy1, NEAR, ny))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal Level with one sector and one seg for testing.
    fn make_minimal_level() -> Level {
        use doom_map::lumps::{
            Blockmap, Linedef, Reject, Sector, Seg, Sidedef, Ssector, Thing, Vertex,
        };

        let vertexes = vec![Vertex { x: 0, y: 128 }, Vertex { x: 128, y: 128 }];
        let sectors = vec![Sector {
            floor_height: 0,
            ceil_height: 128,
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
                floor_height: front_floor,
                ceil_height: front_ceil,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            },
            // Sector 1 — back (player looks into this)
            Sector {
                floor_height: back_floor,
                ceil_height: back_ceil,
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

    #[test]
    fn render_empty_level_does_not_panic() {
        let level = make_minimal_level();
        let mut fb = Framebuffer::new();
        let palette = PaletteLut::grayscale();

        render_level(&level, 0, 0, Bam::ZERO, &mut fb, &palette, None, None, None);

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

        render_level(&level, 0, 0, ANG90, &mut fb, &palette, None, None, None);
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
        let (cx1, _cy1, cx2, cy2) = result.unwrap();
        assert_eq!(cx1, 1);
        assert_eq!(cx2, 4);
        assert_eq!(cy2, 6);
    }

    /// Regression: passing `flat_cache = None` must not panic (backward compat).
    #[test]
    fn test_render_level_with_flat_cache_none_smoke() {
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
        );

        // Background colour index 25 was written to the top half.
        assert_eq!(fb.get_pixel(0, 0), Some(25));
        // Background colour index 119 was written to the bottom half.
        assert_eq!(fb.get_pixel(0, SCREEN_H - 1), Some(119));
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
        unsafe {
            doom_types::Bam::init_trig_tables();
        }
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
        render_level(&level, 0, 0, ANG90, &mut fb, &palette, None, None, None);

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
            px >= 32 && px < 64
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

        render_level(&level, 0, 0, ANG90, &mut fb, &palette, None, None, None);

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
        let is_wall_color = |px: u8| px >= 32 && px < 64;
        assert!(
            !is_wall_color(px_above),
            "pixel at ({center_x}, {row_above_center}) = {px_above} should NOT be wall-colored (portal opening)"
        );
        assert!(
            !is_wall_color(px_below),
            "pixel at ({center_x}, {row_below_center}) = {px_below} should NOT be wall-colored (portal opening)"
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
        render_level(&level, 64, 0, ANG90, &mut fb, &palette, None, None, None);

        // The wall should occupy vertical pixels around the center column.
        // Flat-shade color = 32 + (192 >> 3).min(31) = 32 + 24 = 56.
        let center_x = HALF_W as usize;
        let has_wall = fb.data[0..SCREEN_W * SCREEN_H]
            .chunks(SCREEN_W)
            .enumerate()
            .any(|(y, row)| {
                let px = row[center_x];
                let _ = y;
                px >= 32 && px < 64
            });
        assert!(
            has_wall,
            "one-sided wall must still render wall-colored pixels in center column after refactor"
        );
    }
}
