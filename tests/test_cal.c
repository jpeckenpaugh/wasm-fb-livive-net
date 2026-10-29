/* Native tests for the calendar state, navigation and painting. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "cal.h"

static void test_header_and_month_math(void) {
    cal_state_t c;
    char buf[32];

    cal_init(&c);
    cal_set_today(&c, 2026, 10, 29);
    assert(c.year == 2026 && c.month == 10); /* opens on today's month */
    cal_format_header(&c, buf, sizeof(buf));
    assert(strcmp(buf, "October 2026") == 0);

    /* Month browsing wraps and carries the year. */
    cal_key(&c, CAL_KEY_NEXT_MONTH);
    assert(c.month == 11);
    cal_key(&c, CAL_KEY_PREV_MONTH);
    cal_key(&c, CAL_KEY_PREV_MONTH);
    assert(c.month == 9);
    for (int i = 0; i < 4; i++) cal_key(&c, CAL_KEY_NEXT_MONTH);
    assert(c.year == 2027 && c.month == 1);

    cal_key(&c, CAL_KEY_PREV_YEAR);
    assert(c.year == 2026);
    cal_key(&c, CAL_KEY_NEXT_YEAR);
    assert(c.year == 2027);

    /* Browsing survives a later "today" push (the clock ticks every second). */
    cal_key(&c, CAL_KEY_PREV_YEAR);
    cal_set_today(&c, 2026, 10, 30);
    assert(c.year == 2026 && c.month == 1);
}

static void test_nav_click(void) {
    cal_state_t c;
    cal_init(&c);
    cal_set_today(&c, 2026, 10, 29);

    /* Next-month chevron sits just left of the next-year one. */
    assert(cal_click(&c, 1280 - 160 + 36, 72) == 1);
    assert(c.month == 11);
    /* Next-year chevron. */
    assert(cal_click(&c, 1280 - 72 + 36, 72) == 1);
    assert(c.year == 2027);
    /* A day cell is not a control. */
    assert(cal_click(&c, 400, 400) == 0);
}

static void test_draw(void) {
    const uint32_t SENTINEL = RGBA(255, 0, 255, 255);

    fb_t fb;
    fb.w = 1280;
    fb.h = 720;
    fb.px = (uint32_t *)calloc((size_t)fb.w * fb.h, 4);
    assert(fb.px != NULL);

    cal_state_t c;
    cal_init(&c);
    cal_set_today(&c, 2026, 10, 29);

    fb_clear(&fb, SENTINEL);
    cal_draw(&fb, &c);

    size_t changed = 0;
    for (int y = 0; y < CAL_SCREEN_H; y++) {
        for (int x = 0; x < 1280; x++) {
            if (fb.px[(size_t)y * 1280 + x] != SENTINEL) changed++;
        }
    }
    assert(changed > 50000); /* header + 42 cells */

    /* Tile preview paints inside its box only. */
    fb_clear(&fb, SENTINEL);
    cal_draw_tile(&fb, &c, 288, 620, 128, 72);
    size_t tile = 0;
    for (int y = 648; y < 720; y++) {
        for (int x = 288; x < 416; x++) {
            if (fb.px[(size_t)y * 1280 + x] != SENTINEL) tile++;
        }
    }
    assert(tile > 500);
    assert(fb.px[0] == SENTINEL);

    free(fb.px);
}

int main(void) {
    test_header_and_month_math();
    test_nav_click();
    test_draw();
    printf("test_cal: ok\n");
    return 0;
}
