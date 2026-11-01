# v2 Smooth Upscaling (2x–4x)

The smooth render modes magnify the 1-bit 5×7 FiveSeven source into an 8-bit
coverage mask. `1x` is the native source; `2x` through `8x` use the
averaging ladder below. The older fixed-level path remains only as a fallback
for scales above 8.

The algorithm is deliberately simple: magnify, pad, then fill empty cells with
the average of their non-zero neighbours, in layers.

## Algorithm

For absolute scale `s` (`2` through `8`):

1. Magnify every source pixel to an `s×s` block of `255`.
2. Place the blocks in a `(5s+2) × (9s+2)` mask with a **1-pixel empty ring**
   on every side. Empty is `0`; full is `255`.
3. Run the pass ladder. Each pass walks every empty cell, counts its non-zero
   **4-neighbours**, and if the count is greater than the pass threshold,
   assigns `average(neighbours) × factor`, rounded.
4. Passes are sequential: later passes include the values written by earlier
   passes.

```text
for (threshold, factor) in ladder:
    snapshot = levels
    for each empty cell in snapshot:
        n = non-zero 4-neighbours in snapshot
        if n > threshold:
            levels[cell] = round(average(neighbours) * factor)
```

Only empty cells are written, so a cell is filled by the first pass that
reaches it.

## Ladder

| scale | ring | passes | mask | line | advance |
|---:|---:|---|---|---:|---:|
| 1x | — | native 1-bit | 5×7 | 10 | 6 |
| 2x | 1px | 100%, 66%, 33% | 12×20 | 20 | 13 |
| 3x | 1px | 100%, 75%, 50%, 25% | 17×29 | 29 | 18 |
| 4x | 1px | 100%, 80%, 60%, 40%, 20% | 22×38 | 38 | 23 |
| 5x | 1px | 100%, 83%, 67%, 50%, 33%, 17% | 27×47 | 47 | 28 |
| 6x | 1px | 100%, 86%, 71%, 57%, 43%, 29%, 14% | 32×56 | 56 | 33 |
| 7x | 1px | 100%, 88%, 75%, 63%, 50%, 38%, 25%, 13% | 37×65 | 65 | 38 |
| 8x | 1px | 100%, 89%, 78%, 67%, 56%, 44%, 33%, 22%, 11% | 42×74 | 74 | 43 |

The factors apply to the **average of the neighbours**, not to a fixed level.
A pure 255+255 pair produces:

| pass | 2x | 3x | 4x |
|---|---:|---:|---:|
| 100% | 255 | 255 | 255 |
| 80% | — | — | 204 |
| 75% | — | 191 | — |
| 66% | 170 | — | — |
| 60% | — | — | 153* |
| 50% | — | 128* | — |
| 40% | — | — | 102* |
| 33% | 85 | — | — |
| 25% | — | 64 | — |
| 20% | — | — | 51 |

`*` A pure value appears only when the pass sees untouched 255 neighbours.
Once earlier passes have written 191/204 values, the average is mixed, so the
mask also contains values such as `26`, `46`, `57`, `71`, `112`, `122`, `138`.
That gradient is the point of averaging rather than assigning fixed levels.

## Pattern and 5x–8x

For scale `s` the ladder has `s + 1` passes with factors
`(s + 1 - j) / (s + 1)`, where `j = 0` is the 100% pass and the last pass uses
threshold `> 0`; the others use `> 1`.

| scale | factors | mask | line | advance |
|---:|---|---|---:|---:|
| 5x | 1, 5/6, 2/3, 1/2, 1/3, 1/6 | 27×47 | 47 | 28 |
| 6x | 1, 6/7, 5/7, 4/7, 3/7, 2/7, 1/7 | 32×56 | 56 | 33 |
| 7x | 1, 7/8, 3/4, 5/8, 1/2, 3/8, 1/4, 1/8 | 37×65 | 65 | 38 |
| 8x | 1, 8/9, 7/9, 2/3, 5/9, 4/9, 1/3, 2/9, 1/9 | 42×74 | 74 | 43 |

5x–8x keep the 1px ring and the existing line/advance sizes. Their caches add
about 0.66 MB, 0.93 MB, 1.22 MB, and 1.58 MB of BSS respectively, for a total
of roughly 5.4 MB across 2x–8x. The pure values are 255/213/170/128/85/43 at
5x, 255/219/182/146/109/73/36 at 6x, 255/223/191/159/128/96/64/32 at 7x, and
255/227/198/170/142/113/85/57/28 at 8x.

## Why the ring is 1 pixel

The last pass has an outward reach of 1 mask pixel: a cell two pixels beyond
the current edge has no neighbour in the snapshot, so it is not filled. The
earlier passes fill inward — concave corners, inner contours, and the gradient
between the core and the edge — not outward.

Measured over all 532 glyphs at 3x–6x, a 2-pixel ring produced **zero**
non-zero cells in its outer ring, and the inner regions were pixel-identical to
the 1-pixel versions. The same last-pass rule applies at 7x and 8x. A 1-pixel
ring is therefore enough, and it keeps the mask size equal to the existing
line and advance:

```text
bleed(s)      = 1 for s >= 2
line_height(s)= 9s + 2
char_advance(s)= 5s + 3
glyph_height(s)= 7s + 2
```

## 4x S

