//! Cogmind-mode rendering: top-down ASCII roguelike view of Doom maps.
//!
//! Why does a first-person shooter need a top-down roguelike view? Because
//! terminal interfaces are beautiful. "Cogmind mode" bypasses the standard
//! 3D software renderer and instead paints the raw `doom-map` structures
//! (linedefs, sectors, entities) directly onto a discrete character grid,
//! styling them with ASCII/Unicode glyphs and true-color terminal escapes.
//!
//! The renderer works in a multi-phase composition:
//! 1. **Phase 1: Environment Grid** - The [`doom_map::Level`] BSP tree is rasterized
//!    onto a grid of internal `Tile` structures.
//! 2. **Phase 2: Entity Overlay** - Active [`doom_game::Mobj`] entities are mapped
//!    onto the grid cells.
//! 3. **Phase 2.5: Particles** - Combat debris, projectile trails, and dust.
//! 4. **Phase 3: Sight Lines** - A visual cue showing the player's facing angle.
//!
//! # The `CogmindState` Facade
//!
//! All of the internal machinery (grid rasterization, Bresenham line drawing,
//! lighting calculations, and REJECT-table visibility updates) is hidden behind
//! a single facade: [`CogmindState`]. This state must persist across frames
//! to cache the expensive tile grid and visibility maps, only rebuilding them
//! when the player changes levels.
//!
//! # Example
//!
//! ```no_run
//! use doom_app::cogmind::CogmindState;
//! use doom_game::GameState;
//! use doom_map::Level;
//!
//! // 1. Initialize persistent state (do this once)
//! let mut state = CogmindState::new();
//!
//! // 2. Per-frame render loop
//! fn draw_frame(state: &mut CogmindState, gs: &GameState, level: &Level) {
//!     // Lazy-build the tile grid if the level changed
//!     state.ensure_grid(level);
//!
//!     // Update Fog of War based on the player's current sector
//!     let player_idx = level.sector_index_at(0, 0).unwrap_or(0);
//!     state.update_visibility(player_idx, level);
//!
//!     // Produce a fully-composited frame sized to fit the terminal (e.g. 80x24)
//!     let frame = state.render_frame(gs, level, 80, 24);
//!
//!     // The TUI engine will take the frame and draw it.
//! }
//! ```

pub(crate) mod effects;
pub(crate) mod glyphs;
pub(crate) mod lighting;
pub(crate) mod render;
pub(crate) mod sight_line;
pub(crate) mod tile_grid;
pub(crate) mod visibility;

pub use render::CogmindState;
