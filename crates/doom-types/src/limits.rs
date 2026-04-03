//! Engine limits and domain bounds from the Unofficial Doom Specs.
//!
//! These constants are used as Verus verified upper bounds throughout the engine.

// ---------------------------------------------------------------------------
// Renderer limits
// ---------------------------------------------------------------------------

/// Maximum number of visible things per frame (vanilla limit).
///
/// ## Examples
/// ```
/// use doom_types::limits::MAX_VISIBLE_THINGS;
/// let mut visible = Vec::<i32>::with_capacity(MAX_VISIBLE_THINGS as usize);
/// ```
pub const MAX_VISIBLE_THINGS: u32 = 64;

/// Maximum number of simultaneously active moving platforms.
///
/// ## Examples
/// ```
/// use doom_types::limits::MAX_ACTIVE_PLATS;
/// assert_eq!(MAX_ACTIVE_PLATS, 30);
/// ```
pub const MAX_ACTIVE_PLATS: u32 = 30;

/// Maximum scroll lines in a level.
pub const MAX_SCROLL_LINES: u32 = 64;

/// Maximum texture width in pixels.
pub const MAX_TEXTURE_WIDTH: u32 = 256;

/// Maximum texture height in pixels.
pub const MAX_TEXTURE_HEIGHT: u32 = 128;

// ---------------------------------------------------------------------------
// Blockmap limits
// ---------------------------------------------------------------------------

/// Maximum number of blockmap cells.
pub const MAX_BLOCKMAP_BLOCKS: u32 = 13_000;

/// Size of a blockmap cell in map units.
pub const BLOCKMAP_BLOCK_SIZE: u32 = 128;

// ---------------------------------------------------------------------------
// Physics limits
// ---------------------------------------------------------------------------

/// Maximum step height a player can climb without jumping (in map units).
pub const PASSABLE_STEP_MAX: u32 = 24;

// ---------------------------------------------------------------------------
// Texture / palette data sizes (exact from WAD format spec)
// ---------------------------------------------------------------------------

/// Exactly 64×64 pixels = 4096 bytes for a flat lump.
pub const FLAT_SIZE: usize = 4096;

/// Number of palettes in PLAYPAL.
pub const PLAYPAL_COUNT: usize = 14;

/// Colors per palette.
pub const PLAYPAL_COLORS: usize = 256;

/// Number of colormaps in COLORMAP.
pub const COLORMAP_COUNT: usize = 34;

// ---------------------------------------------------------------------------
// Framebuffer dimensions
// ---------------------------------------------------------------------------

/// Doom's native render width.
pub const FB_WIDTH: usize = 320;

/// Doom's native render height.
pub const FB_HEIGHT: usize = 200;

/// Total framebuffer size in bytes (palette-indexed).
pub const FB_SIZE: usize = FB_WIDTH * FB_HEIGHT;

// ---------------------------------------------------------------------------
// Game limits
// ---------------------------------------------------------------------------

/// Maximum number of players in a game (4-player limit).
pub const MAX_PLAYERS: usize = 4;

/// Number of weapon types.
pub const NUM_WEAPONS: usize = 9;

/// Number of ammo types.
pub const NUM_AMMO: usize = 4;

/// Maximum ammo by type (bullets, shells, cells, rockets).
pub const MAX_AMMO: [u32; NUM_AMMO] = [200, 50, 300, 50];

/// Maximum health (standard, without God mode).
pub const MAX_HEALTH: i32 = 100;

/// Maximum armor.
pub const MAX_ARMOR: i32 = 200;

// ---------------------------------------------------------------------------
// Rollback netcode
// ---------------------------------------------------------------------------

/// Maximum number of tics to roll back in netcode.
pub const MAX_ROLLBACK_TICS: u32 = 8;

/// Fixed simulation rate.
pub const TIC_RATE_HZ: u32 = 35;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_size_is_64x64() {
        assert_eq!(FLAT_SIZE, 64 * 64);
    }

    #[test]
    fn fb_size_is_320x200() {
        assert_eq!(FB_SIZE, 320 * 200);
    }

    #[test]
    fn num_ammo_consistent_with_max_ammo() {
        assert_eq!(MAX_AMMO.len(), NUM_AMMO);
    }
}
