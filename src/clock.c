#include "clock.h"

#include <stdio.h>
#include <string.h>

#include "lcd.h"

void clock_init(clock_state_t *c) {
    c->hour = c->minute = c->second = c->msec = 0;
    c->valid = 0;
    c->year = c->month = c->day = c->weekday = 0;
    c->date_valid = 0;
    c->drawn_hour = c->drawn_minute = c->drawn_second = -1;
    c->has_drawn = 0;
}

void clock_invalidate(clock_state_t *c) {
    c->has_drawn = 0;
}

void clock_set_time(clock_state_t *c, int h, int m, int s, int ms) {
    if (h < 0) h = 0;
    if (h > 23) h = 23;
    if (m < 0) m = 0;
    if (m > 59) m = 59;
    if (s < 0) s = 0;
    if (s > 59) s = 59;
    if (ms < 0) ms = 0;
    if (ms > 999) ms = 999;

    c->hour = h;
    c->minute = m;
    c->second = s;
    c->msec = ms;
    c->valid = 1;
}

void clock_set_date(clock_state_t *c, int year, int month, int day, int weekday) {
    if (month < 1) month = 1;
    if (month > 12) month = 12;
    if (day < 1) day = 1;
    if (day > 31) day = 31;
    if (weekday < 0) weekday = 0;
    if (weekday > 6) weekday = 6;
    if (year < 0) year = 0;

    c->year = year;
    c->month = month;
    c->day = day;
    c->weekday = weekday;
    c->date_valid = 1;
}

void clock_format_date(const clock_state_t *c, char *out, size_t n) {
    static const char *k_months[12] = {
        "JAN", "FEB", "MAR", "APR", "MAY", "JUN",
        "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"
    };
    static const char *k_days[7] = {
        "SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"
    };

    if (!c->date_valid) {
        snprintf(out, n, " ");
        return;
    }
    snprintf(out, n, "%s %d, %d - %s",
             k_months[c->month - 1], c->day, c->year, k_days[c->weekday]);
}

int clock_advance(clock_state_t *c, int with_seconds) {
    if (!c->valid) return 0;

    const int changed = !c->has_drawn ||
                        c->drawn_hour != c->hour ||
                        c->drawn_minute != c->minute ||
                        (with_seconds && c->drawn_second != c->second);
    if (!changed) return 0;

    c->drawn_hour = c->hour;
    c->drawn_minute = c->minute;
    c->drawn_second = c->second;
    c->has_drawn = 1;
    return 1;
}

unsigned int clock_next_timeout_ms(const clock_state_t *c, int with_seconds) {
    if (!c->valid) return 1000;

    int w;
    if (with_seconds) {
        w = 1000 - c->msec;                        /* 1..1000 */
    } else {
        w = 60000 - (c->second * 1000 + c->msec);  /* 1..60000 */
    }
    if (w < 1) w = 1;
    return (unsigned int)w;
}

void clock_layout(int box_w, int box_h, int digits, clock_layout_t *out) {
    if (digits != 6) digits = 4;
    if (box_w <= 0) box_w = 640;
    if (box_h <= 0) box_h = 480;

    const int margin = box_w / 20 < 4 ? 4 : box_w / 20;
    int avail = box_w - 2 * margin;
    if (avail < 32) avail = box_w;

    const int pairs = digits / 2;
    const int colons = pairs - 1;

    /* Widths in hundredths of a digit, so all of this stays integer math. */
    const int units = digits * 100 + pairs * 18 + colons * 50;

    int d = (avail * 100) / units;

    /* Never taller than the box, then derive the rest from the final width. */
    const int max_d = (box_h - 8) / 2;
    if (max_d > 0 && d > max_d) d = max_d;
    if (d < 8) d = 8;

    int g = (d * 18) / 100;
    if (g < 2) g = 2;
    int c = (d * 50) / 100;
    if (c < 4) c = 4;

    out->digits = digits;
    out->pairs = pairs;
    out->digit_w = d;
    out->digit_h = d * 2;
    out->gap = g;
    out->colon_w = c;
    out->total_w = digits * d + pairs * g + colons * c;
    out->x0 = (box_w - out->total_w) / 2;
    out->y0 = (box_h - out->digit_h) / 2;
    if (out->x0 < 0) out->x0 = 0;
    if (out->y0 < 0) out->y0 = 0;
}

