# Clock app — implementation spec (v2)

A second guest app: a seven-segment LCD clock drawn into the same C-owned RGBA
framebuffer, plus the host-side scheduling needed to make it tick on second
boundaries instead of on interaction.

This revision is the v1 spec corrected against the code that actually exists in
this repo. If you are implementing from v1, the changes are:

| v1 said | v2 says | Why |
|---|---|---|
| `fb_t.pixels`, `FB_WIDTH`, `FB_HEIGHT`, `FB_STRIDE` | `fb_t.px`, `fb->w`, `fb->h`, stride is `w * 4` | that is the real `framebuffer.h` |
| `fb_fill_rect(fb, x, y, w, h, c)` | `fb_fill(fb, x, y, w, h, c)` | the primitive is already called `fb_fill` |
| six `int*` out-params from `clock_layout` | one `clock_layout_t` | seven out-params were unreadable and duplicated the width math |
| `clock_draw` dirty check compared `second` always | compares `second` only when seconds are shown | with `CLOCK_SHOW_SECONDS=0` it repainted once a second instead of twice |
| `CLOCK_SHOW_SECONDS` defined in `clock.c` | defined in `clock.h` | tests and library must agree on digit count |
| — | `clock_invalidate()` | switching modes / leaving the diag pattern leaves stale pixels that the dirty check would skip |
| `app_tick(double now_ms)` with an ignored arg | `app_tick(void)` | it never used the argument |
| host replaced an "rAF loop" | host adds a timer; there is no rAF loop to replace | this host already renders on demand |
| `EXPORTED_FUNCTIONS` in the Makefile | nothing to change | exports are `EMSCRIPTEN_KEEPALIVE` |
| global `static fb_t fb;` in tests with `assert(fb.pixels)` | `make_fb()`/`free()` | the v1 tests dereference NULL and crash |
| one `test_*` binary | one binary per suite | each suite has its own `main()` |
| `colon_lit()` and the no-op `fb_fill_rect(...,0,0,0)` | deleted | dead code |

Everything else — segment tables, layout ratios, dirty-then-blit, next-boundary
timeouts, host-pushes-local-time — is unchanged and sound.

---

## 0. Verified assumptions

| Assumption | Verified against |
|---|---|
| `fb_t` is `{ uint32_t *px; int w; int h; }` | `src/framebuffer.h` |
| `fb_clear`, `fb_fill(fb,x,y,w,h,color)`, `fb_border` exist and clip | `src/framebuffer.c` |
| `RGBA(r,g,b,a)` builds the little-endian `0xAABBGGRR` word | `src/framebuffer.h` |
| The guest has no `app_tick`; the host renders on boot/click/key | `src/main.c`, `web/host.js` |
| The host keeps one `render()` per guest and tabs via `activate()` | `web/host.js` |
| There is no app dispatch: the guest is the gallery | `src/main.c` |
| Native tests are a single binary today | `Makefile` |
| Exports use `EMSCRIPTEN_KEEPALIVE`, no `EXPORTED_FUNCTIONS` list | `Makefile`, `src/main.c` |
| `clock.c`/`lcd.c` can stay Emscripten-free | they only include `framebuffer.h` |

---

## 1. `src/lcd.h` — new

```c
#ifndef LCD_H
#define LCD_H

#include <stdint.h>
#include "framebuffer.h"

/*
 * Seven-segment LCD digit renderer.
 *
 *     -- a --
 *    |       |
 *    f       b
 *    |       |
 *     -- g --
 *    |       |
 *    e       c
 *    |       |
 *     -- d --
 *
 * Digits are rectangles drawn through fb_fill(). "Off" segments are drawn in a
 * dim colour so the display reads as a physical LCD rather than floating
 * glyphs. Everything clips through fb_fill(), so a digit may hang off the edge.
 */

/* Draw one digit in the box (x, y, w, h). digit must be 0-9. */
void lcd_draw_digit(fb_t *fb, int x, int y, int w, int h, int digit,
                    uint32_t on_color, uint32_t off_color);

/* Draw a colon of width w, vertically centred in the box (x, y, w, h). */
void lcd_draw_colon(fb_t *fb, int x, int y, int w, int h, int lit,
                    uint32_t on_color, uint32_t off_color);

/* Segment bitmask for a digit, exposed for tests (bit0 = a ... bit6 = g). */
uint8_t lcd_segment_mask(int digit);

#endif /* LCD_H */
```