Pass map (`#` = base, `A` = 100%, `B` = 80%, `C` = 60%, `D` = 40%, `E` = 20%):

```text
....EEEEEEEEEEEEEEEEE.
...ED################E
..EDC################E
.EDCB################E
EDCBA################E
E####ABCDEEEEEEEEEEEE.
E####BCDE.............
E####CDE..............
E####DE...............
E####DE...............
E####CDE..............
E####BCDE.............
E####ABCDEEEEEEEEE....
EDCBA############DE...
.EDCB############CDE..
..EDC############BCDE.
...ED############ABCDE
....EEEEEEEEEDCBA####E
.............EDCB####E
..............EDC####E
...............ED####E
...............ED####E
..............EDC####E
.............EDCB####E
.EEEEEEEEEEEEDCBA####E
E################ABCDE
E################BCDE.
E################CDE..
E################DE...
.EEEEEEEEEEEEEEEEE....
```

Final (`#` = 255, `4` ≥ 180, `3` ≥ 120, `2` ≥ 60, `1` < 60):

```text
....11111111111111111.
...12################1
..113################1
.1134################1
1234#################1
1#####432111111111111.
1####4311.............
1####311..............
1####21...............
1####21...............
1####311..............
1####4311.............
1#####432111111111....
1234#############21...
.1134############311..
..113############4311.
...12#############4321
....111111111234#####1
.............1134####1
..............113####1
...............12####1
...............12####1
..............113####1
.............1134####1
.111111111111234#####1
1#################4321
1################4311.
1################311..
1################21...
.11111111111111111....
```

Distinct values: `0, 10, 13, 16, 33, 51, 52, 79, 122, 138, 204, 255`.

## Implementation

`smooth.rs`:

| item | role |
|---|---|
| `seed_avg` | magnify the 5×7 source into the ringed mask |
| `avg_pass` | one 4-neighbour averaging pass over a `w×h` region |
| `generate_avg2` … `generate_avg8` | the seven ladders |
| `AVG` … `AVG8` | per-glyph 8-bit mask caches (BSS) |
| `glyph_avg2` … `glyph_avg8` | lazy lookup + cache |
| `avg2_reference_s` … `avg8_reference_s` | FNV-pinned S masks |

`font.rs` dispatches in `draw_text_glyph`:

- `scale == 2` → `glyph_avg2`, ring 1
- `scale == 3` → `glyph_avg3`, ring 1
- `scale == 4` → `glyph_avg4`, ring 1
- `scale == 5` → `glyph_avg5`, ring 1
- `scale == 6` → `glyph_avg6`, ring 1
- `scale == 7` → `glyph_avg7`, ring 1
- `scale == 8` → `glyph_avg8`, ring 1
- `scale > 8` → the older fixed-level mask path (fallback)
- `scale == 1` → native 1-bit blocks

The masks are 8-bit, so the draw path blends the level directly (no
`unpack`/`×85`). Cached mask memory is approximately 128 KB (2x), 256 KB (3x),
435 KB (4x), 675 KB (5x), 953 KB (6x), 1.22 MB (7x), and 1.58 MB (8x) of
zero-initialised BSS; none of that is in the wasm file. The wasm memory
minimum is 6.19 MiB and the guest grows its framebuffer/staging area from
there. The current release wasm is 128,591 B, including the redesigned Fonts
page.

## Fonts screen

`System → Fonts` exposes the ladder:

- the header has the compact `Rendering` control (`Native` / `Smooth`);
- the character repertoire is five rows across the top;
- the left column lists `1x`-`8x` with a `Native` and `Smooth` sample per row;
- the right column is a `Specimen` panel for the selected size, followed by
  the five style transforms and two short notes.

The selected size is page-local (`SystemApp::preview_idx`, 4x by default).
Click a ladder row, or use `Up`/`Down`, to change it; the specimen updates
without touching app text or the global scale. At 720p all eight sizes and all
five styles fit. Below 560px the page switches to the compact single-column
layout with the first three sizes.

## Symmetry

The averaging kernel is symmetric, so the upscaler commutes with the geometric
folds:

- left-right mirror: `upscale(fold(x)) == fold(upscale(x))`
- top-bottom mirror (on the 7-row cell)
- 180° (both)

This holds for all 532 glyphs at 2x and 3x; the probes live in
`smooth.rs::fold_tests`. 90° rotation also commutes when the glyph is rendered
in a square 5×5 cell; in the 9-row line frame, adjacent-row ink makes the cell
context asymmetric and the D4 mask is not 4-fold. A future square-cell renderer
would move the D4 symbols onto this model.

## Open items

- The eight averaging caches total about 5.4 MB of BSS. If that becomes a
  concern, an evictable shared 8-bit cache for 5x–8x is the alternative; the
  fonts page draws many glyphs and would thrash a small slot set.
- The old fixed-level path (`generate` / `generate_big`) is now only a
  fallback for scales above 8 and for the legacy mask tests labelled in
  `smooth.rs`.
- The ring is fixed at 1px for every scale. If a future ladder needs a wider
  outward reach, the ring and the layout metrics must move together.
- The 2x/3x/4x caches are fully populated per glyph. A smaller evictable cache
  is possible, but the fonts page draws many glyphs and would thrash it.
- The old 2-bit step-0..2 masks are still compiled for `smooth::glyph` (tests
  and the above-8 fallback); the averaging draw path bypasses them.
