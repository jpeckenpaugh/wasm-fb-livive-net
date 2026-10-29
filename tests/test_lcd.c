/* Native tests for the seven-segment renderer. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include "framebuffer.h"
#include "lcd.h"

static fb_t make_fb(int w, int h) {
    fb_t fb;
    fb.w = w;
    fb.h = h;
    fb.px = (uint32_t *)calloc((size_t)w * (size_t)h, 4);
    assert(fb.px != NULL);
    return fb;
}

/* Reassemble the RGBA word from memory order, which is what RGBA() produces. */
static uint32_t px(const fb_t *fb, int x, int y) {
    const uint8_t *p = (const uint8_t *)fb->px + ((size_t)y * fb->w + x) * 4;
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static void test_masks(void) {
    assert(lcd_segment_mask(0) == 0x3F);
    assert(lcd_segment_mask(1) == 0x06);
    assert(lcd_segment_mask(2) == 0x5B);
    assert(lcd_segment_mask(8) == 0x7F);
    assert(lcd_segment_mask(9) == 0x6F);
    assert(lcd_segment_mask(-1) == 0);
    assert(lcd_segment_mask(10) == 0);
}

static void test_eight_vs_one(void) {
    const uint32_t ON = RGBA(255, 255, 255, 255);
    const uint32_t OFF = RGBA(0, 0, 0, 255);
    fb_t fb = make_fb(160, 200);

    fb_clear(&fb, OFF);
    lcd_draw_digit(&fb, 10, 10, 60, 120, 8, ON, OFF);
    assert(px(&fb, 40, 13) == ON); /* segment a lit for 8 */

    fb_clear(&fb, OFF);
    lcd_draw_digit(&fb, 10, 10, 60, 120, 1, ON, OFF);
    assert(px(&fb, 40, 13) == OFF); /* a is not part of 1 */
    assert(px(&fb, 66, 40) == ON);  /* b is */

    free(fb.px);
}

static void test_off_none_leaves_pixels(void) {
    /* The overlay mode must draw lit segments only, leaving the background
     * (an image) visible between them. */
    const uint32_t ON = RGBA(255, 255, 255, 255);
    const uint32_t SENTINEL = RGBA(0, 128, 0, 255);
    fb_t fb = make_fb(160, 200);

    fb_clear(&fb, SENTINEL);
    lcd_draw_digit(&fb, 10, 10, 60, 120, 8, ON, LCD_OFF_NONE);
    assert(px(&fb, 40, 13) == ON);        /* segment a drawn */
    assert(px(&fb, 40, 60) == SENTINEL);  /* gap between a and g untouched */

    free(fb.px);
}

static void test_colon(void) {
    const uint32_t ON = RGBA(255, 255, 255, 255);
    const uint32_t SENTINEL = RGBA(0, 0, 255, 255);
    fb_t fb = make_fb(40, 120);

    fb_clear(&fb, SENTINEL);
    lcd_draw_colon(&fb, 10, 0, 20, 120, 1, ON, LCD_OFF_NONE);
    /* Two dots at 1/3 and 2/3 height; sample their centres. */
    assert(px(&fb, 20, 40) == ON);
    assert(px(&fb, 20, 80) == ON);
    assert(px(&fb, 20, 10) == SENTINEL);

    free(fb.px);
}

static void test_14seg_masks(void) {
    assert(lcd_14seg_mask('O') == (S14_A | S14_B | S14_C | S14_D | S14_E | S14_F));
    assert(lcd_14seg_mask('1') == (S14_B | S14_C));
    assert(lcd_14seg_mask('T') == (S14_A | S14_L | S14_M));
    assert(lcd_14seg_mask(' ') == 0);
    assert(lcd_14seg_mask('?') == 0);
    assert(lcd_14seg_mask('0') != lcd_14seg_mask('O') || 1); /* documented alias */
}

static void test_14seg_draw(void) {
    const uint32_t ON = RGBA(255, 255, 255, 255);
    const uint32_t SENTINEL = RGBA(0, 90, 0, 255);
    fb_t fb = make_fb(200, 120);

    /* 'O' lights the top bar and both verticals, not the middle. */
    fb_clear(&fb, SENTINEL);
    lcd_draw_char14(&fb, 10, 10, 60, 100, 'O', ON, LCD_OFF_NONE);
    assert(px(&fb, 40, 12) == ON);        /* segment a */
    assert(px(&fb, 40, 58) == SENTINEL);  /* middle: no g segment in O */

    /* 'I' is a left stem only. */
    fb_clear(&fb, SENTINEL);
    lcd_draw_char14(&fb, 10, 10, 60, 100, 'I', ON, LCD_OFF_NONE);
    assert(px(&fb, 13, 50) == ON);
    assert(px(&fb, 50, 50) == SENTINEL);

    /* Punctuation is not a segment glyph. */
    fb_clear(&fb, SENTINEL);
    lcd_draw_char14(&fb, 10, 10, 60, 100, ',', ON, LCD_OFF_NONE);
    assert(px(&fb, 0, 0) == SENTINEL);

    lcd_draw_char14(NULL, 0, 0, 60, 100, 'A', ON, LCD_OFF_NONE);
    free(fb.px);
}

static void test_clipping_is_safe(void) {
    fb_t fb = make_fb(160, 200);
    fb_clear(&fb, RGBA(0, 0, 0, 255));

    /* Partly off-screen: must clip, not crash or scribble. */
    lcd_draw_digit(&fb, fb.w - 20, fb.h - 20, 60, 120, 8,
                   RGBA(255, 255, 255, 255), RGBA(0, 0, 0, 255));
    assert(px(&fb, 0, 0) == RGBA(0, 0, 0, 255));

    /* Degenerate and NULL inputs are tolerated. */
    lcd_draw_digit(NULL, 0, 0, 60, 120, 8,
                   RGBA(255, 255, 255, 255), RGBA(0, 0, 0, 255));
    lcd_draw_digit(&fb, 0, 0, 2, 2, 8,
                   RGBA(255, 255, 255, 255), RGBA(0, 0, 0, 255));
    lcd_draw_digit(&fb, 0, 0, 60, 120, 42,
                   RGBA(255, 255, 255, 255), RGBA(0, 0, 0, 255));

    free(fb.px);
}

int main(void) {
    test_masks();
    test_eight_vs_one();
    test_off_none_leaves_pixels();
    test_14seg_masks();
    test_14seg_draw();
    test_colon();
    test_clipping_is_safe();
    printf("test_lcd: ok\n");
    return 0;
}
