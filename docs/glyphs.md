# v2 Glyphs: FiveSeven text + Icons32

All glyph data for the v2 appliance is authored as ASCII art in `v2/glyphs/` and compiled
into static tables by `v2/build.rs` (`$OUT_DIR/glyph_tables.rs`, pulled in by `src/glyph.rs`).

| Module | Role |
|---|---|
| `src/glyph.rs` | Generated tables, codepoint lookup (`resolve`), 2-bit icon rasterizer |
| `src/font.rs` | Text API: monospace (`draw_str`, `text_width`) and proportional (`draw_str_prop`, `text_width_prop`) |
| `src/icon.rs` | Typed `Icon` enum, `draw_icon(fb, x, y, icon, size, color)` |
| `tools/gen_icons.py` | Starting-point generator for the icon art (re-running overwrites hand edits) |

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
- `dy=N` (0–3) bumps the glyph down N rows. Descenders (`g p q y` = 2, `j ,` = 1) hang below the
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
