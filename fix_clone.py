import re

with open("crates/doom-map/src/bsp.rs", "r") as f:
    content = f.read()

content = content.replace("bbox.clone()", "bbox")

with open("crates/doom-map/src/bsp.rs", "w") as f:
    f.write(content)
