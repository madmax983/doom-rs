import re
import subprocess

with open('crates/doom-game/src/specials.rs', 'r') as f:
    code = f.read()

fns = re.findall(r'^pub fn ([a-zA-Z0-9_]+)', code, re.MULTILINE)

used_outside_doom_game = []
for fn in fns:
    out = subprocess.run(['rg', '-l', r'\b' + fn + r'\b', 'crates/'], capture_output=True, text=True)
    files = out.stdout.strip().split('\n')

    for file in files:
        if file and not file.startswith('crates/doom-game/'):
            used_outside_doom_game.append(fn)
            break

print("Functions used outside doom-game:")
print(used_outside_doom_game)
