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
    int level;    /* 0 = root, 1 = Wallpapers, 2 = demo */
    int selected; /* row index at the current level */
    int scroll;   /* first visible row; only the first few rows fit */
} fm_state_t;

/* Returned by fm_click()/fm_key(). */
enum {
    FM_ACT_NONE = 0,
    FM_ACT_CHANGED,
    FM_ACT_IMAGE,
    FM_ACT_ABOUT,
    FM_ACT_DEMO
};

typedef struct {
    int kind;  /* FM_ACT_* */
    int index; /* image or demo index */
} fm_action_t;

void fm_init(fm_state_t *f);

fm_action_t fm_key(fm_state_t *f, int count, int key);
fm_action_t fm_click(fm_state_t *f, int count, int x, int y);

void fm_draw(fb_t *fb, const fm_state_t *f, int count);
void fm_draw_tile(fb_t *fb, int x, int y, int w, int h);

#endif /* FM_H */
