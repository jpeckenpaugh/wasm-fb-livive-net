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

#define PAD 17
#define BW 144
#define BH 44
#define GAP 8
#define KEY_Y 116

#define CHAR_H 56
#define CHAR_DIGIT_W 28
#define CHAR_DOT_W 10
#define CHAR_MINUS_W 24

#define COL_BG RGB(0x10, 0x14, 0x1a)
#define COL_PANEL RGB(0x0b, 0x10, 0x15)
#define COL_LINE RGB(0x2a, 0x36, 0x44)
#define COL_KEY RGB(0x1a, 0x22, 0x2c)
#define COL_OP RGB(0x15, 0x2b, 0x31)
#define COL_EQ RGB(0x12, 0x3a, 0x3d)
#define COL_AC RGB(0x33, 0x1a, 0x21)
#define COL_ON RGB(0xe8, 0xf2, 0xf8)
#define COL_ACCENT RGB(0x4c, 0xc2, 0xff)

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

    { 0, 4, 3, 1, CALC_KEY_DIGIT(0) },
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
        if (key != CALC_KEY_CLEAR) {
            /* fall through and apply the key against the cleared state */
        } else {
            return 1;
        }
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

static int char_width(char ch) {
    if (ch >= '0' && ch <= '9') return CHAR_DIGIT_W;
    if (ch == '.') return CHAR_DOT_W;
    if (ch == '-') return CHAR_MINUS_W;
    if (ch == 'E') return CHAR_DIGIT_W;
    return 0;
}

static void draw_char(fb_t *fb, int x, int y, char ch, uint32_t color) {
    if (ch >= '0' && ch <= '9') {
        lcd_draw_digit(fb, x, y, CHAR_DIGIT_W, CHAR_H, ch - '0', color, LCD_OFF_NONE);
    } else if (ch == '.') {
        fb_fill(fb, x + 1, y + CHAR_H - 10, 8, 8, color);
    } else if (ch == '-') {
        fb_fill(fb, x, y + CHAR_H / 2 - 3, CHAR_MINUS_W - 6, 6, color);
    } else if (ch == 'E') {
        lcd_draw_symbol(fb, x, y, CHAR_DIGIT_W, CHAR_H, LCD_SYM_E, color);
    }
}

static void draw_value(fb_t *fb, const char *text, int right, int y, uint32_t color) {
    int total = 0;
    for (const char *p = text; *p; p++) total += char_width(*p);

    int x = right - total;
    for (const char *p = text; *p; p++) {
        draw_char(fb, x, y, *p, color);
        x += char_width(*p);
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

static void draw_label(fb_t *fb, const calc_button_t *b,
                       int x, int y, int w, int h) {
    const int key = b->key;

    if (key >= CALC_KEY_0 && key <= CALC_KEY_0 + 9) {
        const int dw = 18, dh = 36;
        lcd_draw_digit(fb, x + (w - dw) / 2, y + (h - dh) / 2, dw, dh,
                       key - CALC_KEY_0, COL_ON, LCD_OFF_NONE);
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

    fb_fill(fb, 0, 0, CALC_W, CALC_SCREEN_H, COL_BG);

    /* Readout panel. */
    fb_fill(fb, DISP_X, DISP_Y, DISP_W, DISP_H, COL_PANEL);
    fb_border(fb, DISP_X, DISP_Y, DISP_W, DISP_H, 2, COL_LINE);

    if (c->pending != 0 && !c->error) {
        lcd_draw_symbol(fb, DISP_X + 16, DISP_Y + (DISP_H - 24) / 2, 24, 24,
                        symbol_for(c->pending), COL_ACCENT);
    }

    char text[16];
    calc_display(c, text, sizeof(text));
    draw_value(fb, text, DISP_X + DISP_W - 18, DISP_Y + (DISP_H - CHAR_H) / 2, COL_ON);

    /* Keypad. */
    for (int i = 0; i < BUTTON_COUNT; i++) {
        int x, y, w, h;
        button_rect(&k_buttons[i], &x, &y, &w, &h);
        fb_fill(fb, x, y, w, h, button_color(k_buttons[i].key));
        fb_border(fb, x, y, w, h, 1, COL_LINE);
        draw_label(fb, &k_buttons[i], x, y, w, h);
    }
}
