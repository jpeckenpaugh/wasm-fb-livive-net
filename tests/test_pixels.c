/* Native (non-wasm) tests for the host-testable half of the guest.
 * Run with `make test`; no Emscripten toolchain required beyond generated assets.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "assets_gen.h"
#include "framebuffer.h"
#include "media.h"
#include "ui.h"

static int failures = 0;

#define CHECK(cond)                                                          \
    do {                                                                     \
        if (!(cond)) {                                                       \
            fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond);  \
            failures++;                                                      \
        }                                                                    \
    } while (0)

/* Byte accessor: the whole point is that memory order is R,G,B,A. */
static const uint8_t *at(const fb_t *fb, int x, int y) {
    return (const uint8_t *)fb->px + ((size_t)y * (size_t)fb->w + (size_t)x) * 4;
}

static void check_pixel(const fb_t *fb, int x, int y, int r, int g, int b, int a,
                        const char *what) {
    const uint8_t *p = at(fb, x, y);
    if (p[0] != r || p[1] != g || p[2] != b || p[3] != a) {
        fprintf(stderr, "FAIL %s: pixel(%d,%d) = %u,%u,%u,%u want %d,%d,%d,%d\n",
                what, x, y, p[0], p[1], p[2], p[3], r, g, b, a);
        failures++;
    }
}

static fb_t make_fb(int w, int h) {
    fb_t fb;
    fb.w = w;
    fb.h = h;
    fb.px = (uint32_t *)calloc((size_t)w * (size_t)h, 4);
    return fb;
}

static void test_pixel_order(void) {
    fb_t fb = make_fb(8, 8);
    fb_clear(&fb, RGB(0x11, 0x22, 0x33));
    check_pixel(&fb, 0, 0, 0x11, 0x22, 0x33, 255, "clear");

    fb_fill(&fb, 2, 2, 2, 2, RGBA(0xaa, 0xbb, 0xcc, 0x40));
    check_pixel(&fb, 2, 2, 0xaa, 0xbb, 0xcc, 0x40, "fill");
    check_pixel(&fb, 1, 2, 0x11, 0x22, 0x33, 255, "fill bounds");
    free(fb.px);
}

static void test_clipping(void) {
    fb_t fb = make_fb(8, 8);
    fb_clear(&fb, RGB(0, 0, 0));

    /* Origin outside on the negative side: only the visible 2x2 corner paints. */
    fb_fill(&fb, -2, -2, 4, 4, RGB(0, 255, 0));
    check_pixel(&fb, 0, 0, 0, 255, 0, 255, "neg clip");
    check_pixel(&fb, 1, 1, 0, 255, 0, 255, "neg clip");
    check_pixel(&fb, 2, 2, 0, 0, 0, 255, "neg clip outside");

    /* Far edge: must not write past the buffer. */
    fb_fill(&fb, 6, 6, 100, 100, RGB(0, 0, 255));
    check_pixel(&fb, 7, 7, 0, 0, 255, 255, "far clip");

    /* Entirely offscreen: no crash. */
    fb_fill(&fb, -50, -50, 10, 10, RGB(255, 0, 0));
    fb_fill(&fb, 100, 100, 10, 10, RGB(255, 0, 0));

    fb_border(&fb, 0, 0, 8, 8, 1, RGB(255, 255, 0));
    check_pixel(&fb, 0, 0, 255, 255, 0, 255, "border tl");
    check_pixel(&fb, 7, 7, 255, 255, 0, 255, "border br");
    check_pixel(&fb, 4, 4, 0, 0, 0, 255, "border interior");

    free(fb.px);
}

static void test_blit(void) {
    /* 2x2 source, distinct corners, blitted with an offset and clipped. */
    const uint8_t src[2 * 2 * 4] = {
        255, 0, 0, 255, 0, 255, 0, 255,
        0, 0, 255, 255, 255, 255, 255, 255,
    };
    fb_t fb = make_fb(4, 4);
    fb_clear(&fb, RGB(0, 0, 0));
    fb_blit(&fb, 1, 1, src, 2, 2);
    check_pixel(&fb, 1, 1, 255, 0, 0, 255, "blit tl");
    check_pixel(&fb, 2, 1, 0, 255, 0, 255, "blit tr");
    check_pixel(&fb, 1, 2, 0, 0, 255, 255, "blit bl");
    check_pixel(&fb, 2, 2, 255, 255, 255, 255, "blit br");
    check_pixel(&fb, 0, 0, 0, 0, 0, 255, "blit untouched");

    /* Clipped on the left/top: still samples the correct source pixels. */
    fb_clear(&fb, RGB(0, 0, 0));
    fb_blit(&fb, -1, -1, src, 2, 2);
    check_pixel(&fb, 0, 0, 255, 255, 255, 255, "blit clip br of src");

    /* Nearest-neighbour upscale of the same 2x2 to 4x4. */
    fb_clear(&fb, RGB(0, 0, 0));
    fb_blit_scaled(&fb, 0, 0, 4, 4, src, 2, 2);
    check_pixel(&fb, 0, 0, 255, 0, 0, 255, "scale tl");
    check_pixel(&fb, 3, 0, 0, 255, 0, 255, "scale tr");
    check_pixel(&fb, 0, 3, 0, 0, 255, 255, "scale bl");
    check_pixel(&fb, 3, 3, 255, 255, 255, 255, "scale br");

    free(fb.px);
}

