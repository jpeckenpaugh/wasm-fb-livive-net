/* Layout constants and drawing for the media browser.
 *
 * The hero fills the whole area above the strip, edge to edge, and assets are
 * authored at exactly that size, so the blit is 1:1 with no scaling. The
 * calculator uses the same rectangle, so switching screens never moves the
 * frame. See tools/prepare_assets.py.
 */
#ifndef UI_H_INCLUDED
#define UI_H_INCLUDED

#include "cal.h"
#include "calc.h"
#include "clock.h"
#include "framebuffer.h"
#include "media.h"

#define UI_W 1280
#define UI_H 720

#define UI_MAIN_W UI_W          /* the hero is the whole canvas */
#define UI_MAIN_H UI_H
#define UI_MAIN_X 0
#define UI_MAIN_Y 0

/* Tiles float over the hero instead of living in a strip. */
#define UI_TH_W 128
#define UI_TH_H 72
#define UI_TH_GAP 16
#define UI_TH_BOTTOM 28
#define UI_TH_Y (UI_H - UI_TH_BOTTOM - UI_TH_H) /* 620 */
#define UI_SLOTS ASSET_COUNT
#define UI_TH_X0 ((UI_W - (UI_SLOTS * UI_TH_W + (UI_SLOTS - 1) * UI_TH_GAP)) / 2)

#define UI_BORDER 3

static inline int ui_thumb_x(int index) {
    return UI_TH_X0 + index * (UI_TH_W + UI_TH_GAP);
}

void ui_draw(fb_t *fb, const media_t *m, const clock_state_t *clock,
             const calc_state_t *calc, const cal_state_t *cal);
void ui_draw_test_pattern(fb_t *fb);

#endif /* UI_H_INCLUDED */
