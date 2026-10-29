/* Layout constants and drawing for the media browser.
 *
 * The hero fills the whole area above the strip, edge to edge, and assets are
 * authored at exactly that size, so the blit is 1:1 with no scaling. The
 * calculator uses the same rectangle, so switching screens never moves the
 * frame. See tools/prepare_assets.py.
 */
#ifndef UI_H_INCLUDED
#define UI_H_INCLUDED

#include "calc.h"
#include "clock.h"
#include "framebuffer.h"
#include "media.h"

#define UI_W 1280
#define UI_H 720

#define UI_MAIN_W UI_W          /* full-bleed hero */
#define UI_MAIN_H 576
#define UI_MAIN_X 0
#define UI_MAIN_Y 0

#define UI_TH_W 144
#define UI_TH_H 108
#define UI_TH_GAP 18
#define UI_TH_PAD 18
#define UI_TH_Y (UI_MAIN_H + UI_TH_PAD) /* 594: thumbs under the hero */
#define UI_SLOTS ASSET_COUNT
#define UI_TH_X0 ((UI_W - (UI_SLOTS * UI_TH_W + (UI_SLOTS - 1) * UI_TH_GAP)) / 2)

#define UI_BORDER 3

static inline int ui_thumb_x(int index) {
    return UI_TH_X0 + index * (UI_TH_W + UI_TH_GAP);
}

void ui_draw(fb_t *fb, const media_t *m, const clock_state_t *clock,
             const calc_state_t *calc);
void ui_draw_test_pattern(fb_t *fb);

#endif /* UI_H_INCLUDED */
