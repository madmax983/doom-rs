import re

with open('crates/doom-game/src/menu.rs', 'r') as f:
    text = f.read()

# Need to insert `GameVersion` after `//!`
text = re.sub(r'(//!.*?)\n\n', r'\1\n\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum GameVersion {\n    Doom1,\n    Doom2,\n}\n\n', text, flags=re.DOTALL)

text = text.replace('pub fn new(is_doom2: bool) -> Self {', 'pub fn new(game_version: GameVersion) -> Self {')
text = text.replace('is_doom2,', 'is_doom2: game_version == GameVersion::Doom2,')

text = text.replace('GameMenu::new(false)', 'GameMenu::new(GameVersion::Doom1)')
text = text.replace('GameMenu::new(true)', 'GameMenu::new(GameVersion::Doom2)')

with open('crates/doom-game/src/menu.rs', 'w') as f:
    f.write(text)
