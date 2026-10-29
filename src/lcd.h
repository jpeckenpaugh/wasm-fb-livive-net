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

/* Symbols that are not digits, drawn as a 7x7 dot matrix so they scale with
 * the box. Used for calculator operators and labels. */
typedef enum {
    LCD_SYM_PLUS = 0,
    LCD_SYM_MINUS,
    LCD_SYM_MUL,
    LCD_SYM_DIV,
    LCD_SYM_EQ,
    LCD_SYM_CLEAR,
    LCD_SYM_BACK,
    LCD_SYM_E,
    LCD_SYM_COUNT
} lcd_symbol_t;

void lcd_draw_symbol(fb_t *fb, int x, int y, int w, int h, lcd_symbol_t symbol,
                     uint32_t color);

/* ---- 14-segment alphanumerics ------------------------------------------
 *
 *     -- a --            a  top          h/i  upper diagonals (to centre)
 *    |\  |  /|           b  upper right  j/k  lower diagonals
 *    f h l i b           c  lower right  g1   middle left
 *    |  \|/  |           d  bottom       g2   middle right
 *     g1--g2             e  lower left   l    upper centre vertical
 *    |  /|\  |           f  upper left   m    lower centre vertical
 *    e j m k c
 *    |/  |  \|
 *     -- d --
 *
 * Uppercase only: 14 segments cannot express x-height or descenders, so
 * lowercase is out of scope. A few glyphs are conventional approximations
 * (O/0 and D, I and 1, M/W/N from the diagonals). Everything else, plus
 * digits and , - . space, is exact.
 */
enum {
    S14_A = 1u << 0,
    S14_B = 1u << 1,
    S14_C = 1u << 2,
    S14_D = 1u << 3,
    S14_E = 1u << 4,
    S14_F = 1u << 5,
    S14_G1 = 1u << 6,
    S14_G2 = 1u << 7,
    S14_H = 1u << 8,
    S14_I = 1u << 9,
    S14_J = 1u << 10,
    S14_K = 1u << 11,
    S14_L = 1u << 12,
    S14_M = 1u << 13
};

/* Bitmask for a character; 0 for anything unsupported (drawn as a space). */
uint16_t lcd_14seg_mask(char ch);

/* Draw one character in the box. Pass LCD_OFF_NONE to light only segments. */
void lcd_draw_char14(fb_t *fb, int x, int y, int w, int h, char ch,
                     uint32_t on_color, uint32_t off_color);

/* ---- 16-segment ---------------------------------------------------------
 *
 * Same starburst as the 14-segment font plus two full-length diagonals:
 *   N  top-left to bottom-right        O  top-right to bottom-left
 * They let X, N and Z use one continuous stroke instead of two halves that
 * meet at the centre. Everything else falls back to the 14-segment mask.
 */
enum {
    S16_N = 1u << 14,
    S16_O = 1u << 15
};

uint16_t lcd_16seg_mask(char ch);
void lcd_draw_char16(fb_t *fb, int x, int y, int w, int h, char ch,
                     uint32_t on_color, uint32_t off_color);

/* ---- 5x7 dot matrix -----------------------------------------------------
 *
 * 35 dots per glyph (5 columns x 7 rows), sized by a single `cell` value so
 * the caller picks the resolution. Lowercase is covered here; 7/14-segment
 * cannot express it.
 */
int lcd_text5x7_width(const char *text, int cell);
void lcd_draw_text5x7(fb_t *fb, const char *text, int x, int y, int cell,
                      uint32_t color);

/* Draw one digit in the box (x, y, w, h). digit must be 0-9. */
void lcd_draw_digit(fb_t *fb, int x, int y, int w, int h, int digit,
                    uint32_t on_color, uint32_t off_color);

/* Draw a colon of width w, vertically centred in the box (x, y, w, h). */
void lcd_draw_colon(fb_t *fb, int x, int y, int w, int h, int lit,
                    uint32_t on_color, uint32_t off_color);

/* Segment bitmask for a digit, exposed for tests (bit0 = a ... bit6 = g). */
uint8_t lcd_segment_mask(int digit);

#endif /* LCD_H */
