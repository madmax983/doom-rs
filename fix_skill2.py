import re

with open('crates/doom-game/src/state.rs', 'r') as f:
    state_code = f.read()

# remove use crate::spawn::Skill;
state_code = state_code.replace("use crate::spawn::Skill;\n", "")

with open('crates/doom-game/src/spawn.rs', 'r') as f:
    spawn_code = f.read()

# move skill to skill.rs and update imports
with open('crates/doom-game/src/skill.rs', 'w') as f:
    f.write("""//! Skill level enum and definitions.

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
""")

skill_enum = re.search(r'// \-+\n// Skill level\n// \-+\n\n/// Skill level for thing filtering\.\n\#\[derive\(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq\)\]\n\#\[repr\(u8\)\]\npub enum Skill \{.*?\}\n\nimpl Skill \{.*?\}\n\n', spawn_code, re.DOTALL)
if skill_enum:
    spawn_code = spawn_code.replace(skill_enum.group(0), "use crate::skill::Skill;\n\n")

with open('crates/doom-game/src/spawn.rs', 'w') as f:
    f.write(spawn_code)

state_code = state_code.replace("use crate::mobj::{MobjHandle, MobjKind, MobjSlab};", "use crate::mobj::{MobjHandle, MobjKind, MobjSlab};\nuse crate::skill::Skill;")

with open('crates/doom-game/src/state.rs', 'w') as f:
    f.write(state_code)

with open('crates/doom-game/src/lib.rs', 'r') as f:
    lib_code = f.read()

lib_code = lib_code.replace('pub mod spawn;', 'pub mod skill;\npub mod spawn;')
lib_code = lib_code.replace('pub use spawn::{Skill, spawn_level_things};', 'pub use skill::Skill;\npub use spawn::spawn_level_things;')

with open('crates/doom-game/src/lib.rs', 'w') as f:
    f.write(lib_code)
