import os
import re
import subprocess

with open('crates/doom-game/src/specials.rs', 'r') as f:
    code = f.read()

# All public functions in specials.rs
fns = re.findall(r'^pub fn ([a-zA-Z0-9_]+)', code, re.MULTILINE)

internal_fns = []
dead_code_fns = []

for fn in fns:
    out = subprocess.run(['rg', '-l', r'\b' + fn + r'\b', 'crates/'], capture_output=True, text=True)
    files = out.stdout.strip().split('\n')

    used_outside = False
    used_inside = False
    for file in files:
        if file:
            if file.startswith('crates/doom-game/src/'):
                if file not in ['crates/doom-game/src/specials.rs', 'crates/doom-game/src/lib.rs']:
                    used_inside = True
            elif not file.startswith('crates/doom-game/'):
                used_outside = True

    # Check if used inside specials.rs outside of declaration and tests
    if not used_inside and not used_outside:
        # Check if used within specials.rs (ignoring tests)
        res = subprocess.run(['rg', r'\b' + fn + r'\b', 'crates/doom-game/src/specials.rs'], capture_output=True, text=True)
        # We need a proper check here, but let's be conservative. If it's ONLY in specials.rs and lib.rs, it's internal.
        # But if it's not even used in tic.rs or linedef_dispatch.rs, it might be dead code!
        # Actually wait: linedef_dispatch.rs uses a lot of ev_* functions. Let's see if they showed up in `files`.
        # Yes, linedef_dispatch.rs is inside doom-game/src/.
        pass

    if not used_outside:
        internal_fns.append(fn)

print(f"Internal FNs ({len(internal_fns)}):", internal_fns)

# We want to change pub fn to pub(crate) fn for internal_fns,
# EXCEPT if it makes them unused entirely (meaning they are completely dead code outside of tests/specials.rs itself)
# If they are completely dead code, maybe we should still make them pub(crate) and #[allow(dead_code)] or just leave them?
# No, we should remove dead code! But Atlas only focuses on architecture. Making it pub(crate) IS the right architectural move.
# If they are part of the public API, they should be public. BUT they are internal game engine details!
