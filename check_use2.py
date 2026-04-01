import subprocess
import sys

warns = [
    'BLAZING_DOOR_SPEED',
    'tick_sector_specials',
    'tick_sector_secrets',
    'spawn_level_specials',
    'next_highest_floor_above',
    'ev_floor_lower_to_nearest',
    'ev_floor_raise_to_ceiling',
    'LIFT_WAIT',
    'activate_lift',
    'close_door',
    'close_wait_open_door',
    'open_blazing_door',
    'close_blazing_door',
    'activate_linedef'
]

for w in warns:
    res = subprocess.run(['rg', r'\b' + w + r'\b', 'crates/'], capture_output=True, text=True)
    if not res.stdout:
        print(f"NOT FOUND: {w}")
    else:
        # Check if used outside of declaration
        lines = res.stdout.strip().split('\n')
        # simple check if there's more than 1 usage
        # (declaration is 1, maybe tests use it?)
        print(f"{w} ({len(lines)} uses):")
        for line in lines[:3]:
            print(f"  {line[:100]}")
