import re

with open('crates/doom-game/src/specials.rs', 'r') as f:
    lines = f.readlines()

import subprocess
out = subprocess.run(['python3', 'check_specials2.py'], capture_output=True, text=True)
to_make_private = set()
for line in out.stdout.split('\n'):
    if line.startswith('- '):
        to_make_private.add(line[2:])

new_lines = []
for line in lines:
    m = re.match(r'^pub fn ([a-zA-Z0-9_]+)', line)
    if m:
        fn_name = m.group(1)
        if fn_name in to_make_private:
            line = line.replace('pub fn', 'pub(crate) fn', 1)
    new_lines.append(line)

with open('crates/doom-game/src/specials.rs', 'w') as f:
    f.writelines(new_lines)
