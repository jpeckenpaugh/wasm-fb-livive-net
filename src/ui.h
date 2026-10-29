/* Layout constants and drawing for the media browser.
 *
 * The geometry is chosen so no scaling is ever needed for the main image:
 * it is authored at exactly UI_MAIN_W x UI_MAIN_H, thumbnails are pre-scaled
 * by the asset pipeline. See tools/prepare_assets.py.
 */
#ifndef UI_H_INCLUDED
#define UI_H_INCLUDED

#include "clock.h"
#include "framebuffer.h"
#include "media.h"

#define UI_W 640
#define UI_H 480

#define UI_MAIN_W 480
#define UI_MAIN_H 360
#define UI_MAIN_X ((UI_W - UI_MAIN_W) / 2) /* 80 */
#define UI_MAIN_Y 12

#define UI_TH_W 96
#define UI_TH_H 72
#define UI_TH_GAP 10
#define UI_TH_Y 396
#define UI_TH_X0 ((UI_W - (ASSET_COUNT * UI_TH_W + (ASSET_COUNT - 1) * UI_TH_GAP)) / 2)

#define UI_BORDER 2

static inline int ui_thumb_x(int index) {
    return UI_TH_X0 + index * (UI_TH_W + UI_TH_GAP);
}

void ui_draw(fb_t *fb, const media_t *m, const clock_state_t *clock);
void ui_draw_test_pattern(fb_t *fb);

#endif /* UI_H_INCLUDED */
