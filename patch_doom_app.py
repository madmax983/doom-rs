import sys

with open("crates/doom-app/src/audio_system.rs", "r") as f:
    lines = f.readlines()

for i, line in enumerate(lines):
    if "fn stack_with_iwad_bytes" in line:
        lines.insert(i, "    #[allow(dead_code)]\n")
        break

for i, line in enumerate(lines):
    if "fn make_iwad" in line:
        lines.insert(i, "    #[allow(dead_code)]\n")
        break

with open("crates/doom-app/src/audio_system.rs", "w") as f:
    f.writelines(lines)
