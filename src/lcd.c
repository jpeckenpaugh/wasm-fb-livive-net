#include "lcd.h"

#include <stddef.h>

enum {
    SEG_A = 1u << 0,
    SEG_B = 1u << 1,
    SEG_C = 1u << 2,
    SEG_D = 1u << 3,
    SEG_E = 1u << 4,
    SEG_F = 1u << 5,
    SEG_G = 1u << 6
};

static const uint8_t k_masks[10] = {
    0x3F, /* 0: a b c d e f   */
    0x06, /* 1: b c           */
    0x5B, /* 2: a b d e g     */
    0x4F, /* 3: a b c d g     */
    0x66, /* 4: b c f g       */
    0x6D, /* 5: a c d f g     */
    0x7D, /* 6: a c d e f g   */
    0x07, /* 7: a b c         */
    0x7F, /* 8: all           */
    0x6F  /* 9: a b c d f g   */
};

uint8_t lcd_segment_mask(int digit) {
    if (digit < 0 || digit > 9) return 0;
    return k_masks[digit];
}

/* Segment thickness: ~1/6 of the width, never more than 1/6 of the height. */
static int seg_thickness(int w, int h) {
    int t = w / 6;
    int max = h / 6;
    if (max < 2) max = 2;
    if (t < 2) t = 2;
    if (t > max) t = max;
    return t;
}

static void put_seg(fb_t *fb, int x, int y, int w, int h,
                    int lit, uint32_t on_color, uint32_t off_color) {
    if (!lit && off_color == LCD_OFF_NONE) return;
    fb_fill(fb, x, y, w, h, lit ? on_color : off_color);
}

void lcd_draw_digit(fb_t *fb, int x, int y, int w, int h, int digit,
                    uint32_t on_color, uint32_t off_color) {
    if (!fb || !fb->px) return;
    if (digit < 0 || digit > 9) return;
    if (w < 8 || h < 16) return;

    const int t = seg_thickness(w, h);
    int vh = (h - 3 * t) / 2; /* vertical segment length */
    if (vh < 1) vh = 1;

    /* Overlap the joints slightly so neighbouring segments visually connect,
     * like a real LCD, instead of leaving gaps a full stroke wide. */
    int sg = t / 2;
    if (sg < 1) sg = 1;

    const uint8_t m = k_masks[digit];

    /* Horizontals: a (top), g (middle), d (bottom). Full width, so the corners
     * meet the verticals instead of leaving square notches. */
    const int hx = x;
    const int hw = w;
    if (hw > 0) {
        put_seg(fb, hx, y, hw, t, m & SEG_A, on_color, off_color);
        put_seg(fb, hx, y + t + vh, hw, t, m & SEG_G, on_color, off_color);
        put_seg(fb, hx, y + h - t, hw, t, m & SEG_D, on_color, off_color);
    }

    /* Verticals: f, b (upper); e, c (lower) */
    put_seg(fb, x, y + t - sg, t, vh + 2 * sg, m & SEG_F, on_color, off_color);
    put_seg(fb, x + w - t, y + t - sg, t, vh + 2 * sg, m & SEG_B, on_color, off_color);
    put_seg(fb, x, y + 2 * t + vh - sg, t, vh + 2 * sg, m & SEG_E, on_color, off_color);
    put_seg(fb, x + w - t, y + 2 * t + vh - sg, t, vh + 2 * sg, m & SEG_C, on_color, off_color);
}

void lcd_draw_colon(fb_t *fb, int x, int y, int w, int h, int lit,
                    uint32_t on_color, uint32_t off_color) {
    if (!fb || !fb->px || w < 2 || h < 8) return;
    if (!lit && off_color == LCD_OFF_NONE) return;

    const uint32_t c = lit ? on_color : off_color;
    /* Square dot, comfortably narrower than the colon cell: at full width the
     * two dots read as blocks that crowd the neighbouring digits. */
    int s = w * 2 / 3;
    if (s > h / 6) s = h / 6;
    if (s < 2) s = 2;
    const int cx = x + (w - s) / 2;
    fb_fill(fb, cx, y + h / 3 - s / 2, s, s, c);
    fb_fill(fb, cx, y + (2 * h) / 3 - s / 2, s, s, c);
}

