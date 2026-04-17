import re

with open("crates/doom-types/src/primitives.rs", "r") as f:
    content = f.read()

# We need to find:
# #[derive(...)]
# ///
# /// ## Examples
# /// ```
# /// ...
# /// ```
# pub struct ...
#
# And move it above the derive macro

def replacer(match):
    derives = match.group(1)
    examples = match.group(2)
    struct_decl = match.group(3)
    return examples + derives + struct_decl

pattern = r'(#\[derive\([^\]]+\)\]\n)+(///\n/// ## Examples\n/// ```\n.*?/// ```\n)(pub struct [A-Za-z0-9_]+)'
new_content = re.sub(pattern, replacer, content, flags=re.DOTALL)

with open("crates/doom-types/src/primitives.rs", "w") as f:
    f.write(new_content)
