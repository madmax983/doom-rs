#!/usr/bin/env bash
# Run the instrumented Chocolate Doom oracle on a shareware demo, headless,
# and emit a per-tic ground-truth CSV directly diffable against doom-rs's
# --verify-log output.
#
# Usage:   ./run.sh <demoname> <out.csv> [iwad]
# Example: ./run.sh demo2 demo2.choco.csv ../../test-wads/doom1.wad
#
# The oracle binary is taken from $ORACLE_BUILD_DIR (default:
# <this-dir>/chocolate-doom, the same location build.sh clones into).
#
# Columns emitted (same as doom-app --verify-log):
#   i,rndindex,px,py,pz,angle,health,kills,items,secrets,leveltime
# Sampling point: end of P_Ticker (after player think + thinkers + specials,
# post leveltime++), one row per tic, i == leveltime-1 (0-based).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
SRC="${ORACLE_BUILD_DIR:-$HERE/chocolate-doom}"
BIN="$SRC/src/chocolate-doom"
DEMO="${1:-demo2}"
OUT="${2:-$HERE/${DEMO}.choco.csv}"
# Default IWAD: shareware doom1.wad in the repo's (gitignored) test-wads/.
# md5 f0cefca49926d00903cf57551d901abe
WAD="${3:-$REPO_ROOT/test-wads/doom1.wad}"
CFG="$(mktemp -d)"

# -timedemo runs the demo uncapped (fast) then quits; the simulation tics are
# identical to -playdemo. dummy SDL drivers = no window, no audio. CHOCO_CSV
# tells the patched P_Ticker where to write the trace.
SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy CHOCO_CSV="$OUT" \
  "$BIN" -iwad "$WAD" -timedemo "$DEMO" -nosound -nomusic \
  -config "$CFG/default.cfg" -extraconfig "$CFG/extra.cfg" \
  >/dev/null 2>&1 || true   # chocolate-doom exits non-zero via I_Quit after timedemo

rows=$(($(wc -l < "$OUT") - 1))
echo "wrote $OUT ($rows tic rows)"
