/* Native tests for the file manager: levels, selection, and opening files. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include "assets_gen.h"
#include "demo_gen.h"
#include "fm.h"

static const int ROW_STEP = 72 + 8;

static int row_y(int row) { return 116 + row * ROW_STEP + 36; }

int main(void) {
    fm_state_t f;
    fm_init(&f);
    assert(f.level == 0 && f.selected == 0);

    /* Root: Wallpapers, demo, ABOUT.md. Row 0 is preselected, so one Enter
     * opens the folder. */
    assert(fm_key(&f, ASSET_COUNT, FM_KEY_OPEN).kind == FM_ACT_CHANGED);
    assert(f.level == 1);
    assert(f.selected == 0); /* ".." */

    /* Selection clamps at both ends. */
    assert(fm_key(&f, ASSET_COUNT, FM_KEY_UP).kind == FM_ACT_CHANGED);
    assert(f.selected == 0);
    for (int i = 0; i < 10; i++) fm_key(&f, ASSET_COUNT, FM_KEY_DOWN);
    assert(f.selected == ASSET_COUNT); /* last row: the last wallpaper */

    fm_action_t a = fm_key(&f, ASSET_COUNT, FM_KEY_OPEN);
    assert(a.kind == FM_ACT_IMAGE && a.index == ASSET_COUNT - 1);

    /* Backspace goes up one level from anywhere inside. */
    assert(fm_key(&f, ASSET_COUNT, FM_KEY_BACK).kind == FM_ACT_CHANGED);
    assert(f.level == 0);
    assert(fm_key(&f, ASSET_COUNT, FM_KEY_BACK).kind == FM_ACT_NONE);

    /* Root row 1 opens the demo folder: first click selects, second opens.
     * Inside, row 1 is the first sample: same select-then-open pattern. */
    fm_init(&f);
    assert(fm_click(&f, ASSET_COUNT, 600, row_y(1)).kind == FM_ACT_CHANGED);
    assert(f.level == 0 && f.selected == 1);
    assert(fm_click(&f, ASSET_COUNT, 600, row_y(1)).kind == FM_ACT_CHANGED);
    assert(f.level == 2);
    assert(fm_click(&f, ASSET_COUNT, 600, row_y(1)).kind == FM_ACT_CHANGED);
    fm_action_t demo = fm_click(&f, ASSET_COUNT, 600, row_y(1));
    assert(demo.kind == FM_ACT_DEMO && demo.index == 0);

    /* Root row 2 is ABOUT.md (again: select, then open). */
    fm_init(&f);
    assert(fm_click(&f, ASSET_COUNT, 600, row_y(2)).kind == FM_ACT_CHANGED);
    assert(fm_click(&f, ASSET_COUNT, 600, row_y(2)).kind == FM_ACT_ABOUT);

    /* Nothing outside a row. */
    assert(fm_click(&f, ASSET_COUNT, 10, 700).kind == FM_ACT_NONE);

    /* Painting stays inside the content area. */
    const uint32_t SENTINEL = RGBA(255, 0, 255, 255);
    fb_t fb;
    fb.w = 1280;
    fb.h = 720;
    fb.px = (uint32_t *)calloc((size_t)fb.w * fb.h, 4);
    assert(fb.px != NULL);
    fb_clear(&fb, SENTINEL);

    fm_state_t g;
    fm_init(&g);
    g.level = 2;
    fm_draw(&fb, &g, ASSET_COUNT);
    size_t changed = 0;
    for (int y = 0; y < 600; y++)
        for (int x = 0; x < 1280; x++)
            if (fb.px[(size_t)y * 1280 + x] != SENTINEL) changed++;
    assert(changed > 1000);
    assert(fb.px[(size_t)700 * 1280 + 640] == SENTINEL);

    free(fb.px);
    printf("test_fm: ok\n");
    return 0;
}
