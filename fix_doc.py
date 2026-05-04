import re

with open("crates/doom-map/src/bsp.rs", "r") as f:
    content = f.read()

# Fix the empty line after doc comment issue
content = content.replace("    /// Verifies `N_SSECTORS == N_NODES + 1`.\n\n    /// Verifies that the BSP tree", "    /// Verifies that the BSP tree")

with open("crates/doom-map/src/bsp.rs", "w") as f:
    f.write(content)
