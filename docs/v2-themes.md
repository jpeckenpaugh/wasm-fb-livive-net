# v2 Typography & Themes

The appliance ships with one 5x7 bitmap source and two render modes, plus ten
runtime color themes. Both are selectable from **System → Theme** and apply
globally to the active guest.

## Rendering modes

| Mode | Cell | Bits | Notes |
|---|---|---|---|
| `Native` | 5×7 | 1 bit/pixel | Draws the FiveSeven source directly. |
| `Smooth 12x16` | 12×19 line | 2 bits/pixel | Runtime-derived; 2× core + doubled `dy` + three 4-neighbor fill passes. |

The smooth mask is generated lazily and cached in guest memory (48 bytes per
glyph); it is not stored in the wasm binary. The active mode is global, so it
applies to the WM, System, Files, Calculator, Mines, and the code preview. The
layout code asks the font module for cell, line, and advance metrics rather than
assuming 5×7.

## Color themes

| # | Name | Character |
|---:|---|---|
| 0 | Midnight | Original dark teal |
| 1 | Slate | Cool blue-gray |
| 2 | Ocean | Deep blue / cyan |
| 3 | Forest | Dark green |
| 4 | Ember | Warm amber |
| 5 | Amethyst | Violet |
| 6 | Mono | Neutral grayscale |
| 7 | Solarized | Solarized dark |
| 8 | Rose | Muted rose |
| 9 | Copper | Warm copper |

Theme tokens are resolved at draw time, so switching a theme does not require
recompiling or re-laying-out the UI.

## Theme page controls

- Click a font-family button to switch globally.
- Click a theme row to apply it immediately.
- Keyboard: `Up` / `Down` moves the theme cursor, `Home` / `End` jumps,
  `Enter` / `Space` applies, `Left` / `Right` picks `FiveSeven` or
  `Smooth 12x16`, and `Tab` cycles System sections.
- `1`..`5` select About, Locale, Fonts, Icons, or Theme.

The bottom System tab bar also exposes Theme directly.
