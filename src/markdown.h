/* Minimal markdown renderer for the Preview app.
 *
 * Deliberately small: headings, bullets, paragraphs and inline emphasis.
 * Styling uses colour plus two tricks, since there is only one font weight:
 *   **bold**   drawn twice, one pixel apart
 *   *italic*   sheared a quarter cell across the seven rows
 *   `code`     green
 *   [link]     accent colour with an underline
 */
#ifndef MARKDOWN_H
#define MARKDOWN_H

#include "framebuffer.h"

/* Draw into the box. Returns the height consumed (may exceed h; the caller
 * clips). `cell` is the body dot size. */
int md_draw(fb_t *fb, const char *text, int x, int y, int w, int h, int cell);

#endif /* MARKDOWN_H */
