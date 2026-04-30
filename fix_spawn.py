import sys
from pathlib import Path

def main():
    path = Path("crates/doom-game/src/spawn.rs")
    content = path.read_text()

    insert_block = """
        // --- Boss Brain targets (Thing type 87) ---
        if thing.kind == 87 {
            gs.boss_brain.targets.push((x, y));
            continue;
        }"""

    if "thing.kind == 87" not in content:
        content = content.replace(
            "// --- Player start ---",
            insert_block.strip() + "\n\n        // --- Player start ---"
        )

    path.write_text(content)

if __name__ == '__main__':
    main()
