#!/bin/sh
# The WebAssembly size budget, checked by CI and by scripts/ci-local.sh after
# `pnpm wasm`. A tripwire for load time, not a target: raised when features
# need the room (3,000,000 bytes until 2026-10-12; the 3D side 3,500,000
# until the drape went on the graphics card, 2026-10-14).

cd "$(dirname "$0")/.." || exit 1
status=0

# check FILE BUDGET: FILE is smaller than BUDGET bytes.
check() {
  if [ ! -f "$1" ]; then
    echo "$1 is missing: run pnpm wasm first"
    status=1
    return
  fi
  size=$(wc -c <"$1" | tr -d ' ')
  echo "$(basename "$1"): $size bytes (budget $2)"
  if [ "$size" -ge "$2" ]; then
    echo "  over the budget: if a feature needs the room, raise it in $0"
    status=1
  fi
}

check packages/engine/pkg/drapewright_wasm_core_bg.wasm 3500000
check packages/engine/pkg-3d/drapewright_wasm_3d_bg.wasm 4000000
exit $status
