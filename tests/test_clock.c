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

    clock_layout(480, 360, 6, &hero);
    assert(hero.digits == 6);
    assert(hero.pairs == 3);
    assert(hero.x0 >= 0 && hero.y0 >= 0);
    assert(hero.x0 + hero.total_w <= 480);
    assert(hero.y0 + hero.digit_h <= 360);
    assert(hero.digit_w >= 8 && hero.gap >= 2 && hero.colon_w >= 4);

    clock_layout(96, 72, 4, &thumb);
    assert(thumb.digits == 4);
    assert(thumb.pairs == 2);
    assert(thumb.x0 >= 0 && thumb.y0 >= 0);
    assert(thumb.x0 + thumb.total_w <= 96);
    assert(thumb.y0 + thumb.digit_h <= 72);

    /* Anything other than 6 falls back to 4 digits. */
    clock_layout_t odd;
    clock_layout(96, 72, 5, &odd);
    assert(odd.digits == 4);

    /* Degenerate boxes still produce a usable layout. */
    clock_layout_t tiny;
    clock_layout(1, 1, 6, &tiny);
    assert(tiny.digit_w >= 8);
}

static void test_draw(void) {
    const uint32_t WHITE = RGBA(255, 255, 255, 255);
    const uint32_t SENTINEL = RGBA(255, 0, 255, 255);

    fb_t fb = make_fb(640, 480);
    clock_state_t c;
    clock_init(&c);

    /* Invalid time draws nothing at all. */
    fb_clear(&fb, SENTINEL);
    clock_draw(&fb, &c, 80, 12, 480, 360, 6, WHITE, RGBA(0, 0, 0, 255));
    assert(fb.px[0] == SENTINEL);

    clock_set_time(&c, 12, 34, 56, 0);
    fb_clear(&fb, SENTINEL);
    clock_draw(&fb, &c, 80, 12, 480, 360, 6, WHITE, RGBA(0, 0, 0, 255));

    /* Pixels changed inside the hero box ... */
    size_t inside = 0;
    for (int y = 12; y < 12 + 360; y++) {
        for (int x = 80; x < 80 + 480; x++) {
            if (fb.px[(size_t)y * 640 + x] != SENTINEL) inside++;
        }
    }
    assert(inside > 1000);

    /* ... and nothing leaked outside it. */
    assert(fb.px[0] == SENTINEL);
    assert(fb.px[(size_t)479 * 640 + 639] == SENTINEL);

    /* NULL inputs are tolerated. */
    clock_draw(NULL, &c, 0, 0, 100, 100, 4, WHITE, 0);
    clock_draw(&fb, NULL, 0, 0, 100, 100, 4, WHITE, 0);

    free(fb.px);
}

int main(void) {
    test_dirty_tracking();
    test_timeout();
    test_layout();
    test_draw();
    printf("test_clock: ok\n");
    return 0;
}
