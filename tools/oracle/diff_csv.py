#!/usr/bin/env python3
import sys

# Columns: i,rndindex,px,py,pz,angle,health,kills,items,secrets,leveltime
COLS = ["i","rndindex","px","py","pz","angle","health","kills","items","secrets","leveltime"]
# Fields that count as a real sim divergence (rndindex ignored per DEMO_SYNC.md,
# though we ALSO report it separately since our oracle emits prndindex).
SIG = ["px","py","pz","angle","health","kills","items","secrets","leveltime"]

def load(p):
    rows=[]
    with open(p) as f:
        next(f)
        for line in f:
            line=line.strip()
            if not line: continue
            rows.append(line.split(","))
    return rows

def main():
    choco, doomrs = sys.argv[1], sys.argv[2]
    a=load(choco); b=load(doomrs)
    n=min(len(a),len(b))
    first_sig=None; first_field=None
    first_any=None; first_any_field=None
    for i in range(n):
        ra, rb = a[i], b[i]
        for f in COLS:
            j=COLS.index(f)
            if ra[j]!=rb[j]:
                if first_any is None:
                    first_any=i; first_any_field=f
                if f in SIG and first_sig is None:
                    first_sig=i; first_field=f
        if first_sig is not None:
            break
    if first_sig is None:
        print(f"  NO significant divergence across {n} tics (fields {SIG})")
    else:
        i=first_sig
        print(f"  FIRST SIGNIFICANT DIVERGENCE at tic i={i} (leveltime={a[i][COLS.index('leveltime')]}), field='{first_field}'")
        # context: 3 before + the diverging tic
        lo=max(0,i-3)
        hdr="tic  "+"".join(f"{c:>14}" for c in COLS)
        print("  "+hdr)
        for k in range(lo,i+1):
            for src,rows in (("CHOC",a),("DRS ",b)):
                marks=[]
                r=rows[k]
                cells=[]
                for c in COLS:
                    jj=COLS.index(c)
                    v=r[jj]
                    diff = (a[k][jj]!=b[k][jj])
                    cells.append(("*" if (diff and src=='DRS ') else " ")+f"{v:>13}")
                print(f"  {src} {k:>3} "+"".join(cells))
            print()
    if first_any is not None and first_any!=first_sig:
        print(f"  (incl rndindex) first ANY divergence at tic {first_any}, field='{first_any_field}'")

if __name__=="__main__":
    main()
