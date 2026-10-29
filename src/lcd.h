#ifndef LCD_H
#define LCD_H

#include <stdint.h>

#include "framebuffer.h"

/*
 * Seven-segment LCD digit renderer.
 *
 *     -- a --
 *    |       |
 *    f       b
 *    |       |
 *     -- g --
 *    |       |
 *    e       c
 *    |       |
 *     -- d --
 *
 * Digits are rectangles drawn through fb_fill(). "Off" segments are drawn in a
 * dim colour so the display reads as a physical LCD rather than floating
 * glyphs. Everything clips through fb_fill(), so a digit may hang off the edge.
 */

/* Pass as off_color to draw only the lit segments. Lets the clock compose over
 * an image without painting a panel behind it. */
#define LCD_OFF_NONE 0u

/* Draw one digit in the box (x, y, w, h). digit must be 0-9. */
void lcd_draw_digit(fb_t *fb, int x, int y, int w, int h, int digit,
                    uint32_t on_color, uint32_t off_color);

/* Draw a colon of width w, vertically centred in the box (x, y, w, h). */
void lcd_draw_colon(fb_t *fb, int x, int y, int w, int h, int lit,
                    uint32_t on_color, uint32_t off_color);

/* Segment bitmask for a digit, exposed for tests (bit0 = a ... bit6 = g). */
uint8_t lcd_segment_mask(int digit);

#endif /* LCD_H */
