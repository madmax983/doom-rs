import os
import re

def process_dir(directory):
    for root, _, files in os.walk(directory):
        for file in files:
            if file.endswith(".rs"):
                file_path = os.path.join(root, file)
                with open(file_path, "r") as f:
                    content = f.read()

                # Replace unwrap with expect in tests
                if 'unwrap()' in content and 'test' in file_path or 'tests' in content:
                    content = content.replace('.unwrap()', '.expect("Expected successful result in test")')

                    with open(file_path, "w") as f:
                        f.write(content)

process_dir("crates/doom-map/src")
