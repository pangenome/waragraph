# match-waragraph-depth validation notes

Date: 2026-06-06

## gfalook source matched

The Waragraph depth color mapping is copied from `/home/erik/gfalook/src/main.rs`:

- `COLORBREWER_SPECTRAL_13`, lines 1982-1999 in the inspected tree.
- `get_depth_color`, lines 4748-4806 in the inspected tree.

Default `gfalook -m` behavior uses no interpolation. It scans these cuts:

```text
0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5, 8.5, 9.5, 10.5, 11.5, 12.5
```

The matched RGB bins are:

```text
depth <= 0.5    -> 196,196,196  light gray base/low-depth color
depth <= 1.5    -> 128,128,128  neutral gray 1x-ish coverage range
depth <= 2.5    -> 158,1,66
depth <= 3.5    -> 213,62,79
depth <= 4.5    -> 244,109,67
depth <= 5.5    -> 253,174,97
depth <= 6.5    -> 254,224,139
depth <= 7.5    -> 255,255,191
depth <= 8.5    -> 230,245,152
depth <= 9.5    -> 171,221,164
depth <= 10.5   -> 102,194,165
depth <= 11.5   -> 50,136,189
depth <= 12.5   -> 94,79,162
depth > 12.5    -> 94,79,162
```

This is the corrected low-depth behavior from the retry prompt: 0x through 0.5x is light gray, greater than 0.5x through 1.5x is neutral gray, and the Spectral/rainbow bins begin above 1.5x.

## odgi reference

`/home/erik/bin/odgi` is available and reports `v0.9.2-20-g5e58a324`.

`odgi viz --help` documents `-G, --no-grey-depth` as: "Use the colorbrewer palette for <0.5x and ~1x coverage bins. By default, these bins are light and neutral grey." This matches the low-depth gray semantics in `gfalook -m`.

A controlled `odgi viz -m` run was captured at `controlled-depth-odgi.png` and produced visible `(128,128,128)` neutral-gray pixels for the controlled 1x path bins. That odgi run did not expose higher global-coverage Spectral bins from this fixture, so Waragraph's higher-color validation is done with the app's controlled global node-coverage fixture and source-level `get_depth_color` equivalence tests.

## Waragraph validation

Automated Rust tests in `app/src/color.rs` cover:

- negative and 0x depth -> `(196,196,196)`
- 0.5x -> `(196,196,196)`
- just above 0.5x, 1x, and 1.5x -> `(128,128,128)`
- just above 1.5x and higher bins -> reversed Spectral colors
- clamping above 12.5x

The scripted UI validation in `validate_ui.sh` opens Waragraph in the default view, captures `/home/erik/waragraph/c4.k311.poa2kb.gfa.zst` and the controlled fixture, and samples rendered RGB histograms for the light gray -> neutral gray -> Spectral transition.
