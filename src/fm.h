/* File manager: a minimal two-level browser for the bundled wallpapers.
 *
 * Root lists one folder, "Wallpapers"; inside it are the five baked-in images.
 * Opening a file reports its index so the guest can switch to the Preview
 * screen. No filesystem, no allocation: the "files" are the asset table.
 */
#ifndef FM_H
#define FM_H

#include <stdint.h>

#include "framebuffer.h"

/* Key codes, above the other screens' ranges. 26 is Backspace, reused here as
 * "up" because only one screen is active at a time. */
#define FM_KEY_UP 50
#define FM_KEY_DOWN 51
#define FM_KEY_OPEN 52
#define FM_KEY_BACK 26

#define FM_SCREEN_H 720

typedef struct {
    int level;    /* 0 = root, 1 = inside Wallpapers */
    int selected; /* row index at the current level */
} fm_state_t;

/* Returned by fm_click()/fm_key(). */
#define FM_NOOP (-1)
#define FM_CHANGED (-2)

void fm_init(fm_state_t *f);

/* Returns FM_NOOP, FM_CHANGED, or an image index to open in Preview. */
int fm_key(fm_state_t *f, int count, int key);
int fm_click(fm_state_t *f, int count, int x, int y);

void fm_draw(fb_t *fb, const fm_state_t *f, int count);
void fm_draw_tile(fb_t *fb, int x, int y, int w, int h);

#endif /* FM_H */