/* 7x7 dot matrix, bit 6 is the leftmost column. */
static const uint8_t k_symbols[LCD_SYM_COUNT][7] = {
    /* PLUS  */ { 0x08, 0x08, 0x08, 0x3E, 0x08, 0x08, 0x00 },
    /* MINUS */ { 0x00, 0x00, 0x00, 0x3E, 0x00, 0x00, 0x00 },
    /* MUL   */ { 0x00, 0x22, 0x14, 0x08, 0x14, 0x22, 0x00 },
    /* DIV   */ { 0x08, 0x08, 0x00, 0x3E, 0x00, 0x08, 0x08 },
    /* EQ    */ { 0x00, 0x3E, 0x00, 0x3E, 0x00, 0x00, 0x00 },
    /* CLEAR */ { 0x1E, 0x20, 0x20, 0x20, 0x20, 0x20, 0x1E },
    /* BACK  */ { 0x08, 0x10, 0x3E, 0x10, 0x08, 0x00, 0x00 },
    /* E     */ { 0x3E, 0x20, 0x20, 0x38, 0x20, 0x20, 0x3E },
};

void lcd_draw_symbol(fb_t *fb, int x, int y, int w, int h, lcd_symbol_t symbol,
                     uint32_t color) {
    if (!fb || !fb->px) return;
    if (symbol < 0 || symbol >= LCD_SYM_COUNT) return;

    int cell = (w < h ? w : h) / 7;
    if (cell < 2) return;
    /* Solid cells: adjacent dots merge into strokes, which reads far better
     * than a dotted matrix at button sizes. */
    const int dot = cell;

    const int ox = x + (w - cell * 7) / 2;
    const int oy = y + (h - cell * 7) / 2;

    for (int row = 0; row < 7; row++) {
        const uint8_t bits = k_symbols[symbol][row];
        for (int col = 0; col < 7; col++) {
            if (!(bits & (1u << (6 - col)))) continue;
            fb_fill(fb, ox + col * cell + (cell - dot) / 2,
                    oy + row * cell + (cell - dot) / 2, dot, dot, color);
        }
    }
}

/* ---- 14-segment alphanumerics ------------------------------------------ */

typedef struct {
    char ch;
    uint16_t mask;
} glyph14_t;

/* Uppercase A-Z, digits, and the punctuation the date line needs. See lcd.h
 * for the segment diagram. V and Y share a shape (upper V plus the centre
 * stem); no current string contains both. */
static const glyph14_t k_glyphs14[] = {
    { '0', S14_A | S14_B | S14_C | S14_D | S14_E | S14_F },
    { '1', S14_B | S14_C },
    { '2', S14_A | S14_B | S14_G1 | S14_G2 | S14_E | S14_D },
    { '3', S14_A | S14_B | S14_G2 | S14_C | S14_D },
    { '4', S14_F | S14_G1 | S14_G2 | S14_B | S14_C },
    { '5', S14_A | S14_F | S14_G1 | S14_G2 | S14_C | S14_D },
    { '6', S14_A | S14_F | S14_E | S14_D | S14_C | S14_G1 | S14_G2 },
    { '7', S14_A | S14_B | S14_C },
    { '8', S14_A | S14_B | S14_C | S14_D | S14_E | S14_F | S14_G1 | S14_G2 },
    { '9', S14_A | S14_B | S14_C | S14_D | S14_F | S14_G1 | S14_G2 },

    { 'A', S14_A | S14_B | S14_C | S14_E | S14_F | S14_G1 | S14_G2 },
    { 'B', S14_A | S14_B | S14_C | S14_D | S14_F | S14_G1 | S14_G2 },
    { 'C', S14_A | S14_D | S14_E | S14_F },
    { 'D', S14_A | S14_B | S14_C | S14_D | S14_E | S14_F },
    { 'E', S14_A | S14_D | S14_E | S14_F | S14_G1 | S14_G2 },
    { 'F', S14_A | S14_E | S14_F | S14_G1 | S14_G2 },
    { 'G', S14_A | S14_C | S14_D | S14_E | S14_F | S14_G2 },
    { 'H', S14_B | S14_C | S14_E | S14_F | S14_G1 | S14_G2 },
    { 'I', S14_E | S14_F },
    { 'J', S14_B | S14_C | S14_D | S14_E },
    { 'K', S14_E | S14_F | S14_I | S14_K },
    { 'L', S14_D | S14_E | S14_F },
    { 'M', S14_B | S14_C | S14_E | S14_F | S14_H | S14_I },
    { 'N', S14_B | S14_C | S14_E | S14_F | S14_H | S14_K },
    { 'O', S14_A | S14_B | S14_C | S14_D | S14_E | S14_F },
    { 'P', S14_A | S14_B | S14_E | S14_F | S14_G1 | S14_G2 },
    { 'Q', S14_A | S14_B | S14_C | S14_D | S14_E | S14_F | S14_K },
    { 'R', S14_A | S14_B | S14_E | S14_F | S14_G1 | S14_G2 | S14_K },
    { 'S', S14_A | S14_C | S14_D | S14_F | S14_G1 | S14_G2 },
    { 'T', S14_A | S14_L | S14_M },
    { 'U', S14_B | S14_C | S14_D | S14_E | S14_F },
    { 'V', S14_H | S14_I | S14_M },
    { 'W', S14_B | S14_C | S14_E | S14_F | S14_J | S14_K },
    { 'X', S14_H | S14_I | S14_J | S14_K },
    { 'Y', S14_H | S14_I | S14_M },
    { 'Z', S14_A | S14_D | S14_I | S14_J },

    { '-', S14_G1 | S14_G2 },
    { ' ', 0 },
};

