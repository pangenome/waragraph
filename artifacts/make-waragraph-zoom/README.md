# make-waragraph-zoom validation notes

Date: 2026-06-06

## Implementation summary

The 1D viewer viewport now stores continuous `f64` left/right bounds in
`View1D` and derives integer ranges only at the rendering/sampling boundary.
Wheel zoom uses an exponential scale factor and calls
`zoom_with_focus_f64(pointer_fraction, scale)`, preserving the graph coordinate
under the pointer before clamping to graph bounds.

## Automated validation

Focused tests:

```bash
cargo test -p waragraph viewer_1d::view::tests
```

Full validation:

```bash
cargo build
cargo test
bash tests/smoke/scenarios/waragraph_zoom_anchor_transform.sh
```

All passed with existing warnings.

The new permanent smoke scenario is:

```text
tests/smoke/manifest.toml
tests/smoke/scenarios/waragraph_zoom_anchor_transform.sh
```

## UI validation

Script:

```bash
DISPLAY=:0 artifacts/make-waragraph-zoom/validate_ui.sh
```

The script launches the real UI against:

```bash
/home/erik/waragraph/c4.k311.poa2kb.gfa.zst
```

It captures:

- `anchored-before.png`: initial whole-graph view
- `anchored-zoomed.png`: after off-center wheel zoom
- `anchored-after-cycles.png`: after alternating wheel in/out cycles at the same off-center pointer
- `anchored.log`: UI log with zstd load and `pixels_per_bp` samples
- `summary.txt`: parsed validation summary

`summary.txt` from the successful run:

```text
graph=/home/erik/waragraph/c4.k311.poa2kb.gfa.zst
parsed_10928_edges=True
before_screenshot_exists=True
zoomed_screenshot_exists=True
after_cycles_screenshot_exists=True
pixels_per_bp_first=0.001772007056243345
pixels_per_bp_last=446.0
pixels_per_bp_max=446.0
pixels_per_bp_samples=1251
```

An attempted Xvfb run was not usable for this WGPU/Vulkan app in the current
environment: the app logged `No DRI3 support detected` and panicked inside the
WGPU wrapper before rendering. The successful UI run used the live X display
path (`DISPLAY=:0`), matching prior Waragraph validation scripts.
