/// Dictates whether to spawn multiplayer-only things.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum GameMode {
    /// Standard single-player mode.
    SinglePlayer,
    /// Deathmatch multiplayer mode.
    Deathmatch,
}
