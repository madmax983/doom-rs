import sys

def replace_in_file(filepath, old, new):
    with open(filepath, 'r') as f:
        content = f.read()
    if old not in content:
        print(f"'{old}' not found in {filepath}!")
        return
    content = content.replace(old, new)
    with open(filepath, 'w') as f:
        f.write(content)
    print(f"Replaced in {filepath}")

replace_in_file(
    'crates/doom-audio/src/opl/mod.rs',
    '*s = mix.clamp(-1.0, 1.0);',
    '*s = if mix.is_nan() { 0.0 } else { mix }.clamp(-1.0, 1.0);'
)
