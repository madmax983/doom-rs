//! The final tally: Level statistics and end-screen metrics.
//!
//! Every demon slain, every secret uncovered, and every item hoarded is meticulously
//! tracked here. The `LevelStats` structure maintains the running totals needed for
//! the iconic intermission screen (the "Level Completed" tally).
//!
//! It serves as the ultimate record of the player's thoroughness, ensuring that
//! the elusive 100% completion remains a precise and tracked goal.

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
