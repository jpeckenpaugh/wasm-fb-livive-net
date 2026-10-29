#include "cal.h"

#include <stdio.h>
#include <string.h>

#include "lcd.h"

#define CAL_W 1280

#define HEADER_X 40
#define HEADER_Y 24
#define HEADER_W (CAL_W - 2 * HEADER_X)
#define HEADER_H 96

#define NAV 72
#define NAV_Y (HEADER_Y + 12)

#define WEEK_Y 146
#define GRID_Y 200

#define COLS 7
#define CELL_W 164
#define CELL_H 60
#define CELL_GAP 5
#define GRID_X ((CAL_W - (COLS * CELL_W + (COLS - 1) * CELL_GAP)) / 2)

#define COL_PANEL RGB(0x0b, 0x10, 0x15)
#define COL_CELL RGB(0x14, 0x1a, 0x22)
#define COL_LINE RGB(0x2a, 0x36, 0x44)
#define COL_TEXT RGB(0xe8, 0xf2, 0xf8)
#define COL_DIM RGB(0x6d, 0x7d, 0x8c)
#define COL_ACCENT RGB(0x4c, 0xc2, 0xff)
#define COL_ACCENT_BG RGB(0x10, 0x33, 0x3d)

static const char *k_months[12] = {
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December"
};
static const char *k_months_short[12] = {
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN",
    "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"
};

/* 0 = Sunday. Sakamoto's algorithm: plenty for a calendar UI. */
static int weekday_of(int y, int m, int d) {
    static const int t[12] = { 0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4 };
    if (m < 3) y -= 1;
    int w = (y + y / 4 - y / 100 + y / 400 + t[m - 1] + d) % 7;
    if (w < 0) w += 7;
    return w;
}

static int days_in_month(int y, int m) {
    static const int d[12] = { 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31 };
    if (m == 2 && ((y % 4 == 0 && y % 100 != 0) || y % 400 == 0)) return 29;
    return d[m - 1];
}

void cal_init(cal_state_t *c) {
    memset(c, 0, sizeof(*c));
    c->month = 1;
    c->today_month = 1;
    c->today_day = 1;
}

void cal_set_today(cal_state_t *c, int year, int month, int day) {
    if (month < 1) month = 1;
    if (month > 12) month = 12;
    if (day < 1) day = 1;
    if (day > 31) day = 31;

    if (!c->today_valid) {
        /* First push: open on the current month. Later pushes only move the
         * highlight, so browsing survives the per-second clock tick. */
        c->year = year;
        c->month = month;
    }
    c->today_year = year;
    c->today_month = month;
    c->today_day = day;
    c->today_valid = 1;
}

static void shift_month(cal_state_t *c, int delta) {
    int m = c->month + delta;
    while (m < 1) { m += 12; c->year -= 1; }
    while (m > 12) { m -= 12; c->year += 1; }
    c->month = m;
    if (c->year < 1) c->year = 1;
}

int cal_key(cal_state_t *c, int key) {
    switch (key) {
        case CAL_KEY_PREV_MONTH: shift_month(c, -1); return 1;
        case CAL_KEY_NEXT_MONTH: shift_month(c, 1); return 1;
        case CAL_KEY_PREV_YEAR: c->year -= 1; if (c->year < 1) c->year = 1; return 1;
        case CAL_KEY_NEXT_YEAR: c->year += 1; return 1;
        default: return 0;
    }
}

/* Header chevrons: year on the outside, month next to the title. */
static void nav_rect(int which, int *x, int *y, int *w, int *h) {
    static const int xs[4] = { 64, 152, CAL_W - 160, CAL_W - 72 };
    *x = xs[which];
    *y = NAV_Y;
    *w = NAV;
    *h = NAV;
}

int cal_click(cal_state_t *c, int x, int y) {
    static const int keys[4] = {
        CAL_KEY_PREV_YEAR, CAL_KEY_PREV_MONTH,
        CAL_KEY_NEXT_MONTH, CAL_KEY_NEXT_YEAR
    };
    for (int i = 0; i < 4; i++) {
        int bx, by, bw, bh;
        nav_rect(i, &bx, &by, &bw, &bh);
        if (x >= bx && x < bx + bw && y >= by && y < by + bh) {
            return cal_key(c, keys[i]);
        }
    }
    return 0;
}

void cal_format_header(const cal_state_t *c, char *out, size_t n) {
    if (!c || !out || n == 0) return;
    int m = c->month;
    if (m < 1) m = 1;
    if (m > 12) m = 12;
    snprintf(out, n, "%s %d", k_months[m - 1], c->year);
}

/* ---------------------------------------------------------------- drawing -- */

static void nav_button(fb_t *fb, int which, const char *label) {
    int x, y, w, h;
    nav_rect(which, &x, &y, &w, &h);

    fb_fill(fb, x, y, w, h, COL_CELL);
    fb_border(fb, x, y, w, h, 2, COL_LINE);

    const int cell = 6;
    const int tw = lcd_text5x7_width(label, cell);
    lcd_draw_text5x7(fb, label, x + (w - tw) / 2, y + (h - 7 * cell) / 2,
                     cell, COL_TEXT);
}

