/* Image table + selection state. No Emscripten dependency: native-testable. */
#ifndef MEDIA_H
#define MEDIA_H

#include "assets_gen.h"

typedef struct {
    const image_t *images;
    const image_t *thumbs;
    int count;
    int selected;
} media_t;

void media_init(media_t *m);
void media_select(media_t *m, int index);
void media_next(media_t *m, int direction);

/* Returns the thumbnail index under (x, y) in framebuffer coordinates, or -1. */
int media_hit(const media_t *m, int x, int y);

#endif /* MEDIA_H */
