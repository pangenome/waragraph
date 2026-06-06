#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."
repo_root="$(pwd)/.."
cd "$repo_root"

if [[ -z "${DISPLAY:-}" ]]; then
  echo "SKIP: DISPLAY is not set; cannot validate visible window controls" >&2
  exit 77
fi

for tool in xdotool xwininfo xprop timeout; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "SKIP: $tool is required for visible window control validation" >&2
    exit 77
  fi
done

data_path="/home/erik/waragraph/c4.k311.poa2kb.gfa.zst"
if [[ ! -f "$data_path" ]]; then
  data_path="test/data/A-3105.fa.353ea42.34ee7b1.1576367.smooth.fix.gfa"
fi
if [[ ! -f "$data_path" ]]; then
  echo "SKIP: no graph data file available for visible window launch" >&2
  exit 77
fi

cargo build -p waragraph

artifact_dir="artifacts/restore-usable-window"
mkdir -p "$artifact_dir"

cleanup_pid=""
cleanup() {
  if [[ -n "$cleanup_pid" ]] && kill -0 "$cleanup_pid" >/dev/null 2>&1; then
    kill "$cleanup_pid" >/dev/null 2>&1 || true
    wait "$cleanup_pid" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

window_id_for_pid() {
  local pid="$1"
  local deadline=$((SECONDS + 15))
  while (( SECONDS < deadline )); do
    local ids
    ids="$(xdotool search --sync --onlyvisible --pid "$pid" --name 'Waragraph 1D' 2>/dev/null || true)"
    if [[ -n "$ids" ]]; then
      echo "$ids" | tail -n1
      return 0
    fi
    sleep 0.25
  done
  return 1
}

window_size() {
  xwininfo -id "$1" |
    awk '/Width:/ { w = $2 } /Height:/ { h = $2 } END { print w " " h }'
}

export WINIT_UNIX_BACKEND=x11
target/debug/waragraph "$data_path" >"$artifact_dir/x11-default.log" 2>&1 &
cleanup_pid="$!"

window_id="$(window_id_for_pid "$cleanup_pid")" || {
  echo "FAIL: default Waragraph window did not become visible" >&2
  exit 1
}

xprop -id "$window_id" >"$artifact_dir/x11-default.xprop"
xwininfo -id "$window_id" >"$artifact_dir/x11-default.xwininfo"

motif_hints="$(grep '_MOTIF_WM_HINTS' "$artifact_dir/x11-default.xprop" || true)"
if [[ "$motif_hints" =~ 0x2,\ 0x0,\ 0x0, ]]; then
  echo "FAIL: default window disables Motif decorations; expected normal decorated window" >&2
  exit 1
fi

xdotool windowactivate --sync "$window_id"
before_resize="$(window_size "$window_id")"
xdotool windowsize "$window_id" 640 480
sleep 1
after_resize="$(window_size "$window_id")"
if [[ "$before_resize" == "$after_resize" ]]; then
  echo "FAIL: window size did not change after window-manager resize request ($before_resize)" >&2
  exit 1
fi

cleanup
cleanup_pid=""

target/debug/waragraph --fullscreen "$data_path" \
  >"$artifact_dir/x11-start-fullscreen.log" 2>&1 &
cleanup_pid="$!"
start_fullscreen_window_id="$(window_id_for_pid "$cleanup_pid")" || {
  echo "FAIL: --fullscreen Waragraph window did not become visible" >&2
  exit 1
}

start_fullscreen_size="$(window_size "$start_fullscreen_window_id")"
if [[ "$start_fullscreen_size" == "800 600" ]]; then
  echo "FAIL: --fullscreen window still has default 800x600 size" >&2
  exit 1
fi

cleanup
cleanup_pid=""

target/debug/waragraph "$data_path" >"$artifact_dir/x11-f11.log" 2>&1 &
cleanup_pid="$!"
fullscreen_window_id="$(window_id_for_pid "$cleanup_pid")" || {
  echo "FAIL: F11 test Waragraph window did not become visible" >&2
  exit 1
}

before_fullscreen="$(window_size "$fullscreen_window_id")"
xdotool windowactivate --sync "$fullscreen_window_id"
xdotool key F11
sleep 1
after_fullscreen="$(window_size "$fullscreen_window_id")"
if [[ "$before_fullscreen" == "$after_fullscreen" ]]; then
  echo "FAIL: window size did not change after F11 fullscreen ($before_fullscreen)" >&2
  exit 1
fi

xdotool key F11
sleep 1
after_exit_fullscreen="$(window_size "$fullscreen_window_id")"
if [[ "$after_exit_fullscreen" == "$after_fullscreen" ]]; then
  echo "FAIL: window size did not change after exiting F11 fullscreen ($after_fullscreen)" >&2
  exit 1
fi

cleanup
cleanup_pid=""

if [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
  unset WINIT_UNIX_BACKEND
  status=0
  timeout 8s target/debug/waragraph "$data_path" \
    >"$artifact_dir/wayland-session-default.log" 2>&1 || status=$?
  if [[ "$status" != "0" && "$status" != "124" ]]; then
    echo "FAIL: Wayland-session default launch exited unexpectedly with status $status" >&2
    cat "$artifact_dir/wayland-session-default.log" >&2
    exit 1
  fi
  if grep -q 'Buffer size .* buffer_scale' "$artifact_dir/wayland-session-default.log"; then
    echo "FAIL: Wayland-session default launch regressed into buffer_scale crash" >&2
    cat "$artifact_dir/wayland-session-default.log" >&2
    exit 1
  fi

  status=0
  WINIT_UNIX_BACKEND=wayland timeout 8s \
    target/debug/waragraph --borderless "$data_path" \
    >"$artifact_dir/wayland-native-borderless.log" 2>&1 || status=$?
  if [[ "$status" != "0" && "$status" != "124" ]]; then
    echo "FAIL: native Wayland --borderless launch exited unexpectedly with status $status" >&2
    cat "$artifact_dir/wayland-native-borderless.log" >&2
    exit 1
  fi
  if grep -q 'Buffer size .* buffer_scale' "$artifact_dir/wayland-native-borderless.log"; then
    echo "FAIL: native Wayland --borderless launch regressed into buffer_scale crash" >&2
    cat "$artifact_dir/wayland-native-borderless.log" >&2
    exit 1
  fi
fi

echo "PASS: decorated default window, resize, startup fullscreen, F11 fullscreen toggle, and Wayland launch smoke passed"
