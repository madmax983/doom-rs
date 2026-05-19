import re

with open("crates/doom-map/src/graph.rs", "r") as f:
    text = f.read()

# look for pub fn with no examples
functions = re.findall(r'pub fn \w+\(', text)
print("Functions found:", functions)
