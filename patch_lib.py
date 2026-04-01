import re

with open('crates/doom-game/src/lib.rs', 'r') as f:
    text = f.read()

# We need to remove the items from the pub use specials::{ ... } block that we made pub(crate).
# They are no longer accessible from outside the crate, so they shouldn't be re-exported.

import subprocess
out = subprocess.run(['python3', 'check_specials2.py'], capture_output=True, text=True)
to_remove = set()
for line in out.stdout.split('\n'):
    if line.startswith('- '):
        to_remove.add(line[2:])

# Read the pub use block for specials
m = re.search(r'pub use specials::\{([^}]+)\};', text, re.MULTILINE | re.DOTALL)
if m:
    block = m.group(1)

    # Extract all the symbols
    symbols = [s.strip() for s in block.replace('\n', ' ').split(',') if s.strip()]

    # Keep only those not in to_remove
    new_symbols = [s for s in symbols if s not in to_remove]

    new_block = ',\n    '.join(new_symbols)
    if new_block:
        new_block = f'\n    {new_block},\n'

    text = text[:m.start(1)] + new_block + text[m.end(1):]

with open('crates/doom-game/src/lib.rs', 'w') as f:
    f.write(text)
