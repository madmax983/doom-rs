import re

with open('crates/doom-game/src/specials.rs') as f:
    content = f.read()

# Look for large groupings of specials
print("Functions in specials.rs:")
for match in re.finditer(r'^pub fn ([a-zA-Z0-9_]+)\(', content, re.MULTILINE):
    print(match.group(1))
