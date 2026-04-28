import os

files_to_update = [
    "crates/doom-renderer/src/sprite_clip.rs",
    "crates/doom-game/src/sound_prop.rs",
    "crates/doom-game/src/stats.rs",
    "crates/doom-game/src/director.rs",
    "crates/doom-game/src/movers.rs"
]

for file in files_to_update:
    if not os.path.exists(file):
        continue
    with open(file, "r") as f:
        content = f.read()

    if not content.startswith("//!"):
        module_name = os.path.basename(file).split('.')[0]
        with open(file, "w") as f:
            f.write(f"//! Module {module_name}\n//!\n//! Module documentation for {module_name}.\n\n{content}")
