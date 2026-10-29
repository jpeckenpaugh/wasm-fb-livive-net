/* Month calendar: 7x6 grid for the displayed month, today highlighted.
 *
 * Pure state + drawing, same shape as calc.c: the host pushes today's date,
 * the guest owns the browsing state and the pixels. Month/year navigation is
 * by key or by clicking the header chevrons.
 */
#ifndef CAL_H
#define CAL_H

#include <stddef.h>
#include <stdint.h>

#include "framebuffer.h"

/* Key codes: above the gallery (1..3) and calculator (10..27) ranges. */
#define CAL_KEY_PREV_MONTH 40
#define CAL_KEY_NEXT_MONTH 41
#define CAL_KEY_PREV_YEAR 42
#define CAL_KEY_NEXT_YEAR 43

#define CAL_SCREEN_H 720

typedef struct {
    int year, month;                 /* displayed month, 1-based */
    int today_year, today_month, today_day;
    int today_valid;
} cal_state_t;

void cal_init(cal_state_t *c);

/* Host -> guest. The first call selects today's month; later calls only move
 * the highlight, so browsing is not interrupted by the clock tick. */
void cal_set_today(cal_state_t *c, int year, int month, int day);

/* Apply a key. Returns 1 when the displayed month changed. */
int cal_key(cal_state_t *c, int key);

/* Hit-test the header chevrons. Returns 1 when the month changed. */
int cal_click(cal_state_t *c, int x, int y);

void cal_draw(fb_t *fb, const cal_state_t *c);

/* "October 2026", for tests. */
void cal_format_header(const cal_state_t *c, char *out, size_t n);

/* Tile preview: month abbreviation over the day-of-month. */
void cal_draw_tile(fb_t *fb, const cal_state_t *c, int x, int y, int w, int h);

#endif /* CAL_H */
