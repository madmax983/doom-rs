with open("crates/doom-game/src/state.rs", "r") as f:
    content = f.read()

# Make sure `use crate::movers::*;` is there.
if "pub use crate::movers::*;" not in content and "use crate::movers::*;" not in content:
    imports_end = content.find("\n// ---")
    if imports_end != -1:
        content = content[:imports_end] + "\npub use crate::movers::*;\n" + content[imports_end:]

with open("crates/doom-game/src/state.rs", "w") as f:
    f.write(content)
