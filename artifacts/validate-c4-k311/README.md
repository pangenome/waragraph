# validate-c4-k311 validation notes

Date: 2026-06-05

## Chosen browser path

I used the waragraph current 1D viewer:

```bash
target/debug/waragraph /home/erik/waragraph/c4.k311.poa2kb.gfa.zst
```

This follows `doc/assess-gfalook-as.md`, which recommends the shared-library
path with waragraph as the near-term interactive browser surface for this
target. The reason is that `gfalook` can batch-render decompressed GFA, but
waragraph is the downstream user-visible interactive viewer that uses the
shared `PathIndex::from_gfa` loader.

## Target input

The validation used the real compressed target:

```bash
/home/erik/waragraph/c4.k311.poa2kb.gfa.zst
```

I did not manually decompress it for the successful UI run. The file was present
on this machine at 180 KB.

## UI flow exercised

The successful UI run used the live desktop display with the app forced through
the X11/Xwayland backend so `xdotool` could drive the real window:

```bash
DISPLAY=:0 \
WINIT_UNIX_BACKEND=x11 \
RUST_BACKTRACE=1 \
target/debug/waragraph /home/erik/waragraph/c4.k311.poa2kb.gfa.zst \
  > artifacts/validate-c4-k311/waragraph-ui.log 2>&1
```

Then I located the actual `Waragraph 1D` window and used ordinary window/mouse
controls:

```bash
xdotool search --name "Waragraph 1D"
xdotool windowsize "$WIN" 1400 900
xdotool windowmove "$WIN" 20 20
xdotool windowactivate "$WIN"
xdotool mousemove --window "$WIN" 760 450
for i in $(seq 1 150); do
  xdotool click 4
  sleep 0.015
done
```

That is the same zoom path a user exercises with the mouse wheel over the path
slot. The viewer started at whole-graph scale and zoomed progressively to
base-pair scale.

## Evidence

The app loaded the compressed graph and reported the real target edge count:

```text
parsed 10928 edges
```

The viewer remained live until the script terminated it intentionally. There was
no panic, crash, or hang in the successful UI run.

The 1D viewer prints `pixels_per_bp` each frame. In `app/src/viewer_1d.rs`, base
text rendering is enabled when `pixels_per_bp > 4.0`; the successful UI run
reached:

```text
first_pixels_per_bp=0.001772007056243345
last_pixels_per_bp=1843
max_pixels_per_bp=1843
samples=2755
base_text_threshold=4.0
```

The final zoom is therefore far past the renderer's base-text threshold, which
means the base-pair text rendering path was active at the final view.

## Screenshot/readback limitation

I attempted several screenshot methods:

- `import -window "$WIN"` on the live Xwayland window;
- `xwd -id "$WIN"` and `xwd -root`;
- GNOME Shell screenshot DBus API;
- `ffmpeg -f x11grab`;
- Xvfb with Vulkan and with `WGPU_BACKEND=gl`.

They did not produce a useful visual artifact in this session. Direct X window
readback failed on the accelerated surface, GNOME Shell denied screenshot DBus
access, Xvfb lacked the WGPU presentation support required by this app, and the
desktop `x11grab` output was black aside from the cursor. The committed
inspectable artifact for base-pair-scale validation is therefore the live UI log
summary above, plus the exact UI driving commands.

## Build/test/smoke

Commands run:

```bash
cargo build
cargo test
```

Both passed with existing warnings. There is no `tests/smoke/manifest.toml` in
this worktree, and this validation did not expose or fix a waragraph
user-visible behavior requiring a new permanent smoke scenario. The only
remaining split is that the loader/browser path is shared-library plus
waragraph UI now; `gfalook` remains a batch-rendering comparison tool rather
than the downstream interactive browser.
