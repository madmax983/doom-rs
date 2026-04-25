import re

with open("crates/doom-app/src/demo_mode.rs", "r") as f:
    content = f.read()

# Replace inner method in DemoRecordingWrapper
content = re.sub(
    r"impl DemoRecordingWrapper \{\n    pub\(crate\) fn inner\(&self\) -> &DoomGame \{\n        &self\.inner\n    \}\n",
    "impl DemoRecordingWrapper {\n",
    content
)

# Replace inner method in DemoPlaybackApp
content = re.sub(
    r"impl DemoPlaybackApp \{\n    pub\(crate\) fn inner\(&self\) -> &DoomGame \{\n        &self\.inner\n    \}\n",
    "impl DemoPlaybackApp {\n",
    content
)

with open("crates/doom-app/src/demo_mode.rs", "w") as f:
    f.write(content)