void cal_draw(fb_t *fb, const cal_state_t *c) {
    if (!fb || !fb->px || !c) return;

    /* Header plate + title. */
    fb_fill(fb, HEADER_X, HEADER_Y, HEADER_W, HEADER_H, COL_PANEL);
    fb_border(fb, HEADER_X, HEADER_Y, HEADER_W, HEADER_H, 2, COL_LINE);

    char title[32];
    cal_format_header(c, title, sizeof(title));
    const int title_cell = 10;
    const int title_w = lcd_text5x7_width(title, title_cell);
    lcd_draw_text5x7(fb, title, (CAL_W - title_w) / 2,
                     HEADER_Y + (HEADER_H - 7 * title_cell) / 2,
                     title_cell, COL_TEXT);

    nav_button(fb, 0, "<<");
    nav_button(fb, 1, "<");
    nav_button(fb, 2, ">");
    nav_button(fb, 3, ">>");

    /* Weekday scale, one letter per column. */
    static const char *k_week = "SMTWTFS";
    const int week_cell = 5;
    for (int i = 0; i < COLS; i++) {
        char one[2] = { k_week[i], '\0' };
        const int tw = lcd_text5x7_width(one, week_cell);
        const int cx = GRID_X + i * (CELL_W + CELL_GAP) + (CELL_W - tw) / 2;
        lcd_draw_text5x7(fb, one, cx, WEEK_Y, week_cell,
                         (i == 0 || i == 6) ? COL_DIM : COL_TEXT);
    }

    /* Day grid. */
    const int first = weekday_of(c->year, c->month, 1);
    const int days = days_in_month(c->year, c->month);

    for (int slot = 0; slot < 42; slot++) {
        const int col = slot % COLS;
        const int row = slot / COLS;
        const int x = GRID_X + col * (CELL_W + CELL_GAP);
        const int y = GRID_Y + row * (CELL_H + CELL_GAP);
        const int day = slot - first + 1;

        const int is_today = c->today_valid && day >= 1 && day <= days &&
                             c->year == c->today_year &&
                             c->month == c->today_month &&
                             day == c->today_day;

        fb_fill(fb, x, y, CELL_W, CELL_H, is_today ? COL_ACCENT_BG : COL_CELL);
        fb_border(fb, x, y, CELL_W, CELL_H, is_today ? 2 : 1,
                  is_today ? COL_ACCENT : COL_LINE);

        if (day < 1 || day > days) continue;

        char num[4];
        snprintf(num, sizeof(num), "%d", day);
        const int nd = (int)strlen(num);

        /* 7-segment day numbers, centred. */
        const int dw = 22, dh = 44;
        const int gap = 6;
        const int total = nd * dw + (nd - 1) * gap;
        int nx = x + (CELL_W - total) / 2;
        const int ny = y + (CELL_H - dh) / 2;
        const uint32_t color = is_today ? COL_ACCENT : COL_TEXT;
        for (int i = 0; i < nd; i++) {
            lcd_draw_digit(fb, nx, ny, dw, dh, num[i] - '0', color, LCD_OFF_NONE);
            nx += dw + gap;
        }
    }
}

void cal_draw_tile(fb_t *fb, const cal_state_t *c, int x, int y, int w, int h) {
    if (!fb || !fb->px || !c) return;

    /* A calendar page: month abbreviation over the day of month. */
    fb_fill(fb, x + 6, y + 4, w - 12, h - 8, COL_PANEL);
    fb_border(fb, x + 6, y + 4, w - 12, h - 8, 1, COL_LINE);

    /* Month abbreviation in the 16-segment font: it is the only place the
     * long-diagonal glyphs are exercised on screen. */
    const int month = (c->month >= 1 && c->month <= 12) ? c->month : 1;
    const int mh = 22, mw = 13, mgap = 4;
    const int tw = 3 * mw + 2 * mgap;
    int mx = x + (w - tw) / 2;
    for (int i = 0; i < 3; i++) {
        lcd_draw_char16(fb, mx, y + 8, mw, mh, k_months_short[month - 1][i],
                        COL_TEXT, LCD_OFF_NONE);
        mx += mw + mgap;
    }

    char day[4];
    snprintf(day, sizeof(day), "%d", c->today_valid ? c->today_day : 1);
    const int nd = (int)strlen(day);
    const int dw = 16, dh = 32, gap = 4;
    const int total = nd * dw + (nd - 1) * gap;
    int nx = x + (w - total) / 2;
    const int ny = y + h - dh - 7;
    for (int i = 0; i < nd; i++) {
        lcd_draw_digit(fb, nx, ny, dw, dh, day[i] - '0', COL_ACCENT, LCD_OFF_NONE);
        nx += dw + gap;
    }
}
