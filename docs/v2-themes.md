# v2 Color Themes

The appliance ships with ten runtime color themes. They are selectable from
**System → Theme** and apply globally to the active guest. Typography
(render mode and scale) now lives on **System → Fonts**; see
[glyphs.md](glyphs.md).

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

Theme tokens are semantic sentinels resolved at draw time, so switching a theme
does not require recompiling or re-laying-out the UI. A palette change only
changes pixel colors.

## Theme page controls

- Click a theme row to apply it immediately.
- Keyboard: `Up` / `Down` moves the theme cursor, `Home` / `End` jumps,
  `Enter` / `Space` applies, and `Tab` cycles System sections.
- `1`..`5` select About, Locale, Fonts, Icons, or Theme.

The bottom System tab bar also exposes Theme directly.
