# default-waragraph-to validation notes

Date: 2026-06-05

## Implementation notes

Waragraph already had a 1D path/depth visualization path:

- `app/src/viewer_1d.rs` registered a `depth` visualization mode.
- `app/src/app/resource.rs` already populated graph path depth and path depth data sources.
- `app/src/color.rs` already had a `spectral` color scheme using the ColorBrewer Spectral 11 palette plus two greys.

The default 1D startup mode previously used `path_name`. This task changes the default startup mode to `depth`, while keeping the existing settings UI buttons for `depth`, `strand`, and `path_name`.

The configurable startup surface added here is:

```bash
waragraph --view-mode depth <gfa>
waragraph --view-mode path-name <gfa>
waragraph --view-mode strand <gfa>
```

`depth` is the default when no flag is provided. `path-name` is normalized internally to the existing `path_name` mode so users can select the prior startup behavior.

## gfalook -m comparison

I inspected `/home/erik/gfalook/src/main.rs` and `/home/erik/gfalook/README.md`.
`gfalook -m` uses ColorBrewer Spectral 11 with two greys prepended for low coverage:

- `(196, 196, 196)` for very low coverage
- `(128, 128, 128)` for low coverage
- then Spectral 11 from red through yellow/green to purple/blue

Waragraph already had the same Spectral RGB values but the two prepended greys were reversed. I changed waragraph's `spectral` scheme to match the `gfalook -m` grey order. Waragraph still samples through its existing GPU color-map texture path rather than porting gfalook's exact CPU-side coverage cut loop, so the pattern intentionally matches the palette and low-depth ordering while retaining waragraph's existing continuous color-map behavior.

## Row separators

The 1D path-slot fragment shader now draws a subtle light grey separator at the top of each row:

```text
RGB ~= 0.86, 0.86, 0.86
```

This is visible between rows without replacing the path/depth fill content.

## UI validation

Script:

```bash
DISPLAY=:0 artifacts/default-waragraph-to/validate_ui.sh
```

The script launches the real UI against:

```bash
/home/erik/waragraph/c4.k311.poa2kb.gfa.zst
```

It captures:

- `default-depth.png`: no `--view-mode` flag, default depth startup
- `default-depth-zoomed.png`: same window after mouse-wheel zoom over the graph slot
- `path-name.png`: `--view-mode path-name`, proving a non-depth startup mode is selectable
- `default-depth.log`
- `path-name.log`
- `summary.txt`

`summary.txt` from the successful run:

```text
default-depth_mode=depth
default-depth_parsed_10928=true
default-depth_first_pixels_per_bp=0.001772007056243345
default-depth_last_pixels_per_bp=446.0
default-depth_max_pixels_per_bp=446.0
default-depth_screenshot_exists=True
path-name_mode=path_name
path-name_parsed_10928=true
path-name_first_pixels_per_bp=0.001772007056243345
path-name_last_pixels_per_bp=0.001772007056243345
path-name_max_pixels_per_bp=0.001772007056243345
path-name_screenshot_exists=True
default-depth-zoomed_screenshot_exists=True
```

This validates that the default launch uses depth, the override can select the prior path-name mode, the zstd graph still loads with the expected `parsed 10928 edges`, and the viewer zooms from whole-graph scale toward base-pair scale.

## Smoke coverage

This worktree still has no `tests/smoke/manifest.toml` or `tests/smoke/scenarios/` harness to extend. I therefore could not register a smoke-gate-owned scenario for this user-visible behavior without inventing a new harness format. The reusable scripted UI validation for this task is checked in under `artifacts/default-waragraph-to/validate_ui.sh`, and permanent unit coverage was added for `--view-mode` parsing in `app/src/app.rs`.
