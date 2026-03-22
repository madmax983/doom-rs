import re

with open("crates/doom-game/src/specials.rs", "r") as f:
    content = f.read()

def replacer(match):
    before_let = match.group(1)
    let_stmt = match.group(2)
    for_loop = match.group(3)

    # Extract the iterator logic inside let_stmt
    # Something like `level.sectors.iter().enumerate().filter(|(_, s)| s.tag == tag).map(|(i, _)| (i, lowest_adjacent_floor(level, i))).collect();`
    # We want everything between `=` and `.collect();`
    iter_match = re.search(r'=\s*(.*?)\.collect\(\);', let_stmt, re.DOTALL)
    if not iter_match:
        return match.group(0) # fallback

    iter_logic = iter_match.group(1)

    # Find the variables in `for (idx, target) in per_sector {`
    for_match = re.search(r'for\s+\(([^,]+),\s*([^)]+)\)\s+in\s+[^{]+\{', for_loop)
    if not for_match:
        return match.group(0) # fallback

    idx_var = for_match.group(1).strip()
    target_var = for_match.group(2).strip()

    # Extract body of for loop
    body_match = re.search(r'for\s+[^{]+\{([^}]+)\}', for_loop, re.DOTALL)
    if not body_match:
        return match.group(0) # fallback

    body = body_match.group(1)

    return f"{before_let}for ({idx_var}, {target_var}) in {iter_logic} {{\n{body}\n    }}"


pattern = r'(pub fn ev_floor_[^{]+\{[^{}]*)\s*let\s+per_sector:\s*Vec<\(usize,\s*i16\)>[^;]+;\s*(for\s+\([^)]+\)\s+in\s+per_sector\s*\{[^}]+\})'
new_content = re.sub(pattern, replacer, content)

# Check if there are other occurrences
print("Modified blocks:")
for match in re.finditer(pattern, content):
    print(match.group(1))

with open("crates/doom-game/src/specials.rs", "w") as f:
    f.write(new_content)
