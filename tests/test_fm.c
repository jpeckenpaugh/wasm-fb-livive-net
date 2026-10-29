/* Native tests for the file manager: levels, selection, and opening files. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include "assets_gen.h"
#include "fm.h"

int main(void) {
    fm_state_t f;
    fm_init(&f);
    assert(f.level == 0 && f.selected == 0);

    /* Root has one row: the folder. Opening it goes inside. */
    assert(fm_key(&f, ASSET_COUNT, FM_KEY_OPEN) == FM_CHANGED);
    assert(f.level == 1);
    assert(f.selected == 0); /* ".." */

    /* Selection clamps at both ends. */
    assert(fm_key(&f, ASSET_COUNT, FM_KEY_UP) == FM_CHANGED);
    assert(f.selected == 0);
    for (int i = 0; i < 10; i++) fm_key(&f, ASSET_COUNT, FM_KEY_DOWN);
    assert(f.selected == ASSET_COUNT); /* last row: the last file */

    /* Opening the last file reports its index. */
    assert(fm_key(&f, ASSET_COUNT, FM_KEY_OPEN) == ASSET_COUNT - 1);

    /* Backspace goes up one level from anywhere inside. */
    assert(fm_key(&f, ASSET_COUNT, FM_KEY_BACK) == FM_CHANGED);
    assert(f.level == 0);
    assert(fm_key(&f, ASSET_COUNT, FM_KEY_BACK) == FM_NOOP);

    /* Clicks: row 0 is preselected, so one click opens the folder. */
    fm_init(&f);
    assert(fm_click(&f, ASSET_COUNT, 600, 160) == FM_CHANGED);
    assert(f.level == 1);
    assert(f.selected == 0); /* ".." */

    /* Row 2 is the second file: first click selects, second opens it. */
    const int row2_y = 116 + 2 * (72 + 8) + 36;
    assert(fm_click(&f, ASSET_COUNT, 600, row2_y) == FM_CHANGED);
    assert(f.selected == 2);
    assert(fm_click(&f, ASSET_COUNT, 600, row2_y) == 1); /* image index 1 */

    /* Nothing outside a row. */
    assert(fm_click(&f, ASSET_COUNT, 10, 700) == FM_NOOP);

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
