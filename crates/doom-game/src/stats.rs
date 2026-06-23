//! End-of-level map statistics and tracking (kills, items, secrets).
//!
//! This module defines the [`LevelStats`] structure, which monitors the player's
//! progress throughout a map. It tracks the number of monsters killed, items collected,
//! and secret sectors discovered, as well as the map's totals.

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
