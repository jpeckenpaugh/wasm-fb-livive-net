/* Image table + selection state. No Emscripten dependency: native-testable.
 *
 * The strip has one slot per bundled image. Slot MEDIA_CALC_SLOT is the
 * calculator screen: it reuses that image's artwork as its backdrop, so the
 * calculator is "merged" into the strip rather than added as a sixth tile.
 */
#ifndef MEDIA_H
#define MEDIA_H

#include "assets_gen.h"

/* Tile order: calendar, clock, files, preview, calculator. */
#define MEDIA_CAL_SLOT 0
#define MEDIA_CLOCK_SLOT 1
#define MEDIA_FM_SLOT 2
#define MEDIA_PREVIEW_SLOT 3
#define MEDIA_CALC_SLOT 4

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

static inline int media_is_clock(const media_t *m) {
    return m->selected == MEDIA_CLOCK_SLOT;
}

static inline int media_is_cal(const media_t *m) {
    return m->selected == MEDIA_CAL_SLOT;
}

static inline int media_is_fm(const media_t *m) {
    return m->selected == MEDIA_FM_SLOT;
}

static inline int media_is_preview(const media_t *m) {
    return m->selected == MEDIA_PREVIEW_SLOT;
}

#endif /* MEDIA_H */
