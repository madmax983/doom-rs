import re

with open('crates/doom-game/src/specials.rs') as f:
    lines = f.readlines()

test_start_idx = -1
for i, line in enumerate(lines):
    if line.strip() == "#[cfg(test)]" and test_start_idx == -1:
        test_start_idx = i
        break

if test_start_idx != -1:
    print(f"Test module starts at line {test_start_idx + 1}")
    print(f"File has {len(lines)} lines total")
