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
pub mod font;
pub mod framebuffer;
pub mod fuzz;
pub mod hud_messages;
pub mod intermission;
pub mod lighting;
pub mod menu_render;
pub mod palette;
pub mod palette_flash;
pub mod render;
pub mod render_flags;
pub mod seg;
pub mod sky;
pub mod span;
pub mod sprite;
pub mod sprite_lookup;
pub mod statusbar;
pub mod texture;
pub mod texture_compose;
pub mod visplane;
pub mod weapon_anim;
pub mod wipe;

pub use anim::{AnimSequence, AnimState, AnimType, SwitchList};
pub use automap::{
    AutomapState, RendererAutomapCanvas, automap_colors, draw_automap, draw_automap_ex,
    draw_grid_on_fb, draw_line_fb, draw_player_arrow_on_fb, line_color, map_to_screen,
    render_automap,
};
pub use colormap::{ColormapCache, INVULN_COLORMAP, build_invuln_colormap};
pub use column::{DrawColumnParams, IDENTITY_COLORMAP, draw_column, draw_column_solid};
pub use flat_cache::FlatCache;
pub use font::BitmapFont;
pub use framebuffer::Framebuffer;
pub use fuzz::{FUZZ_TABLE, draw_fuzz_column};
pub use hud_messages::{HudMessage, HudMessageQueue};
pub use intermission::{
    IntermissionPhase, IntermissionRenderer, draw_intermission, draw_intermission_text,
    draw_percentage, draw_time, format_map_name,
};
pub use lighting::{
    LightParams, compute_flat_light, compute_wall_light, compute_wall_light_with_falloff,
    light_to_colormap_index, shade_column, shade_pixel, shade_span,
};
pub use menu_render::{
    darken_framebuffer, draw_large_text, draw_menu, draw_title_screen, menu_colors,
};
pub use palette::{PaletteLut, Rgb};
pub use palette_flash::{PaletteFlash, PaletteFlashState};
pub use render::{
    MaskedColumnDraw, PLAYER_HEIGHT, RenderOut, draw_masked_columns, render_level,
    render_level_with_view_height,
};
pub use render_flags::RenderFlag;
pub use sky::{
    SKY_FALLBACK_COLOR, SKY_FLAT_NAME, column_to_angle, draw_sky_columns, draw_sky_fallback,
    is_sky_flat, sky_texel_column, sky_texture_for_episode, sky_texture_name,
};
pub use span::{DrawSpanParams, FLAT_DIM, FLAT_MASK, draw_span, draw_span_solid};
pub use sprite::{
    SpriteCache, SpriteClip, SpriteFrame, compute_sprite_rotation, draw_sprite, draw_sprite_ex,
    draw_weapon_sprite, render_actors_ex, render_actors_with_masked_ex, render_flag_for_thing,
    render_things, render_things_ex, sector_for_point, sprite_lump_name, thing_has_rotations,
    thing_sprite_prefix,
};
pub use sprite_lookup::{
    ActorRenderInfo, ResolvedSprite, compute_rotation, render_flag_from_state, resolve_sprite,
    sprite_lump_name_str, sprite_lump_name_with_mirror,
};
pub use statusbar::{StatusBarData, draw_status_bar, draw_status_bar_data};
pub use texture::TextureCache;
pub use texture_compose::{
    ComposedTexture, PatchDef, PatchImage, PatchPost, TextureDef, TextureDirectory,
    compose_texture, parse_patch, parse_pnames, parse_texture_lump,
};
pub use weapon_anim::{
    MAX_BOB, RAISE_SPEED, WEAPON_BASE_X, WEAPON_BASE_Y, WEAPON_BOTTOM, WEAPON_FLASH_LIGHT_BONUS,
    WEAPON_TOP, WeaponAnimState, WeaponBob, WeaponSprite, draw_weapon_animated, draw_weapon_shaded,
    get_weapon_light_params, weapon_light_bonus, weapon_sprite_name,
};
pub use wipe::ScreenWipe;
