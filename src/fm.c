#include "fm.h"

#include <stdio.h>
#include <string.h>

#include "assets_gen.h"
#include "code.h"
#include "demo_gen.h"
#include "lcd.h"

#define FM_X 40
#define FM_W 1200

#define HEADER_Y 24
#define HEADER_H 76

#define ROW_Y 116
#define ROW_H 72
#define ROW_GAP 8
/* Rows must stay clear of the floating tiles at the bottom. */
#define ROWS_VISIBLE 6
#define ROW_LIMIT (ROW_Y + ROWS_VISIBLE * (ROW_H + ROW_GAP))

#define COL_PANEL RGB(0x0b, 0x10, 0x15)
#define COL_ROW RGB(0x14, 0x1a, 0x22)
#define COL_ROW_SEL RGB(0x10, 0x33, 0x3d)
#define COL_LINE RGB(0x2a, 0x36, 0x44)
#define COL_TEXT RGB(0xe8, 0xf2, 0xf8)
#define COL_DIM RGB(0x6d, 0x7d, 0x8c)
#define COL_ACCENT RGB(0x4c, 0xc2, 0xff)
#define COL_FOLDER RGB(0xd8, 0xb0, 0x6a)

/* Display names for the baked-in wallpapers; index matches g_images[]. */
static const char *k_names[] = {
    "01_aurora.png", "02_rings.png", "03_checker.png",
    "04_dunes.png", "05_orbits.png"
};

static const char *file_name(int i) {
    const int n = (int)(sizeof(k_names) / sizeof(k_names[0]));
    return (i >= 0 && i < n) ? k_names[i] : "image";
}

static fm_action_t act(int kind, int index) {
    fm_action_t a = { kind, index };
    return a;
}

static int row_count(const fm_state_t *f, int count) {
    (void)count;
    if (f->level == 0) return 3;      /* Wallpapers, demo, ABOUT.md */
    if (f->level == 2) return DEMO_FILE_COUNT + 1;
    return count + 1;                 /* ".." plus the wallpapers */
}

void fm_init(fm_state_t *f) {
    f->level = 0;
    f->selected = 0;
    f->scroll = 0;
}

static void clamp_selection(fm_state_t *f, int count) {
    const int rows = row_count(f, count);
    if (f->selected < 0) f->selected = 0;
    if (f->selected >= rows) f->selected = rows - 1;

    /* Keep the selection in view; the demo folder has more rows than fit. */
    if (f->selected < f->scroll) f->scroll = f->selected;
    if (f->selected >= f->scroll + ROWS_VISIBLE) {
        f->scroll = f->selected - ROWS_VISIBLE + 1;
    }
    if (f->scroll > rows - ROWS_VISIBLE) f->scroll = rows - ROWS_VISIBLE;
    if (f->scroll < 0) f->scroll = 0;
}

static void reset_level(fm_state_t *f, int level) {
    f->level = level;
    f->selected = 0;
    f->scroll = 0;
}

static fm_action_t activate(fm_state_t *f) {
    if (f->level == 0) {
        if (f->selected == 2) return act(FM_ACT_ABOUT, 0);
        reset_level(f, (f->selected == 1) ? 2 : 1);
        return act(FM_ACT_CHANGED, 0);
    }
    if (f->selected == 0) { /* ".." */
        reset_level(f, 0);
        return act(FM_ACT_CHANGED, 0);
    }
    if (f->level == 2) return act(FM_ACT_DEMO, f->selected - 1);
    return act(FM_ACT_IMAGE, f->selected - 1);
}

fm_action_t fm_key(fm_state_t *f, int count, int key) {
    switch (key) {
        case FM_KEY_DOWN:
            f->selected += 1;
            clamp_selection(f, count);
            return act(FM_ACT_CHANGED, 0);
        case FM_KEY_UP:
            f->selected -= 1;
            clamp_selection(f, count);
            return act(FM_ACT_CHANGED, 0);
        case FM_KEY_OPEN:
            return activate(f);
        case FM_KEY_BACK:
            if (f->level == 0) return act(FM_ACT_NONE, 0);
            reset_level(f, 0);
            return act(FM_ACT_CHANGED, 0);
        default:
            return act(FM_ACT_NONE, 0);
    }
}