static void test_media(void) {
    media_t m;
    media_init(&m);
    CHECK(m.count == ASSET_COUNT);
    CHECK(m.selected == 0);

    CHECK(media_hit(&m, 5, 5) == -1);
    CHECK(media_hit(&m, 320, UI_TH_Y - 1) == -1);
    CHECK(media_hit(&m, 320, UI_TH_Y + UI_TH_H) == -1);

    int tx = ui_thumb_x(2);
    CHECK(media_hit(&m, tx, UI_TH_Y) == 2);
    CHECK(media_hit(&m, tx + UI_TH_W - 1, UI_TH_Y + UI_TH_H - 1) == 2);
    CHECK(media_hit(&m, tx + UI_TH_W, UI_TH_Y + 5) == -1); /* gap */

    media_select(&m, 4);
    CHECK(m.selected == 4);
    media_select(&m, 99);
    CHECK(m.selected == 4); /* out of range is ignored */
    media_next(&m, 1);
    CHECK(m.selected == 0); /* wraps */
    media_next(&m, -1);
    CHECK(m.selected == 4);

    /* The asset pipeline and the layout must agree: this is the contract that
     * makes the hero blit 1:1 and lets hit testing assume thumbnail size.
     * Cross-file checks, because ASSET_* == ASSET_* alone proves nothing. */
    CHECK(ASSET_MAIN_W == UI_MAIN_W);
    CHECK(ASSET_MAIN_H == UI_MAIN_H);
    CHECK(ASSET_THUMB_W == UI_TH_W);
    CHECK(ASSET_THUMB_H == UI_TH_H);

    CHECK(g_images[0].w == ASSET_MAIN_W && g_images[0].h == ASSET_MAIN_H);
    CHECK(g_thumbs[0].w == ASSET_THUMB_W && g_thumbs[0].h == ASSET_THUMB_H);
    CHECK(g_images[0].px != NULL && g_thumbs[0].px != NULL);
    /* Opaque assets only: a zero alpha would show up as an invisible image. */
    CHECK(g_images[0].px[3] == 255);
}

static void test_ui_draw(void) {
    fb_t fb = make_fb(UI_W, UI_H);
    media_t m;
    media_init(&m);

    ui_draw(&fb, &m, NULL);

    /* The hero image occupies its exact slot; compare every pixel against the
     * source. Stop at the first mismatch so a bad row is one failure, not 480. */
    int mismatch = -1;
    for (int y = 0; y < UI_MAIN_H && mismatch < 0; y++) {
        for (int x = 0; x < UI_MAIN_W; x++) {
            const uint8_t *s = g_images[m.selected].px +
                               ((size_t)y * UI_MAIN_W + (size_t)x) * 4;
            const uint8_t *d = at(&fb, UI_MAIN_X + x, UI_MAIN_Y + y);
            if (d[0] != s[0] || d[1] != s[1] || d[2] != s[2] || d[3] != s[3]) {
                mismatch = y * UI_MAIN_W + x;
                break;
            }
        }
    }
    if (mismatch >= 0) {
        fprintf(stderr, "FAIL ui_draw: hero pixel mismatch at %d,%d\n",
                mismatch % UI_MAIN_W, mismatch / UI_MAIN_W);
        failures++;
    }

    /* Every thumbnail placement must be inside the canvas. */
    for (int i = 0; i < ASSET_COUNT; i++) {
        CHECK(ui_thumb_x(i) >= 0);
        CHECK(ui_thumb_x(i) + UI_TH_W <= UI_W);
    }
    CHECK(UI_TH_Y + UI_TH_H + UI_BORDER + 2 + 3 <= UI_H);

    fb_clear(&fb, RGB(7, 7, 7));
    ui_draw_test_pattern(&fb);
    /* Points chosen away from the diagonal and the corner markers. */
    check_pixel(&fb, 20, 20, 0, 0, 0, 255, "pattern origin marker");
    check_pixel(&fb, 60, 60, 255, 0, 0, 255, "pattern red");
    check_pixel(&fb, 400, 60, 0, 255, 0, 255, "pattern green");
    check_pixel(&fb, 60, 300, 0, 0, 255, 255, "pattern blue");
    check_pixel(&fb, 500, 400, 255, 255, 255, 255, "pattern white");
    check_pixel(&fb, UI_W - 24, UI_H - 24, 255, 0, 255, 255, "pattern magenta");

    free(fb.px);
}

int main(void) {
    test_pixel_order();
    test_clipping();
    test_blit();
    test_media();
    test_ui_draw();

    if (failures) {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    puts("ok: all guest tests passed");
    return 0;
}
