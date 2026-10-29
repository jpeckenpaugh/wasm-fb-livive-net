/* Lifecycle and the exported guest API.
 *
 * Only this translation unit touches Emscripten. Everything else stays plain C
 * so `make test` can exercise it with the host compiler.
 */
#include <emscripten/emscripten.h>
#include <stddef.h>
#include <stdlib.h>

#include "cal.h"
#include "calc.h"
#include "demo_gen.h"
#include "fm.h"
#include "preview.h"
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
static calc_state_t g_calc;
static cal_state_t g_cal;
static fm_state_t g_fm;
static preview_state_t g_preview;

static const ui_state_t g_ui = {
    &g_clock, &g_calc, &g_cal, &g_fm, &g_preview
};
static int g_ready = 0;
static int g_test_pattern = 0;

static void draw(void) {
    if (!g_ready) return;
    if (g_test_pattern) {
        ui_draw_test_pattern(&g_fb);
    } else {
        ui_draw(&g_fb, &g_media, &g_ui);
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
    calc_init(&g_calc);
    cal_init(&g_cal);
    fm_init(&g_fm);
    preview_init(&g_preview);
    media_init(&g_media);
    g_ready = 1;
    draw();
    return 1;
}

EMSCRIPTEN_KEEPALIVE
void app_render(void) {
    draw();
}

/* Returns 1 when the click changed something on screen. */
EMSCRIPTEN_KEEPALIVE
int app_click(int x, int y) {
    if (!g_ready || g_test_pattern) return 0;

    int hit = media_hit(&g_media, x, y);
    if (hit >= 0) {
        if (hit == g_media.selected) return 0;
        media_select(&g_media, hit);
        draw();
        return 1;
    }

    if (media_is_calc(&g_media) && calc_click(&g_calc, x, y)) {
        draw();
        return 1;
    }
    if (media_is_cal(&g_media) && cal_click(&g_cal, x, y)) {
        draw();
        return 1;
    }

    if (media_is_fm(&g_media)) {
        const fm_action_t a = fm_click(&g_fm, g_media.count, x, y);
        if (a.kind == FM_ACT_NONE) return 0;
        if (a.kind == FM_ACT_IMAGE) {
            preview_set_image(&g_preview, a.index, g_media.count);
            media_select(&g_media, MEDIA_PREVIEW_SLOT);
        } else if (a.kind == FM_ACT_ABOUT) {
            preview_set_doc(&g_preview);
            media_select(&g_media, MEDIA_PREVIEW_SLOT);
        } else if (a.kind == FM_ACT_DEMO) {
            preview_set_code(&g_preview, &k_demo_files[a.index]);
            media_select(&g_media, MEDIA_PREVIEW_SLOT);
        }
        draw();
        return 1;
    }
    return 0;
}

/* Returns 1 when the key changed something on screen. */
EMSCRIPTEN_KEEPALIVE
int app_key(int key) {
    if (!g_ready || g_test_pattern) return 0;

    /* In the code viewer the arrows scroll the text; at the edge they fall
     * through to slot navigation so the tiles stay reachable. */
    if (media_is_preview(&g_media) && (key == KEY_LEFT || key == KEY_RIGHT) &&
        preview_scroll(&g_preview, key)) {
        draw();
        return 1;
    }

    /* Slot navigation works from every screen: it is the same path that has
     * always swapped images, just with the calculator appended as a slot. */
    if (key == KEY_LEFT || key == KEY_RIGHT || key == KEY_NEXT) {
        int before = g_media.selected;
        media_next(&g_media, key == KEY_LEFT ? -1 : 1);
        if (g_media.selected == before) return 0;
        draw();
        return 1;
    }

    if (media_is_calc(&g_media) && calc_key(&g_calc, key)) {
        draw();
        return 1;
    }
    if (media_is_cal(&g_media) && cal_key(&g_cal, key)) {
        draw();
        return 1;
    }

    if (media_is_fm(&g_media)) {
        /* Enter shares the calculator's "=" code; only one screen is active. */
        const int fm_key_code = (key == 24) ? FM_KEY_OPEN : key;
        const fm_action_t a = fm_key(&g_fm, g_media.count, fm_key_code);
        if (a.kind == FM_ACT_NONE) return 0;
        if (a.kind == FM_ACT_IMAGE) {
            preview_set_image(&g_preview, a.index, g_media.count);
            media_select(&g_media, MEDIA_PREVIEW_SLOT);
        } else if (a.kind == FM_ACT_ABOUT) {
            preview_set_doc(&g_preview);
            media_select(&g_media, MEDIA_PREVIEW_SLOT);
        } else if (a.kind == FM_ACT_DEMO) {
            preview_set_code(&g_preview, &k_demo_files[a.index]);
            media_select(&g_media, MEDIA_PREVIEW_SLOT);
        }
        draw();
        return 1;
    }

    if (media_is_preview(&g_media) && preview_scroll(&g_preview, key)) {
        draw();
        return 1;
    }
    return 0;
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

EMSCRIPTEN_KEEPALIVE
void app_set_date(int year, int month, int day, int weekday) {
    clock_set_date(&g_clock, year, month, day, weekday);
    cal_set_today(&g_cal, year, month, day);
}

/* Returns 1 when the framebuffer changed and needs copying to the canvas.
 * The hero clock shows seconds only while image 1 is selected, so the tick
 * granularity follows the selection. */
EMSCRIPTEN_KEEPALIVE
int app_tick(void) {
    if (!g_ready || g_test_pattern) return 0;
    if (!clock_advance(&g_clock, media_is_clock(&g_media))) return 0;
    draw();
    return 1;
}

/* Drag-to-pan for the code viewer, in lines and columns. */
EMSCRIPTEN_KEEPALIVE
int app_scroll_by(int lines, int cols) {
    if (!g_ready || !media_is_preview(&g_media)) return 0;
    if (g_preview.kind != PREVIEW_CODE) return 0;

    int changed = 0;
    if (lines != 0) {
        int v = g_preview.line_off + lines;
        if (v < 0) v = 0;
        if (v > g_preview.max_line) v = g_preview.max_line;
        if (v != g_preview.line_off) {
            g_preview.line_off = v;
            changed = 1;
        }
    }
    if (cols != 0) {
        int v = g_preview.col_off + cols;
        if (v < 0) v = 0;
        if (v > g_preview.max_col) v = g_preview.max_col;
        if (v != g_preview.col_off) {
            g_preview.col_off = v;
            changed = 1;
        }
    }
    if (changed) draw();
    return changed;
}

EMSCRIPTEN_KEEPALIVE
unsigned int app_next_timeout_ms(void) {
    return clock_next_timeout_ms(&g_clock, media_is_clock(&g_media));
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
