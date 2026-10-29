#include "markdown.h"

#include <string.h>

#include "lcd.h"

#define COL_BODY RGB(0xe4, 0xec, 0xf2)
#define COL_BOLD RGB(0xff, 0xff, 0xff)
#define COL_ITALIC RGB(0xc2, 0xcf, 0xda)
#define COL_CODE RGB(0x8f, 0xd8, 0xa8)
#define COL_LINK RGB(0x4c, 0xc2, 0xff)
#define COL_HEAD RGB(0x4c, 0xc2, 0xff)

static int find_char(const char *s, int from, int len, char ch) {
    for (int i = from; i < len; i++) {
        if (s[i] == ch) return i;
    }
    return -1;
}

static int find_pair(const char *s, int from, int len, char a, char b) {
    for (int i = from; i + 1 < len; i++) {
        if (s[i] == a && s[i + 1] == b) return i;
    }
    return -1;
}

/* Draw inline markup in [s, s+len) at x. Returns the width used. */
static int draw_inline(fb_t *fb, const char *s, int len, int x, int y, int cell,
                       uint32_t plain_color) {
    int i = 0;
    int cx = x;

    while (i < len) {
        if (s[i] == '*' && i + 1 < len && s[i + 1] == '*') {
            int close = find_pair(s, i + 2, len, '*', '*');
            int end = (close < 0) ? len : close;
            int n = end - (i + 2);
            if (n > 0) {
                lcd_draw_text5x7_ex(fb, s + i + 2, n, cx, y, cell, COL_BOLD, 1, 0);
                cx += lcd_text5x7_width_n(s + i + 2, n, cell) + 1;
            }
            i = (close < 0) ? len : close + 2;
        } else if (s[i] == '*') {
            int close = find_char(s, i + 1, len, '*');
            int end = (close < 0) ? len : close;
            int n = end - (i + 1);
            if (n > 0) {
                lcd_draw_text5x7_ex(fb, s + i + 1, n, cx, y, cell, COL_ITALIC, 0, 1);
                cx += lcd_text5x7_width_n(s + i + 1, n, cell);
            }
            i = (close < 0) ? len : close + 1;
        } else if (s[i] == '`') {
            int close = find_char(s, i + 1, len, '`');
            int end = (close < 0) ? len : close;
            int n = end - (i + 1);
            if (n > 0) {
                lcd_draw_text5x7_ex(fb, s + i + 1, n, cx, y, cell, COL_CODE, 0, 0);
                cx += lcd_text5x7_width_n(s + i + 1, n, cell);
            }
            i = (close < 0) ? len : close + 1;
        } else if (s[i] == '[') {
            int close = find_char(s, i + 1, len, ']');
            int end = (close < 0) ? len : close;
            int n = end - (i + 1);
            if (n > 0) {
                int w = lcd_text5x7_width_n(s + i + 1, n, cell);
                lcd_draw_text5x7_ex(fb, s + i + 1, n, cx, y, cell, COL_LINK, 0, 0);
                fb_fill(fb, cx, y + 7 * cell + 2, w, 2, COL_LINK);
                cx += w;
            }
            i = end + 1;
            /* Skip the (url) part if present. */
            if (i < len && s[i] == '(') {
                int rp = find_char(s, i + 1, len, ')');
                i = (rp < 0) ? len : rp + 1;
            }
        } else {
            int next = i;
            while (next < len && s[next] != '*' && s[next] != '`' && s[next] != '[') {
                next++;
            }
            int n = next - i;
            if (n > 0) {
                lcd_draw_text5x7_ex(fb, s + i, n, cx, y, cell, plain_color, 0, 0);
                cx += lcd_text5x7_width_n(s + i, n, cell);
            }
            i = next;
        }
    }
    return cx - x;
}

/* Word-wrap one line and draw it. Returns the height used. */
static int draw_wrapped(fb_t *fb, const char *s, int len, int x, int y,
                        int w, int cell, int bold) {
    const int advance = 6 * cell;
    const int max_chars = w / advance;
    if (max_chars < 4) return 0;

    int used = 0;
    int line_h = 7 * cell + 10;
    int i = 0;

    while (i < len) {
        int take = len - i;
        if (take > max_chars) {
            take = max_chars;
            while (take > 0 && s[i + take] != ' ') take--;
            if (take == 0) take = max_chars; /* no space: hard break */
        }
        int end = i + take;
        while (end > i && s[end - 1] == ' ') end--;

        if (bold) {
            lcd_draw_text5x7_ex(fb, s + i, end - i, x, y + used, cell, COL_HEAD, 0, 0);
        } else {
            draw_inline(fb, s + i, end - i, x, y + used, cell, COL_BODY);
        }
        used += line_h;

        i += take;
        while (i < len && s[i] == ' ') i++;
    }
    return used;
}

int md_draw(fb_t *fb, const char *text, int x, int y, int w, int h, int cell) {
    if (!fb || !fb->px || !text || cell <= 0) return 0;

    const int bottom = y + h;
    int cy = y;
    const char *p = text;

    while (*p && cy < bottom) {
        const char *eol = strchr(p, '\n');
        int n = eol ? (int)(eol - p) : (int)strlen(p);

        if (n == 0) {
            cy += cell * 5; /* paragraph break */
        } else if (p[0] == '#') {
            int level = 1;
            while (level < n && p[level] == '#') level++;
            int start = level;
            while (start < n && p[start] == ' ') start++;
            int size = (level == 1) ? cell * 8 / 5
                     : (level == 2) ? cell * 6 / 5
                                    : cell * 5 / 5;
            if (size < cell) size = cell;
            draw_inline(fb, p + start, n - start, x, cy, size, COL_HEAD);
            cy += 7 * size + 16;
        } else if (n >= 2 && (p[0] == '-' || p[0] == '*') && p[1] == ' ') {
            const int dot = cell * 2 / 3;
            if (dot < 2) fb_fill(fb, x + 1, cy + 3 * cell, 2, 2, COL_HEAD);
            else fb_fill(fb, x + 1, cy + 3 * cell, dot, dot, COL_HEAD);
            cy += draw_wrapped(fb, p + 2, n - 2, x + 4 * cell, cy, w - 4 * cell, cell, 0);
        } else {
            cy += draw_wrapped(fb, p, n, x, cy, w, cell, 0);
        }

        p = eol ? eol + 1 : p + n;
    }
    return cy - y;
}
