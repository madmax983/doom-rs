import re

def main():
    with open('crates/doom-game/src/state.rs', 'r') as f:
        state_code = f.read()

    with open('crates/doom-game/src/spawn.rs', 'r') as f:
        spawn_code = f.read()

    # Move `Skill` enum from spawn.rs to state.rs to break the cycle state -> spawn -> state.

main()