static void row_rect(int row, int *x, int *y, int *w, int *h) {
    *x = FM_X;
    *y = ROW_Y + row * (ROW_H + ROW_GAP);
    *w = FM_W;
    *h = ROW_H;
}

fm_action_t fm_click(fm_state_t *f, int count, int x, int y) {
    const int rows = row_count(f, count);
    for (int i = f->scroll; i < rows && i < f->scroll + ROWS_VISIBLE; i++) {
        int rx, ry, rw, rh;
        row_rect(i - f->scroll, &rx, &ry, &rw, &rh);
        if (x < rx || x >= rx + rw || y < ry || y >= ry + rh) continue;
        if (f->selected == i) return activate(f);
        f->selected = i;
        clamp_selection(f, count);
        return act(FM_ACT_CHANGED, 0);
    }
    return act(FM_ACT_NONE, 0);
}

/* ---------------------------------------------------------------- drawing -- */

static void folder_icon(fb_t *fb, int x, int y, int w, int h, uint32_t color) {
    const int tab_h = h / 4;
    fb_fill(fb, x, y + tab_h / 2, w / 3, tab_h, color);
    fb_fill(fb, x, y + tab_h, w, h - tab_h, color);
}

static void doc_icon(fb_t *fb, int x, int y, int w, int h, uint32_t color) {
    fb_fill(fb, x, y, w, h, color);
    const int fold = h / 4;
    fb_fill(fb, x + w - fold, y, fold, fold, RGB(0x1a, 0x22, 0x2c));
    for (int i = 0; i < 3; i++) {
        fb_fill(fb, x + 8, y + h / 3 + i * (h / 6), w - 16, 2, RGB(0x1a, 0x22, 0x2c));
    }
}

static void draw_row(fb_t *fb, const fm_state_t *f, int count, int row) {
    int x, y, w, h;
    row_rect(row, &x, &y, &w, &h);

    const int sel = (f->selected == row);
    fb_fill(fb, x, y, w, h, sel ? COL_ROW_SEL : COL_ROW);
    fb_border(fb, x, y, w, h, sel ? 2 : 1, sel ? COL_ACCENT : COL_LINE);

    /* Thumbnails are authored at 128x72, exactly the row height. */
    const int icon_x = x + 12;
    const int icon_y = y;
    const int text_x = x + 12 + 128 + 16;

    if (f->level == 0) {
        const int cell = 5;
        const int ty = y + (h - 7 * cell) / 2;

        if (row < 2) {
            folder_icon(fb, icon_x + 16, y + 14, 96, h - 28, COL_FOLDER);
            const char *label = (row == 0) ? "Wallpapers" : "demo";
            lcd_draw_text5x7(fb, label, text_x, ty, cell, COL_TEXT);

            char meta[24];
            if (row == 0) snprintf(meta, sizeof(meta), "%d items", count);
            else snprintf(meta, sizeof(meta), "%d files", DEMO_FILE_COUNT);
            const int mw = lcd_text5x7_width(meta, cell);
            lcd_draw_text5x7(fb, meta, x + w - mw - 24, ty, cell, COL_DIM);
        } else {
            doc_icon(fb, icon_x + 24, y + 12, 80, h - 24, COL_TEXT);
            lcd_draw_text5x7(fb, "ABOUT.md", text_x, ty, cell, COL_TEXT);

            const char *meta = "markdown";
            const int mw = lcd_text5x7_width(meta, cell);
            lcd_draw_text5x7(fb, meta, x + w - mw - 24, ty, cell, COL_DIM);
        }
        return;
    }

    if (row == 0) {
        const int cell = 5;
        lcd_draw_text5x7(fb, "..", text_x, y + (h - 7 * cell) / 2, cell, COL_DIM);
        const int label_cell = 4;
        lcd_draw_text5x7(fb, "back", text_x + lcd_text5x7_width(".. ", cell),
                         y + (h - 7 * label_cell) / 2, label_cell, COL_DIM);
        return;
    }

    if (f->level == 2) {
        const int index = row - 1;
        if (index < DEMO_FILE_COUNT) {
            const char *name = k_demo_files[index].name;
            doc_icon(fb, icon_x + 24, y + 12, 80, h - 24, COL_TEXT);
            lcd_draw_text5x7(fb, name, text_x, y + (h - 7 * 5) / 2, 5, COL_TEXT);

            const char *lang = code_lang_name(code_lang_for(name));
            const int mw = lcd_text5x7_width(lang, 5);
            lcd_draw_text5x7(fb, lang, x + w - mw - 24, y + (h - 7 * 5) / 2, 5,
                             COL_DIM);
        }
        return;
    }

    const int index = row - 1;
    if (index < count) {
        /* Thumbnail, then the file name and its dimensions. */
        fb_blit(fb, icon_x, icon_y, g_thumbs[index].px,
                g_thumbs[index].w, g_thumbs[index].h);

        const int cell = 5;
        const int ty = y + (h - 7 * cell) / 2;
        lcd_draw_text5x7(fb, file_name(index), text_x, ty, cell, COL_TEXT);

        char meta[32];
        snprintf(meta, sizeof(meta), "%d x %d  RGBA", g_images[index].w,
                 g_images[index].h);
        const int mw = lcd_text5x7_width(meta, 4);
        lcd_draw_text5x7(fb, meta, x + w - mw - 24, y + (h - 7 * 4) / 2,
                         4, COL_DIM);
    }
}

