#include "ui.h"

#define COL_BG RGB(0x18, 0x18, 0x1c)
#define COL_STRIP RGB(0x11, 0x11, 0x14)
#define COL_FRAME RGB(0x3a, 0x3a, 0x44)
#define COL_ACCENT RGB(0x4c, 0xc2, 0xff)
#define COL_SHADOW RGB(0x08, 0x08, 0x0a)
#define COL_CLOCK RGB(0xf2, 0xf8, 0xfa)
#define COL_CLOCK_EDGE RGB(0x08, 0x0c, 0x0e)

void ui_draw(fb_t *fb, const media_t *m, const clock_state_t *clock,
             const calc_state_t *calc, const cal_state_t *cal) {
    const int on_calc = media_is_calc(m);
    const int on_cal = media_is_cal(m);

    if (on_calc && calc) {
        /* The calculator is merged into its strip slot, so the slot's image is
         * its wallpaper: scale it over the screen area and dim it so the keys
         * stay readable. */
        const image_t *backdrop = &m->images[MEDIA_CALC_SLOT];
        fb_blit_scaled(fb, 0, 0, UI_W, UI_MAIN_H,
                       backdrop->px, backdrop->w, backdrop->h);
        fb_shade(fb, 0, 0, UI_W, UI_MAIN_H, 88);
        calc_draw(fb, calc);
    } else if (on_cal && cal) {
        const image_t *backdrop = &m->images[MEDIA_CAL_SLOT];
        fb_blit_scaled(fb, 0, 0, UI_W, UI_MAIN_H,
                       backdrop->px, backdrop->w, backdrop->h);
        fb_shade(fb, 0, 0, UI_W, UI_MAIN_H, 74);
        cal_draw(fb, cal);
    } else {
        fb_clear(fb, COL_BG);

        if (m->count > 0) {
            const image_t *hero = &m->images[m->selected];
            /* Assets are authored at the hero size, so this is a 1:1 blit with
             * no scaling and no letterboxing. */
            if (hero->w == UI_MAIN_W && hero->h == UI_MAIN_H) {
                fb_blit(fb, UI_MAIN_X, UI_MAIN_Y, hero->px, hero->w, hero->h);
            } else {
                fb_blit_scaled(fb, UI_MAIN_X, UI_MAIN_Y, UI_MAIN_W, UI_MAIN_H,
                               hero->px, hero->w, hero->h);
            }
        }
    }

    /* Hero clock: HH:MM:SS stacked over the date line, centred in the space
     * above the floating tiles. Drawn before the tiles so they sit on top. */
    if (clock && clock->valid && m->selected == 0) {
        const int time_h = 304;
        const int date_h = 112;
        const int gap = 24;
        const int content_bottom = UI_TH_Y - 28;
        const int top = (content_bottom - (time_h + gap + date_h)) / 2;

        clock_draw(fb, clock, UI_MAIN_X, top, UI_MAIN_W, time_h,
                   6, COL_CLOCK, COL_CLOCK_EDGE);
        clock_draw_date(fb, clock, UI_MAIN_X, top + time_h + gap, UI_MAIN_W, date_h,
                        COL_CLOCK, COL_CLOCK_EDGE);
    }

    for (int i = 0; i < m->slots; i++) {
        int tx = ui_thumb_x(i);
        int selected = (i == m->selected);

        /* Floating tiles: a dark plate behind the frame keeps them readable on
         * top of whatever the hero is showing. */
        const int plate = UI_BORDER + 4;
        fb_fill(fb, tx - plate, UI_TH_Y - plate,
                UI_TH_W + 2 * plate, UI_TH_H + 2 * plate, COL_SHADOW);

        fb_fill(fb, tx - UI_BORDER, UI_TH_Y - UI_BORDER,
                UI_TH_W + 2 * UI_BORDER, UI_TH_H + 2 * UI_BORDER,
                selected ? COL_ACCENT : COL_FRAME);

        fb_blit(fb, tx, UI_TH_Y, m->thumbs[i].px, m->thumbs[i].w, m->thumbs[i].h);

        /* The calculator and calendar preview their state on their tiles. */
        if (i == MEDIA_CALC_SLOT && calc) {
            calc_draw_tile(fb, calc, tx, UI_TH_Y, UI_TH_W, UI_TH_H);
        } else if (i == MEDIA_CAL_SLOT && cal) {
            cal_draw_tile(fb, cal, tx, UI_TH_Y, UI_TH_W, UI_TH_H);
        }

        if (selected) {
            /* Selection underline: makes the active slot obvious at a glance. */
            fb_fill(fb, tx, UI_TH_Y + UI_TH_H + UI_BORDER + 2, UI_TH_W, 3, COL_ACCENT);
        }
    }

    /* Compact HH:MM on the clock's tile, over its thumbnail. */
    if (clock && clock->valid) {
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
