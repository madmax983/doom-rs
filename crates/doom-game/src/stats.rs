//! End-of-level statistics and map tracking.
//!
//! This module tracks the player's progress through a level for the intermission screen.
//! It maintains the kill count, item pickups, secret discoveries, and total level time.
//!
//! By keeping this state encapsulated in `LevelStats`, we can easily persist it across
//! savegames and hand it off cleanly to the `doom-renderer`'s intermission drawer
//! when the level ends.

/// End-of-level statistics and map tracking.
///
/// ## Examples
///
/// ```
/// use doom_game::stats::LevelStats;
///
/// let mut stats = LevelStats::default();
/// stats.total_kills = 10;
/// stats.kill_count = 5;
///
/// assert_eq!(stats.kill_count, 5);
/// assert_eq!(stats.total_kills, 10);
/// ```
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
