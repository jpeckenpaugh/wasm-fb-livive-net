/* Image table + selection state. No Emscripten dependency: native-testable.
 *
 * The strip has one slot per bundled image. Slot MEDIA_CALC_SLOT is the
 * calculator screen: it reuses that image's artwork as its backdrop, so the
 * calculator is "merged" into the strip rather than added as a sixth tile.
 */
#ifndef MEDIA_H
#define MEDIA_H

#include "assets_gen.h"

#define MEDIA_CALC_SLOT 1 /* second tile, right after the clock */
#define MEDIA_CAL_SLOT 2  /* third tile: month calendar */

typedef struct {
    const image_t *images;
    const image_t *thumbs;
    int count;    /* bundled images == slots */
    int slots;
    int selected; /* 0..slots-1 */
} media_t;

void media_init(media_t *m);
void media_select(media_t *m, int index);
void media_next(media_t *m, int direction);

/* Returns the slot under (x, y) in framebuffer coordinates, or -1. */
int media_hit(const media_t *m, int x, int y);

static inline int media_is_calc(const media_t *m) {
    return m->selected == MEDIA_CALC_SLOT;
}

static inline int media_is_cal(const media_t *m) {
    return m->selected == MEDIA_CAL_SLOT;
}

#endif /* MEDIA_H */
