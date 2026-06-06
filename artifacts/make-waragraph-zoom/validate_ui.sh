#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/artifacts/make-waragraph-zoom"
GRAPH="/home/erik/waragraph/c4.k311.poa2kb.gfa.zst"
BIN="$ROOT/target/debug/waragraph"

mkdir -p "$OUT"
rm -f "$OUT"/anchored-*.png "$OUT"/anchored.log "$OUT"/summary.txt

if [[ ! -x "$BIN" ]]; then
  cargo build -p waragraph
fi

WINIT_UNIX_BACKEND=x11 "$BIN" "$GRAPH" >"$OUT/anchored.log" 2>&1 &
pid=$!

cleanup() {
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
}
trap cleanup EXIT

win=""
for _ in $(seq 1 120); do
  if ! kill -0 "$pid" 2>/dev/null; then
    wait "$pid" || true
    echo "waragraph exited before window appeared" >&2
    exit 1
  fi
  win="$(xdotool search --onlyvisible --pid "$pid" --name "Waragraph 1D" | head -n 1 || true)"
  if [[ -n "$win" ]]; then
    break
  fi
  sleep 0.1
done

if [[ -z "$win" ]]; then
  echo "Waragraph 1D window not found" >&2
  exit 1
fi

xdotool windowsize "$win" 1200 850
xdotool windowmove "$win" 20 20
xdotool windowactivate "$win" 2>/dev/null || true
xdotool windowfocus "$win" 2>/dev/null || true
sleep 1

capture() {
  local target="$1"
  for _ in $(seq 1 20); do
    if kill -0 "$pid" 2>/dev/null && timeout 10s import -window "$win" "$target"; then
      return 0
    fi
    sleep 0.25
  done
  echo "failed to capture $target" >&2
  return 1
}

capture "$OUT/anchored-before.png"

# Off-center graph slot location. The repeated in/out cycles exercise the real
# wheel event path while the unit tests pin the exact coordinate invariant.
xdotool mousemove --window "$win" 330 360
for _ in $(seq 1 60); do
  xdotool click 4
  sleep 0.01
done
sleep 0.5
capture "$OUT/anchored-zoomed.png"

for _ in $(seq 1 20); do
  xdotool click 5
  xdotool click 4
  sleep 0.01
done
sleep 0.5
capture "$OUT/anchored-after-cycles.png"

python3 - <<'PY' > "$OUT/summary.txt"
from pathlib import Path
import re

out = Path("artifacts/make-waragraph-zoom")
log = (out / "anchored.log").read_text(errors="replace")
pixels = [float(x) for x in re.findall(r"pixels_per_bp: ([0-9.eE+-]+)", log)]

print("graph=/home/erik/waragraph/c4.k311.poa2kb.gfa.zst")
print(f"parsed_10928_edges={'parsed 10928 edges' in log}")
print(f"before_screenshot_exists={(out / 'anchored-before.png').exists()}")
print(f"zoomed_screenshot_exists={(out / 'anchored-zoomed.png').exists()}")
print(f"after_cycles_screenshot_exists={(out / 'anchored-after-cycles.png').exists()}")
if pixels:
    print(f"pixels_per_bp_first={pixels[0]}")
    print(f"pixels_per_bp_last={pixels[-1]}")
    print(f"pixels_per_bp_max={max(pixels)}")
    print(f"pixels_per_bp_samples={len(pixels)}")
else:
    print("pixels_per_bp_samples=0")
PY
