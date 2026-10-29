#ifndef CLOCK_H
#define CLOCK_H

#include <stddef.h>
#include <stdint.h>

#include "framebuffer.h"

/*
 * Clock app state. Pure C: no Emscripten, no wall-clock syscalls. The host
 * pushes broken-down local time in; the guest decides what to draw and how long
 * it can sleep.
 *
 * The clock is an overlay on the gallery: HH:MM:SS over the hero image while
 * image 1 is selected, HH:MM over its thumbnail slot. See ui.c.
 */

typedef struct {
    /* time most recently pushed by the host */
    int hour, minute, second, msec;
    int valid;

    /* date most recently pushed by the host (1-based month, 0 = Sunday) */
    int year, month, day, weekday;
    int date_valid;

    /* what is currently considered drawn */
    int drawn_hour, drawn_minute, drawn_second;
    int has_drawn;
} clock_state_t;

typedef struct {
    int digit_w, digit_h;
    int gap, colon_w;
    int total_w;
    int x0, y0;
    int digits;
    int pairs;
} clock_layout_t;

void clock_init(clock_state_t *c);

/* Force the next clock_advance() to report a change. Needed whenever something
 * else has drawn over the clock (test pattern, mode switches, reuse). */
void clock_invalidate(clock_state_t *c);

/* Host -> guest. h=0..23, m=0..59, s=0..59, ms=0..999 (local time). */
void clock_set_time(clock_state_t *c, int h, int m, int s, int ms);

/* Host -> guest. month=1..12, weekday=0..6 with 0 = Sunday. */
void clock_set_date(clock_state_t *c, int year, int month, int day, int weekday);

/* "OCT 29, 2026 - THU" (day is not zero padded). */
void clock_format_date(const clock_state_t *c, char *out, size_t n);

/* Compare against what was last reported and update the drawn state.
 * with_seconds selects second-level or minute-level granularity.
 * Returns 1 when the visible content changed. */
int clock_advance(clock_state_t *c, int with_seconds);

/* Milliseconds until the next visible change (1..1000, or 1..60000). */
unsigned int clock_next_timeout_ms(const clock_state_t *c, int with_seconds);

/* Centred layout for HH:MM (digits=4) or HH:MM:SS (digits=6) in a box. */
void clock_layout(int box_w, int box_h, int digits, clock_layout_t *out);

/* Draw HH:MM[:SS] centred in the box, outlined so it stays readable over an
 * image. Lit segments only; does not touch the dirty state. */
void clock_draw(fb_t *fb, const clock_state_t *c, int x, int y, int w, int h,
                int digits, uint32_t on_color, uint32_t outline_color);

/* Draw the date line with the 14-segment alphanumeric renderer, scaled to fit
 * the box and outlined like the time. No-op until clock_set_date(). */
void clock_draw_date(fb_t *fb, const clock_state_t *c, int x, int y, int w, int h,
                     uint32_t on_color, uint32_t outline_color);

#endif /* CLOCK_H */
