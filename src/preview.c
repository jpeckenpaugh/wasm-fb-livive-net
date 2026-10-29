#include "preview.h"

#include <stdio.h>

#include "about_gen.h"
#include "code.h"
#include "lcd.h"
#include "markdown.h"

#define COL_PLATE RGB(0x08, 0x0c, 0x10)
#define COL_TEXT RGB(0xe8, 0xf2, 0xf8)
#define COL_DIM RGB(0x8b, 0x9b, 0xaa)
#define COL_ACCENT RGB(0x4c, 0xc2, 0xff)
#define COL_DOC_EDGE RGB(0x8f, 0xd8, 0xa8)
#define COL_CODE_EDGE RGB(0x4c, 0xc2, 0xff)

/* Panels stop above the floating tiles (they start 72px from the bottom). */
#define PREVIEW_BOTTOM_MARGIN 90

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
    p->kind = PREVIEW_NONE;
}

void preview_set_image(preview_state_t *p, int index, int count) {
    if (index < 0 || index >= count) return;
    p->index = index;
    p->kind = PREVIEW_IMAGE;
}

void preview_set_doc(preview_state_t *p) {
    p->kind = PREVIEW_DOC;
}

void preview_set_code(preview_state_t *p, const demo_file_t *file) {
    if (!file) return;
    p->kind = PREVIEW_CODE;
    p->file = file;
    p->line_off = 0;
    p->col_off = 0;
    p->max_line = 0;
    p->max_col = 0;
}

static void draw_caption(fb_t *fb, const char *title, const char *meta_text,
                         int pad, uint32_t edge);

int preview_scroll(preview_state_t *p, int key) {
    if (!p || p->kind != PREVIEW_CODE) return 0;

    int line = p->line_off;
    int col = p->col_off;

    switch (key) {
        case 1:  /* left  */ col -= 4; break;
        case 2:  /* right */ col += 4; break;
        case 40: /* [     */ col -= 4; break;
        case 41: /* ]     */ col += 4; break;
        case 50: /* up    */ line -= 1; break;
        case 51: /* down  */ line += 1; break;
        case 60: /* pgup  */ line -= 13; break;
        case 61: /* pgdn  */ line += 13; break;
        case 62: /* home  */ line = 0; break;
        case 63: /* end   */ line = p->max_line; break;
        default: return 0;
    }

    if (line < 0) line = 0;
    if (line > p->max_line) line = p->max_line;
    if (col < 0) col = 0;
    if (col > p->max_col) col = p->max_col;

    if (line == p->line_off && col == p->col_off) return 0; /* at the edge */
    p->line_off = line;
    p->col_off = col;
    return 1;
}

static void draw_code(fb_t *fb, preview_state_t *p) {
    fb_clear(fb, RGB(0x08, 0x0b, 0x0f));

    const int margin = 30;
    const int top = 108;
    const int panel_h = fb->h - top - PREVIEW_BOTTOM_MARGIN;
    fb_fill(fb, margin, top, fb->w - 2 * margin, panel_h, RGB(0x0c, 0x10, 0x15));
    fb_border(fb, margin, top, fb->w - 2 * margin, panel_h, 1,
              RGB(0x2a, 0x36, 0x44));

    const code_lang_t lang = code_lang_for(p->file->name);
    code_draw(fb, p->file->text, margin + 6, top + 6,
              fb->w - 2 * margin - 12, panel_h - 12, 4, lang,
              &p->line_off, &p->col_off, &p->max_line, &p->max_col);

    char meta[64];
    snprintf(meta, sizeof(meta), "%s  %d lines", code_lang_name(lang),
             code_line_count(p->file->text));
    draw_caption(fb, p->file->name, meta, 18, COL_CODE_EDGE);
}

static void draw_caption(fb_t *fb, const char *title, const char *meta_text,
                         int pad, uint32_t edge) {
    const int h = 74;
    fb_fill(fb, pad, pad, fb->w - 2 * pad, h, COL_PLATE);
    fb_border(fb, pad, pad, fb->w - 2 * pad, h, 2, edge);

    const int cell = 6;
    lcd_draw_text5x7(fb, title, pad + 18, pad + (h - 7 * cell) / 2, cell, COL_TEXT);

    if (meta_text && meta_text[0]) {
        const int mw = lcd_text5x7_width(meta_text, 4);
        lcd_draw_text5x7(fb, meta_text, fb->w - pad - 18 - mw,
                         pad + (h - 7 * 4) / 2, 4, COL_DIM);
    }
}

static void draw_doc(fb_t *fb) {
    /* A page on a dark desk, with the markdown rendered inside it. */
    fb_clear(fb, RGB(0x0c, 0x10, 0x15));

    const int margin_x = 90;
    const int top = 118;
    const int bottom = fb->h - PREVIEW_BOTTOM_MARGIN;
    fb_fill(fb, margin_x - 24, top - 24, fb->w - 2 * (margin_x - 24),
            bottom - top + 40, RGB(0x14, 0x1a, 0x22));
    fb_border(fb, margin_x - 24, top - 24, fb->w - 2 * (margin_x - 24),
              bottom - top + 40, 2, RGB(0x2a, 0x36, 0x44));

    md_draw(fb, k_about_md, margin_x, top, fb->w - 2 * margin_x,
            bottom - top, 5);
}

void preview_draw(fb_t *fb, preview_state_t *p, const image_t *images, int count) {
    if (!fb || !fb->px || !p) return;

    if (p->kind == PREVIEW_DOC) {
        draw_doc(fb);
        draw_caption(fb, "ABOUT.md", "markdown", 18, COL_DOC_EDGE);
        return;
    }

    if (p->kind == PREVIEW_CODE && p->file) {
        draw_code(fb, p);
        return;
    }

    if (p->kind != PREVIEW_IMAGE || p->index < 0 || p->index >= count) {
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

    char meta[48];
    snprintf(meta, sizeof(meta), "%d x %d  RGBA8888", img->w, img->h);
    draw_caption(fb, file_name(p->index), meta, 18, COL_ACCENT);
}
