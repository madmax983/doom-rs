import re

# 1. Revert `"Fire"` back to `"FIRE"` in states.rs
with open("crates/doom-game/src/states.rs", "r") as f:
    content = f.read()
content = content.replace('"Fire"', '"FIRE"')

# 2. Rename `Action::None` to `Action::NoneAction` across files to prevent `Option::None` shadowing
content = re.sub(r"\bNone\b", "NoneAction", content)
# Ensure we don't accidentally rename things we shouldn't. Wait, we ONLY renamed `NONE` to `None`.
# So the table has `action: None,`.
# We should be careful not to rename actual `Option::None` if it exists.
# But states.rs doesn't use Option? Let's check `states.rs` size and occurrences.
with open("crates/doom-game/src/states.rs", "w") as f:
    f.write(content)

# Update actions.rs
with open("crates/doom-game/src/actions.rs", "r") as f:
    content = f.read()
content = content.replace("    None = 0,", "    NoneAction = 0,")
content = content.replace("Action::None", "Action::NoneAction")

# We should also remove `strum_macros::FromRepr` from `Action` enum since it's unused.
content = content.replace("#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::FromRepr)]", "#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
with open("crates/doom-game/src/actions.rs", "w") as f:
    f.write(content)

# Update mobj.rs and tic.rs and weapons.rs
for file in ["crates/doom-game/src/mobj.rs", "crates/doom-game/src/tic.rs", "crates/doom-game/src/weapons.rs"]:
    with open(file, "r") as f:
        content = f.read()
    content = content.replace("Action::None", "Action::NoneAction")
    with open(file, "w") as f:
        f.write(content)

print("Fixed review issues")
