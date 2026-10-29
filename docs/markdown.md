# Markdown in Preview

`ABOUT.md` lives at the repo root (so GitHub renders it too) and is embedded
into the guest at build time by `tools/embed_about.py` → `build/gen/about_gen.c`.
Editing the markdown updates the app; there is one source of truth.

`src/markdown.c` renders a deliberately small subset with the 5x7 font:

| Syntax | Rendering |
|---|---|
| `#`, `##`, `###` | 1.6x / 1.2x / 1x body size, accent colour |
| `- item`, `* item` | accent bullet, hanging indent |
| blank line | paragraph gap |
| paragraph | wrapped at the panel width |
| `**bold**` | drawn twice, two pixels apart |
| `*italic*` | sheared a quarter cell across the seven rows |
| `` `code` `` | green |
| `[text](url)` | accent text with an underline, url dropped |

Bold and italic are geometry tricks rather than a second font: the renderer
only has one weight and one slant, so it doubles the run or shears it row by
row (`lcd_draw_text5x7_ex`).

Not supported: tables, code blocks, images, nested lists, clickable links, and
scrolling. The document is expected to be short; overflow is clipped rather
than paginated.