## 2. `src/lcd.c` — new

```c
#include "lcd.h"

enum {
    SEG_A = 1u << 0,
    SEG_B = 1u << 1,
    SEG_C = 1u << 2,
    SEG_D = 1u << 3,
    SEG_E = 1u << 4,
    SEG_F = 1u << 5,
    SEG_G = 1u << 6
};

static const uint8_t k_masks[10] = {
    0x3F, /* 0: a b c d e f   */
    0x06, /* 1: b c           */
    0x5B, /* 2: a b d e g     */
    0x4F, /* 3: a b c d g     */
    0x66, /* 4: b c f g       */
    0x6D, /* 5: a c d f g     */
    0x7D, /* 6: a c d e f g   */
    0x07, /* 7: a b c         */
    0x7F, /* 8: all           */
    0x6F  /* 9: a b c d f g   */
};

uint8_t lcd_segment_mask(int digit)
{
    if (digit < 0 || digit > 9) return 0;
    return k_masks[digit];
}

/* Segment thickness: ~1/6 of the width, never more than 1/6 of the height. */
static int seg_thickness(int w, int h)
{
    int t   = w / 6;
    int max = h / 6;
    if (max < 2) max = 2;
    if (t < 2) t = 2;
    if (t > max) t = max;
    return t;
}

void lcd_draw_digit(fb_t *fb, int x, int y, int w, int h, int digit,
                    uint32_t on_color, uint32_t off_color)
{
    if (!fb || !fb->px) return;
    if (digit < 0 || digit > 9) return;
    if (w < 8 || h < 16) return;

    const int t  = seg_thickness(w, h);
    int vh = (h - 3 * t) / 2;          /* vertical segment length */
    if (vh < 1) vh = 1;

    const uint8_t m = k_masks[digit];

    /* Horizontals: a (top), g (middle), d (bottom) */
    const int hx = x + t;
    const int hw = w - 2 * t;
    if (hw > 0) {
        fb_fill(fb, hx, y,          hw, t, (m & SEG_A) ? on_color : off_color);
        fb_fill(fb, hx, y + t + vh, hw, t, (m & SEG_G) ? on_color : off_color);
        fb_fill(fb, hx, y + h - t,  hw, t, (m & SEG_D) ? on_color : off_color);
    }

    /* Verticals: f, b (upper); e, c (lower) */
    fb_fill(fb, x,         y + t,             t, vh, (m & SEG_F) ? on_color : off_color);
    fb_fill(fb, x + w - t, y + t,             t, vh, (m & SEG_B) ? on_color : off_color);
    fb_fill(fb, x,         y + 2 * t + vh,    t, vh, (m & SEG_E) ? on_color : off_color);
    fb_fill(fb, x + w - t, y + 2 * t + vh,    t, vh, (m & SEG_C) ? on_color : off_color);
}

void lcd_draw_colon(fb_t *fb, int x, int y, int w, int h, int lit,
                    uint32_t on_color, uint32_t off_color)
{
    if (!fb || !fb->px || w < 2 || h < 8) return;

    const uint32_t c = lit ? on_color : off_color;
    int s = w;                       /* square dot */
    if (s > h / 4) s = h / 4;
    if (s < 2) s = 2;
    const int cx = x + (w - s) / 2;
    fb_fill(fb, cx, y + h / 3 - s / 2,       s, s, c);
    fb_fill(fb, cx, y + (2 * h) / 3 - s / 2, s, s, c);
}
```

## 3. `src/clock.h` — new

