import re

with open('crates/doom-game/src/state.rs', 'r') as f:
    content = f.read()

content = content.replace("let melee = SoundRequest::PlayerWeaponMeleeHit;\n        assert!(matches!(melee, SoundRequest::PlayerWeaponMeleeHit));\n\n", "")

with open('crates/doom-game/src/state.rs', 'w') as f:
    f.write(content)
