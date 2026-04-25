import re

with open("crates/doom-app/src/demo_mode.rs", "r") as f:
    content = f.read()

# Add #[allow(dead_code)] to inner methods
content = re.sub(
    r"    pub\(crate\) fn inner\(&self\) -> &DoomGame \{\n        &self\.inner\n    \}\n",
    "    #[allow(dead_code)]\n    pub(crate) fn inner(&self) -> &DoomGame {\n        &self.inner\n    }\n",
    content
)

with open("crates/doom-app/src/demo_mode.rs", "w") as f:
    f.write(content)