```c
#ifndef CLOCK_H
#define CLOCK_H

#include <stdint.h>
#include "framebuffer.h"

/*
 * Clock app state. Pure C: no Emscripten, no wall-clock syscalls. The host
 * pushes broken-down local time in; the guest decides what to draw and how long
 * it can sleep.
 */

/* 1 = HH:MM:SS with a steady colon. 0 = HH:MM with a 1 Hz blinking colon.
 * Defined here, not in clock.c, so tests and the library agree on digit count. */
#ifndef CLOCK_SHOW_SECONDS
#define CLOCK_SHOW_SECONDS 1
#endif

typedef struct {
    /* time most recently pushed by the host */
    int hour, minute, second, msec;
    int time_valid;

    /* what is currently on screen */
    int drawn_hour, drawn_minute, drawn_second, drawn_colon;
    int has_drawn;
} clock_state_t;

typedef struct {
    int digit_w, digit_h;
    int gap, colon_w;
    int total_w;
    int x0, y0;
    int digits;   /* 4 or 6 */
    int pairs;    /* digits / 2 */
} clock_layout_t;

void clock_init(clock_state_t *c);

/* Force the next clock_draw() to repaint. Needed whenever something else has
 * drawn into the framebuffer (mode switch, test pattern, reuse). */
void clock_invalidate(clock_state_t *c);

/* Host -> guest. h=0..23, m=0..59, s=0..59, ms=0..999 (local time). */
void clock_set_time(clock_state_t *c, int h, int m, int s, int ms);

/* Draw if anything changed. Returns 1 if the framebuffer was touched. */
int clock_draw(clock_state_t *c, fb_t *fb);

/* How long the host may sleep before calling us again, in ms (1..1000). */
unsigned int clock_next_timeout_ms(const clock_state_t *c);

/* Layout for the given framebuffer, exposed for tests. */
void clock_layout(const fb_t *fb, clock_layout_t *out);

#endif /* CLOCK_H */
```

## 4. `src/clock.c` — new

```c
#include "clock.h"

#include "lcd.h"

#define CLK_BG    RGBA(14,  17,  22,  255)
#define CLK_ON    RGBA(232, 240, 214, 255)
#define CLK_OFF   RGBA(38,  44,  52,  255)
#define CLK_FRAME RGBA(58,  66,  78,  255)

void clock_init(clock_state_t *c)
{
    c->hour = c->minute = c->second = c->msec = 0;
    c->time_valid = 0;
    c->drawn_hour = c->drawn_minute = c->drawn_second = -1;
    c->drawn_colon = -1;
    c->has_drawn = 0;
}

void clock_invalidate(clock_state_t *c)
{
    c->has_drawn = 0;
}

void clock_set_time(clock_state_t *c, int h, int m, int s, int ms)
{
    if (h < 0) h = 0;   if (h > 23) h = 23;
    if (m < 0) m = 0;   if (m > 59) m = 59;
    if (s < 0) s = 0;   if (s > 59) s = 59;
    if (ms < 0) ms = 0; if (ms > 999) ms = 999;

    c->hour = h; c->minute = m; c->second = s; c->msec = ms;
    c->time_valid = 1;
}

void clock_layout(const fb_t *fb, clock_layout_t *out)
{
    const int width  = (fb && fb->w > 0) ? fb->w : 640;
    const int height = (fb && fb->h > 0) ? fb->h : 480;

    const int margin = 32;
    int avail = width - 2 * margin;
    if (avail < 64) avail = width;         /* tiny canvas: use all of it */

    const int digits = CLOCK_SHOW_SECONDS ? 6 : 4;
    const int pairs  = digits / 2;
    const int colons = pairs - 1;

    /* Widths in hundredths of a digit, so all of this stays integer math. */
    const int units = digits * 100 + pairs * 18 + colons * 50;

    int d = (avail * 100) / units;
    if (d < 8) d = 8;

    int g = (d * 18) / 100;
    if (g < 2) g = 2;
    int c = (d * 50) / 100;
    if (c < 4) c = 4;

    out->digits  = digits;
    out->pairs   = pairs;
    out->digit_w = d;
    out->digit_h = d * 2;
    out->gap     = g;
    out->colon_w = c;
    out->total_w = digits * d + pairs * g + colons * c;
    out->x0      = (width - out->total_w) / 2;
    out->y0      = (height - out->digit_h) / 2;
}

static int desired_colon(const clock_state_t *c)
{
#if CLOCK_SHOW_SECONDS
    (void)c;
    return 1;                    /* seconds are visible; colon stays solid */
#else
    return (c->second & 1) == 0; /* blink at 1 Hz */
#endif
}

int clock_draw(clock_state_t *c, fb_t *fb)
{
    if (!c->time_valid || !fb || !fb->px) return 0;

    const int colon = desired_colon(c);

    int unchanged = c->has_drawn && c->drawn_colon == colon;
#if CLOCK_SHOW_SECONDS
    /* Minutes are implied by the seconds comparison. */
    unchanged = unchanged && c->drawn_hour == c->hour &&
                c->drawn_minute == c->minute && c->drawn_second == c->second;
#else
    /* Seconds are not drawn, so they must not force a repaint. */
    unchanged = unchanged && c->drawn_hour == c->hour &&
                c->drawn_minute == c->minute;
#endif
    if (unchanged) return 0;

    clock_layout_t L;
    clock_layout(fb, &L);

    fb_clear(fb, CLK_BG);

    /* Thin frame, so the app reads as a distinct panel. */
    const int frame_x = L.x0 - 16;
    const int frame_w = L.total_w + 32;
    fb_fill(fb, frame_x, L.y0 - 16, frame_w, 2, CLK_FRAME);
    fb_fill(fb, frame_x, L.y0 + L.digit_h + 14, frame_w, 2, CLK_FRAME);

    int digits[6];
    int n = 0;
    digits[n++] = c->hour   / 10;
    digits[n++] = c->hour   % 10;
    digits[n++] = c->minute / 10;
    digits[n++] = c->minute % 10;
#if CLOCK_SHOW_SECONDS
    digits[n++] = c->second / 10;
    digits[n++] = c->second % 10;
#endif

    int cx = L.x0;
    for (int i = 0; i < n; i++) {
        lcd_draw_digit(fb, cx, L.y0, L.digit_w, L.digit_h, digits[i],
                       CLK_ON, CLK_OFF);
        cx += L.digit_w;
        if (i == n - 1) break;
        if (i % 2 == 1) {
            lcd_draw_colon(fb, cx, L.y0, L.colon_w, L.digit_h, colon,
                           CLK_ON, CLK_OFF);
            cx += L.colon_w;
        } else {
            cx += L.gap;
        }
    }

    c->drawn_hour   = c->hour;
    c->drawn_minute = c->minute;
    c->drawn_second = c->second;
    c->drawn_colon  = colon;
    c->has_drawn    = 1;
    return 1;
}

unsigned int clock_next_timeout_ms(const clock_state_t *c)
{
    if (!c->time_valid) return 1000;

    /* Wake exactly on the next boundary the display cares about. */
#if CLOCK_SHOW_SECONDS
    int w = 1000 - c->msec;             /* 1..1000 */
#else
    int w = 500 - (c->msec % 500);      /* 1..500 */
#endif
    if (w < 1) w = 1;
    return (unsigned int)w;
}
```

