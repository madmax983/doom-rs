#!/usr/bin/env python3
import sys
COLS=["i","rndindex","px","py","pz","angle","health","kills","items","secrets","leveltime"]
def load(p):
    rows=[]
    with open(p) as f:
        next(f)
        for line in f:
            line=line.strip()
            if line: rows.append(line.split(","))
    return rows
for demo in ("1","2","3"):
    a=load(f"oracle/demo{demo}.choco.csv"); b=load(f"demo{demo}.doomrs.csv")
    n=min(len(a),len(b))
    firsts={c:None for c in COLS}
    for i in range(n):
        for c in COLS:
            j=COLS.index(c)
            if firsts[c] is None and a[i][j]!=b[i][j]:
                firsts[c]=i
    print(f"DEMO{demo}: rows={n}  first-divergence tic per field (leveltime=tic+1):")
    for c in COLS:
        if c=="i" or c=="leveltime": continue
        v=firsts[c]
        print(f"   {c:>8}: {'none' if v is None else f'tic {v} (lt {v+1})'}")
    print()
