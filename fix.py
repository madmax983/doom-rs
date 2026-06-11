import re
with open("crates/doom-game/src/savegame.rs", "r") as f:
    text = f.read()

text = text.replace("assert_eq!(loaded.state.brain_awake, true);", "assert!(loaded.state.brain_awake);")

with open("crates/doom-game/src/savegame.rs", "w") as f:
    f.write(text)

print("ok")
