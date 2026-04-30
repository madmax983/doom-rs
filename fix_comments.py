import sys
from pathlib import Path

def main():
    path = Path("crates/doom-game/src/actions.rs")
    content = path.read_text()

    content = content.replace("brain_targets", "boss_brain.targets")

    path.write_text(content)

if __name__ == '__main__':
    main()
