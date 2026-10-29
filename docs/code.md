# Source code viewer

`demo/` holds one sample per supported language, embedded at build time by
`tools/embed_demo.py` (same pattern as `ABOUT.md`). The File Manager lists them
under `/ demo`; opening one switches Preview into code mode.

## Viewer

Full-bleed panel with a line-number gutter, 4-column tabs, and scroll state in
the preview (`line_off` / `col_off`). No wrapping: long lines scroll sideways,
because wrapped code loses its shape.

| Input | Action |
|---|---|
| wheel | scroll lines |
| `Shift` + wheel | scroll columns |
| `↑` / `↓` | scroll a line |
| `PageUp` / `PageDown` | scroll a screenful |
| `Home` / `End` | jump to the first/last line |
| `[` / `]` | scroll four columns |

Scrollbars appear on the right and bottom edges whenever the content is larger
than the panel, so the extent is visible without trying to scroll first. The
caption shows the language and total line count.

Only visible lines are scanned, so a long file costs the same as a short one.
The panel stops above the floating tiles; the document view does the same.

## Highlighting

Three scanners in `src/code.c` cover eight languages. Tokens are produced per
line, which keeps the scanners stateless.

| Scanner | Languages | Handles |
|---|---|---|
| C-like | C, JS, TS, JSON, Python | keyword/type tables, `//` or `#` comments, `/* */`, `' " \`` strings, numbers, `#include` |
| Markup | HTML, XML | tags, attributes, quoted values, `<!-- -->`, text |
| CSS | CSS | comments, strings, `@rules`, `#hex`, numbers, `property:` names |

Colours: keyword cyan, string green, number amber, comment grey, type purple,
tag cyan, attribute amber, preprocessor orange. Line numbers stay dim so the
code keeps the contrast.

Adding a language is a table entry (`keywords`, comment markers, extension) —
nine of the ten are already covered by the three scanners.
