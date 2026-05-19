//! Constants and primitives for the automap.

/// Framebuffer width in pixels.
pub const SCREEN_W: i32 = 320;
/// Framebuffer height in pixels.
pub const SCREEN_H: i32 = 200;
pub const HALF_W: i32 = SCREEN_W / 2;
pub const HALF_H: i32 = SCREEN_H / 2;

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
pub const FLAG_SECRET: u16 = 0x0020;

/// Grid spacing in map units.
pub const GRID_SPACING: i32 = 128;
