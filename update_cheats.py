import re

with open("crates/doom-app/src/cheats.rs", "r") as f:
    lines = f.readlines()

new_lines = []
for i, line in enumerate(lines):
    if "pub(crate) struct CheatDef" in line:
        new_lines.append("/// # Examples\n")
        new_lines.append("/// ```\n")
        new_lines.append("/// use doom_app::cheats::CheatDef;\n")
        new_lines.append("///\n")
        new_lines.append("/// let cheat = CheatDef {\n")
        new_lines.append("///     name: \"IDDQD\",\n")
        new_lines.append("///     sequence: \"iddqd\",\n")
        new_lines.append("/// };\n")
        new_lines.append("/// assert_eq!(cheat.name, \"IDDQD\");\n")
        new_lines.append("/// ```\n")
    if "pub(crate) struct CheatDetector" in line:
        new_lines.append("/// # Examples\n")
        new_lines.append("/// ```\n")
        new_lines.append("/// use doom_app::cheats::CheatDetector;\n")
        new_lines.append("///\n")
        new_lines.append("/// let mut det = CheatDetector::new();\n")
        new_lines.append("/// for ch in \"iddqd\".chars() {\n")
        new_lines.append("///     det.feed(ch);\n")
        new_lines.append("/// }\n")
        new_lines.append("/// ```\n")
    if "pub(crate) fn apply_cheat" in line:
        new_lines.insert(len(new_lines), "///\n")
        new_lines.insert(len(new_lines), "/// # Examples\n")
        new_lines.insert(len(new_lines), "/// ```\n")
        new_lines.insert(len(new_lines), "/// use doom_app::cheats::apply_cheat;\n")
        new_lines.insert(len(new_lines), "/// use doom_game::GameState;\n")
        new_lines.insert(len(new_lines), "///\n")
        new_lines.insert(len(new_lines), "/// let mut gs = GameState::new(\"E1M1\");\n")
        new_lines.insert(len(new_lines), "/// let msg = apply_cheat(&mut gs, \"IDDQD\");\n")
        new_lines.insert(len(new_lines), "/// assert_eq!(msg, \"Degreelessness mode ON\");\n")
        new_lines.insert(len(new_lines), "/// ```\n")
    new_lines.append(line)

with open("crates/doom-app/src/cheats.rs", "w") as f:
    f.writelines(new_lines)
