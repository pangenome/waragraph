#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/artifacts/default-waragraph-to"
GRAPH="/home/erik/waragraph/c4.k311.poa2kb.gfa.zst"
BIN="$ROOT/target/debug/waragraph"

mkdir -p "$OUT"

run_case() {
  local name="$1"
  shift
  local log="$OUT/${name}.log"
  local shot="$OUT/${name}.png"

  rm -f "$log" "$shot"

  WINIT_UNIX_BACKEND=x11 "$BIN" "$@" "$GRAPH" >"$log" 2>&1 &
  local pid=$!

  local win=""
  for _ in $(seq 1 120); do
    win="$(xdotool search --onlyvisible --name "Waragraph 1D" | head -n 1 || true)"
    if [[ -n "$win" ]]; then
      break
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      wait "$pid" || true
      echo "waragraph exited before window appeared for $name" >&2
      return 1
    fi
    sleep 0.1
  done

  if [[ -z "$win" ]]; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    echo "Waragraph 1D window not found for $name" >&2
    return 1
  fi

  xdotool windowsize "$win" 1200 850
  xdotool windowmove "$win" 20 20
  xdotool windowactivate "$win" 2>/dev/null || true
  xdotool windowfocus "$win" 2>/dev/null || true
  sleep 1
  timeout 10s import -window "$win" "$shot"

  if [[ "$name" == "default-depth" ]]; then
    xdotool mousemove --window "$win" 400 360
    for _ in $(seq 1 150); do
      xdotool click 4
      sleep 0.015
    done
    sleep 1
    timeout 10s import -window "$win" "$OUT/${name}-zoomed.png"
  fi

  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
}

run_case default-depth
run_case path-name --view-mode path-name

python3 - <<'PY' > "$OUT/summary.txt"
from pathlib import Path
import re

out = Path("artifacts/default-waragraph-to")

for name in ("default-depth", "path-name"):
    log = (out / f"{name}.log").read_text(errors="replace")
    mode = re.search(r"Viewer1D initial visualization mode: ([A-Za-z0-9_]+)", log)
    edges = "parsed 10928 edges" in log
    pixels = [float(x) for x in re.findall(r"pixels_per_bp: ([0-9.eE+-]+)", log)]
    print(f"{name}_mode={mode.group(1) if mode else 'missing'}")
    print(f"{name}_parsed_10928={str(edges).lower()}")
    if pixels:
        print(f"{name}_first_pixels_per_bp={pixels[0]}")
        print(f"{name}_last_pixels_per_bp={pixels[-1]}")
        print(f"{name}_max_pixels_per_bp={max(pixels)}")
    print(f"{name}_screenshot_exists={(out / f'{name}.png').exists()}")

zoomed = out / "default-depth-zoomed.png"
print(f"default-depth-zoomed_screenshot_exists={zoomed.exists()}")
PY
