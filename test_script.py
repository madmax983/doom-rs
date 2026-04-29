import sys

with open("crates/doom-app/src/main.rs", "r") as f:
    content = f.read()

import re
matches = list(re.finditer(r'IDDT', content))
print([m.span() for m in matches])