uint16_t lcd_14seg_mask(char ch) {
    for (size_t i = 0; i < sizeof(k_glyphs14) / sizeof(k_glyphs14[0]); i++) {
        if (k_glyphs14[i].ch == ch) return k_glyphs14[i].mask;
    }
    return 0;
}

static void seg14(fb_t *fb, int x, int y, int w, int h,
                  int lit, uint32_t on, uint32_t off) {
    if (!lit && off == LCD_OFF_NONE) return;
    if (w <= 0 || h <= 0) return;
    fb_fill(fb, x, y, w, h, lit ? on : off);
}

static void diag14(fb_t *fb, int x0, int y0, int x1, int y1, int t,
                   int lit, uint32_t on, uint32_t off) {
    if (!lit && off == LCD_OFF_NONE) return;
    fb_line(fb, x0, y0, x1, y1, t, lit ? on : off);
}

void lcd_draw_char14(fb_t *fb, int x, int y, int w, int h, char ch,
                     uint32_t on_color, uint32_t off_color) {
    if (!fb || !fb->px || w < 12 || h < 16) return;

    /* Punctuation is not a segment shape. */
    if (ch == '.' || ch == ',') {
        const int s = h / 8 < 3 ? 3 : h / 8;
        fb_fill(fb, x + w / 3, y + h - s, s, s, on_color);
        if (ch == ',') {
            fb_fill(fb, x + w / 3 - s / 2, y + h - s / 2, s / 2, s, on_color);
        }
        return;
    }

    const uint16_t m = lcd_14seg_mask(ch);
    /* ~1/5 of the narrow side: the starburst needs a bolder stroke than the
     * 7-segment digits or it reads as an outline font. */
    int t = (w < h ? w : h) / 5;
    if (t < 2) t = 2;

    const int mid = y + h / 2;
    int vh = h / 2 - t / 2 - t; /* vertical segment length */
    if (vh < 1) vh = 1;

    /* Horizontals: a, g1/g2, d */
    seg14(fb, x + t, y, w - 2 * t, t, m & S14_A, on_color, off_color);
    seg14(fb, x + t, y + h - t, w - 2 * t, t, m & S14_D, on_color, off_color);
    seg14(fb, x + t, mid - t / 2, (w - 2 * t) / 2, t, m & S14_G1, on_color, off_color);
    seg14(fb, x + w / 2, mid - t / 2, (w - 2 * t) / 2, t, m & S14_G2, on_color, off_color);

    /* Verticals: f, b (upper); e, c (lower); l, m (centre) */
    seg14(fb, x, y + t, t, vh, m & S14_F, on_color, off_color);
    seg14(fb, x + w - t, y + t, t, vh, m & S14_B, on_color, off_color);
    seg14(fb, x, mid + t / 2, t, vh, m & S14_E, on_color, off_color);
    seg14(fb, x + w - t, mid + t / 2, t, vh, m & S14_C, on_color, off_color);
    seg14(fb, x + w / 2 - t / 2, y + t, t, vh, m & S14_L, on_color, off_color);
    seg14(fb, x + w / 2 - t / 2, mid + t / 2, t, vh, m & S14_M, on_color, off_color);

    /* Diagonals: h/i upper, j/k lower, all meeting at the centre */
    const int dt = t;
    diag14(fb, x + t, y + t, x + w / 2, mid, dt, m & S14_H, on_color, off_color);
    diag14(fb, x + w - t, y + t, x + w / 2, mid, dt, m & S14_I, on_color, off_color);
    diag14(fb, x + t, y + h - t, x + w / 2, mid, dt, m & S14_J, on_color, off_color);
    diag14(fb, x + w - t, y + h - t, x + w / 2, mid, dt, m & S14_K, on_color, off_color);
}
