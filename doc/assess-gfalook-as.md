# assess-gfalook-as recommendation

Date: 2026-06-05

## Recommendation

Use the **shared-library path**, with **waragraph as the near-term interactive
browser surface** for `validate-c4-k311`.

Do not switch downstream validation to `~/gfalook` as the primary browser.
`gfalook` is useful evidence for the lower-level loader/parser direction: it
loads the real target once decompressed and handles sparse numeric segment IDs.
But it is currently a batch renderer that writes PNG/SVG, not an interactive
browser from whole-graph overview down to base-pair text. Waragraph already has
the interactive 1D viewer mechanics and base-pair text rendering; its blocker is
the core GFA input layer.

For `validate-c4-k311`, run waragraph after these loader prerequisites are done:

- `add-zstd-support`: stream `.gfa.zst` through the shared core loader.
- `fix-waragraph-sparse`: accept sparse numeric GFA segment IDs from the real
  c4 target.

## Observed facts

### gfalook surface

`~/gfalook` exists and has a release binary at:

```bash
/home/erik/gfalook/target/release/gfalook
```

The runnable surface is CLI batch rendering:

```bash
/home/erik/gfalook/target/release/gfalook --help
```

Observed result: usage is `gfalook [OPTIONS] --idx <FILE> --out <FILE>`.
Options are image-oriented (`--width`, `--height`, path filtering, color modes,
2D layout, SVG/PNG output). I found no web server, browser URL, egui app, or
long-running interactive UI surface in `~/gfalook`; `Cargo.toml` depends on
`clap`, `image`, `rayon`, logging, hashing, and related batch-rendering crates.

The GFA parser opens inputs directly as text:

```rust
let file = File::open(path)?;
let reader = BufReader::new(file);
```

This means `.gfa.zst` is not supported directly.

### gfalook on the target and representative inputs

Direct compressed target load failed:

```bash
/usr/bin/time -f 'elapsed=%E maxrss_kb=%M exit=%x' \
  /home/erik/gfalook/target/release/gfalook \
  -i /home/erik/waragraph/c4.k311.poa2kb.gfa.zst \
  -o /tmp/gfalook-target.svg -x 1000 -y 500 --x-axis pangenomic
```

Observed result:

```text
Error loading GFA file: stream did not contain valid UTF-8
elapsed=0:00.00 maxrss_kb=4756 exit=1
```

Closest plain included fixture rendered successfully:

```bash
/usr/bin/time -f 'elapsed=%E maxrss_kb=%M exit=%x' \
  /home/erik/gfalook/target/release/gfalook \
  -i /home/erik/gfalook/test/chr6.C4.gfa \
  -o /tmp/gfalook-chr6.svg -x 1000 -y 500 --x-axis pangenomic
```

Observed result:

```text
Found 1748 segments, total length: 51672 bp
Found 90 paths, 2366 edges
elapsed=0:00.06 maxrss_kb=8152 exit=0
```

For the real target, I decompressed only to `/tmp` to distinguish compression
support from parser/rendering support:

```bash
zstd -dc /home/erik/waragraph/c4.k311.poa2kb.gfa.zst \
  > /tmp/c4.k311.poa2kb.gfa
```

Observed result: the compressed 180 KB target expands to a 21 MB plain GFA.

Then:

```bash
/usr/bin/time -f 'elapsed=%E maxrss_kb=%M exit=%x' \
  /home/erik/gfalook/target/release/gfalook \
  -i /tmp/c4.k311.poa2kb.gfa \
  -o /tmp/gfalook-target-plain.svg -x 1200 -y 700 --x-axis pangenomic
```

Observed result:

```text
Found 8160 segments, total length: 251692 bp
Found 465 paths, 10928 edges
elapsed=0:00.75 maxrss_kb=63240 exit=0
```

This is strong evidence that gfalook's text GFA parser handles the target's
sparse numeric segment IDs and that overview rendering of this target is not a
performance problem for gfalook after decompression.

### gfalook human-visible check

Because gfalook exposes no runnable browser/UI surface, I did not claim
interactive browser usefulness. The closest human-visible validation was opening
the generated SVG through Chrome headless and taking screenshots:

```bash
google-chrome --headless=new --no-sandbox --disable-gpu \
  --window-size=1280,900 \
  --screenshot=/tmp/gfalook-chr6-browser.png \
  file:///tmp/gfalook-chr6.svg
```

Observed result: `184665 bytes written to file /tmp/gfalook-chr6-browser.png`.

```bash
google-chrome --headless=new --no-sandbox --disable-gpu \
  --window-size=1280,900 --force-device-scale-factor=2 \
  --screenshot=/tmp/gfalook-chr6-browser-zoom2.png \
  file:///tmp/gfalook-chr6.svg
```

