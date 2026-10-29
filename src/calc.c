#include "calc.h"

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "lcd.h"

#define CALC_W 640

#define DISP_X 17
#define DISP_Y 12
#define DISP_W (CALC_W - 2 * DISP_X)
#define DISP_H 88
#define DISP_TEXT_H 56
#define DISP_GAP 18

#define PAD 17
#define BW 144
#define BH 44
#define GAP 8
#define KEY_Y 116

#define COL_PANEL RGB(0x0b, 0x10, 0x15)
#define COL_LINE RGB(0x2a, 0x36, 0x44)
#define COL_KEY RGB(0x17, 0x1e, 0x27)
#define COL_OP RGB(0x12, 0x26, 0x2c)
#define COL_EQ RGB(0x10, 0x34, 0x37)
#define COL_AC RGB(0x2e, 0x17, 0x1d)
#define COL_ON RGB(0xe8, 0xf2, 0xf8)
#define COL_ACCENT RGB(0x4c, 0xc2, 0xff)
#define COL_TILE_ON RGB(0xf2, 0xf8, 0xfa)
#define COL_TILE_EDGE RGB(0x08, 0x0c, 0x0e)

typedef struct {
    unsigned char col, row, cspan, rspan;
    int key;
} calc_button_t;

static const calc_button_t k_buttons[] = {
    { 0, 0, 1, 1, CALC_KEY_CLEAR }, { 1, 0, 1, 1, CALC_KEY_BACK },
    { 2, 0, 1, 1, CALC_KEY_DIV },   { 3, 0, 1, 1, CALC_KEY_MUL },

    { 0, 1, 1, 1, CALC_KEY_DIGIT(7) }, { 1, 1, 1, 1, CALC_KEY_DIGIT(8) },
    { 2, 1, 1, 1, CALC_KEY_DIGIT(9) }, { 3, 1, 1, 1, CALC_KEY_MINUS },

    { 0, 2, 1, 1, CALC_KEY_DIGIT(4) }, { 1, 2, 1, 1, CALC_KEY_DIGIT(5) },
    { 2, 2, 1, 1, CALC_KEY_DIGIT(6) }, { 3, 2, 1, 1, CALC_KEY_PLUS },

    { 0, 3, 1, 1, CALC_KEY_DIGIT(1) }, { 1, 3, 1, 1, CALC_KEY_DIGIT(2) },
    { 2, 3, 1, 1, CALC_KEY_DIGIT(3) }, { 3, 3, 1, 2, CALC_KEY_EQ },

    { 0, 4, 2, 1, CALC_KEY_DIGIT(0) }, { 2, 4, 1, 1, CALC_KEY_DOT },
};

#define BUTTON_COUNT (int)(sizeof(k_buttons) / sizeof(k_buttons[0]))

/* ------------------------------------------------------------------ state -- */

void calc_init(calc_state_t *c) {
    memset(c, 0, sizeof(*c));
    c->acc = 0.0;
}

static void clear_entry(calc_state_t *c) {
    c->entry_len = 0;
    c->entry[0] = '\0';
}

static double entry_value(const calc_state_t *c) {
    return c->entry_len > 0 ? strtod(c->entry, NULL) : 0.0;
}

/* Entry limit: 10 characters keeps the display inside the panel. */
static int entry_push(calc_state_t *c, char ch) {
    if (c->entry_len >= 10) return 0;
    if (ch == '.') {
        if (strchr(c->entry, '.') != NULL) return 0;
        if (c->entry_len == 0) entry_push(c, '0');
    }
    c->entry[c->entry_len++] = ch;
    c->entry[c->entry_len] = '\0';
    return 1;
}

static int apply(calc_state_t *c, char op, double rhs) {
    double out = rhs;
    switch (op) {
        case '+': out = c->acc + rhs; break;
        case '-': out = c->acc - rhs; break;
        case '*': out = c->acc * rhs; break;
        case '/':
            if (rhs == 0.0) return 0;
            out = c->acc / rhs;
            break;
        default: out = rhs; break;
    }
    if (!isfinite(out) || fabs(out) >= 1e9) return 0; /* 9 digits max on screen */
    c->acc = out;
    return 1;
}

static int press_operator(calc_state_t *c, char op) {
    if (c->entry_len > 0) {
        if (!apply(c, c->pending, entry_value(c))) {
            c->error = 1;
            return 1;
        }
        clear_entry(c);
    }
    c->pending = op;
    return 1;
}

static int press_equals(calc_state_t *c) {
    if (c->entry_len > 0) {
        if (!apply(c, c->pending, entry_value(c))) {
            c->error = 1;
            return 1;
        }
        clear_entry(c);
    } else if (c->pending != 0) {
        if (!apply(c, c->pending, c->acc)) {
            c->error = 1;
            return 1;
        }
    }
    c->pending = 0;
    return 1;
}

