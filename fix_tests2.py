import re

with open("crates/doom-map/src/bsp.rs", "r") as f:
    content = f.read()

# Remove the duplicate test test_cyclic_bsp_traverse_hang
pattern = r'    #\[test\]\n    fn test_cyclic_bsp_traverse_hang\(\) \{.*?\n    \}\n'
content = re.sub(pattern, '', content, flags=re.DOTALL)

with open("crates/doom-map/src/bsp.rs", "w") as f:
    f.write(content)
