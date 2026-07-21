//! End-of-level Statistics and Map Tracking.
//!
//! This module defines the state structures used to track the player's
//! progression, kills, items, and secrets throughout a level.
//!
//! # The "Black Box"
//!
//! At the end of every map, the intermission screen tallies up the player's
//! performance. The `LevelStats` struct gathers this data over time during gameplay
//! so it is ready when the map completes.

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
