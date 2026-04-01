import re
with open('crates/doom-game/src/specials.rs', 'r') as f:
    code = f.read()

fns = re.findall(r'^pub fn ([a-zA-Z0-9_]+)', code, re.MULTILINE)

# Now check where these functions are used across the workspace
import subprocess

for fn in fns:
    # Use ripgrep or grep to find occurrences outside of crates/doom-game/src/specials.rs
    out = subprocess.run(['rg', '-l', fn, 'crates/'], capture_output=True, text=True)
    files = out.stdout.strip().split('\n')

    # Check if used outside doom-game/src/specials.rs AND doom-game/src/lib.rs (which re-exports it)
    used_outside = False
    for file in files:
        if file and file != 'crates/doom-game/src/specials.rs' and file != 'crates/doom-game/src/lib.rs':
            # It's used somewhere else
            used_outside = True
            break

    if not used_outside:
        print(f"{fn} is NOT used outside doom-game! Can be made pub(crate)")
