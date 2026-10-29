# Calculator screen — as built

The second mini app. It is **not** a separate guest kind and adds **no guest
exports**: the calculator is the sixth slot in the existing thumbnail strip, so
the same `app_click()` / `app_key()` path that swaps images also swaps screens.

Source: `src/calc.{h,c}` (state + paint), `src/lcd.c` (`lcd_draw_symbol`),
`src/media.c` (slot model), `src/ui.c` (composition), `tests/test_calc.c`.

## Entry and exit

The strip is now `ASSET_COUNT + 1` slots wide: five images, then the
calculator. Clicking the last tile or pressing <kbd>→</kbd> past image 5 selects
it; clicking any image or pressing <kbd>←</kbd> leaves it. `media_is_calc()`
is the only test anyone needs.

`app_selected()` now returns `0..5`, so the host's telemetry and status line
print `calculator` when `selected >= count`.

The clock keeps working across the switch: the hero overlay only draws when
image 1 is selected *and* we are not on the calculator, while the thumbnail
clock keeps updating at minute granularity.

## Arithmetic model

Immediate execution, like a pocket calculator — `2 + 3 × 4 =` is `20`. One
accumulator, one pending operator, one entry buffer. The entry is stored as the
**typed string** (`"12.5"`), not a float, so the display shows exactly what was
typed, and backspace is a character delete.

| Action | Behaviour |
|---|---|
| digit | append, max 10 characters |
| `.` | once per entry; a leading `.` becomes `0.` |
| operator | fold the entry into the accumulator, keep the operator |
| operator twice | replaces the pending operator (`6 + × 2 =` → `12`) |
| `=` | fold and clear the operator; the result becomes the accumulator |
| `=` with no entry | re-applies to the accumulator (`2 + =` → `4`) |
| backspace | delete the last character, no-op when empty |
| clear | reset everything |

Range is `|v| < 1e9` (nine on-screen digits). Anything larger, `NaN`, or
infinite becomes the error state, which any key clears and then applies.

Division is floating point, so `1/3` shows `0.333`. Formatting tries 3, then 2,
1 and 0 decimals and takes the first that fits nine characters, then trims
trailing zeros — `2.50` prints `2.5`, `3.00` prints `3`.

## Input

Both surfaces feed the same `calc_key()`:

| Keyboard | Key code | Effect |
|---|---|---|
| `0`-`9` | `CALC_KEY_DIGIT()` | digit |
| `.` or `,` | 27 | decimal point |
| `+ - * /` (`x` alias) | 20-23 | operator |
| `Enter` or `=` | 24 | equals |
| `Escape` or `c` | 25 | clear |
| `Backspace` | 26 | delete |

`app_key()` handles the arrow/space codes first (screen navigation always
works), then routes everything else to `calc_key()` when the calculator slot is
selected. `app_click()` tries the strip first, then the keypad.

Key codes live in `calc.h` and are deliberately above the gallery's 1-3, so the
two vocabularies cannot collide.

## Screen and symbols

The calculator paints `0..CALC_SCREEN_H` (384) and leaves the strip visible
below it, so there is always a way back.

Layout: readout panel at y 12..100, keypad of 17 buttons in a 4x5 grid below it
(`0` spans three columns, `=` spans two rows). Hit-testing uses the same button
table that draws them, so the two cannot drift.

`+ − × ÷ = C ⌫ E` are a 7x7 dot matrix (`lcd_draw_symbol`), because `×` needs
diagonals and `fb_fill` only draws axis-aligned rectangles. Cells are solid, so
adjacent dots merge into strokes. The same helper renders the `E` in the
readout and keeps the door open for any future non-digit label.

Readout characters are drawn by `calc.c`: digits via `lcd_draw_digit()` with
`LCD_OFF_NONE` (so no ghost segments), `.` as a baseline block, `-` as a bar,
`E` via the symbol font. Values are right-aligned from a precomputed width.

## Tests

`tests/test_calc.c` covers arithmetic, chaining, operator replacement, entry
editing and caps, decimal formatting, divide-by-zero and overflow, keypad
hit-testing (including the gaps and the display area), and that painting never
touches the strip. It runs as part of `make test` alongside `test_pixels`,
`test_lcd` and `test_clock`.

## Deliberate limits

- Immediate execution, no precedence, no parentheses, no memory keys.
- Nine on-screen characters; larger results become `E`.
- Three decimal places maximum; the value is shown rounded, not truncated.
- No history tape — the readout is one line.
- Repeated `=` does not repeat the last operation (v1 keeps it simple).
