import re
import os

with open("crates/doom-game/src/specials.rs", "r") as f:
    content = f.read()

# I want to split out tests first as they are likely the majority of the 9999 lines
test_start = content.find("#[cfg(test)]\nmod tests {")
if test_start != -1:
    print("Found test module start at char:", test_start)
    print("Total length:", len(content))
