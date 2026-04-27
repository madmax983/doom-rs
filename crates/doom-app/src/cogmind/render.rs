//! Frame compositor for cogmind-mode rendering.
//!
//! [`CogmindState`] holds the cached tile grid and per-sector visibility map.
//! Each frame, [`CogmindState::render_frame`] composites tiles and entities
//! into a [`CogmindFrame`] centered on the player.

use doom_game::GameState;
use doom_map::Level;
use doom_tui::{CogmindCell, CogmindFrame};

use super::effects::EffectLayer;
use super::glyphs::{TileKind, apply_light, dim_remembered, entity_glyph, tile_glyph, wall_glyph};
use super::lighting::{blend_hazard_glow, effective_light, entity_light_boost};
use super::sight_line::sight_line_cells;
use super::tile_grid::{CELL_SIZE, TileGrid};
use super::visibility::{SectorVisibility, VisibilityMap};

// ---------------------------------------------------------------------------
// CogmindState
// ---------------------------------------------------------------------------

/// Persistent state for cogmind-mode rendering across frames.
///
/// Caches the tile grid and visibility map for the current level, rebuilding
/// them only when the level changes.
#[doc(alias = "cogmind")]
#[doc(alias = "roguelike")]
pub(crate) struct CogmindState {
    /// Tile grid for the current level (lazily built).
    #[doc(hidden)]
    pub(crate) tile_grid: Option<TileGrid>,
    /// Per-sector visibility map (lazily built).
    #[doc(hidden)]
    pub(crate) visibility: Option<VisibilityMap>,
    /// Name of the level the cached grid was built for.
    cached_level_name: String,
    /// Particle effect layer (combat debris, projectile trails, dust).
    #[doc(hidden)]
    pub(crate) effects: EffectLayer,
}

