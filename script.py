with open('crates/doom-map/src/analyzer.rs') as f:
    lines = f.readlines()
for i, line in enumerate(lines[110:145]):
    print(f"{i+111:3d}: {line.rstrip()}")
