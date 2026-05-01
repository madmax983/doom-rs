import re

with open('crates/doom-game/src/specials.rs') as f:
    lines = f.readlines()

structs = set()
for line in lines:
    if line.strip().startswith('use '):
        continue

    # Check for direct dependencies on game state or movers from specials
    if 'SectorMovers' in line:
        print(f"Found SectorMovers dependency: {line.strip()}")
    if 'GameState' in line:
        pass # this is fine
