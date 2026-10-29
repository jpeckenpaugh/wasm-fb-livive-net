#include "lcd.h"

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
    int s = w; /* square dot */
    if (s > h / 4) s = h / 4;
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