impl CogmindState {
    /// Create a new state with no cached data.
    ///
    /// The actual grid and visibility maps remain empty until the first time
    /// [`CogmindState::ensure_grid`] is called with a loaded `Level`.
    ///
    /// ## Examples
    ///
    /// ```
    /// use doom_app::cogmind::CogmindState;
    ///
    /// let state = CogmindState::new();
    /// ```
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            tile_grid: None,
            visibility: None,
            cached_level_name: String::new(),
            effects: EffectLayer::new(),
        }
    }

    /// Ensure the tile grid and visibility map are built for the given level.
    ///
    /// Because BSP-to-grid rasterization is expensive, `CogmindState` caches the
    /// results. If the provided `Level::name` matches the cached one, this is a no-op.
    /// Otherwise, the previous grid is discarded and a new one is built from scratch.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use doom_app::cogmind::CogmindState;
    /// # use doom_map::{Level, Blockmap, Reject};
    /// # let mut bm_data = vec![0u8; 14];
    /// # bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
    /// # bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
    /// # bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
    /// # bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
    /// # bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
    /// # let blockmap = Blockmap::parse_lump(&bm_data).unwrap();
    /// # let reject = Reject::parse_lump(&[0u8], 0).unwrap();
    /// # let level = Level {
    /// #     name: "E1M1".to_string(),
    /// #     things: vec![], linedefs: vec![], sidedefs: vec![], vertexes: vec![],
    /// #     segs: vec![], ssectors: vec![], nodes: vec![], sectors: vec![],
    /// #     reject, blockmap,
    /// # };
    /// let mut state = CogmindState::new();
    ///
    /// // First call builds the grid:
    /// state.ensure_grid(&level);
    ///
    /// // Second call with the same level is instant:
    /// state.ensure_grid(&level);
    /// ```
    pub(crate) fn ensure_grid(&mut self, level: &Level) {
        if self.tile_grid.is_some() && self.cached_level_name == level.name {
            return;
        }
        self.tile_grid = Some(TileGrid::from_level(level));
        self.visibility = Some(VisibilityMap::new(level.sectors.len()));
        self.cached_level_name = level.name.clone();
        self.effects = EffectLayer::new();
    }

    /// Update the visibility map based on the player's current sector.
    ///
    /// Uses the level's REJECT table to determine which sectors are visible
    /// from the player's sector. Visible sectors are fully lit based on their
    /// `light_level`. Previously seen sectors are "remembered" and dimmed (Fog of War).
    ///
    /// ## Edge Cases
    ///
    /// Does nothing if [`CogmindState::ensure_grid`] has not been called yet.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use doom_app::cogmind::CogmindState;
    /// # use doom_map::{Level, Blockmap, Reject};
    /// # let mut bm_data = vec![0u8; 14];
    /// # bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
    /// # bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
    /// # bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
    /// # bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
    /// # bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
    /// # let blockmap = Blockmap::parse_lump(&bm_data).unwrap();
    /// # let reject = Reject::parse_lump(&[0u8], 0).unwrap();
    /// # let level = Level {
    /// #     name: "E1M1".to_string(),
    /// #     things: vec![], linedefs: vec![], sidedefs: vec![], vertexes: vec![],
    /// #     segs: vec![], ssectors: vec![], nodes: vec![], sectors: vec![],
    /// #     reject, blockmap,
    /// # };
    /// let mut state = CogmindState::new();
    /// state.ensure_grid(&level);
    ///
    /// // Mark sectors visible from sector index 0
    /// state.update_visibility(0, &level);
    /// ```
    pub(crate) fn update_visibility(&mut self, player_sector: usize, level: &Level) {
        let Some(vis) = self.visibility.as_mut() else {
            return;
        };
        vis.update(
            player_sector,
            level.sectors.len(),
            |a, b| level.reject.visible(a, b),
            |i| level.sectors[i].light_level.clamp(0, 255) as u8,
        );
    }

    /// Composite tiles, visibility, and entities into a [`CogmindFrame`].
    ///
    /// The viewport is automatically centered on the player. Each terminal cell
    /// maps to a `CELL_SIZE x CELL_SIZE` region of map space. The resulting frame
    /// can be directly drawn to the screen by a TUI driver.
    ///
    /// ## Returns
    ///
    /// A [`CogmindFrame`] populated with styled ASCII cells. If [`CogmindState::ensure_grid`]
    /// hasn't been called, returns an empty (blank) frame of the requested dimensions.
    ///
    /// ## Examples
    ///
    /// ```
    /// # use doom_app::cogmind::CogmindState;
    /// # use doom_map::{Level, Blockmap, Reject};
    /// # use doom_game::GameState;
    /// # let mut bm_data = vec![0u8; 14];
    /// # bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
    /// # bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
    /// # bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
    /// # bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
    /// # bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
    /// # let blockmap = Blockmap::parse_lump(&bm_data).unwrap();
    /// # let reject = Reject::parse_lump(&[0u8], 0).unwrap();
    /// # let level = Level {
    /// #     name: "E1M1".to_string(),
    /// #     things: vec![], linedefs: vec![], sidedefs: vec![], vertexes: vec![],
    /// #     segs: vec![], ssectors: vec![], nodes: vec![], sectors: vec![],
    /// #     reject, blockmap,
    /// # };
    /// # let gs = GameState::new("E1M1");
    /// let mut state = CogmindState::new();
    /// state.ensure_grid(&level);
    ///
    /// // Render a frame for a typical 80x24 terminal
    /// let frame = state.render_frame(&gs, &level, 80, 24);
    /// assert_eq!(frame.width(), 80);
    /// assert_eq!(frame.height(), 24);
    /// ```
    pub(crate) fn render_frame(
        &mut self,
        gs: &GameState,
        level: &Level,
        term_w: u16,
        term_h: u16,
    ) -> CogmindFrame {
        let mut frame = CogmindFrame::new(term_w, term_h);

        let grid = match self.tile_grid.as_ref() {
            Some(g) => g,
            None => return frame,
        };
        let vis = match self.visibility.as_ref() {
            Some(v) => v,
            None => return frame,
        };

        // --- Locate the player ---
        let Some(player_mobj) = gs.mobjslab.get(gs.player.handle) else {
            return frame;
        };
        let player_x = player_mobj.x.to_int();
        let player_y = player_mobj.y.to_int();

        // Viewport origin in map units.  The player is centered in the terminal.
        let tw = i32::from(term_w);
        let th = i32::from(term_h);
        let vp_origin_x = player_x - (tw / 2) * CELL_SIZE;
        let vp_origin_y = player_y - (th / 2) * CELL_SIZE;

        // Tick effects and spawn new particles.
        self.effects.tick();
        self.effects.spawn_combat_debris(gs);
        self.effects.spawn_projectile_trails(gs);

        let tic = gs.tic_num;
        let player_tx = tw / 2;
        let player_ty = th / 2;
        let mut visible_floors: Vec<(i32, i32)> =
            Vec::with_capacity((term_w as usize) * (term_h as usize));

        // --- Phase 1: tile rendering ---
        for ty in 0..term_h {
            for tx in 0..term_w {
                let map_x = vp_origin_x + i32::from(tx) * CELL_SIZE + CELL_SIZE / 2;
                let map_y = vp_origin_y + i32::from(ty) * CELL_SIZE + CELL_SIZE / 2;

                let (gx, gy) = grid.map_to_grid(map_x, map_y);
                if gx < 0 || gy < 0 {
                    // Off the grid => void cell (already default black).
                    continue;
                }

                let tile = match grid.get(gx as usize, gy as usize) {
                    Some(t) => t,
                    None => continue,
                };

                // Determine effective tile kind (doors may have opened at runtime).
                let effective_kind = if tile.kind == TileKind::DoorClosed {
                    // Check live sector heights: if ceiling > floor, door is open.
                    if let Some(si) = tile.sector_idx {
                        if let Some(sector) = level.sectors.get(si) {
                            if sector.ceil_height > sector.floor_height {
                                TileKind::DoorOpen
                            } else {
                                TileKind::DoorClosed
                            }
                        } else {
                            tile.kind
                        }
                    } else {
                        tile.kind
                    }
                } else {
                    tile.kind
                };

                // Determine glyph.
                let tg = if effective_kind == TileKind::Wall {
                    let neighbors = grid.wall_neighbors(gx as usize, gy as usize);
                    let ch = wall_glyph(neighbors);
                    let base = tile_glyph(TileKind::Wall);
                    super::glyphs::TileGlyph {
                        glyph: ch,
                        fg: base.fg,
                        bg: base.bg,
                    }
                } else {
                    tile_glyph(effective_kind)
                };

                // Apply visibility/lighting.
                let sector_idx = match tile.sector_idx {
                    Some(si) => si,
                    None => {
                        // No sector => treat as void.
                        continue;
                    }
                };

                let cell = match vis.get(sector_idx) {
                    SectorVisibility::Visible(light) => {
                        let sector_special = level.sectors.get(sector_idx).map_or(0, |s| s.special);
                        let eff_light = effective_light(
                            light,
                            sector_special,
                            sector_idx,
                            tic,
                            player_tx,
                            player_ty,
                            i32::from(tx),
                            i32::from(ty),
                        );
                        let mut fg = apply_light(tg.fg, eff_light);
                        let mut bg = apply_light(tg.bg, eff_light);
                        // Apply hazard glow if present on this tile.
                        if let Some(glow) = tile.glow {
                            fg = blend_hazard_glow(fg, glow);
                            bg = blend_hazard_glow(bg, glow);
                        }
                        // Track visible floor positions for ambient dust spawning.
                        if matches!(effective_kind, TileKind::Floor | TileKind::DoorOpen) {
                            visible_floors.push((map_x, map_y));
                        }
                        CogmindCell {
                            glyph: tg.glyph,
                            fg,
                            bg,
                        }
                    }
                    SectorVisibility::Remembered(_light) => {
                        let fg = dim_remembered(tg.fg);
                        let bg = dim_remembered(tg.bg);
                        CogmindCell {
                            glyph: tg.glyph,
                            fg,
                            bg,
                        }
                    }
                    SectorVisibility::Unexplored => {
                        // Leave as default black.
                        continue;
                    }
                };

                // Terminal Y is flipped vs Doom Y (Doom Y+ is north/up,
                // terminal row 0 is top).  Flip so north is screen-top.
                let screen_y = term_h.saturating_sub(1).saturating_sub(ty);
                frame.set(tx, screen_y, cell);
            }
        }

        // --- Phase 2: entity overlay ---
        for handle in gs.mobjslab.iter_handles() {
            let Some(mobj) = gs.mobjslab.get(handle) else {
                continue;
            };

            let mx = mobj.x.to_int();
            let my = mobj.y.to_int();

            // Map position -> terminal cell.
            let tx = (mx - vp_origin_x) / CELL_SIZE;
            let ty = (my - vp_origin_y) / CELL_SIZE;

            if tx < 0 || ty < 0 || tx >= tw || ty >= th {
                continue;
            }

            // Check visibility of the entity's sector.
            if let Some(sector_idx) = level.sector_index_at(mx, my) {
                match vis.get(sector_idx) {
                    SectorVisibility::Visible(light) => {
                        let eg = entity_glyph(mobj.kind, mobj.health);
                        let boosted = entity_light_boost(light);
                        let fg = apply_light(eg.fg, boosted);
                        let bg = apply_light(eg.bg, boosted);
                        // Flip Y for terminal coordinates.
                        let screen_y = (term_h.saturating_sub(1)).saturating_sub(ty as u16);
                        frame.set(
                            tx as u16,
                            screen_y,
                            CogmindCell {
                                glyph: eg.glyph,
                                fg,
                                bg,
                            },
                        );
                    }
                    SectorVisibility::Remembered(_) | SectorVisibility::Unexplored => {
                        // Don't render entities in non-visible sectors.
                    }
                }
            }
        }

        // --- Phase 2.5: effect particles ---
        for effect in &self.effects.effects {
            let etx = (effect.x - vp_origin_x) / CELL_SIZE;
            let ety = (effect.y - vp_origin_y) / CELL_SIZE;
            if etx < 0 || ety < 0 || etx >= tw || ety >= th {
                continue;
            }
            // Don't overwrite the player glyph.
            if etx == player_tx && ety == player_ty {
                continue;
            }
            let screen_y = (term_h.saturating_sub(1)).saturating_sub(ety as u16);
            let fg = effect.current_fg();
            frame.set(
                etx as u16,
                screen_y,
                CogmindCell {
                    glyph: effect.glyph,
                    fg,
                    bg: (0, 0, 0),
                },
            );
        }

        // --- Phase 3: player sight line ---
        let player_screen_x = player_tx;
        let player_screen_y = (term_h.saturating_sub(1)).saturating_sub(player_ty as u16);

        let sight_cells = sight_line_cells(player_mobj.angle, |dx, dy| {
            let check_tx = player_tx + dx;
            let check_ty = player_ty + dy;
            if check_tx < 0 || check_ty < 0 || check_tx >= tw || check_ty >= th {
                return true; // out of bounds = treat as wall
            }
            let map_check_x = vp_origin_x + check_tx * CELL_SIZE + CELL_SIZE / 2;
            let map_check_y = vp_origin_y + check_ty * CELL_SIZE + CELL_SIZE / 2;
            let (gx, gy) = grid.map_to_grid(map_check_x, map_check_y);
            if gx < 0 || gy < 0 {
                return true;
            }
            grid.get(gx as usize, gy as usize)
                .is_none_or(|t| t.kind == TileKind::Wall)
        });

        for sc in &sight_cells {
            let sx = player_screen_x + sc.dx;
            let sy = i32::from(player_screen_y) + sc.dy;
            if sx >= 0 && sy >= 0 && sx < tw && sy < i32::from(term_h) {
                frame.set(
                    sx as u16,
                    sy as u16,
                    CogmindCell {
                        glyph: sc.glyph,
                        fg: sc.fg,
                        bg: (0, 0, 0),
                    },
                );
            }
        }

        // Spawn ambient dust using this frame's visible floors (for next frame).
        self.effects.spawn_ambient_dust(&visible_floors);
        self.effects.clean_stale_handles(gs);

        frame
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use doom_game::mobj::flags;
    use doom_game::player::PlayerState;
    use doom_game::{GameState, Mobj};
    use doom_map::{Blockmap, Reject, Sector, Seg, Sidedef, Ssector};
    use doom_types::mobj_kind::MobjKind;
    use doom_types::{Bam, Fixed16_16};

    /// Build a minimal level with valid BSP data (0 nodes, 1 ssector, 1 seg).
    fn make_bsp_test_level() -> Level {
        let mut bm_data = vec![0u8; 14];
        bm_data[4..6].copy_from_slice(&1u16.to_le_bytes());
        bm_data[6..8].copy_from_slice(&1u16.to_le_bytes());
        bm_data[8..10].copy_from_slice(&5u16.to_le_bytes());
        bm_data[10..12].copy_from_slice(&0x0000u16.to_le_bytes());
        bm_data[12..14].copy_from_slice(&0xFFFFu16.to_le_bytes());
        let blockmap = Blockmap::parse_lump(&bm_data).expect("blockmap parse");
        let reject = Reject::parse_lump(&[0u8], 1).expect("reject parse");

        Level {
            name: "TEST".to_owned(),
            things: vec![],
            linedefs: vec![doom_map::Linedef {
                from_vertex: 0,
                to_vertex: 1,
                flags: 0,
                special: 0,
                tag: 0,
                right_sidedef: 0,
                left_sidedef: 0xFFFF,
            }],
            sidedefs: vec![Sidedef {
                x_offset: 0,
                y_offset: 0,
                upper_texture: *b"\0\0\0\0\0\0\0\0",
                lower_texture: *b"\0\0\0\0\0\0\0\0",
                middle_texture: *b"\0\0\0\0\0\0\0\0",
                sector: 0,
            }],
            vertexes: vec![
                doom_map::Vertex { x: 0, y: 0 },
                doom_map::Vertex { x: 128, y: 0 },
            ],
            segs: vec![Seg {
                from_vertex: 0,
                to_vertex: 1,
                angle: 0,
                linedef: 0,
                direction: 0,
                offset: 0,
            }],
            ssectors: vec![Ssector {
                seg_count: 1,
                first_seg: 0,
            }],
            nodes: vec![],
            sectors: vec![Sector {
                floor_height: 0,
                ceil_height: 128,
                floor_flat: *b"FLAT1\0\0\0",
                ceil_flat: *b"FLAT2\0\0\0",
                light_level: 192,
                special: 0,
                tag: 0,
            }],
            reject,
            blockmap,
        }
    }

    /// Build a GameState with a player mobj at the origin.
    fn make_test_game_state() -> GameState {
        let mut gs = GameState::new("TEST");
        let mut mo = Mobj::new(
            MobjKind::Player,
            Fixed16_16::ZERO,
            Fixed16_16::ZERO,
            Bam::ZERO,
        );
        mo.health = 100;
        mo.flags = flags::MF_SOLID | flags::MF_SHOOTABLE;
        mo.radius = Fixed16_16::from_int(16);
        mo.height = Fixed16_16::from_int(56);
        let handle = gs.mobjslab.alloc(mo);
        gs.player = PlayerState::pistol_start(handle);
        gs
    }

    #[test]
    fn cogmind_state_new_has_no_grid() {
        let state = CogmindState::new();
        assert!(state.tile_grid.is_none());
        assert!(state.visibility.is_none());
        assert!(state.cached_level_name.is_empty());
        assert!(state.effects.effects.is_empty());
    }

    #[test]
    fn ensure_grid_builds_on_first_call() {
        let mut state = CogmindState::new();
        let level = make_bsp_test_level();
        state.ensure_grid(&level);

        assert!(state.tile_grid.is_some());
        assert!(state.visibility.is_some());
        assert_eq!(state.cached_level_name, "TEST");
    }

    #[test]
    fn ensure_grid_caches_for_same_level() {
        let mut state = CogmindState::new();
        let level = make_bsp_test_level();

        state.ensure_grid(&level);
        let grid_ptr =
            state.tile_grid.as_ref().expect("value must exist in test") as *const TileGrid;

        // Second call with same level name should not rebuild.
        state.ensure_grid(&level);
        let grid_ptr2 =
            state.tile_grid.as_ref().expect("value must exist in test") as *const TileGrid;
        assert_eq!(grid_ptr, grid_ptr2, "grid should be cached, not rebuilt");
    }

    #[test]
    fn render_frame_returns_correct_dimensions() {
        let mut state = CogmindState::new();
        let gs = make_test_game_state();
        let level = make_bsp_test_level();

        // Even without a grid, render_frame should return a frame with correct dims.
        let frame = state.render_frame(&gs, &level, 80, 24);
        assert_eq!(frame.width(), 80);
        assert_eq!(frame.height(), 24);
    }

    #[test]
    fn render_frame_with_grid_returns_correct_dimensions() {
        let mut state = CogmindState::new();
        let gs = make_test_game_state();
        let level = make_bsp_test_level();

        state.ensure_grid(&level);
        state.update_visibility(0, &level);

        let frame = state.render_frame(&gs, &level, 60, 20);
        assert_eq!(frame.width(), 60);
        assert_eq!(frame.height(), 20);
    }
}
