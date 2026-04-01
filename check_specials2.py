import re
import subprocess

with open('crates/doom-game/src/specials.rs', 'r') as f:
    code = f.read()

fns = re.findall(r'^pub fn ([a-zA-Z0-9_]+)', code, re.MULTILINE)

print("Functions exported from specials.rs but only used internally within doom-game:")

for fn in fns:
    out = subprocess.run(['rg', '-l', fn, 'crates/'], capture_output=True, text=True)
    files = out.stdout.strip().split('\n')

    used_outside_doom_game = False
    for file in files:
        if file and not file.startswith('crates/doom-game/'):
            used_outside_doom_game = True
            break

    if not used_outside_doom_game:
        print(f"- {fn}")
