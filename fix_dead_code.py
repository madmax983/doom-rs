import re

with open('crates/doom-game/src/specials.rs', 'r') as f:
    code = f.read()

# Make activate_linedef and p_use_lines public again, since they're used by tic.rs? Wait, they are used by tic.rs which is in the same crate!
# If they are used by tic.rs they shouldn't trigger dead_code if they are pub(crate).
# Wait, let's see why they trigger dead_code.
