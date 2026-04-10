import os
import re

def main():
    with open('crates/doom-app/src/main.rs', 'r') as f:
        main_code = f.read()

    print("pub things in main:")
    for line in main_code.split('\n'):
        if line.startswith('pub ') or line.startswith('pub('):
            print(line)

    for mod in ['audio_system.rs', 'cheats.rs', 'console.rs', 'demo_mode.rs', 'net_mode.rs', 'savegame.rs', 'cogmind/effects.rs', 'cogmind/glyphs.rs', 'cogmind/lighting.rs', 'cogmind/render.rs', 'cogmind/visibility.rs', 'cogmind/tile_grid.rs', 'cogmind/sight_line.rs']:
        with open('crates/doom-app/src/' + mod, 'r') as f:
            code = f.read()
        print(f"\n{mod}:")
        for line in code.split('\n'):
            if line.startswith('pub ') or line.startswith('pub('):
                print(line)

main()
