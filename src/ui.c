#include "ui.h"

#define COL_BG RGB(0x18, 0x18, 0x1c)
#define COL_STRIP RGB(0x11, 0x11, 0x14)
#define COL_FRAME RGB(0x3a, 0x3a, 0x44)
#define COL_ACCENT RGB(0x4c, 0xc2, 0xff)
#define COL_SHADOW RGB(0x08, 0x08, 0x0a)
#define COL_CLOCK RGB(0xf2, 0xf8, 0xfa)
#define COL_CLOCK_EDGE RGB(0x08, 0x0c, 0x0e)

void ui_draw(fb_t *fb, const media_t *m, const clock_state_t *clock) {
    fb_clear(fb, COL_BG);

    /* Drop shadow + frame behind the main image. */
    fb_fill(fb, UI_MAIN_X - 4, UI_MAIN_Y - 4, UI_MAIN_W + 8, UI_MAIN_H + 8, COL_SHADOW);
    fb_fill(fb, UI_MAIN_X - UI_BORDER, UI_MAIN_Y - UI_BORDER,
            UI_MAIN_W + 2 * UI_BORDER, UI_MAIN_H + 2 * UI_BORDER, COL_FRAME);

    if (m->count > 0) {
        const image_t *hero = &m->images[m->selected];
        /* Blit 1:1 when the asset matches the layout; otherwise stretch into
         * the slot. Assets are authored at the exact size, so this is only a
         * fallback for a bad pipeline run. */
        if (hero->w == UI_MAIN_W && hero->h == UI_MAIN_H) {
            fb_blit(fb, UI_MAIN_X, UI_MAIN_Y, hero->px, hero->w, hero->h);
        } else {
            fb_blit_scaled(fb, UI_MAIN_X, UI_MAIN_Y, UI_MAIN_W, UI_MAIN_H,
                           hero->px, hero->w, hero->h);
        }
    }

    fb_fill(fb, 0, UI_TH_Y - 12, UI_W, UI_TH_H + 24, COL_STRIP);

    for (int i = 0; i < m->count; i++) {
        int tx = ui_thumb_x(i);
        int selected = (i == m->selected);

        fb_fill(fb, tx - UI_BORDER, UI_TH_Y - UI_BORDER,
                UI_TH_W + 2 * UI_BORDER, UI_TH_H + 2 * UI_BORDER,
                selected ? COL_ACCENT : COL_FRAME);
        fb_blit(fb, tx, UI_TH_Y, m->thumbs[i].px, m->thumbs[i].w, m->thumbs[i].h);

        if (selected) {
            /* Selection underline: makes the active image obvious at a glance. */
            fb_fill(fb, tx, UI_TH_Y + UI_TH_H + UI_BORDER + 2, UI_TH_W, 3, COL_ACCENT);
        }
    }

    /* Clock overlay: HH:MM:SS over the hero while image 1 is selected, and a
     * compact HH:MM on its thumbnail slot. Lit segments only, with a dark halo,
     * so it composites on top of the image instead of hiding it. */
    if (clock && clock->valid) {
        if (m->selected == 0) {
            clock_draw(fb, clock, UI_MAIN_X, UI_MAIN_Y, UI_MAIN_W, UI_MAIN_H,
                       6, COL_CLOCK, COL_CLOCK_EDGE);
        }
        clock_draw(fb, clock, ui_thumb_x(0), UI_TH_Y, UI_TH_W, UI_TH_H,
                   4, COL_CLOCK, COL_CLOCK_EDGE);
    }
}

/* Deliberately garish: proves channel order, orientation and clipping in one
 * screenshot. If red and blue are swapped here, everything downstream is wrong. */
void ui_draw_test_pattern(fb_t *fb) {
    int half_w = fb->w / 2;
    int half_h = fb->h / 2;

    fb_fill(fb, 0, 0, half_w, half_h, RGB(255, 0, 0));                  /* top-left  red   */
    fb_fill(fb, half_w, 0, fb->w - half_w, half_h, RGB(0, 255, 0));     /* top-right green */
    fb_fill(fb, 0, half_h, half_w, fb->h - half_h, RGB(0, 0, 255));     /* bottom-l red    */
    fb_fill(fb, half_w, half_h, fb->w - half_w, fb->h - half_h, RGB(255, 255, 255));

    /* Diagonal: top-left to bottom-right. Catches vertical flips. */
    for (int i = 0; i < fb->w; i++) {
        fb_fill(fb, i, i * fb->h / fb->w, 3, 3, RGB(255, 255, 0));
    }

    /* Orientation markers: black at origin, magenta at the opposite corner. */
    fb_fill(fb, 8, 8, 32, 32, RGB(0, 0, 0));
    fb_fill(fb, fb->w - 40, fb->h - 40, 32, 32, RGB(255, 0, 255));
}
