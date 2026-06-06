#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/artifacts/match-waragraph-depth"
C4_GRAPH="/home/erik/waragraph/c4.k311.poa2kb.gfa.zst"
CONTROLLED_GRAPH="$OUT/controlled-depth.gfa"
BIN="$ROOT/target/debug/waragraph"

mkdir -p "$OUT"

if [[ ! -r "$C4_GRAPH" ]]; then
  echo "missing_graph=$C4_GRAPH" > "$OUT/summary.txt"
  exit 1
fi

summary="$OUT/summary.txt"

rm -f "$OUT"/*.log "$OUT"/*.png "$OUT"/*.histogram.txt "$summary"

run_case() {
  local name="$1"
  local graph="$2"
  local log="$OUT/${name}.log"
  local shot="$OUT/${name}.png"
  local hist="$OUT/${name}.histogram.txt"

  WINIT_UNIX_BACKEND=x11 "$BIN" "$graph" >"$log" 2>&1 &
  pid=$!
  trap 'kill "$pid" 2>/dev/null || true' EXIT

  win=""
  for _ in $(seq 1 120); do
    win="$(xdotool search --onlyvisible --name "Waragraph 1D" | head -n 1 || true)"
    if [[ -n "$win" ]]; then
      break
    fi
    if ! kill -0 "$pid" 2>/dev/null; then
      wait "$pid" || true
      echo "waragraph exited before window appeared for $name" >&2
      exit 1
    fi
    sleep 0.1
  done

  if [[ -z "$win" ]]; then
    echo "Waragraph 1D window not found for $name" >&2
    exit 1
  fi

  xdotool windowsize "$win" 1200 850
  xdotool windowmove "$win" 20 20
  xdotool windowactivate "$win" 2>/dev/null || true
  xdotool windowfocus "$win" 2>/dev/null || true
  sleep 2
  timeout 10s import -window "$win" "$shot"
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  trap - EXIT

  convert "$shot" -format "%c" histogram:info:- > "$hist"
}

run_case default-depth-c4 "$C4_GRAPH"
run_case controlled-depth "$CONTROLLED_GRAPH"

python3 - <<'PY' > "$summary"
from pathlib import Path
import re

out = Path("artifacts/match-waragraph-depth")

palette = {
    "light_gray_0_5x": (196, 196, 196),
    "dark_gray_1x": (128, 128, 128),
    "spectral_2x": (158, 1, 66),
    "spectral_3x": (213, 62, 79),
    "spectral_4x": (244, 109, 67),
}

def count_rgb(hist, rgb):
    pattern = rf"\s*(\d+): \({rgb[0]},{rgb[1]},{rgb[2]}\)"
    m = re.search(pattern, hist)
    return int(m.group(1)) if m else 0

print(f"source_gfalook=/home/erik/gfalook/src/main.rs COLORBREWER_SPECTRAL_13 get_depth_color")

for case, graph in [
    ("default-depth-c4", "/home/erik/waragraph/c4.k311.poa2kb.gfa.zst"),
    ("controlled-depth", "artifacts/match-waragraph-depth/controlled-depth.gfa"),
]:
    log = (out / f"{case}.log").read_text(errors="replace")
    hist = (out / f"{case}.histogram.txt").read_text(errors="replace")
    mode = re.search(r"Viewer1D initial visualization mode: ([A-Za-z0-9_]+)", log)
    print(f"{case}_source_graph={graph}")
    print(f"{case}_default_view_mode={mode.group(1) if mode else 'missing'}")
    print(f"{case}_screenshot_exists={(out / f'{case}.png').exists()}")
    if case == "default-depth-c4":
        print(f"{case}_parsed_10928_edges={'true' if 'parsed 10928 edges' in log else 'false'}")
    for name, rgb in palette.items():
        print(f"{case}_{name}_rgb={rgb[0]},{rgb[1]},{rgb[2]}")
        print(f"{case}_{name}_pixel_count={count_rgb(hist, rgb)}")
    colored = sum(
        count_rgb(hist, rgb)
        for name, rgb in palette.items()
        if name.startswith("spectral_")
    )
    print(f"{case}_sampled_higher_coverage_colored_pixels={colored}")
PY