Geometry check at 640×480: `d = 76`, `g = 13`, `c = 38`, `total_w = 571`,
`x0 = 34`, `digit_h = 152`, `y0 = 164`, segment thickness `12`, vertical
segments `58` tall — every segment lands inside the box, and `total_w + 32` fits
inside the canvas.

## 5. `src/main.c` — additions

The guest is currently only the gallery, so the clock needs an explicit mode.
Do not invent an "app index" that collides with the selected image.

```c
#include "clock.h"

#define APP_MODE_GALLERY 1
#define APP_MODE_CLOCK   2

static clock_state_t g_clock;
static int g_mode = APP_MODE_GALLERY;

/* --- dispatch --------------------------------------------------------- */

static void draw(void)
{
    if (!g_ready) return;
    if (g_test_pattern) {
        ui_draw_test_pattern(&g_fb);
        return;
    }
    if (g_mode == APP_MODE_CLOCK) {
        clock_draw(&g_clock, &g_fb);
        return;
    }
    ui_draw(&g_fb, &g_media);
}

/* --- new exports ------------------------------------------------------ */

EMSCRIPTEN_KEEPALIVE
void app_set_mode(int mode)
{
    g_mode = (mode == APP_MODE_CLOCK) ? APP_MODE_CLOCK : APP_MODE_GALLERY;
    /* Whatever is on screen did not come from the clock's dirty tracking. */
    clock_invalidate(&g_clock);
    draw();
}

EMSCRIPTEN_KEEPALIVE
void app_set_time(int h, int m, int s, int ms)
{
    clock_set_time(&g_clock, h, m, s, ms);
}

/* Returns 1 when the framebuffer changed and needs copying to the canvas. */
EMSCRIPTEN_KEEPALIVE
int app_tick(void)
{
    if (!g_ready || g_mode != APP_MODE_CLOCK || g_test_pattern) return 0;
    return clock_draw(&g_clock, &g_fb);
}

EMSCRIPTEN_KEEPALIVE
unsigned int app_next_timeout_ms(void)
{
    return clock_next_timeout_ms(&g_clock);
}
```

Changes to existing functions:

