import re

with open(".jules/atlas.md", "r") as f:
    text = f.read()

bad_entry = """## YYYY-MM-DD - Move Skill enum
**Tangle:**  had circular dependencies  because  enum was in  but used by .
**Blueprint:** Extracted  enum to a new  module in  and updated imports.

## YYYY-MM-DD - Reduce pub visibility in binary
**Tangle:**  (a binary crate) was exposing many types as  internally (e.g. , , ), creating a 'Leaky Abstraction' intent despite being fundamentally private.
**Blueprint:** Changed  to  across internal modules (, , , , , , ) to tighten boundaries.

"""

text = text.replace(bad_entry, "")

with open(".jules/atlas.md", "w") as f:
    f.write(text)
