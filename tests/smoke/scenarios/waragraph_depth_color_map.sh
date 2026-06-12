#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."
repo_root="$(pwd)/.."
cd "$repo_root"

if [[ -z "${DISPLAY:-}" ]]; then
  echo "SKIP: DISPLAY is not set; cannot validate depth color map UI" >&2
  exit 77
fi

for tool in xdotool import convert timeout; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "SKIP: $tool is required for depth color map UI validation" >&2
    exit 77
  fi
done

if [[ ! -r /home/erik/waragraph/c4.k311.poa2kb.gfa.zst ]]; then
  echo "SKIP: /home/erik/waragraph/c4.k311.poa2kb.gfa.zst is not available" >&2
  exit 77
fi

cargo build -p waragraph
artifacts/match-waragraph-depth/validate_ui.sh

summary="artifacts/match-waragraph-depth/summary.txt"
if ! grep -q '^default-depth-c4_default_view_mode=depth$' "$summary"; then
  echo "FAIL: default c4 launch did not use depth view" >&2
  cat "$summary" >&2
  exit 1
fi

if ! grep -q '^default-depth-c4_parsed_10928_edges=true$' "$summary"; then
  echo "FAIL: zstd c4 graph did not parse as expected" >&2
  cat "$summary" >&2
  exit 1
fi

for case in controlled-depth controlled-high-depth; do
  for color in gray red orange yellow green blue indigo violet; do
    key="${case}_${color}_pixel_count"
    value="$(awk -F= -v key="$key" '$1 == key { print $2 }' "$summary")"
    if [[ -z "$value" || "$value" == "0" ]]; then
      echo "FAIL: expected nonzero $key in depth color UI summary" >&2
      cat "$summary" >&2
      exit 1
    fi
  done

  key="${case}_sampled_higher_coverage_colored_pixels"
  value="$(awk -F= -v key="$key" '$1 == key { print $2 }' "$summary")"
  if [[ -z "$value" || "$value" == "0" ]]; then
    echo "FAIL: expected nonzero $key in depth color UI summary" >&2
    cat "$summary" >&2
    exit 1
  fi
done

echo "PASS: default depth view and gray-to-ROYGBIV path-depth colors validated"
