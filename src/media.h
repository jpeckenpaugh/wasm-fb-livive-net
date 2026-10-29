/* Image table + selection state. No Emscripten dependency: native-testable.
 *
 * The selectable strip has count+1 slots: the five images and, last, the
 * calculator screen. Selecting that slot is the app switch; there is no
 * separate API for it.
 */
#ifndef MEDIA_H
#define MEDIA_H

#include "assets_gen.h"

typedef struct {
    const image_t *images;
    const image_t *thumbs;
    int count;    /* bundled images */
    int slots;    /* count + 1 (calculator) */
    int selected; /* 0..slots-1 */
} media_t;

void media_init(media_t *m);
void media_select(media_t *m, int index);
void media_next(media_t *m, int direction);

/* Returns the slot under (x, y) in framebuffer coordinates, or -1. */
int media_hit(const media_t *m, int x, int y);

static inline int media_is_calc(const media_t *m) {
    return m->selected >= m->count;
}

#endif /* MEDIA_H */
