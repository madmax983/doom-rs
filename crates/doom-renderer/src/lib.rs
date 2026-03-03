//! Software renderer: palette-indexed 320x200 framebuffer, column/span drawing,
//! BSP traversal, sprite compositing, fuzz effect, palette flash.
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
pub mod fuzz;
pub mod palette;
pub mod palette_flash;
pub mod render;
pub mod render_flags;
pub mod seg;
pub mod sky;
pub mod span;
pub mod sprite;
pub mod statusbar;
pub mod texture;
pub mod visplane;

pub use anim::{AnimSequence, AnimState, AnimType, SwitchList};
pub use automap::{AutomapState, draw_automap, draw_automap_ex, line_color};
pub use colormap::{ColormapCache, INVULN_COLORMAP, build_invuln_colormap};
pub use column::{DrawColumnParams, IDENTITY_COLORMAP, draw_column, draw_column_solid};
pub use flat_cache::FlatCache;
pub use framebuffer::Framebuffer;
pub use fuzz::{FUZZ_TABLE, draw_fuzz_column};
pub use palette::{PaletteLut, Rgb};
pub use palette_flash::PaletteFlash;
pub use render::render_level;
pub use render_flags::RenderFlag;
pub use sky::{
    SKY_FALLBACK_COLOR, SKY_FLAT_NAME, column_to_angle, draw_sky_columns, draw_sky_fallback,
    is_sky_flat, sky_texel_column, sky_texture_for_episode, sky_texture_name,
};
pub use span::{DrawSpanParams, FLAT_DIM, FLAT_MASK, draw_span, draw_span_solid};
pub use sprite::{
    SpriteCache, SpriteFrame, compute_sprite_rotation, draw_sprite, draw_sprite_ex,
    draw_weapon_sprite, render_things, sprite_lump_name, thing_has_rotations, thing_sprite_prefix,
};
pub use statusbar::{StatusBarData, draw_status_bar, draw_status_bar_data};
pub use texture::TextureCache;
