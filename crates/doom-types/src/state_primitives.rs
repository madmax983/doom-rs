/// Represents the required key color when a locked door is denied access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockedDoorColor {
    /// A blue keycard or skull key is required.
    Blue,
    /// A red keycard or skull key is required.
    Red,
    /// A yellow keycard or skull key is required.
    Yellow,
}

/// The type of level exit the player triggered.
///
/// Set by `activate_linedef` when a switch or walk-trigger exit line is
/// activated.  Cleared to `None` at the start of each tick so the caller
/// can observe it exactly once.
#[derive(strum_macros::FromRepr, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitRequest {
    /// Normal exit (next sequential map).
    Normal,
    /// Secret exit (secret map).
    Secret,
}
