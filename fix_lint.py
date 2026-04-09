import re

with open('crates/doom-app/src/cogmind/glyphs.rs', 'r') as f:
    content = f.read()

# restore original import and put it inside the test function specifically
content = content.replace(
    "    #[test]\n    fn all_mobj_kinds_have_glyphs() {\n",
    "    #[test]\n    fn all_mobj_kinds_have_glyphs() {\n        use strum::IntoEnumIterator;\n"
)

with open('crates/doom-app/src/cogmind/glyphs.rs', 'w') as f:
    f.write(content)
