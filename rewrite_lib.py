with open('crates/doom-game/src/lib.rs', 'r') as f:
    text = f.read()

text = text.replace('pub use menu::{Menu, MenuAction, MenuItem, MenuPage, MenuResult, TitlePhase, TitleScreen};', 'pub use menu::{GameVersion, Menu, MenuAction, MenuItem, MenuPage, MenuResult, TitlePhase, TitleScreen};')

with open('crates/doom-game/src/lib.rs', 'w') as f:
    f.write(text)