void fm_draw(fb_t *fb, const fm_state_t *f, int count) {
    if (!fb || !fb->px || !f) return;

    /* Header: breadcrumb + item count. */
    fb_fill(fb, FM_X, HEADER_Y, FM_W, HEADER_H, COL_PANEL);
    fb_border(fb, FM_X, HEADER_Y, FM_W, HEADER_H, 2, COL_LINE);

    const int cell = 6;
    const char *path = (f->level == 0) ? "/"
                     : (f->level == 2) ? "/ demo"
                                       : "/ Wallpapers";
    lcd_draw_text5x7(fb, path, FM_X + 18,
                     HEADER_Y + (HEADER_H - 7 * cell) / 2, cell, COL_TEXT);

    char meta[32];
    if (f->level == 0) {
        snprintf(meta, sizeof(meta), "3 items");
    } else if (f->level == 2) {
        snprintf(meta, sizeof(meta), "%d files", DEMO_FILE_COUNT);
    } else {
        snprintf(meta, sizeof(meta), "%d files", count);
    }
    const int mw = lcd_text5x7_width(meta, 4);
    lcd_draw_text5x7(fb, meta, FM_X + FM_W - mw - 18,
                     HEADER_Y + (HEADER_H - 7 * 4) / 2, 4, COL_DIM);

    const int rows = row_count(f, count);
    const int last = rows < f->scroll + ROWS_VISIBLE ? rows : f->scroll + ROWS_VISIBLE;
    for (int i = f->scroll; i < last; i++) draw_row(fb, f, count, i);

    if (rows > ROWS_VISIBLE) {
        const int track_x = FM_X + FM_W - 6;
        const int track_y = ROW_Y;
        const int track_h = ROWS_VISIBLE * (ROW_H + ROW_GAP) - ROW_GAP;
        fb_fill(fb, track_x, track_y, 3, track_h, COL_LINE);
        const int thumb_h = track_h * ROWS_VISIBLE / rows;
        const int thumb_y = track_y + (track_h - thumb_h) * f->scroll / (rows - ROWS_VISIBLE);
        fb_fill(fb, track_x, thumb_y, 3, thumb_h, COL_ACCENT);
    }
}

void fm_draw_tile(fb_t *fb, int x, int y, int w, int h) {
    if (!fb || !fb->px) return;
    /* Folder glyph over the tile's artwork. */
    const int iw = w * 47 / 100;
    const int ih = h * 55 / 100;
    const int ix = x + (w - iw) / 2;
    const int iy = y + (h - ih) / 2;
    folder_icon(fb, ix, iy, iw, ih, COL_FOLDER);
    fb_border(fb, ix, iy, iw, ih, 1, RGB(0x8a, 0x6c, 0x38));
}
