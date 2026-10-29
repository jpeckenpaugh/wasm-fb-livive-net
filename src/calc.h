/* Basic arithmetic calculator: immediate execution, decimal input.
 *
 * Pure state + drawing. No Emscripten, no heap, no globals: the whole
 * calculator is the calc_state_t the caller owns, so it is trivially testable
 * natively and can be embedded in any guest.
 */
#ifndef CALC_H
#define CALC_H

#include <stddef.h>
#include <stdint.h>

#include "framebuffer.h"

/* Key codes. These share app_key()/app_click() with the gallery, so they must
 * not collide with KEY_LEFT/RIGHT/NEXT (1..3) in main.c. */
#define CALC_KEY_0 10
#define CALC_KEY_DIGIT(d) (CALC_KEY_0 + (d))
#define CALC_KEY_PLUS 20
#define CALC_KEY_MINUS 21
#define CALC_KEY_MUL 22
#define CALC_KEY_DIV 23
#define CALC_KEY_EQ 24
#define CALC_KEY_CLEAR 25
#define CALC_KEY_BACK 26
#define CALC_KEY_DOT 27

/* The calculator paints 0..CALC_SCREEN_H; the thumbnail strip below stays
 * visible, so the top of UI_TH_Y - 12 in ui.h must match this. */
#define CALC_SCREEN_H 384

typedef struct {
    double acc;
    char pending;     /* '+', '-', '*', '/' or 0 */
    char entry[12];   /* digits typed since the last operator, NUL terminated */
    int entry_len;
    int error;        /* divide by zero / out of range */
} calc_state_t;

void calc_init(calc_state_t *c);

/* Apply a key. Returns 1 when the display changed. */
int calc_key(calc_state_t *c, int key);

/* Hit-test the on-screen keypad and apply the matching key. */
int calc_click(calc_state_t *c, int x, int y);

/* Draw the whole calculator screen over whatever is already there. */
void calc_draw(fb_t *fb, const calc_state_t *c);

/* Draw just the current value, scaled to fit a thumbnail-sized box, with a dark
 * halo so it reads on top of the image. Used by the strip preview. */
void calc_draw_tile(fb_t *fb, const calc_state_t *c, int x, int y, int w, int h);

/* Current readout as text ("0", "-2", "3.75", "E"), for tests and telemetry. */
void calc_display(const calc_state_t *c, char *out, size_t n);

#endif /* CALC_H */
