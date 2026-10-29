# Fonts

Four renderers, each in `src/lcd.c` except the 5x7 data (`src/font5x7.c`).
All of them draw only lit pixels when passed `LCD_OFF_NONE`, so they compose
over an image instead of painting a panel behind the text.

| Font | Segments/dots | Used by | Notes |
|---|---|---|---|
| 7-segment | 7 | clock time, calculator, calendar day numbers | digits only, boldest |
| 14-segment | 14 | clock date line (`OCT 29, 2026 - THU`) | uppercase A-Z, digits, `, . - space` |
| 16-segment | 16 | calendar tile month (`NOV`) | 14-segment + two full diagonals |
| 5x7 dot matrix | 35 | calendar header (`November 2026`), weekday scale, nav chevrons | the only font with lowercase |

## 7-segment

`lcd_draw_digit` / `lcd_draw_colon`. Horizontal bars span the full cell width
and the verticals overlap the joints by half a stroke, so digits read as
connected glyphs. Colon dots are two-thirds of the cell wide so they do not
crowd neighbouring digits at 720p.

## 14-segment

`lcd_draw_char14` / `lcd_14seg_mask`, a starburst:

```
    -- a --            a    top              h/i  upper diagonals (to centre)
   |\  |  /|           b    upper right      j/k  lower diagonals
   f h l i b           c    lower right      g1   middle left
   |  \|/  |           d    bottom           g2   middle right
    g1--g2             e    lower left       l    upper centre vertical
   |  /|\  |           f    upper left       m    lower centre vertical
   e j m k c
   |/  |  \|
    -- d --
```

Uppercase only: 14 segments cannot express x-height or descenders, so
lowercase is out of scope rather than badly approximated. Conventional
approximations: `O`/`0`/`D` share a shape, `I` is the left stem (so it never
collides with `1`), `V`/`Y` share the upper-V-plus-stem. No current string
contains both members of an ambiguous pair.

Stroke thickness is 1/5 of the narrow side; at 1/8 the glyphs read as an
outline font next to the bold 7-segment time.

## 16-segment

`lcd_draw_char16` / `lcd_16seg_mask`. Identical geometry plus two full-length
diagonals, `N` (top-left to bottom-right) and `O` (top-right to bottom-left),
which give `X`, `N` and `Z` a single continuous stroke instead of two halves
meeting at the centre. Every other character falls back to the 14-segment mask.

## 5x7 dot matrix

`lcd_draw_text5x7` / `lcd_text5x7_width`. Each glyph is 7 rows of 5 bits, drawn
as square dots with a 1px gap so it reads as a matrix. `cell` sets the dot size,
so the same data renders at 30px or 210px cap height. Covers A-Z, a-z, 0-9 and
`.,:-/()+=!?<>` plus space — the lowercase coverage is the reason it exists.
Descenders (`g j p q y`) use the last two rows.
