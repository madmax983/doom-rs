import os
import re

files = [
    "crates/doom-types/src/angle.rs",
    "crates/doom-types/src/bbox.rs",
    "crates/doom-types/src/compat.rs",
    "crates/doom-types/src/fixed.rs",
    "crates/doom-types/src/primitives.rs",
    "crates/doom-types/src/ticcmd.rs",
    "crates/doom-types/src/vec2.rs"
]

# We will just revert the files and re-apply cleanly.
# Wait, reverting loses the changes. I will manually fix them using regex.

def fix_file(filepath):
    with open(filepath, "r") as f:
        content = f.read()

    # Find the pattern where we have:
    # #[derive(...)]
    # ///
    # /// ## Examples
    # /// ```
    # /// ...
    # /// ```
    # pub struct ...

    # We want to move the examples above the #[derive] macro.

    # Let's find all such patterns
    pattern = r'(#\[.*?\]\n)+(///\n/// ## Examples\n/// ```\n.*?/// ```\n)(pub (struct|enum) [A-Za-z0-9_]+)'

    def replacer(match):
        derives = match.group(1)
        examples = match.group(2)
        decl = match.group(3)
        return examples + derives + decl

    new_content = re.sub(pattern, replacer, content, flags=re.DOTALL)

    # Also check if primitives already had examples
    if "primitives.rs" in filepath:
        # If there are two ``` blocks back to back, or an example block right after another, let's just remove the one we added.
        # Actually, let's check if the file has duplicate examples.
        pass

    with open(filepath, "w") as f:
        f.write(new_content)

for f in files:
    fix_file(f)