```c
int app_init(int width, int height)
{
    ...
    g_test_pattern = 0;
    g_mode = APP_MODE_GALLERY;
    clock_init(&g_clock);
    media_init(&g_media);
    ...
}

int app_click(int x, int y)
{
    if (!g_ready || g_test_pattern || g_mode != APP_MODE_GALLERY) return 0;
    ...
}

int app_key(int key)
{
    if (!g_ready || g_test_pattern || g_mode != APP_MODE_GALLERY) return 0;
    ...
}

void app_set_test_pattern(int enabled)
{
    g_test_pattern = enabled ? 1 : 0;
    clock_invalidate(&g_clock);   /* the pattern is not the clock's output */
    draw();
}
```

Required call order from the host: `app_set_mode(CLOCK)` → `app_set_time(...)` →
`app_tick()`. `app_set_mode` alone cannot paint a clock because no time has been
pushed yet; `clock_draw` correctly refuses to draw garbage.

## 6. `web/host.js` — scheduler + tabs

There is no frame loop to replace. The gallery renders on boot/click/key; the
clock adds the first *timer-driven* render. Two rules:

1. Never call `render()` on a timer — call `_app_tick()` and blit only on `1`.
2. One timer for the active clock guest; stop it when the tab is destroyed.

```js
const MIN_WAIT_MS = 16;

let clockTimer = null;

function stopClock() {
  if (clockTimer !== null) { clearTimeout(clockTimer); clockTimer = null; }
}

function scheduleClock() {
  stopClock();
  const guest = guests.find((g) => g.id === activeId);
  if (!guest || !guest.module || guest.mode !== 'clock') return;
  const wait = Math.max(MIN_WAIT_MS, guest.module._app_next_timeout_ms());
  clockTimer = setTimeout(pumpClock, wait);
}

function pumpClock() {
  clockTimer = null;
  const guest = guests.find((g) => g.id === activeId);
  if (!guest || !guest.module || guest.mode !== 'clock') return;

  const now = new Date();
  guest.module._app_set_time(now.getHours(), now.getMinutes(),
                             now.getSeconds(), now.getMilliseconds());
  if (guest.module._app_tick()) guest.render();   // draw + copy + present
  scheduleClock();
}
```

Boot, inside `createGuest()` after `_app_init` and before the first `render()`:

```js
vm.mode = mode;                       // 'gallery' | 'clock'
if (vm.mode === 'clock') {
  vm.module._app_set_mode(2);         // APP_MODE_CLOCK
  pumpClock();                        // first paint + schedules the next tick
} else {
  render();
}
```

Wire the lifecycle:

```js
function activate(id) {
  activeId = id;
  /* ...existing tab/panel toggling... */
  pumpClock();        // catch up immediately; no-op when the guest is not a clock
}

function destroyGuest(guest) {
  if (guest.id === activeId) stopClock();
  /* ...existing teardown... */
  activate(next ? next.id : null);    // reschedules or stops
}

document.addEventListener('visibilitychange', () => {
  if (!document.hidden) pumpClock();  // background tabs are throttled; catch up
});
```

Notes:

- No drift accumulates: every pump re-reads `Date`, and the next wait is
  recomputed from the fresh milliseconds.
- Inactive tabs are not pumped at all; activating one repaints immediately.
- `guest.render()` is the existing closure (it already measures render/blit/
  present separately), so the telemetry row keeps working unchanged.
- Clearing the timer in `destroyGuest` is mandatory: destroying a tab while a
  `setTimeout` is pending would otherwise pump a freed module.
- Product decision still open: how the user asks for a clock guest. The minimal
  change is a second button (`+ clock`) and a `mode` argument to `createGuest()`;
  the tab label can read `vm-02 · clock`.

## 7. Tests

Both suites need a real framebuffer — the v1 versions declared a zeroed global
and dereferenced it. One binary per suite because each has its own `main()`.

### `tests/test_lcd.c`

