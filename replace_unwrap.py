import re

files = [
    "crates/doom-app/src/cogmind/render.rs",
    "crates/doom-app/src/cogmind/tile_grid.rs",
    "crates/doom-app/src/audio_system.rs",
]

for file_path in files:
    with open(file_path, "r") as f:
        content = f.read()

    # Replace unwrap with expect in tests, but leave doctests alone
    content = re.sub(
        r"tile_grid\.as_ref\(\)\.unwrap\(\)",
        r'tile_grid.as_ref().expect("TileGrid should be initialized")',
        content
    )
    content = re.sub(
        r"tiles\[0\]\.glow\.unwrap\(\)",
        r'tiles[0].glow.expect("Glow color should be present")',
        content
    )
    content = re.sub(
        r"mixer\.lock\(\)\.unwrap\(\)",
        r'mixer.lock().expect("Mixer lock should not be poisoned")',
        content
    )

    with open(file_path, "w") as f:
        f.write(content)
