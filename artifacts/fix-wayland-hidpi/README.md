# fix-wayland-hidpi validation notes

Date: 2026-06-05

## Bug reproduction

The pre-fix installed CLI reproduced the user-visible Wayland/HiDPI crash with
the real compressed graph:

```bash
RUST_BACKTRACE=1 \
  waragraph /home/erik/waragraph/c4.k311.poa2kb.gfa.zst \
  > artifacts/fix-wayland-hidpi/installed-before.log 2>&1
```

Observed in `installed-before.log`:

```text
parsed 10928 edges
wl_surface@24: error 2: Buffer size (820x45) must be an integer multiple of the buffer_scale (2).
```

## Fixed Wayland launch

After `cargo install --path app`, the same installed command was run on the
native Wayland session (`WAYLAND_DISPLAY=wayland-0`, `XDG_SESSION_TYPE=wayland`):

```bash
RUST_BACKTRACE=1 timeout 12s \
  waragraph /home/erik/waragraph/c4.k311.poa2kb.gfa.zst \
  > artifacts/fix-wayland-hidpi/installed-after-wayland.log 2>&1
```

The command timed out intentionally with exit code 124 after the viewer stayed
alive. The log contains the real compressed graph load and repeated
`pixels_per_bp` frames, and does not contain `Buffer size ... buffer_scale` or
`wl_surface ... error 2`.

## Zoom validation

Wayland-native input automation is not available in this environment: `ydotool`
is not installed and the repository has no Wayland smoke harness. For the
human-visible zoom flow, I used the same installed binary with the X11 backend
only so `xdotool` could drive wheel input:

```bash
DISPLAY=:0 WINIT_UNIX_BACKEND=x11 \
  waragraph /home/erik/waragraph/c4.k311.poa2kb.gfa.zst \
  > artifacts/fix-wayland-hidpi/installed-after-x11-zoom.log 2>&1
```

The scripted flow resized and activated the real `Waragraph 1D` window, moved
the pointer over the graph view, and sent 150 mouse-wheel-up events.
`x11-zoom-summary.txt` recorded:

```text
parsed_10928=yes
samples=2799
last_pixels_per_bp=1046
max_pixels_per_bp=1046
```

That is above the viewer's base-text threshold (`pixels_per_bp > 4.0`), so the
real compressed target still loads and can zoom to base-pair view.

## Smoke coverage

This worktree has no `tests/smoke/manifest.toml` or `tests/smoke/scenarios/`
smoke harness to extend. The needed reliable regression is also display-backend
specific: it requires a Wayland compositor configured with output scale 2 and
input/window automation. I therefore could not add a permanent project smoke
scenario here. The permanent automated coverage added for this task is the
unit-level window sizing/CSD guard test in `app/src/app/window.rs`.
