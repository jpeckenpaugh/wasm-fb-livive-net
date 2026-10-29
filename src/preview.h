/* Preview: full-bleed view of one bundled image with a caption.
 *
 * The guest owns the image bytes, so "preview" is just a 1:1 blit plus an
 * opaque caption plate; there is no decoding and no file I/O.
 */
#ifndef PREVIEW_H
#define PREVIEW_H

#include "assets_gen.h"
#include "framebuffer.h"

typedef struct {
    int index; /* image index, -1 when nothing has been opened yet */
} preview_state_t;

void preview_init(preview_state_t *p);
void preview_set(preview_state_t *p, int index, int count);
void preview_draw(fb_t *fb, const preview_state_t *p, const image_t *images, int count);

#endif /* PREVIEW_H */
