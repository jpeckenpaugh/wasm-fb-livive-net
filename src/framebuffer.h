/* Minimal RGBA8888 software framebuffer.
 *
 * Byte order in memory is R,G,B,A (what Canvas ImageData expects). Since wasm
 * is little-endian, a uint32_t word is 0xAABBGGRR, which is what RGBA() builds.
 * Native-testable: this file must not include any Emscripten headers.
 */
#ifndef FRAMEBUFFER_H
#define FRAMEBUFFER_H

#include <stdint.h>

typedef struct {
    uint32_t *px;
    int w;
    int h;
} fb_t;

#define RGBA(r, g, b, a) \
    ((uint32_t)(a) << 24 | (uint32_t)(b) << 16 | (uint32_t)(g) << 8 | (uint32_t)(r))
#define RGB(r, g, b) RGBA(r, g, b, 255)

void fb_clear(fb_t *fb, uint32_t color);
void fb_fill(fb_t *fb, int x, int y, int w, int h, uint32_t color);
void fb_border(fb_t *fb, int x, int y, int w, int h, int thickness, uint32_t color);

/* src is tightly packed RGBA8888, sw * sh pixels. Clipped to the framebuffer. */
void fb_blit(fb_t *fb, int x, int y, const uint8_t *src, int sw, int sh);
void fb_blit_scaled(fb_t *fb, int x, int y, int dw, int dh,
                    const uint8_t *src, int sw, int sh);

#endif /* FRAMEBUFFER_H */
