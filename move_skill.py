import re

with open('crates/doom-game/src/spawn.rs', 'r') as f:
    spawn_code = f.read()

# match pub enum Skill
skill_enum = re.search(r'// \-+\n// Skill level\n// \-+\n\n/// Skill level for thing filtering\.\n\#\[derive\(strum_macros::FromRepr, Clone, Copy, Debug, PartialEq, Eq\)\]\n\#\[repr\(u8\)\]\npub enum Skill \{.*?\}\n\nimpl Skill \{.*?\}\n\n', spawn_code, re.DOTALL)

if skill_enum:
    skill_code = skill_enum.group(0)

    # remove from spawn.rs
    spawn_code = spawn_code.replace(skill_code, "")

    with open('crates/doom-game/src/spawn.rs', 'w') as f:
        f.write(spawn_code)

    # add to state.rs right after GameState definitions
    with open('crates/doom-game/src/state.rs', 'r') as f:
        state_code = f.read()

    state_code = state_code.replace('use crate::spawn::Skill;\n', '')
    state_code = state_code.replace('pub enum LockedDoorColor {', skill_code + '\n/// Represents the required key color when a locked door is denied access.\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum LockedDoorColor {')

    with open('crates/doom-game/src/state.rs', 'w') as f:
        f.write(state_code)

    print("Done")
else:
    print("Could not find Skill enum")
