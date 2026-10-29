/* Native tests for the clock state machine, layout and overlay drawing. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include "clock.h"
#include "framebuffer.h"

static fb_t make_fb(int w, int h) {
    fb_t fb;
    fb.w = w;
    fb.h = h;
    fb.px = (uint32_t *)calloc((size_t)w * (size_t)h, 4);
    assert(fb.px != NULL);
    return fb;
}

static void test_dirty_tracking(void) {
    clock_state_t c;

    clock_init(&c);
    assert(clock_advance(&c, 1) == 0); /* no time pushed yet */
    assert(clock_advance(&c, 0) == 0);

    clock_set_time(&c, 12, 34, 56, 0);
    assert(clock_advance(&c, 1) == 1); /* first report */
    assert(clock_advance(&c, 1) == 0); /* nothing moved */

    clock_set_time(&c, 12, 34, 57, 0);
    assert(clock_advance(&c, 1) == 1); /* second-level sees it */
    assert(clock_advance(&c, 0) == 0); /* minute-level does not */

    clock_set_time(&c, 12, 35, 0, 0);
    assert(clock_advance(&c, 0) == 1); /* minute-level sees it */

    clock_set_time(&c, 0, 0, 0, 0);
    assert(clock_advance(&c, 1) == 1); /* midnight rollover is just a value */

    clock_invalidate(&c);
    assert(clock_advance(&c, 1) == 1); /* forced repaint */
    assert(clock_advance(&c, 1) == 0);
}

static void test_timeout(void) {
    clock_state_t c;
    clock_init(&c);
    assert(clock_next_timeout_ms(&c, 1) == 1000); /* unknown time */

    clock_set_time(&c, 1, 2, 3, 250);
    assert(clock_next_timeout_ms(&c, 1) == 750);
    clock_set_time(&c, 1, 2, 3, 999);
    assert(clock_next_timeout_ms(&c, 1) == 1);
    clock_set_time(&c, 1, 2, 3, 0);
    assert(clock_next_timeout_ms(&c, 1) == 1000);

    /* Minute granularity: wake on the next minute boundary. */
    clock_set_time(&c, 1, 2, 3, 250);
    assert(clock_next_timeout_ms(&c, 0) == 60000 - 3250);
    clock_set_time(&c, 1, 2, 59, 999);
    assert(clock_next_timeout_ms(&c, 0) == 1);
}

static void test_layout(void) {
    clock_layout_t hero;
    clock_layout_t thumb;

    clock_layout(1280, 720, 6, &hero);
    assert(hero.digits == 6);
    assert(hero.pairs == 3);
    assert(hero.x0 >= 0 && hero.y0 >= 0);
    assert(hero.x0 + hero.total_w <= 1280);
    assert(hero.y0 + hero.digit_h <= 720);
    assert(hero.digit_w >= 8 && hero.gap >= 2 && hero.colon_w >= 4);

    clock_layout(128, 72, 4, &thumb);
    assert(thumb.digits == 4);
    assert(thumb.pairs == 2);
    assert(thumb.x0 >= 0 && thumb.y0 >= 0);
    assert(thumb.x0 + thumb.total_w <= 128);
    assert(thumb.y0 + thumb.digit_h <= 72);

    /* Anything other than 6 falls back to 4 digits. */
    clock_layout_t odd;
    clock_layout(128, 72, 5, &odd);
    assert(odd.digits == 4);

    /* Degenerate boxes still produce a usable layout. */
    clock_layout_t tiny;
    clock_layout(1, 1, 6, &tiny);
    assert(tiny.digit_w >= 8);
}

static void test_draw(void) {
    const uint32_t WHITE = RGBA(255, 255, 255, 255);
    const uint32_t SENTINEL = RGBA(255, 0, 255, 255);

    fb_t fb = make_fb(1280, 720);
    clock_state_t c;
    clock_init(&c);

    /* Invalid time draws nothing at all. */
    fb_clear(&fb, SENTINEL);
    clock_draw(&fb, &c, 0, 0, 1280, 720, 6, WHITE, RGBA(0, 0, 0, 255));
    assert(fb.px[0] == SENTINEL);

    clock_set_time(&c, 12, 34, 56, 0);
    fb_clear(&fb, SENTINEL);
    clock_draw(&fb, &c, 0, 0, 1280, 720, 6, WHITE, RGBA(0, 0, 0, 255));

    /* Pixels changed inside the hero box ... */
    size_t inside = 0;
    for (int y = 0; y < 720; y++) {
        for (int x = 0; x < 1280; x++) {
            if (fb.px[(size_t)y * 1280 + x] != SENTINEL) inside++;
        }
    }
    assert(inside > 1000);

    /* ... and the overlay paints no background of its own. */
    assert(fb.px[0] == SENTINEL);
    assert(fb.px[(size_t)719 * 1280 + 1279] == SENTINEL);

    /* NULL inputs are tolerated. */
    clock_draw(NULL, &c, 0, 0, 100, 100, 4, WHITE, 0);
    clock_draw(&fb, NULL, 0, 0, 100, 100, 4, WHITE, 0);

    free(fb.px);
}

static void test_date(void) {
    clock_state_t c;
    char buf[40];

    clock_init(&c);
    clock_format_date(&c, buf, sizeof(buf));
    assert(strcmp(buf, " ") == 0);          /* no date pushed yet */

    clock_set_date(&c, 2026, 10, 29, 4);    /* Thu */
    clock_format_date(&c, buf, sizeof(buf));
    assert(strcmp(buf, "OCT 29, 2026 - THU") == 0);

    clock_set_date(&c, 2026, 1, 1, 0);      /* Sun, no zero padding */
    clock_format_date(&c, buf, sizeof(buf));
    assert(strcmp(buf, "JAN 1, 2026 - SUN") == 0);

    clock_set_date(&c, 2026, 13, 40, 9);    /* clamped */
    assert(c.month == 12 && c.day == 31 && c.weekday == 6);

    /* The date line draws inside its box and leaves the rest alone. */
    const uint32_t WHITE = RGBA(255, 255, 255, 255);
    const uint32_t SENTINEL = RGBA(255, 0, 255, 255);
    fb_t fb = make_fb(1280, 720);
    fb_clear(&fb, SENTINEL);

    clock_state_t d;
    clock_init(&d);
    clock_draw_date(&fb, &d, 0, 400, 1280, 112, WHITE, RGBA(0, 0, 0, 255));
    assert(fb.px[0] == SENTINEL);           /* invalid date: no paint */

    clock_set_date(&d, 2026, 10, 29, 4);
    clock_draw_date(&fb, &d, 0, 400, 1280, 112, WHITE, RGBA(0, 0, 0, 255));

    size_t inside = 0;
    for (int y = 400; y < 512; y++) {
        for (int x = 0; x < 1280; x++) {
            if (fb.px[(size_t)y * 1280 + x] != SENTINEL) inside++;
        }
    }
    assert(inside > 500);
    assert(fb.px[0] == SENTINEL);
    assert(fb.px[(size_t)700 * 1280 + 640] == SENTINEL);

    free(fb.px);
}

int main(void) {
    test_dirty_tracking();
    test_timeout();
    test_layout();
    test_date();
    test_draw();
    printf("test_clock: ok\n");
    return 0;
}
