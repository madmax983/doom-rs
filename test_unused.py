import subprocess
import re

with open('crates/doom-game/src/specials.rs', 'r') as f:
    code = f.read()

fns = re.findall(r'^pub fn ([a-zA-Z0-9_]+)', code, re.MULTILINE)

unused = []

for fn in fns:
    out = subprocess.run(['rg', '-w', fn, 'crates/doom-game/src/'], capture_output=True, text=True)
    files = out.stdout.strip().split('\n')

    # Exclude lib.rs and specials.rs declarations/tests
    used = False
    for line in files:
        if not line: continue
        file_path = line.split(':')[0]
        if file_path not in ['crates/doom-game/src/lib.rs', 'crates/doom-game/src/specials.rs', 'crates/doom-game/src/linedef_dispatch.rs']:
            used = True
            break

        # If it's used in linedef_dispatch.rs, check if linedef_dispatch itself is called somewhere
        if file_path == 'crates/doom-game/src/linedef_dispatch.rs':
            used = True
            break

    if not used:
        unused.append(fn)

print("Actually completely unused outside specials.rs / lib.rs:")
for u in unused: print(u)
