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
    print(f"=== {w} ===")
    subprocess.run(['rg', '-l', w, 'crates/'])
