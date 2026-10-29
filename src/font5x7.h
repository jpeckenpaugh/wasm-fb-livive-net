/* 5x7 dot-matrix font: 5 columns x 7 rows = 35 dots per glyph.
 *
 * Rows are top to bottom, bits are left to right (bit 4 = leftmost column),
 * so one row is a 5-bit mask:
 *
 *     0x0E  01110
 *     0x11  10001
 *     ...
 *
 * Lowercase sits on the same 7-row grid with a 5-row x-height; descenders
 * (g j p q y) use the last two rows. Generated from hand-drawn art; edit the
 * table directly and re-render a sample if a glyph needs fixing.
 */
#ifndef FONT5X7_H
#define FONT5X7_H

#include <stdint.h>

typedef struct {
    char ch;
    uint8_t rows[7];
} glyph5x7_t;

extern const glyph5x7_t k_font5x7[];
extern const int k_font5x7_count;

#endif /* FONT5X7_H */
