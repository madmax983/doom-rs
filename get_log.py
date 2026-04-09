import re

with open('tarpaulin_out.log', 'r') as f:
    text = f.read()

matches = re.findall(r'\|\| (.*?): (\d+)/(\d+)', text)
lowest = []
for file, covered, total in matches:
    if int(total) > 20: # ignore trivial files
        ratio = int(covered) / int(total)
        lowest.append((ratio, file, covered, total))

lowest.sort()
for r, f, c, t in lowest[:40]:
    print(f"{f}: {c}/{t} ({r*100:.2f}%)")
