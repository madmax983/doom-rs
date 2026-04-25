import os
import re

def process_dir(directory):
    for root, _, files in os.walk(directory):
        for file in files:
            if file.endswith(".rs"):
                file_path = os.path.join(root, file)
                with open(file_path, "r") as f:
                    content = f.read()

                # Find test module and replace unwraps within it
                # We do this generally as replacing `.unwrap()` with `.expect("...")` in test files.
                if 'unwrap()' in content and file_path.endswith('.rs'):
                    content = content.replace('.unwrap()', '.expect("Expected successful result in test")')

                    with open(file_path, "w") as f:
                        f.write(content)

process_dir("crates/doom-demo/src")
