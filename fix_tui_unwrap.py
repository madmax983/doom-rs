import re

files = [
    "crates/doom-tui/src/event_loop.rs",
    "crates/doom-tui/src/widget.rs",
    "crates/doom-tui/src/cogmind.rs",
]

for file_path in files:
    with open(file_path, "r") as f:
        content = f.read()

    # Replace unwrap with expect in tests
    content = re.sub(
        r"MODIFIER_COUNT_LOCK\.lock\(\)\.unwrap\(\)",
        r'MODIFIER_COUNT_LOCK.lock().expect("Failed to lock MODIFIER_COUNT_LOCK")',
        content
    )
    content = re.sub(
        r"buf\.cell\((.*?)\)\.unwrap\(\)",
        r'buf.cell(\1).expect("Failed to get cell")',
        content
    )
    content = re.sub(
        r"frame\.get\((.*?)\)\.unwrap\(\)",
        r'frame.get(\1).expect("Failed to get frame cell")',
        content
    )

    with open(file_path, "w") as f:
        f.write(content)