int calc_key(calc_state_t *c, int key) {
    if (c->error) {
        /* Any key after an error starts clean; AC does the same, which matches
         * what a physical calculator does. */
        calc_init(c);
        if (key == CALC_KEY_CLEAR) return 1;
    }

    if (key >= CALC_KEY_0 && key <= CALC_KEY_0 + 9) {
        return entry_push(c, (char)('0' + (key - CALC_KEY_0)));
    }

    switch (key) {
        case CALC_KEY_DOT:
            return entry_push(c, '.');
        case CALC_KEY_PLUS:
            return press_operator(c, '+');
        case CALC_KEY_MINUS:
            return press_operator(c, '-');
        case CALC_KEY_MUL:
            return press_operator(c, '*');
        case CALC_KEY_DIV:
            return press_operator(c, '/');
        case CALC_KEY_EQ:
            return press_equals(c);
        case CALC_KEY_BACK:
            if (c->entry_len == 0) return 0;
            c->entry[--c->entry_len] = '\0';
            return 1;
        case CALC_KEY_CLEAR:
            calc_init(c);
            return 1;
        default:
            return 0;
    }
}

/* --------------------------------------------------------------- display -- */

static void format_value(double value, char *out, size_t n) {
    if (!isfinite(value) || fabs(value) >= 1e9) {
        snprintf(out, n, "E");
        return;
    }

    /* Most decimals that still fit the panel; the display is ~9 characters. */
    for (int prec = 3; prec >= 0; prec--) {
        snprintf(out, n, "%.*f", prec, value);
        if (strlen(out) <= 9) break;
    }
    if (strlen(out) > 9) {
        snprintf(out, n, "E");
        return;
    }

    if (strchr(out, '.') != NULL) {
        size_t len = strlen(out);
        while (len > 0 && out[len - 1] == '0') out[--len] = '\0';
        if (len > 0 && out[len - 1] == '.') out[--len] = '\0';
    }
    if (strcmp(out, "-0") == 0) snprintf(out, n, "0");
}

void calc_display(const calc_state_t *c, char *out, size_t n) {
    if (!c || !out || n == 0) return;
    if (c->error) {
        snprintf(out, n, "E");
    } else if (c->entry_len > 0) {
        snprintf(out, n, "%s", c->entry);
    } else {
        format_value(c->acc, out, n);
    }
}

/* Glyph metrics scale with the cap height, so the same renderer serves the
 * 56px readout and the ~20px tile. */
static int glyph_w(char ch, int h) {
    if (ch == '.') return h / 5;
    if (ch == '-') return (h / 2) * 4 / 5;
    return h / 2; /* digits and E */
}

static int glyph_gap(int h) {
    int g = h / 12;
    return g < 1 ? 1 : g;
}

static int text_width(const char *text, int h) {
    int total = 0;
    for (const char *p = text; *p; p++) total += glyph_w(*p, h) + glyph_gap(h);
    return total > 0 ? total - glyph_gap(h) : 0;
}

static void glyph(fb_t *fb, char ch, int x, int y, int h, uint32_t color) {
    const int w = glyph_w(ch, h);

    if (ch >= '0' && ch <= '9') {
        lcd_draw_digit(fb, x, y, w, h, ch - '0', color, LCD_OFF_NONE);
    } else if (ch == '.') {
        const int s = h / 6;
        fb_fill(fb, x, y + h - s, s < 2 ? 2 : s, s < 2 ? 2 : s, color);
    } else if (ch == '-') {
        int t = h / 10;
        if (t < 2) t = 2;
        fb_fill(fb, x, y + h / 2 - t / 2, w, t, color);
    } else if (ch == 'E') {
        lcd_draw_symbol(fb, x, y, w, h, LCD_SYM_E, color);
    }
}

static void draw_text(fb_t *fb, const char *text, int right, int top, int h,
                      uint32_t color, uint32_t edge) {
    int x = right - text_width(text, h);

    for (const char *p = text; *p; p++) {
        if (edge != 0) {
            for (int dy = -2; dy <= 2; dy += 2) {
                for (int dx = -2; dx <= 2; dx += 2) {
                    if (dx == 0 && dy == 0) continue;
                    glyph(fb, *p, x + dx, top + dy, h, edge);
                }
            }
        }
        glyph(fb, *p, x, top, h, color);
        x += glyph_w(*p, h) + glyph_gap(h);
    }
}

static lcd_symbol_t symbol_for(char op) {
    switch (op) {
        case '+': return LCD_SYM_PLUS;
        case '-': return LCD_SYM_MINUS;
        case '*': return LCD_SYM_MUL;
        default: return LCD_SYM_DIV;
    }
}

