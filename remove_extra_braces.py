import re

with open('crates/doom-wad/src/stack.rs', 'r') as f:
    stack_content = f.read()

# Fix the if let which seems to have uncovered lines
# stack.rs: 413, 456 are uncovered lines. Let's see what they are.
