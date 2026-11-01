# v2 Glyphs: FiveSeven text + Icons32

All glyph data for the v2 appliance is authored as ASCII art in `v2/glyphs/` and compiled
into static tables by `v2/build.rs` (`$OUT_DIR/glyph_tables.rs`, pulled in by `src/glyph.rs`).

| Module | Role |
|---|---|
| `src/glyph.rs` | Generated tables, codepoint lookup (`resolve`), 2-bit icon rasterizer |
| `src/font.rs` | Text API: monospace (`draw_str`, `text_width`) and proportional (`draw_str_prop`, `text_width_prop`) |
| `src/icon.rs` | Typed `Icon` enum, `draw_icon(fb, x, y, icon, size, color)` |
| `tools/gen_icons.py` | Starting-point generator for the icon art (re-running overwrites hand edits) |

## Fonts vs render modes

The appliance has **one font, `FiveSeven`**, and two **render modes**:

| Mode | What it does |
|---|---|
| `Native` | Draws the 5x7 source as `scale x scale` blocks. |
| `Smooth` | Generates a softened mask at the same physical size. |

`scale` is absolute. `1x` is the 5x7 base in both modes, so the two modes
occupy the same layout box at every scale:

```text
ink     = 5s x 7s
box     = (5s+2) x (7s+2)   // 1px bleed ring on every side
advance = 5s+3              // ink + ring + 1 gap
line    = 9s+2              // exactly the smooth mask height
```

The bleed ring is layout, not just painting: both modes reserve it, so the
antialiased edge always has a pixel to land in and toggling modes never
reflows. At `1x` there is no ring (ink 5x7, advance 6, line 10); `Smooth` uses
the native path and the two modes are pixel-identical.

## Smooth mask generation

A smooth mask is generated at render time from the FiveSeven source at the
requested scale `s >= 2`:

1. each source pixel becomes a solid `s x s` block in a `5s x 7s` core;
2. one pixel of ring padding is left around the core;
3. the source `dy` is multiplied by `s` and applied as the glyph offset;
4. three sequential 4-neighbor passes fill empty cells:
   - more than one non-empty neighbor -> 100%;
   - more than one non-empty neighbor -> 66%;
   - more than zero non-empty neighbors -> 33%.

Each pass sees the pixels written by earlier passes. The 1px ring lands in the
inter-glyph gap and line leading, so it adds no layout. The mask is anchored so
the ink core aligns exactly with the native blocks.

| Scale | Mask (incl. ring) | Encoding |
|---:|---|---|
| 1 | none (native) | 1-bit |
| 2 | 12x20 | 2-bit |
| 3 | 17x29 | 2-bit |
| 4 | 22x38 | 2-bit |
| 5 | 27x47 | 8-bit |
| 6 | 32x56 | 8-bit |
| 7 | 37x65 | 8-bit |
| 8 | 42x74 | 8-bit |

Steps 2-4 keep the original 2-bit fill path and are fully cached per glyph;
steps 5-8 use 8-bit masks with border-gated rings and a 12-slot evictable cache.
Nothing is baked into the wasm binary. The mode is selected from
**System → Fonts → Rendering**.

## Compiled form

`build.rs` packs each glyph into **5 bytes** (`FIVESEVEN_PACKED`):

```text
bits  0..34   7 rows x 5 bits (row 0 first, LSB = leftmost pixel)
bits 35..36   dy (0..2)
bits 37..39   unused
```

Left bearing and ink width are derived from the rows at lookup time
(`glyph::derive_metrics`), so they are not stored. The table is
532 x 5 = 2,660 bytes versus 4,256 for the previous 7-byte rows plus metric
byte. `FIVESEVEN_RANGES` remains the 67-run binary-search index.

## Smooth upscaling

The `2x`–`8x` averaging ladder (magnify, 1px ring, sequential 4-neighbour
averaging passes) is documented in [v2-smooth.md](v2-smooth.md). The older fixed-level path
remains only as a fallback above 8x described above.

## Codepoint resolution

1. **FiveSeven** sheets (`glyphs/fiveseven/*.txt`), binary search over contiguous ranges.
2. **Icons32** Private Use Area: `U+E000 + Icon as u16` (e.g. `"\u{E000} HOME"`).
3. **Alias table** of real Unicode symbols (`⌂ ← → ↑ ✕ ✓ ☰ ⚙ ▶ 🎮 …`) → icons.
   Once FiveSeven gains its own glyph for a codepoint (e.g. arrows), that glyph wins.
4. Missing-glyph box.

## FiveSeven source format

```
@U+0067 g dy=2
.####
#...#
#...#
#...#
.####
....#
.###.
```

- 7 rows × 5 cells, `#` = ink, `.` = blank.
- `dy=N` (0–2) bumps the glyph down N rows. Descenders (`g p q y` = 2, `j ,` = 1) hang below the
  7-row cell, so allow `9 * scale` of ink height per line.
- Left bearing and ink width are computed automatically for proportional spacing
  (advance = ink width + 1 gap; SPACE = 3).
- New blocks: add a file such as `00A0-00FF_latin1.txt`. Duplicate codepoints fail the build.

## Icons32 source format

One file per icon, `NN_name.txt`, sorted order = `Icon` discriminant (a unit test checks names).
32 rows × 32 cells, 2 bits per pixel. Each level is a coverage step of a single ink color:

| Char | Level | Coverage |
|---|---|---|
| `.` | 0 | transparent |
| `-` | 1 | 33% |
| `+` | 2 | 66% |
| `#` | 3 | 100% |

Rendering at `32·k` px scales by integer blocks. Other sizes use area-averaged (box filter)
resampling. Inline in text, an icon is a square one text cell tall (`7 * scale`): it takes
2 cells in monospace and `7 + 1` columns in proportional text.

## Size & Coverage

FiveSeven ships 6 sheets (`0020-007E_basic_latin.txt`, `00A0-00FF_latin1.txt`, `0100-017F_latin_ext_a.txt`,
`0370-03FF_greek.txt`, `0400-04FF_cyrillic.txt`, `2000-26FF_symbols.txt`) totaling 532 text glyphs (~4.3 KB),
plus 32 icons × 256 B = 8 KB.

Preview: **System → Icons** (key `4`), **System → Fonts** (key `3`), and **System → Locale** (key `2`).
