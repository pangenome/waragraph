# path-depth color validation notes

Date: 2026-06-12

Waragraph depth mode colors nodes by path depth over the graph. The active
palette is grey followed by ROYGBIV:

```text
128,128,128  grey, lowest observed depth
228,26,28    red
255,127,0    orange
255,255,51   yellow
77,175,74    green
55,126,184   blue
75,0,130     indigo
148,0,211    violet, highest observed depth
```

The color range is derived from the observed graph node-depth stats, not a
fixed gfalook-style cutoff. This keeps high-path-count graphs from collapsing
to the final violet bin when every visible node depth is above a small fixed
threshold.

## Validation

Automated Rust tests in `app/src/color.rs` cover:

- the exact grey-to-ROYGBIV palette
- high-depth ranges such as `0..210` using the actual data range instead of a
  fixed cap
- single-depth graphs mapping to the low/grey end instead of producing an
  undefined or final-bin color

The scripted UI validation in `validate_ui.sh` opens Waragraph in the default
depth view, captures `/home/erik/waragraph/c4.k311.poa2kb.gfa.zst`, captures an
8-depth controlled fixture, and generates a high-depth controlled fixture with
node depths 203 through 210. It checks live screenshots for nonzero grey, red,
orange, yellow, green, blue, indigo, and violet pixels in both controlled cases.
