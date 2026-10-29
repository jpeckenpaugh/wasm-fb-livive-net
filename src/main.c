/* Lifecycle and the exported guest API.
 *
 * Only this translation unit touches Emscripten. Everything else stays plain C
 * so `make test` can exercise it with the host compiler.
 */
#include <emscripten/emscripten.h>
#include <stddef.h>
#include <stdlib.h>

#include "clock.h"
#include "framebuffer.h"
#include "media.h"
#include "ui.h"

#define KEY_LEFT 1
#define KEY_RIGHT 2
#define KEY_NEXT 3

static fb_t g_fb;
static media_t g_media;
static clock_state_t g_clock;
static int g_ready = 0;
static int g_test_pattern = 0;

static void draw(void) {
    if (!g_ready) return;
    if (g_test_pattern) {
        ui_draw_test_pattern(&g_fb);
    } else {
        ui_draw(&g_fb, &g_media, &g_clock);
    }
}

EMSCRIPTEN_KEEPALIVE
int app_init(int width, int height) {
    if (width <= 0) width = UI_W;
    if (height <= 0) height = UI_H;

    free(g_fb.px);
    g_fb.px = NULL;
    g_fb.w = width;
    g_fb.h = height;

    g_fb.px = (uint32_t *)malloc((size_t)width * (size_t)height * 4u);
    if (!g_fb.px) {
        g_ready = 0;
        return 0;
    }

    g_test_pattern = 0;
    clock_init(&g_clock);
    media_init(&g_media);
    g_ready = 1;
    draw();
    return 1;
}

EMSCRIPTEN_KEEPALIVE
void app_render(void) {
    draw();
}

/* Returns 1 when the click changed the selection. */
EMSCRIPTEN_KEEPALIVE
int app_click(int x, int y) {
    if (!g_ready || g_test_pattern) return 0;
    int hit = media_hit(&g_media, x, y);
    if (hit < 0 || hit == g_media.selected) return 0;
    media_select(&g_media, hit);
    draw();
    return 1;
}

/* Returns 1 when the key changed the selection. */
EMSCRIPTEN_KEEPALIVE
int app_key(int key) {
    if (!g_ready || g_test_pattern) return 0;
    int before = g_media.selected;
    switch (key) {
        case KEY_LEFT:
            media_next(&g_media, -1);
            break;
        case KEY_RIGHT:
        case KEY_NEXT:
            media_next(&g_media, 1);
            break;
        default:
            return 0;
    }
    if (g_media.selected == before) return 0;
    draw();
    return 1;
}

EMSCRIPTEN_KEEPALIVE
uint8_t *app_framebuffer(void) {
    return (uint8_t *)g_fb.px;
}

EMSCRIPTEN_KEEPALIVE
int app_width(void) {
    return g_fb.w;
}

EMSCRIPTEN_KEEPALIVE
int app_height(void) {
    return g_fb.h;
}

EMSCRIPTEN_KEEPALIVE
int app_selected(void) {
    return g_media.selected;
}

EMSCRIPTEN_KEEPALIVE
int app_count(void) {
    return g_media.count;
}

/* --- clock ------------------------------------------------------------- */

EMSCRIPTEN_KEEPALIVE
void app_set_time(int hour, int minute, int second, int msec) {
    clock_set_time(&g_clock, hour, minute, second, msec);
}

/* Returns 1 when the framebuffer changed and needs copying to the canvas.
 * The hero clock shows seconds only while image 1 is selected, so the tick
 * granularity follows the selection. */
EMSCRIPTEN_KEEPALIVE
int app_tick(void) {
    if (!g_ready || g_test_pattern) return 0;
    if (!clock_advance(&g_clock, g_media.selected == 0)) return 0;
    draw();
    return 1;
}

EMSCRIPTEN_KEEPALIVE
unsigned int app_next_timeout_ms(void) {
    return clock_next_timeout_ms(&g_clock, g_media.selected == 0);
}

EMSCRIPTEN_KEEPALIVE
void app_set_test_pattern(int enabled) {
    g_test_pattern = enabled ? 1 : 0;
    draw();
}

EMSCRIPTEN_KEEPALIVE
void app_destroy(void) {
    free(g_fb.px);
    g_fb.px = NULL;
    g_fb.w = 0;
    g_fb.h = 0;
    g_test_pattern = 0;
    g_ready = 0;
}
