import re

def main():
    with open('crates/doom-game/src/state.rs', 'r') as f:
        state_code = f.read()

    with open('crates/doom-game/src/spawn.rs', 'r') as f:
        spawn_code = f.read()

    # ensure that state.rs no longer imports spawn::Skill
    if 'use crate::spawn::Skill;' in state_code:
        state_code = state_code.replace('use crate::spawn::Skill;\n', '')

    with open('crates/doom-game/src/state.rs', 'w') as f:
        f.write(state_code)

main()
