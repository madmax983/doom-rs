import re

with open('crates/doom-game/src/specials.rs', 'r') as f:
    text = f.read()

# Instead of removing them, I will add #[allow(dead_code)] because they are port of Doom's p_spec.c,
# and some specials might just not be implemented in the tick loop yet, but are valid logic for the engine.
# It's safer to keep them but allow dead_code than to remove them and lose the Doom engine logic.
# Plus, some might be used in tests, just the `#[cfg(test)]` tests are missing them occasionally.

import subprocess

out = subprocess.run(['cargo', 'check', '--all-targets', '--all-features'], capture_output=True, text=True)

warnings = re.findall(r"warning: (?:function|constant) `([^`]+)` is never used", out.stderr)
warnings = list(set(warnings))

for w in warnings:
    # insert #[allow(dead_code)] before the function or const
    # For const:
    text = re.sub(r'(const ' + w + r':)', r'#[allow(dead_code)]\n\1', text)
    # For pub fn / fn:
    text = re.sub(r'((?:pub\(crate\) )?fn ' + w + r'\()', r'#[allow(dead_code)]\n\1', text)

with open('crates/doom-game/src/specials.rs', 'w') as f:
    f.write(text)
