import re

with open('crates/doom-game/src/state.rs', 'r') as f:
    state_code = f.read()

skill_code = """
/// Skill level for thing filtering.
#[derive(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Skill {
    /// I'm Too Young To Die.
    Baby = 0,
    /// Hey, Not Too Rough.
    Easy = 1,
    /// Hurt Me Plenty.
    Medium = 2,
    /// Ultra-Violence.
    Hard = 3,
    /// Nightmare!
    Nightmare = 4,
}

impl Skill {
    /// Convert an integer to a `Skill`.
    ///
    /// Returns `None` for any out-of-range value.
    pub fn from_num(n: u8) -> Option<Self> {
        Self::from_repr(n)
    }
}
"""

if "pub enum Skill" not in state_code:
    state_code = state_code.replace("pub enum LockedDoorColor", skill_code + "\npub enum LockedDoorColor")

with open('crates/doom-game/src/state.rs', 'w') as f:
    f.write(state_code)
