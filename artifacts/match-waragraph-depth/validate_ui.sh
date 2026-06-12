#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/artifacts/match-waragraph-depth"
C4_GRAPH="/home/erik/waragraph/c4.k311.poa2kb.gfa.zst"
CONTROLLED_GRAPH="$OUT/controlled-depth.gfa"
CONTROLLED_HIGH_DEPTH_GRAPH="$OUT/controlled-high-depth.gfa"
BIN="$ROOT/target/debug/waragraph"

mkdir -p "$OUT"

if [[ ! -r "$C4_GRAPH" ]]; then
  echo "missing_graph=$C4_GRAPH" > "$OUT/summary.txt"
  exit 1
fi

summary="$OUT/summary.txt"

rm -f \
  "$OUT"/default-depth-c4.log \
  "$OUT"/default-depth-c4.png \
  "$OUT"/default-depth-c4.histogram.txt \
  "$OUT"/controlled-depth.log \
  "$OUT"/controlled-depth.png \
  "$OUT"/controlled-depth.histogram.txt \
  "$OUT"/controlled-high-depth.log \
  "$OUT"/controlled-high-depth.png \
  "$OUT"/controlled-high-depth.histogram.txt \
  "$summary"

path_nodes() {
  local start="$1"
  local nodes=()
  local node
  for node in $(seq "$start" 8); do
    nodes+=("${node}+")
  done
  local IFS=,
  printf "%s" "${nodes[*]}"
}

write_high_depth_graph() {
  {
    printf 'H\tVN:Z:1.0\n'
    for node in $(seq 1 8); do
      printf 'S\t%s\tAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n' "$node"
    done
    for node in $(seq 1 7); do
      printf 'L\t%s\t+\t%s\t+\t0M\n' "$node" "$((node + 1))"
    done
    for path in $(seq 1 203); do
      printf 'P\tshared_%03d\t%s\t*\n' "$path" "$(path_nodes 1)"
    done
    for start in $(seq 2 8); do
      printf 'P\textra_%s\t%s\t*\n' "$start" "$(path_nodes "$start")"
    done
  } > "$CONTROLLED_HIGH_DEPTH_GRAPH"
}

write_high_depth_graph

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
run_case controlled-high-depth "$CONTROLLED_HIGH_DEPTH_GRAPH"

python3 - <<'PY' > "$summary"
from pathlib import Path
import re

out = Path("artifacts/match-waragraph-depth")

palette = {
    "gray": (128, 128, 128),
    "red": (228, 26, 28),
    "orange": (255, 127, 0),
    "yellow": (255, 255, 51),
    "green": (77, 175, 74),
    "blue": (55, 126, 184),
    "indigo": (75, 0, 130),
    "violet": (148, 0, 211),
}

def count_rgb(hist, rgb):
    pattern = rf"\s*(\d+): \({rgb[0]},{rgb[1]},{rgb[2]}\)"
    m = re.search(pattern, hist)
    return int(m.group(1)) if m else 0

print("source_palette=waragraph gray_to_roygbiv_path_depth")

for case, graph in [
    ("default-depth-c4", "/home/erik/waragraph/c4.k311.poa2kb.gfa.zst"),
    ("controlled-depth", "artifacts/match-waragraph-depth/controlled-depth.gfa"),
    ("controlled-high-depth", "artifacts/match-waragraph-depth/controlled-high-depth.gfa"),
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
        if name != "gray"
    )
    print(f"{case}_sampled_higher_coverage_colored_pixels={colored}")
PY
