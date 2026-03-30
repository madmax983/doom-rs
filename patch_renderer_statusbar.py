import re

with open("crates/doom-renderer/src/statusbar.rs", "r") as f:
    content = f.read()

content = content.replace(
    "    pub face_index: u8,\n}",
    "    pub face_index: u8,\n    /// Player arcade score.\n    pub score: u32,\n}"
)

content = content.replace(
    "            face_index: 0,\n        }\n    }\n}",
    "            face_index: 0,\n            score: player.score,\n        }\n    }\n}"
)

content = content.replace(
    "        draw_stysnum(fb, cache, wad, 314, *ty, max, 3);\n    }\n}",
    "        draw_stysnum(fb, cache, wad, 314, *ty, max, 3);\n    }\n\n    // 9. Arcade Score (drawn on top left edge of the HUD or somewhere visible)\n    draw_stnum(fb, cache, wad, 314, BAR - 16, data.score as i32, 7);\n}"
)

with open("crates/doom-renderer/src/statusbar.rs", "w") as f:
    f.write(content)
