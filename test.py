with open("crates/doom-app/src/cheats.rs", "r") as f:
    lines = f.readlines()

new_lines = []
for i, line in enumerate(lines):
    if line.strip() == "pub(crate) struct CheatDef {":
        new_lines.append("///\n")
        new_lines.append("/// # Examples\n")
        new_lines.append("/// ```\n")
        new_lines.append("/// use doom_game::cheats::CheatDef;\n")
        new_lines.append("///\n")
        new_lines.append("/// let cheat = CheatDef {\n")
        new_lines.append("///     name: \"IDDQD\",\n")
        new_lines.append("///     sequence: \"iddqd\",\n")
        new_lines.append("/// };\n")
        new_lines.append("/// assert_eq!(cheat.name, \"IDDQD\");\n")
        new_lines.append("/// ```\n")
    if line.strip() == "pub(crate) struct CheatDetector {":
        new_lines.append("///\n")
        new_lines.append("/// # Examples\n")
        new_lines.append("/// ```\n")
        new_lines.append("/// use doom_game::cheats::CheatDetector;\n")
        new_lines.append("///\n")
        new_lines.append("/// let mut det = CheatDetector::new();\n")
        new_lines.append("/// for ch in \"iddqd\".chars() {\n")
        new_lines.append("///     det.feed(ch);\n")
        new_lines.append("/// }\n")
        new_lines.append("/// ```\n")
    if line.strip() == "pub(crate) fn apply_cheat(gs: &mut GameState, cheat_name: &str) -> &'static str {":
        new_lines.insert(len(new_lines) - 2, "///\n")
        new_lines.insert(len(new_lines) - 2, "/// # Examples\n")
        new_lines.insert(len(new_lines) - 2, "/// ```\n")
        new_lines.insert(len(new_lines) - 2, "/// use doom_game::cheats::apply_cheat;\n")
        new_lines.insert(len(new_lines) - 2, "/// use doom_game::GameState;\n")
        new_lines.insert(len(new_lines) - 2, "///\n")
        new_lines.insert(len(new_lines) - 2, "/// let mut gs = GameState::new(\"E1M1\");\n")
        new_lines.insert(len(new_lines) - 2, "/// let msg = apply_cheat(&mut gs, \"IDDQD\");\n")
        new_lines.insert(len(new_lines) - 2, "/// assert_eq!(msg, \"Degreelessness mode ON\");\n")
        new_lines.insert(len(new_lines) - 2, "/// ```\n")
    new_lines.append(line)

with open("crates/doom-app/src/cheats.rs", "w") as f:
    f.writelines(new_lines)
