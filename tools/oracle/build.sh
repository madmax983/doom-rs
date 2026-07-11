#!/usr/bin/env bash
# Build the instrumented Chocolate Doom demo-sync oracle.
# Reproduces the reference binary used to validate doom-rs demo parity.
#
# The clone lives in $ORACLE_BUILD_DIR (default: <this-dir>/chocolate-doom, which
# is gitignored — see tools/oracle/.gitignore). Point ORACLE_BUILD_DIR at a temp
# dir if you prefer to keep the working tree pristine, e.g.
#   ORACLE_BUILD_DIR=$(mktemp -d)/oracle ./build.sh
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLONE_URL="https://github.com/chocolate-doom/chocolate-doom.git"
PIN_SHA="353cf5001dfd5777c13327010fa58acb57b913b2"   # master, 2026-06-08
SRC="${ORACLE_BUILD_DIR:-$HERE/chocolate-doom}"

# 1. Build dependencies (Ubuntu/Debian). Run once.
#    NOTE: `sudo apt-get update` first — a stale index 404s on libsystemd-dev.
if [ "${SKIP_APT:-0}" != "1" ]; then
  sudo apt-get update
  sudo apt-get install -y \
    automake autoconf pkg-config \
    libsdl2-dev libsdl2-mixer-dev libsdl2-net-dev
fi

# 2. Clone + pin the exact commit.
if [ ! -d "$SRC" ]; then
  git clone "$CLONE_URL" "$SRC"
fi
git -C "$SRC" checkout "$PIN_SHA"

# 3. Apply the demo-sync instrumentation patch (idempotent).
if ! git -C "$SRC" apply --reverse --check "$HERE/oracle-instrumentation.patch" 2>/dev/null; then
  git -C "$SRC" apply "$HERE/oracle-instrumentation.patch"
fi

# 4. Configure + build. autogen.sh runs autoreconf + ./configure.
cd "$SRC"
./autogen.sh
make -j"$(nproc)"

echo "Built oracle: $SRC/src/chocolate-doom"
