#include "preview.h"

#include <stdio.h>

#include "lcd.h"

#define COL_PLATE RGB(0x08, 0x0c, 0x10)
#define COL_TEXT RGB(0xe8, 0xf2, 0xf8)
#define COL_DIM RGB(0x8b, 0x9b, 0xaa)
#define COL_ACCENT RGB(0x4c, 0xc2, 0xff)

static const char *k_names[] = {
    "01_aurora.png", "02_rings.png", "03_checker.png",
    "04_dunes.png", "05_orbits.png"
};

static const char *file_name(int i) {
    const int n = (int)(sizeof(k_names) / sizeof(k_names[0]));
    return (i >= 0 && i < n) ? k_names[i] : "image";
}

void preview_init(preview_state_t *p) {
    p->index = -1;
}

void preview_set(preview_state_t *p, int index, int count) {
    if (index < 0 || index >= count) return;
    p->index = index;
}

void preview_draw(fb_t *fb, const preview_state_t *p, const image_t *images, int count) {
    if (!fb || !fb->px || !p) return;

    if (p->index < 0 || p->index >= count) {
        fb_clear(fb, COL_PLATE);
        const int cell = 6;
        const char *msg = "no image selected";
        const int w = lcd_text5x7_width(msg, cell);
        lcd_draw_text5x7(fb, msg, (fb->w - w) / 2, (fb->h - 7 * cell) / 2,
                         cell, COL_DIM);
        return;
    }

    const image_t *img = &images[p->index];

    /* The hero is authored at canvas size, so this is a 1:1 blit. */
    if (img->w == fb->w && img->h == fb->h) {
        fb_blit(fb, 0, 0, img->px, img->w, img->h);
    } else {
        fb_blit_scaled(fb, 0, 0, fb->w, fb->h, img->px, img->w, img->h);
    }

    /* Caption plate: opaque, so it reads over any image. */
    const int pad = 18;
    const int h = 74;
    fb_fill(fb, pad, pad, fb->w - 2 * pad, h, COL_PLATE);
    fb_border(fb, pad, pad, fb->w - 2 * pad, h, 2, COL_ACCENT);

    const int cell = 6;
    lcd_draw_text5x7(fb, file_name(p->index), pad + 18,
                     pad + (h - 7 * cell) / 2, cell, COL_TEXT);

    char meta[48];
    snprintf(meta, sizeof(meta), "%d x %d  RGBA8888", img->w, img->h);
    const int mw = lcd_text5x7_width(meta, 4);
    lcd_draw_text5x7(fb, meta, fb->w - pad - 18 - mw,
                     pad + (h - 7 * 4) / 2, 4, COL_DIM);
}
