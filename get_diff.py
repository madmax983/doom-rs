import os
import re
import subprocess

with open('crates/doom-game/src/specials.rs', 'r') as f:
    code = f.read()

# All public functions in specials.rs
fns = re.findall(r'^pub fn ([a-zA-Z0-9_]+)', code, re.MULTILINE)

internal_fns = []

for fn in fns:
    out = subprocess.run(['rg', '-l', r'\b' + fn + r'\b', 'crates/'], capture_output=True, text=True)
    files = out.stdout.strip().split('\n')

    used_outside = False
    for file in files:
        if file and not file.startswith('crates/doom-game/'):
            used_outside = True
            break

    if not used_outside:
        internal_fns.append(fn)

print(f"Internal FNs ({len(internal_fns)}):", internal_fns)

# Now apply visibility changes: 'pub fn' -> 'pub(crate) fn'
lines = code.split('\n')
for i, line in enumerate(lines):
    m = re.match(r'^pub fn ([a-zA-Z0-9_]+)', line)
    if m and m.group(1) in internal_fns:
        lines[i] = line.replace('pub fn ', 'pub(crate) fn ')

with open('crates/doom-game/src/specials.rs', 'w') as f:
    f.write('\n'.join(lines))

# And update lib.rs exports
with open('crates/doom-game/src/lib.rs', 'r') as f:
    lib_text = f.read()

m = re.search(r'pub use specials::\{([^}]+)\};', lib_text, re.MULTILINE | re.DOTALL)
if m:
    block = m.group(1)
    symbols = [s.strip() for s in block.replace('\n', ' ').split(',') if s.strip()]

    new_symbols = [s for s in symbols if s not in internal_fns]

    new_block = ',\n    '.join(new_symbols)
    if new_block:
        new_block = f'\n    {new_block},\n'

    lib_text = lib_text[:m.start(1)] + new_block + lib_text[m.end(1):]

with open('crates/doom-game/src/lib.rs', 'w') as f:
    f.write(lib_text)