/* ----------------------------------------------------------------- layout -- */

static void button_rect(const calc_button_t *b, int *x, int *y, int *w, int *h) {
    *x = PAD + b->col * (BW + GAP);
    *y = KEY_Y + b->row * (BH + GAP);
    *w = b->cspan * BW + (b->cspan - 1) * GAP;
    *h = b->rspan * BH + (b->rspan - 1) * GAP;
}

int calc_click(calc_state_t *c, int x, int y) {
    for (int i = 0; i < BUTTON_COUNT; i++) {
        int bx, by, bw, bh;
        button_rect(&k_buttons[i], &bx, &by, &bw, &bh);
        if (x >= bx && x < bx + bw && y >= by && y < by + bh) {
            return calc_key(c, k_buttons[i].key);
        }
    }
    return 0;
}

/* ------------------------------------------------------------------ paint -- */

static uint32_t button_color(int key) {
    if (key == CALC_KEY_CLEAR) return COL_AC;
    if (key == CALC_KEY_EQ) return COL_EQ;
    if (key == CALC_KEY_PLUS || key == CALC_KEY_MINUS || key == CALC_KEY_MUL ||
        key == CALC_KEY_DIV) {
        return COL_OP;
    }
    return COL_KEY;
}

static void draw_label(fb_t *fb, const calc_button_t *b, int x, int y, int w, int h) {
    const int key = b->key;

    if (key >= CALC_KEY_0 && key <= CALC_KEY_0 + 9) {
        const int dw = 18, dh = 36;
        lcd_draw_digit(fb, x + (w - dw) / 2, y + (h - dh) / 2, dw, dh,
                       key - CALC_KEY_0, COL_ON, LCD_OFF_NONE);
        return;
    }

    if (key == CALC_KEY_DOT) {
        const int s = 12;
        fb_fill(fb, x + (w - s) / 2, y + (h + s) / 2 - s, s, s, COL_ON);
        return;
    }

    lcd_symbol_t sym;
    switch (key) {
        case CALC_KEY_PLUS: sym = LCD_SYM_PLUS; break;
        case CALC_KEY_MINUS: sym = LCD_SYM_MINUS; break;
        case CALC_KEY_MUL: sym = LCD_SYM_MUL; break;
        case CALC_KEY_DIV: sym = LCD_SYM_DIV; break;
        case CALC_KEY_EQ: sym = LCD_SYM_EQ; break;
        case CALC_KEY_CLEAR: sym = LCD_SYM_CLEAR; break;
        case CALC_KEY_BACK: sym = LCD_SYM_BACK; break;
        default: return;
    }

    const int s = 24;
    lcd_draw_symbol(fb, x + (w - s) / 2, y + (h - s) / 2, s, s, sym, COL_ON);
}

void calc_draw(fb_t *fb, const calc_state_t *c) {
    if (!fb || !fb->px || !c) return;

    /* The caller owns the backdrop (the image behind the keys); this function
     * only paints the readout panel and the keypad. */
    fb_fill(fb, DISP_X, DISP_Y, DISP_W, DISP_H, COL_PANEL);
    fb_border(fb, DISP_X, DISP_Y, DISP_W, DISP_H, 2, COL_LINE);

    if (c->pending != 0 && !c->error) {
        lcd_draw_symbol(fb, DISP_X + 16, DISP_Y + (DISP_H - 24) / 2, 24, 24,
                        symbol_for(c->pending), COL_ACCENT);
    }

    char text[16];
    calc_display(c, text, sizeof(text));
    draw_text(fb, text, DISP_X + DISP_W - DISP_GAP,
              DISP_Y + (DISP_H - DISP_TEXT_H) / 2, DISP_TEXT_H, COL_ON, 0);

    for (int i = 0; i < BUTTON_COUNT; i++) {
        int x, y, w, h;
        button_rect(&k_buttons[i], &x, &y, &w, &h);
        fb_fill(fb, x, y, w, h, button_color(k_buttons[i].key));
        fb_border(fb, x, y, w, h, 1, COL_LINE);
        draw_label(fb, &k_buttons[i], x, y, w, h);
    }
}

void calc_draw_tile(fb_t *fb, const calc_state_t *c, int x, int y, int w, int h) {
    if (!fb || !fb->px || !c) return;

    char text[16];
    calc_display(c, text, sizeof(text));

    /* Fit the tile: start at a third of its height and shrink until it fits. */
    int th = h / 3;
    if (th < 12) th = 12;
    while (th > 12 && text_width(text, th) > w - 10) th -= 2;

    const int top = y + (h - th) / 2;
    const int right = x + w - 5;
    draw_text(fb, text, right, top, th, COL_TILE_ON, COL_TILE_EDGE);
}