/* Lit segments only, offset around the glyph, so the digits stay readable on
 * top of an image without painting a panel behind them. */
static void digit_outlined(fb_t *fb, int x, int y, int w, int h, int digit,
                           uint32_t color, uint32_t outline, int t) {
    for (int dy = -t; dy <= t; dy += t) {
        for (int dx = -t; dx <= t; dx += t) {
            if (dx == 0 && dy == 0) continue;
            lcd_draw_digit(fb, x + dx, y + dy, w, h, digit, outline, LCD_OFF_NONE);
        }
    }
    lcd_draw_digit(fb, x, y, w, h, digit, color, LCD_OFF_NONE);
}

static void colon_outlined(fb_t *fb, int x, int y, int w, int h,
                           uint32_t color, uint32_t outline, int t) {
    for (int dy = -t; dy <= t; dy += t) {
        for (int dx = -t; dx <= t; dx += t) {
            if (dx == 0 && dy == 0) continue;
            lcd_draw_colon(fb, x + dx, y + dy, w, h, 1, outline, LCD_OFF_NONE);
        }
    }
    lcd_draw_colon(fb, x, y, w, h, 1, color, LCD_OFF_NONE);
}

void clock_draw(fb_t *fb, const clock_state_t *c, int x, int y, int w, int h,
                int digits, uint32_t on_color, uint32_t outline_color) {
    if (!fb || !fb->px || !c || !c->valid) return;

    clock_layout_t L;
    clock_layout(w, h, digits, &L);

    int t = L.digit_w / 24;
    if (t < 1) t = 1;
    if (t > 3) t = 3;

    int values[6];
    int n = 0;
    values[n++] = c->hour / 10;
    values[n++] = c->hour % 10;
    values[n++] = c->minute / 10;
    values[n++] = c->minute % 10;
    if (L.digits == 6) {
        values[n++] = c->second / 10;
        values[n++] = c->second % 10;
    }

    int cx = x + L.x0;
    const int cy = y + L.y0;

    for (int i = 0; i < n; i++) {
        digit_outlined(fb, cx, cy, L.digit_w, L.digit_h, values[i],
                       on_color, outline_color, t);
        cx += L.digit_w;
        if (i == n - 1) break;
        if (i % 2 == 1) {
            colon_outlined(fb, cx, cy, L.colon_w, L.digit_h,
                           on_color, outline_color, t);
            cx += L.colon_w;
        } else {
            cx += L.gap;
        }
    }
}

void clock_draw_date(fb_t *fb, const clock_state_t *c, int x, int y, int w, int h,
                     uint32_t on_color, uint32_t outline_color) {
    if (!fb || !fb->px || !c || !c->date_valid || h < 12) return;

    char text[32];
    clock_format_date(c, text, sizeof(text));
    const int len = (int)strlen(text);
    if (len <= 0) return;

    /* Character cells are 3:5 (14-segment proportions); shrink to fit. */
    int ch = h;
    int cw = ch * 3 / 5;
    int gap = cw / 5;
    while (ch > 12 && len * (cw + gap) - gap > w - 16) {
        ch -= 2;
        cw = ch * 3 / 5;
        gap = cw / 5;
    }
    if (len * (cw + gap) - gap > w - 16) return; /* cannot fit at all */

    int cx = x + (w - (len * (cw + gap) - gap)) / 2;
    const int cy = y + (h - ch) / 2;

    int off = ch / 36;
    if (off < 1) off = 1;
    if (off > 3) off = 3;

    for (int i = 0; i < len; i++) {
        if (outline_color != 0) {
            for (int dy = -off; dy <= off; dy += off) {
                for (int dx = -off; dx <= off; dx += off) {
                    if (dx == 0 && dy == 0) continue;
                    lcd_draw_char14(fb, cx + dx, cy + dy, cw, ch, text[i],
                                    outline_color, LCD_OFF_NONE);
                }
            }
        }
        lcd_draw_char14(fb, cx, cy, cw, ch, text[i], on_color, LCD_OFF_NONE);
        cx += cw + gap;
    }
}
