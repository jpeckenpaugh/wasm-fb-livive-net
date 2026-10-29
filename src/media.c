#include "media.h"

#include "ui.h"

void media_init(media_t *m) {
    m->images = g_images;
    m->thumbs = g_thumbs;
    m->count = ASSET_COUNT;
    m->selected = 0;
}

void media_select(media_t *m, int index) {
    if (index < 0 || index >= m->count) return;
    m->selected = index;
}

void media_next(media_t *m, int direction) {
    if (m->count <= 0) return;
    int step = direction < 0 ? -1 : 1;
    m->selected = (m->selected + step + m->count) % m->count;
}

int media_hit(const media_t *m, int x, int y) {
    if (y < UI_TH_Y || y >= UI_TH_Y + UI_TH_H) return -1;
    for (int i = 0; i < m->count; i++) {
        int tx = ui_thumb_x(i);
        if (x >= tx && x < tx + UI_TH_W) return i;
    }
    return -1;
}
