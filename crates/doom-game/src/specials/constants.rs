// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Distance ahead the player can activate a linedef.
pub const USE_RANGE: i32 = 64;

/// Door open/close speed in map units per tic (Doom standard: 2 units/tic).
pub const DOOR_SPEED: i16 = 2;

/// Tics a door stays open before auto-closing (3.5 seconds at 35 Hz ≈ 120 tics).
pub const DOOR_WAIT: i32 = 120;

/// Door speed for blazing (fast) doors in map units per tic.
pub const BLAZING_DOOR_SPEED: i16 = 8;

/// Period for fast blinking lights (tics).
pub const BLINK_FAST_PERIOD: i32 = 15;

/// Period for slow blinking lights (tics).
pub const BLINK_SLOW_PERIOD: i32 = 35;

// Sector damage constants (legacy per tic)
pub const LEGACY_DAMAGE_HELLSLIME: i32 = 10;
pub const LEGACY_DAMAGE_NUKAGE: i32 = 5;
pub const LEGACY_DAMAGE_SUPER_HELLSLIME: i32 = 20;

// Sector damage constants (periodic every 32 tics)
pub const PERIODIC_DAMAGE_NUKAGE_BLINK: i32 = 5;
pub const PERIODIC_DAMAGE_HELLSLIME: i32 = 5;
pub const PERIODIC_DAMAGE_NUKAGE: i32 = 2;
pub const PERIODIC_DAMAGE_GOD_EXIT: i32 = 20;
pub const PERIODIC_DAMAGE_SUPER_HELLSLIME: i32 = 20;