```c
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include "framebuffer.h"
#include "lcd.h"

static fb_t make_fb(int w, int h)
{
    fb_t fb;
    fb.w = w;
    fb.h = h;
    fb.px = (uint32_t *)calloc((size_t)w * (size_t)h, 4);
    assert(fb.px != NULL);
    return fb;
}

/* Reassemble the RGBA word from memory order, which is what RGBA() produces. */
static uint32_t px(const fb_t *fb, int x, int y)
{
    const uint8_t *p = (const uint8_t *)fb->px + ((size_t)y * fb->w + x) * 4;
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) |
           ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static void test_masks(void)
{
    assert(lcd_segment_mask(0) == 0x3F);
    assert(lcd_segment_mask(1) == 0x06);
    assert(lcd_segment_mask(8) == 0x7F);
    assert(lcd_segment_mask(9) == 0x6F);
    assert(lcd_segment_mask(-1) == 0);
    assert(lcd_segment_mask(10) == 0);
}

static void test_eight_vs_one(void)
{
    const uint32_t ON  = RGBA(255, 255, 255, 255);
    const uint32_t OFF = RGBA(0, 0, 0, 255);
    fb_t fb = make_fb(160, 200);

    fb_clear(&fb, OFF);
    lcd_draw_digit(&fb, 10, 10, 60, 120, 8, ON, OFF);
    assert(px(&fb, 40, 13) == ON);          /* segment a lit for 8 */

    fb_clear(&fb, OFF);
    lcd_draw_digit(&fb, 10, 10, 60, 120, 1, ON, OFF);
    assert(px(&fb, 40, 13) == OFF);         /* a is not part of 1 */
    assert(px(&fb, 66, 40) == ON);          /* b is */

    free(fb.px);
}

static void test_clipping_is_safe(void)
{
    fb_t fb = make_fb(160, 200);
    fb_clear(&fb, RGBA(0, 0, 0, 255));

    /* Partly off-screen: must clip, not crash or scribble. */
    lcd_draw_digit(&fb, fb.w - 20, fb.h - 20, 60, 120, 8,
                   RGBA(255, 255, 255, 255), RGBA(0, 0, 0, 255));
    assert(px(&fb, 0, 0) == RGBA(0, 0, 0, 255));

    /* NULL framebuffer is tolerated. */
    lcd_draw_digit(NULL, 0, 0, 60, 120, 8,
                   RGBA(255, 255, 255, 255), RGBA(0, 0, 0, 255));

    free(fb.px);
}

int main(void)
{
    test_masks();
    test_eight_vs_one();
    test_clipping_is_safe();
    printf("test_lcd: ok\n");
    return 0;
}
```

### `tests/test_clock.c`

```c
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>

#include "clock.h"
#include "framebuffer.h"

static fb_t make_fb(int w, int h)
{
    fb_t fb;
    fb.w = w;
    fb.h = h;
    fb.px = (uint32_t *)calloc((size_t)w * (size_t)h, 4);
    assert(fb.px != NULL);
    return fb;
}

int main(void)
{
    fb_t fb = make_fb(640, 480);
    clock_state_t c;

    clock_init(&c);
    assert(clock_draw(&c, &fb) == 0);        /* no time yet: nothing drawn */

    clock_set_time(&c, 12, 34, 56, 0);
    assert(clock_draw(&c, &fb) == 1);        /* first paint */
    assert(clock_draw(&c, &fb) == 0);        /* same state: no touch */

    /* A new second repaints (with seconds hidden this flips the colon). */
    clock_set_time(&c, 12, 34, 57, 0);
    assert(clock_draw(&c, &fb) == 1);

    /* Midnight rollover is just another value pair. */
    clock_set_time(&c, 0, 0, 0, 0);
    assert(clock_draw(&c, &fb) == 1);

    /* invalidate() forces a repaint even when nothing changed. */
    assert(clock_draw(&c, &fb) == 0);
    clock_invalidate(&c);
    assert(clock_draw(&c, &fb) == 1);

    /* It really wrote pixels: the framebuffer is no longer the sentinel. */
    fb_clear(&fb, RGBA(255, 255, 255, 255));
    clock_invalidate(&c);
    assert(clock_draw(&c, &fb) == 1);
    assert(fb.px[0] != RGBA(255, 255, 255, 255));

    /* Timeout lands on the next boundary the display cares about. */
#if CLOCK_SHOW_SECONDS
    clock_set_time(&c, 1, 2, 3, 250);
    assert(clock_next_timeout_ms(&c) == 750);
    clock_set_time(&c, 1, 2, 3, 0);
    assert(clock_next_timeout_ms(&c) == 1000);
    clock_set_time(&c, 1, 2, 3, 999);
    assert(clock_next_timeout_ms(&c) == 1);
#else
    clock_set_time(&c, 1, 2, 3, 250);
    assert(clock_next_timeout_ms(&c) == 250);
    clock_set_time(&c, 1, 2, 3, 499);
    assert(clock_next_timeout_ms(&c) == 1);
#endif

    /* Invalid time asks the host to come back in a second. */
    clock_state_t fresh;
    clock_init(&fresh);
    assert(clock_next_timeout_ms(&fresh) == 1000);

    /* Layout must fit, at both the production size and a small canvas. */
    const int sizes[2][2] = { { 640, 480 }, { 320, 240 } };
    for (int i = 0; i < 2; i++) {
        fb.w = sizes[i][0];
        fb.h = sizes[i][1];
        clock_layout_t L;
        clock_layout(&fb, &L);
        assert(L.digits == (CLOCK_SHOW_SECONDS ? 6 : 4));
        assert(L.pairs == L.digits / 2);
        assert(L.x0 >= 0 && L.y0 >= 0);
        assert(L.x0 + L.total_w <= fb.w);
        assert(L.y0 + L.digit_h <= fb.h);
        assert(L.digit_w >= 8 && L.gap >= 2 && L.colon_w >= 4);
    }

    free(fb.px);
    printf("test_clock: ok\n");
    return 0;
}
```

