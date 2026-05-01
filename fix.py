with open("crates/doom-game/src/specials.rs", "r") as f:
    content = f.read()

content = content.replace("crate::state::SectorDamageType", "crate::movers::SectorDamageType")

with open("crates/doom-game/src/specials.rs", "w") as f:
    f.write(content)
