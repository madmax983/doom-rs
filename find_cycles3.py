import os
import re

def find_imports(filepath):
    with open(filepath, 'r') as f:
        content = f.read()
    imports = set()
    for line in content.split('\n'):
        if line.startswith('use crate::'):
            parts = line[11:].split('::')
            if len(parts) > 0:
                imported_mods = parts[0].strip().split('{')
                for imported_mod in imported_mods:
                    imported_mod = imported_mod.strip(',;} ')
                    if imported_mod:
                        imports.add(imported_mod)
    return imports

for file in ["state.rs", "spawn.rs", "pickups.rs", "actions.rs", "states.rs"]:
    print(f"{file}: {find_imports(os.path.join('crates/doom-game/src', file))}")