## 8. `Makefile` — add the units and split the test binaries

```make
SRC := src/main.c src/framebuffer.c src/media.c src/ui.c src/lcd.c src/clock.c

NATIVE_COMMON := $(OBJ)/native/src/framebuffer.o
NATIVE_ASSETS := $(OBJ)/native/$(GEN)/assets_gen.o \
                 $(addprefix $(OBJ)/native/$(GEN)/,$(addsuffix .o,$(ASSET_UNITS)))

$(BUILD)/test_pixels: $(OBJ)/native/tests/test_pixels.o \
                      $(OBJ)/native/src/media.o $(OBJ)/native/src/ui.o \
                      $(NATIVE_ASSETS) $(NATIVE_COMMON)
	$(CC) $^ -o $@

$(BUILD)/test_lcd: $(OBJ)/native/tests/test_lcd.o $(OBJ)/native/src/lcd.o \
                   $(NATIVE_COMMON)
	$(CC) $^ -o $@

$(BUILD)/test_clock: $(OBJ)/native/tests/test_clock.o $(OBJ)/native/src/clock.o \
                     $(NATIVE_COMMON)
	$(CC) $^ -o $@

test: $(BUILD)/test_pixels $(BUILD)/test_lcd $(BUILD)/test_clock
	@for t in $^; do ./$$t; done
```

The existing `$(OBJ)/native/%.o: %.c` pattern rule covers `tests/*.c` and
`src/*.c` unchanged, so no new pattern rules are needed. `lcd.c` and `clock.c`
are Emscripten-free, so they compile for both targets with the same flags.

Nothing to add to `EXPORTED_FUNCTIONS`: this build exports through
`EMSCRIPTEN_KEEPALIVE`.

## 9. When you can test

```bash
docker compose --profile dev run --rm dev make -j4 all test
```

Expect `test_lcd: ok` and `test_clock: ok` alongside the existing
`ok: all guest tests passed`. That covers masks, segment geometry, clipping,
the dirty check, `invalidate()`, midnight rollover, timeout math, and layout fit
at two canvas sizes — no browser needed.

Then:

```bash
docker compose up --build serve
```

Open a clock guest and watch the telemetry row:

- `blits` should increase by **about 1 per second** (plus one at boot).
- `render` should stay at a fraction of a millisecond; `blit` ~0.1 ms;
  `present` ~0.1-0.2 ms.
- Switching to the gallery tab and back should repaint immediately with the
  current time, and the count should not jump by the number of seconds you were
  away.

There is no "before" measurement to compare against: this host never ran a
frame loop, so the clock introduces the timer rather than removing 60 Hz work.
The failure mode to look for is the opposite of v1's — if `blits` climbs at
~60/s, something is calling `render()` from a loop instead of honouring
`_app_tick()`.

## 10. Open questions

- Which guest kind is the default, and how does the user choose? A second
  `+ clock` button plus `mode` in `createGuest()` is the smallest change; a
  single "new guest" with a mode selector is nicer but needs UI work.
- Should the clock show a date line? It would need a tiny bitmap font or a
  second LCD row; out of scope for v1.
- Timezone/DST: the host already passes local time, so the guest inherits the
  browser's answer. Nothing to do.
- Sub-second display: `clock_init`/`clock_set_time` carry `msec`, so a
  milliseconds readout is a rendering change plus a ~16 ms timeout, not a
  plumbing change.
