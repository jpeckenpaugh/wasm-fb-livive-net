#include "framebuffer.h"

#include <stddef.h>

static inline uint32_t rgba_from_bytes(const uint8_t *s) {
    return (uint32_t)s[3] << 24 | (uint32_t)s[2] << 16 | (uint32_t)s[1] << 8 | (uint32_t)s[0];
}

void fb_clear(fb_t *fb, uint32_t color) {
    if (!fb || !fb->px) return;
    size_t n = (size_t)fb->w * (size_t)fb->h;
    for (size_t i = 0; i < n; i++) fb->px[i] = color;
}

void fb_fill(fb_t *fb, int x, int y, int w, int h, uint32_t color) {
    if (!fb || !fb->px || w <= 0 || h <= 0) return;

    int x0 = x, y0 = y, x1 = x + w, y1 = y + h;
    if (x0 < 0) x0 = 0;
    if (y0 < 0) y0 = 0;
    if (x1 > fb->w) x1 = fb->w;
    if (y1 > fb->h) y1 = fb->h;
    if (x1 <= x0 || y1 <= y0) return;

    for (int yy = y0; yy < y1; yy++) {
        uint32_t *row = fb->px + (size_t)yy * (size_t)fb->w;
        for (int xx = x0; xx < x1; xx++) row[xx] = color;
    }
}

void fb_border(fb_t *fb, int x, int y, int w, int h, int thickness, uint32_t color) {
    if (thickness <= 0) return;
    fb_fill(fb, x, y, w, thickness, color);                       /* top */
    fb_fill(fb, x, y + h - thickness, w, thickness, color);        /* bottom */
    fb_fill(fb, x, y, thickness, h, color);                        /* left */
    fb_fill(fb, x + w - thickness, y, thickness, h, color);        /* right */
}

void fb_blit(fb_t *fb, int x, int y, const uint8_t *src, int sw, int sh) {
    if (!fb || !fb->px || !src || sw <= 0 || sh <= 0) return;

    for (int sy = 0; sy < sh; sy++) {
        int dy = y + sy;
        if (dy < 0 || dy >= fb->h) continue;
        uint32_t *row = fb->px + (size_t)dy * (size_t)fb->w;
        const uint8_t *srow = src + (size_t)sy * (size_t)sw * 4;
        for (int sx = 0; sx < sw; sx++) {
            int dx = x + sx;
            if (dx < 0 || dx >= fb->w) continue;
            row[dx] = rgba_from_bytes(srow + (size_t)sx * 4);
        }
    }
}

void fb_shade(fb_t *fb, int x, int y, int w, int h, int shade) {
    if (!fb || !fb->px || shade >= 255 || w <= 0 || h <= 0) return;
    if (shade < 0) shade = 0;

    int x0 = x, y0 = y, x1 = x + w, y1 = y + h;
    if (x0 < 0) x0 = 0;
    if (y0 < 0) y0 = 0;
    if (x1 > fb->w) x1 = fb->w;
    if (y1 > fb->h) y1 = fb->h;
    if (x1 <= x0 || y1 <= y0) return;

    for (int yy = y0; yy < y1; yy++) {
        uint32_t *row = fb->px + (size_t)yy * (size_t)fb->w;
        for (int xx = x0; xx < x1; xx++) {
            /* Memory order is R,G,B,A on little-endian wasm. */
            uint8_t *p = (uint8_t *)&row[xx];
            p[0] = (uint8_t)(p[0] * shade / 255);
            p[1] = (uint8_t)(p[1] * shade / 255);
            p[2] = (uint8_t)(p[2] * shade / 255);
        }
    }
}
void fb_blit_scaled(fb_t *fb, int x, int y, int dw, int dh,
                    const uint8_t *src, int sw, int sh) {
    if (!fb || !fb->px || !src || sw <= 0 || sh <= 0 || dw <= 0 || dh <= 0) return;

    for (int dyi = 0; dyi < dh; dyi++) {
        int dy = y + dyi;
        if (dy < 0 || dy >= fb->h) continue;
        int sy = (int)((long long)dyi * sh / dh);
        if (sy >= sh) sy = sh - 1;
        uint32_t *row = fb->px + (size_t)dy * (size_t)fb->w;
        const uint8_t *srow = src + (size_t)sy * (size_t)sw * 4;
        for (int dxi = 0; dxi < dw; dxi++) {
            int dx = x + dxi;
            if (dx < 0 || dx >= fb->w) continue;
            int sx = (int)((long long)dxi * sw / dw);
            if (sx >= sw) sx = sw - 1;
            row[dx] = rgba_from_bytes(srow + (size_t)sx * 4);
        }
    }
}