Observed result:
`536686 bytes written to file /tmp/gfalook-chr6-browser-zoom2.png`.

This confirms the SVG is browser-visible, but it does not provide gfalook-native
pan/zoom controls or base-pair inspection.

### waragraph surface

Waragraph's current app package is `waragraph` under `app/`. With no arguments:

```bash
target/debug/waragraph
```

Observed result:

```text
Usage: target/debug/waragraph <gfa> [tsv]
4-column BED file can be provided using the --bed flag
```

The app initializes the visible path through:

```rust
let path_index = waragraph_core::graph::PathIndex::from_gfa(&args.gfa)?;
app.init_viewer_1d(...)
```

The 1D viewer already has human navigation mechanics:

- drag panning on the path slots;
- wheel zoom with focus under the cursor;
- hover context for pangenome bp and node;
- base text rendering when `pixels_per_bp > 4.0`.

Relevant observed code locations:

- `app/src/app.rs`: `PathIndex::from_gfa(&args.gfa)` is the app entry point.
- `app/src/viewer_1d.rs`: wheel zoom and drag pan update the view.
- `app/src/viewer_1d.rs`: base text is drawn once `pixels_per_bp > 4.0`.
- `app/src/viewer_1d/render.rs`: `sequence_shapes_in_slot` emits per-base text.

Waragraph currently has two target-load blockers.

Compressed target:

```bash
target/debug/waragraph /home/erik/waragraph/c4.k311.poa2kb.gfa.zst
```

Observed result:

```text
thread 'main' panicked at lib/src/graph.rs:302:9:
attempt to subtract with overflow
```

Decompressed target:

```bash
/usr/bin/time -f 'elapsed=%E maxrss_kb=%M exit=%x' \
  target/debug/waragraph /tmp/c4.k311.poa2kb.gfa
```

Observed result:

```text
thread 'main' panicked at lib/src/graph.rs:301:9:
GFA segments must be tightly packed: min ID 1, max ID 14141, node count 8160, was 14140
elapsed=0:00.05 maxrss_kb=96844 exit=101
```

The sparse-ID failure is independent of zstd and must be fixed before
`validate-c4-k311` can succeed in waragraph.

## Path comparison

Zstd load effort:

- gfalook: no zstd support; direct `.gfa.zst` fails as invalid UTF-8. Adding
  zstd would be straightforward locally, but it would still leave no interactive
  browser.
- waragraph: zstd support is already scoped in `add-zstd-support` at the core
  loader layer, which is the right place for reuse.
- shared-library path: best fit. Put zstd opening and sparse-ID GFA parsing in
  a lower-level loader used by waragraph now and potentially by gfalook later.

Whole-graph to base-pair detail:

- gfalook: overview SVG/PNG works, including on the decompressed real target,
  but no native interactive navigation or base-pair text inspection flow exists.
- waragraph: already has interactive 1D view state, zoom/pan controls, hover
  context, and sequence text rendering at high zoom.
- shared-library path: use waragraph as the UI while making the loader robust.

Performance and memory risk on `c4.k311.poa2kb.gfa.zst`:

- gfalook plain target render: 0.75 s, about 63 MB max RSS.
- waragraph plain target currently panics before meaningful performance can be
  measured; the failed run reached about 97 MB max RSS after GPU initialization.
- Target size is modest after decompression: 21 MB GFA, 8160 segments, 465
  paths, 10928 edges, 251692 bp. The near-term risk is correctness of loader
  assumptions, not raw scale.

Implementation complexity and time to useful browser:

- gfalook-first would require zstd input, an interactive app/browser surface,
  viewport state, event handling, progressive zoom, base-pair rendering, and
  validation automation. That is more than a near-term browser fix.
- waragraph-first alone is close on UI but currently blocked by core loader
  correctness.
- shared-library path is the shortest path to a useful browser: repair the
  loader once, validate in waragraph's existing UI, and optionally let gfalook
  reuse the same opener/parser later.

## Follow-up tasks

I added `fix-waragraph-sparse` and made `validate-c4-k311` depend on it.

Concrete validation criteria for `fix-waragraph-sparse`:

- add a `waragraph-core` regression test that loads a small plain GFA with
  sparse numeric segment IDs;
- verify segment/node count, path count, path steps, and sequence length;
- verify sparse original IDs map to compact internal node indices without
  panic;
- if zstd support has landed, run the same sparse fixture through `.gfa.zst`;
- run `cargo build` and `cargo test`;
- after `add-zstd-support` and `fix-waragraph-sparse`, use the real waragraph UI
  in `validate-c4-k311` to load
  `/home/erik/waragraph/c4.k311.poa2kb.gfa.zst` and demonstrate visible
  whole-graph-to-base-pair zoom.
