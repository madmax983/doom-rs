//! Gameplay Statistics Tracking.
//!
//! Tracking player performance for the classic Doom intermission screen.
//!
//! This module manages the `LevelStats` struct, which records the trinity of Doom metrics:
//! Kills, Items, and Secrets. The game loop increments these counters when a monster
//! dies, an item is picked up, or a sector with the secret property is entered.
//!
//! **The "Why":** Isolating stats from player state allows intermission screens to
//! easily calculate percentages without needing to traverse the entire map or actor list
//! at the end of a level.

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
