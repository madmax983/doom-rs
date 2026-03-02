//! Software renderer: palette-indexed 320×200 framebuffer, column/span drawing,
//! BSP traversal, sprite compositing.
//!
//! Hot path — no Verus proofs in this crate.
//! `Framebuffer` and `PaletteLut` are the primary types consumed by `doom-tui`.

pub mod anim;
pub mod automap;
pub mod clip;
pub mod colormap;
pub mod column;
pub mod flat_cache;
pub mod framebuffer;
pub mod palette;
pub mod render;
pub mod seg;
pub mod sky;
pub mod span;
pub mod sprite;
pub mod statusbar;
pub mod texture;
pub mod visplane;

pub use anim::{AnimSequence, AnimState};
pub use automap::draw_automap;
pub use colormap::ColormapCache;
pub use column::{DrawColumnParams, IDENTITY_COLORMAP, draw_column, draw_column_solid};
pub use flat_cache::FlatCache;
pub use framebuffer::Framebuffer;
pub use palette::{PaletteLut, Rgb};
pub use render::render_level;
pub use sky::{SKY_FLAT_NAME, column_to_angle, draw_sky_columns, is_sky_flat, sky_texture_name};
pub use span::{DrawSpanParams, FLAT_DIM, FLAT_MASK, draw_span, draw_span_solid};
pub use sprite::{SpriteCache, SpriteFrame, draw_sprite, draw_weapon_sprite, render_things};
pub use statusbar::draw_status_bar;
pub use texture::TextureCache;
