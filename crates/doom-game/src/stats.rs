//! End-of-level statistics tracking.
//!
//! Tracks player progress through a level for the classic intermission screen,
//! tallying kills, item pickups, and discovered secrets.
//!
//! # Examples
//!
//! ```
//! use doom_game::stats::LevelStats;
//!
//! let mut stats = LevelStats::default();
//! stats.total_kills = 100;
//! stats.kill_count = 50;
//!
//! // Calculate percentage without panicking on zero
//! let percentage = if stats.total_kills > 0 {
//!     (stats.kill_count * 100) / stats.total_kills
//! } else {
//!     100
//! };
//! assert_eq!(percentage, 50);
//! ```

/// End-of-level statistics and map tracking.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelStats {
    /// Number of monsters killed by the player so far.
    pub kill_count: u32,
    /// Number of special items collected by the player.
    pub item_count: u32,
    /// Number of secret areas discovered.
    pub secret_count: u32,
    /// Total killable monsters in the map (for percentage display).
    pub total_kills: u32,
    /// Total collectable items.
    pub total_items: u32,
    /// Total secret sectors in the map (sectors with special type 9).
    pub total_secrets: u32,
    /// Number of tics elapsed in the current level (incremented each tick).
    pub level_time: u32,
}
