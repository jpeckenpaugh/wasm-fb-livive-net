# Clock overlay — as built (v3)

A seven-segment LCD clock composited over the gallery, plus the host-side
timer that drives it. This replaces the v1/v2 "app mode" design: there is no
mode switch and no second guest kind, because the clock is an overlay on the
gallery that is already there.

Superseded by code in `src/lcd.c`, `src/clock.c`, `src/ui.c`, `src/main.c`,
`web/host.js` and `tests/test_{lcd,clock}.c`. This note records the design
decisions; the source is authoritative.

## Placement

| Where | Format | When |
|---|---|---|
| Hero image, centred | `HH:MM:SS` | only while image 1 is selected (the default at boot) |
| Thumbnail slot 1, centred | `HH:MM` | always |

Both are drawn in the guest, on top of the image, in `ui_draw()`:

```c
if (clock && clock->valid) {
    if (m->selected == 0) {
        clock_draw(fb, clock, UI_MAIN_X, UI_MAIN_Y, UI_MAIN_W, UI_MAIN_H,
                   6, COL_CLOCK, COL_CLOCK_EDGE);
    }
    clock_draw(fb, clock, ui_thumb_x(0), UI_TH_Y, UI_TH_W, UI_TH_H,
               4, COL_CLOCK, COL_CLOCK_EDGE);
}
```

`ui_draw()` takes a `const clock_state_t *`, so it stays a pure function of its
inputs and the native tests keep working with `NULL` (no overlay).

## Modules

- **`src/lcd.{h,c}`** — segment masks and rectangle drawing through `fb_fill`.
  `LCD_OFF_NONE` as `off_color` means "draw lit segments only", which is what
  makes the overlay possible: unlit segments leave the image visible instead of
  painting a panel. Joint geometry overlaps neighbouring segments by `t/2`
  (segment thickness / 2) so digits read as connected strokes rather than
  floating bars.
- **`src/clock.{h,c}`** — time state, dirty tracking, timeout computation,
  proportional layout for a box, and the outlined overlay renderer. No
  Emscripten, no wall-clock calls.
- **`src/main.c`** — four exports and the wiring:

  | Export | Purpose |
  |---|---|
  | `app_set_time(h,m,s,ms)` | host pushes local time |
  | `app_tick()` | advances dirty state; draws and returns 1 only when visible content changed |
  | `app_next_timeout_ms()` | milliseconds until the next visible change |
  | `app_count()` | image count (already existed; the UI no longer hardcodes 5) |

## Timing model

The guest decides both *whether* to repaint and *when* to be woken again:

```c
int app_tick(void) {
    if (!g_ready || g_test_pattern) return 0;
    if (!clock_advance(&g_clock, g_media.selected == 0)) return 0;
    draw();
    return 1;
}
```

- On image 1 the hero shows seconds, so granularity is seconds and
  `app_next_timeout_ms()` returns `1000 - msec`.
- On images 2-5 only the thumbnail clock is visible, so granularity drops to
  minutes: `60000 - (second * 1000 + msec)`.

That asymmetry is the point: the clock costs ~1 blit/s when seconds are on
screen and ~1 blit/minute otherwise. `clock_advance()` compares hour+minute
always and seconds only when asked, so an invisible second change does not
provoke a 1.17 MiB copy.

`clock_draw()` itself never touches dirty state; it is a pure renderer. That
keeps `app_tick()` and the drawing path independent, and lets a future overlay
render at a different cadence without lying to the scheduler.

## Overlay rendering

Digits are drawn as **lit segments only**, each haloed in near-black by drawing
the glyph eight times at ±`t` before the bright pass. That keeps the clock
readable on top of arbitrary imagery without a background plate, so the image
stays visible inside the digit counters.

Halo thickness is `digit_w / 24`, clamped to 1..3 px: 2 px on the 480x360 hero
(digit 57x114) and 1 px on the 96x72 thumbnail (digit 18x36).

Layout is proportional to the box, never to the canvas: `clock_layout(box_w,
box_h, digits, &L)` is called with the hero rectangle and with the thumbnail
rectangle, so the same renderer produces both sizes. A 6-digit request in a
4-digit-sized box falls back to 4 digits rather than overflowing.

## Host integration

One timer for the active guest. Inactive tabs are not pumped at all; switching
tabs catches up instantly.

```js
function pumpClock() {
  clockTimer = null;
  const guest = guests.find((g) => g.id === activeId);
  if (!guest || !guest.module || !guest.render) return;

  const now = new Date();
  guest.module._app_set_time(now.getHours(), now.getMinutes(),
                             now.getSeconds(), now.getMilliseconds());
  if (guest.module._app_tick()) guest.render();   // draw + copy + present
  scheduleClock();
}
```

Lifecycle rules:

- `activate()` calls `pumpClock()`, which repaints with the current time and
  restarts the timer for the newly visible guest.
- Clicking a thumbnail or pressing a key calls `scheduleClock()`, because
  selecting image 1 changes the granularity from minute to second.
- `destroyGuest()` calls `stopClock()` for the active guest before tearing the
  module down, so a pending `setTimeout` never touches a freed instance.
- `visibilitychange` pumps on return, since background tabs throttle timers.
- No drift accumulates: every pump re-reads `Date` and recomputes the wait from
  the fresh milliseconds.

## Tests

Three native binaries, all built and run by `make test`:

| Suite | Covers |
|---|---|
| `test_pixels` | framebuffer primitives, hit testing, gallery layout (unchanged) |
| `test_lcd` | segment masks, `8` vs `1`, `LCD_OFF_NONE` leaving pixels untouched, colon placement, clipping and degenerate inputs |
| `test_clock` | dirty tracking at both granularities, midnight rollover, `clock_invalidate`, timeout math, layout fit for hero and thumbnail boxes, overlay writes nothing outside its box, NULL tolerance |

## Verification

```bash
docker compose --profile dev run --rm dev make -j4 all test
docker compose up --build serve
```

Observed on the reference machine:

| State | Blits |
|---|---|
| image 1 selected (hero HH:MM:SS) | ~1/s |
| images 2-5 selected (thumbnail HH:MM) | ~1/min |
| gallery interaction | +1 per selection change |

Per-blit cost: render ~0.4-0.7 ms, blit ~0.1 ms, present ~0.1-0.2 ms, on top of
the existing 1.17 MiB copy. The clock repaints the whole gallery, not a dirty
rectangle, because a blit copies the full framebuffer anyway; sub-region
rendering would only matter if the guest exposed partial updates.

## Deliberate limits

- No date line: needs a font or a second LCD row; out of scope.
- No blinking colon: the seconds are already visible; a blink would repaint
  every 500 ms for no information gain.
- The hero clock exists only on image 1. That is a product decision, not a
  technical limit; removing the `m->selected == 0` guard makes it always on.
- Sub-second display: `clock_set_time` already carries milliseconds, so a
  millisecond readout is a rendering change plus a ~16 ms timeout.
- Local time is whatever the browser reports; DST and timezone are inherited,
  never computed in the guest.
