//! Bitmap font engine (`font.rs`).
//!
//! Two families share the same Unicode and icon namespace:
//! - **FiveSeven**: 5x7, 1 bit per pixel.
//! - **TwelveSixteen**: 12x19 line (12x16 nominal cell), 2 bits per pixel
//!   (four coverage levels), derived from the FiveSeven shape with a 2x core
//!   and softened edges.
//!
//! `glyph::family()` selects the active default family. All public drawing and
//! width helpers use it; `*_for` variants render a specific family for galleries
//! and previews.
//!
//! Two spacing modes:
//! - **Monospace** ([`draw_str`], [`text_width`]): one cell plus a gap.
//! - **Proportional** ([`draw_str_prop`], [`text_width_prop`]): blank columns are
//!   trimmed and inline icons take a square cell.

use crate::fb::Framebuffer;
use crate::glyph::{self, Glyph, TextGlyph, MISSING_ROWS};
use crate::smooth;
use crate::theme;

pub use crate::glyph::FontFamily;

// FiveSeven reference constants, retained for layout compatibility.
pub const GLYPH_W: i32 = 5;
pub const GLYPH_H: i32 = 7;
pub const DESCENT: i32 = 2;
pub const GLYPH_COUNT: usize = glyph::FIVESEVEN_GLYPH_COUNT;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontStyle {
    Regular = 0,
    Bold = 1,
    Italic = 2,
    Matrix = 3,
    Underline = 4,
}

#[derive(Clone, Copy, Debug)]
pub struct FontMetrics {
    pub cell_w: i32,
    pub cell_h: i32,
    pub line_h: i32,
    pub descent: i32,
    pub inline_icon: i32,
}

const FIVE_METRICS: FontMetrics = FontMetrics {
    cell_w: 5,
    cell_h: 7,
    line_h: 10,
    descent: 2,
    inline_icon: 7,
};

const TWELVE_METRICS: FontMetrics = FontMetrics {
    cell_w: 12,
    cell_h: 19,
    line_h: 19,
    descent: 4,
    inline_icon: 16,
};

pub fn metrics_for(family: FontFamily) -> FontMetrics {
    match family {
        FontFamily::FiveSeven => FIVE_METRICS,
        FontFamily::TwelveSixteen => TWELVE_METRICS,
    }
}

pub fn metrics() -> FontMetrics {
    metrics_for(glyph::family())
}

pub fn family() -> FontFamily {
    glyph::family()
}

pub fn set_family(family: FontFamily) -> bool {
    glyph::set_family(family)
}

/// Map a logical FiveSeven-oriented scale to the active family. TwelveSixteen
/// is roughly a 2x face, so scale 2 maps to its native 12x19 line.
pub fn effective_scale_for(family: FontFamily, scale: i32) -> i32 {
    let s = scale.max(1);
    match family {
        FontFamily::FiveSeven => s,
        FontFamily::TwelveSixteen => ((s + 1) / 2).max(1),
    }
}

pub fn effective_scale(scale: i32) -> i32 {
    effective_scale_for(glyph::family(), scale)
}

pub fn glyph_height(scale: i32) -> i32 {
    match glyph::family() {
        FontFamily::FiveSeven => metrics().cell_h * effective_scale(scale),
        FontFamily::TwelveSixteen => smooth::cell_h(smooth::step_for_scale(effective_scale(scale))),
    }
}

pub fn line_height(scale: i32) -> i32 {
    match glyph::family() {
        FontFamily::FiveSeven => metrics().line_h * effective_scale(scale),
        FontFamily::TwelveSixteen => smooth::cell_h(smooth::step_for_scale(effective_scale(scale))),
    }
}

pub fn char_advance(scale: i32) -> i32 {
    let f = glyph::family();
    match f {
        FontFamily::FiveSeven => (metrics_for(f).cell_w + 1) * effective_scale_for(f, scale),
        FontFamily::TwelveSixteen => {
            smooth::cell_w(smooth::step_for_scale(effective_scale_for(f, scale))) + 1
        }
    }
}

pub fn inline_icon(scale: i32) -> i32 {
    match glyph::family() {
        FontFamily::FiveSeven => metrics().inline_icon * effective_scale(scale),
        FontFamily::TwelveSixteen => smooth::cell_w(smooth::step_for_scale(effective_scale(scale))),
    }
}

/// Native cell size (width, height) for a family at a native scale.
pub fn native_cell(family: FontFamily, scale: i32) -> (i32, i32) {
    let s = scale.max(1);
    match family {
        FontFamily::FiveSeven => (GLYPH_W * s, GLYPH_H * s),
        FontFamily::TwelveSixteen => {
            let step = smooth::step_for_scale(s);
            (smooth::cell_w(step), smooth::cell_h(step))
        }
    }
}

/// Extensible descriptor for a character block / locale font sheet.
pub struct FontSheet {
    pub id: &'static str,
    pub locale: &'static str,
    pub range_label: &'static str,
    pub first_code: u32,
    pub last_code: u32,
    pub family: FontFamily,
    pub lookup: fn(u32) -> Option<TextGlyph>,
}

pub const FIVESEVEN_US_LATIN: FontSheet = FontSheet {
    id: "FiveSeven",
    locale: "Multi-Script (Latin / Greek / Cyrillic / Symbols)",
    range_label: "532 Glyphs + 32 Icons (UTF-8)",
    first_code: 0x0020,
    last_code: 0x26FF,
    family: FontFamily::FiveSeven,
    lookup: lookup_us_latin,
};

pub const TWELVESIXTEEN_US_LATIN: FontSheet = FontSheet {
    id: "Smooth 12x16",
    locale: "Multi-Script (Latin / Greek / Cyrillic / Symbols)",
    range_label: "Runtime 2-bit Scale-up",
    first_code: 0x0020,
    last_code: 0x26FF,
    family: FontFamily::TwelveSixteen,
    lookup: lookup_twelve,
};

pub const FONT_REGISTRY: &[FontSheet] = &[FIVESEVEN_US_LATIN, TWELVESIXTEEN_US_LATIN];

/// Monospace advance per character cell.
pub fn mono_cells(cp: u32) -> i32 {
    match glyph::resolve(cp) {
        Glyph::Icon(_) => 2,
        _ => 1,
    }
}

/// Monospace string width in pixels (no trailing gap).
pub fn text_width(s: &str, scale: i32) -> i32 {
    let f = glyph::family();
    text_width_for(f, s, scale)
}

pub fn text_width_for(family: FontFamily, s: &str, scale: i32) -> i32 {
    let eff = effective_scale_for(family, scale);
    let cells: i32 = s
        .chars()
        .map(|c| match glyph::resolve_for(family, c as u32) {
            Glyph::Icon(_) => 2,
            _ => 1,
        })
        .sum();
    if cells <= 0 {
        0
    } else {
        match family {
            FontFamily::FiveSeven => cells * (metrics_for(family).cell_w + 1) * eff - eff,
            FontFamily::TwelveSixteen => {
                let step = smooth::step_for_scale(eff);
                cells * (smooth::cell_w(step) + 1) - 1
            }
        }
    }
}

fn blank_advance(family: FontFamily) -> i32 {
    match family {
        FontFamily::FiveSeven => 3,
        FontFamily::TwelveSixteen => 6,
    }
}

/// Proportional advance (including the 1-column gap) of a resolved glyph, in
/// source pixels at scale 1.
fn prop_advance_units(family: FontFamily, g: &Glyph) -> i32 {
    match g {
        Glyph::Text(t) if t.ink_w == 0 => blank_advance(family),
        Glyph::Text(t) => match family {
            FontFamily::FiveSeven => t.ink_w as i32 + 1,
            FontFamily::TwelveSixteen => smooth::ink_width(t.index, t.rows, t.dy, 0) as i32 + 1,
        },
        Glyph::Icon(_) => metrics_for(family).inline_icon + 1,
        Glyph::Missing => metrics_for(family).cell_w + 1,
    }
}

/// Proportional advance in pixels at the requested scale.
fn prop_advance_px(family: FontFamily, g: &Glyph, scale: i32) -> i32 {
    let eff = effective_scale_for(family, scale);
    match family {
        FontFamily::FiveSeven => prop_advance_units(family, g) * eff,
        FontFamily::TwelveSixteen => {
            let step = smooth::step_for_scale(eff);
            match g {
                Glyph::Text(t) if t.ink_w == 0 => blank_advance(family),
                Glyph::Text(t) => smooth::ink_width(t.index, t.rows, t.dy, step) as i32 + 1,
                Glyph::Icon(_) => smooth::cell_w(step) + 1,
                Glyph::Missing => smooth::cell_w(step) + 1,
            }
        }
    }
}

/// Proportional advance of a single character at `scale`.
pub fn char_advance_prop(ch: char, scale: i32) -> i32 {
    let f = glyph::family();
    prop_advance_px(f, &glyph::resolve(ch as u32), scale)
}

/// Proportional string width in pixels (no trailing gap).
pub fn text_width_prop(s: &str, scale: i32) -> i32 {
    let f = glyph::family();
    text_width_prop_for(f, s, scale)
}

pub fn text_width_prop_for(family: FontFamily, s: &str, scale: i32) -> i32 {
    let sum: i32 = s
        .chars()
        .map(|c| prop_advance_px(family, &glyph::resolve_for(family, c as u32), scale))
        .sum();
    if sum <= 0 {
        0
    } else {
        match family {
            FontFamily::FiveSeven => sum - effective_scale_for(family, scale),
            FontFamily::TwelveSixteen => sum - 1,
        }
    }
}

pub fn draw_str(fb: &mut Framebuffer, x: i32, y: i32, s: &str, scale: i32, color: u32) {
    draw_str_styled(fb, x, y, s, scale, FontStyle::Regular, color);
}

pub fn draw_str_family(
    fb: &mut Framebuffer,
    family: FontFamily,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    color: u32,
) {
    let eff = effective_scale_for(family, scale);
    draw_str_styled_impl(
        fb,
        family,
        x,
        y,
        s,
        eff,
        FontStyle::Regular,
        theme::resolve(color),
    );
}

/// Draw at the family's native scale (no FiveSeven-oriented normalization).
pub fn draw_str_native(
    fb: &mut Framebuffer,
    family: FontFamily,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    color: u32,
) {
    draw_str_styled_impl(
        fb,
        family,
        x,
        y,
        s,
        scale.max(1),
        FontStyle::Regular,
        theme::resolve(color),
    );
}

pub fn text_width_native(family: FontFamily, s: &str, scale: i32) -> i32 {
    let scale = scale.max(1);
    let cells: i32 = s
        .chars()
        .map(|c| match glyph::resolve_for(family, c as u32) {
            Glyph::Icon(_) => 2,
            _ => 1,
        })
        .sum();
    if cells <= 0 {
        0
    } else {
        match family {
            FontFamily::FiveSeven => cells * (metrics_for(family).cell_w + 1) * scale - scale,
            FontFamily::TwelveSixteen => {
                let step = smooth::step_for_scale(scale);
                cells * (smooth::cell_w(step) + 1) - 1
            }
        }
    }
}

/// Monospace styled string.
pub fn draw_str_styled(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    let f = glyph::family();
    let eff = effective_scale_for(f, scale);
    draw_str_styled_impl(fb, f, x, y, s, eff, style, theme::resolve(color));
}

#[allow(clippy::too_many_arguments)]
fn draw_str_styled_impl(
    fb: &mut Framebuffer,
    family: FontFamily,
    mut x: i32,
    y: i32,
    s: &str,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    if scale <= 0 {
        return;
    }
    let metrics = metrics_for(family);
    let adv = (metrics.cell_w + 1) * scale;
    for ch in s.chars() {
        match glyph::resolve_for(family, ch as u32) {
            Glyph::Icon(i) => {
                let slot = 2 * adv - scale;
                let sz = metrics.inline_icon * scale;
                glyph::draw_icon_index(fb, x + (slot - sz) / 2, y, i, sz, color);
                x += 2 * adv;
            }
            Glyph::Text(t) => {
                draw_text_glyph(fb, family, x, y, &t, false, scale, style, color);
                x += adv;
            }
            Glyph::Missing => {
                draw_text_glyph(
                    fb,
                    FontFamily::FiveSeven,
                    x,
                    y,
                    &missing(),
                    false,
                    scale,
                    style,
                    color,
                );
                x += adv;
            }
        }
    }
}

pub fn draw_str_prop(fb: &mut Framebuffer, x: i32, y: i32, s: &str, scale: i32, color: u32) {
    draw_str_prop_styled(fb, x, y, s, scale, FontStyle::Regular, color);
}

pub fn draw_str_prop_family(
    fb: &mut Framebuffer,
    family: FontFamily,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    color: u32,
) {
    let eff = effective_scale_for(family, scale);
    draw_str_prop_styled_impl(
        fb,
        family,
        x,
        y,
        s,
        eff,
        FontStyle::Regular,
        theme::resolve(color),
    );
}

/// Proportional styled string: glyphs are drawn shifted left by their blank
/// leading columns, then advance by ink width + 1 gap column.
pub fn draw_str_prop_styled(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    let f = glyph::family();
    let eff = effective_scale_for(f, scale);
    draw_str_prop_styled_impl(fb, f, x, y, s, eff, style, theme::resolve(color));
}

#[allow(clippy::too_many_arguments)]
fn draw_str_prop_styled_impl(
    fb: &mut Framebuffer,
    family: FontFamily,
    mut x: i32,
    y: i32,
    s: &str,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    if scale <= 0 {
        return;
    }
    for ch in s.chars() {
        let g = glyph::resolve_for(family, ch as u32);
        match g {
            Glyph::Icon(i) => {
                let sz = match family {
                    FontFamily::FiveSeven => metrics_for(family).inline_icon * scale,
                    FontFamily::TwelveSixteen => smooth::cell_w(smooth::step_for_scale(scale)),
                };
                glyph::draw_icon_index(fb, x, y, i, sz, color);
            }
            Glyph::Text(t) => {
                draw_text_glyph(fb, family, x, y, &t, true, scale, style, color);
            }
            Glyph::Missing => {
                draw_text_glyph(
                    fb,
                    FontFamily::FiveSeven,
                    x,
                    y,
                    &missing(),
                    false,
                    scale,
                    style,
                    color,
                );
            }
        }
        x += prop_advance_px(family, &g, scale);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_str_centered(
    fb: &mut Framebuffer,
    rx: i32,
    ry: i32,
    rw: i32,
    rh: i32,
    s: &str,
    scale: i32,
    color: u32,
) {
    let tw = text_width(s, scale);
    let th = glyph_height(scale);
    let x = rx + (rw - tw) / 2;
    let y = ry + (rh - th) / 2;
    draw_str(fb, x, y, s, scale, color);
}

/// Proportional variant of [`draw_str_centered`].
#[allow(clippy::too_many_arguments)]
pub fn draw_str_prop_centered(
    fb: &mut Framebuffer,
    rx: i32,
    ry: i32,
    rw: i32,
    rh: i32,
    s: &str,
    scale: i32,
    color: u32,
) {
    let tw = text_width_prop(s, scale);
    let th = glyph_height(scale);
    draw_str_prop(fb, rx + (rw - tw) / 2, ry + (rh - th) / 2, s, scale, color);
}

pub fn draw_char(fb: &mut Framebuffer, x: i32, y: i32, ch: char, scale: i32, color: u32) {
    draw_char_styled(fb, x, y, ch, scale, FontStyle::Regular, color);
}

/// Draw one character in a monospace cell at `(x, y)`.
pub fn draw_char_styled(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    ch: char,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    let mut buf = [0u8; 4];
    draw_str_styled(fb, x, y, ch.encode_utf8(&mut buf), scale, style, color);
}

fn missing() -> TextGlyph {
    TextGlyph {
        rows: &MISSING_ROWS,
        dy: 0,
        lsb: 0,
        ink_w: 5,
        index: u16::MAX,
    }
}

/// Rasterize a text glyph. In proportional mode blank leading columns are
/// trimmed; `t.dy` bumps rows down (descenders).
#[allow(clippy::too_many_arguments)]
fn draw_text_glyph(
    fb: &mut Framebuffer,
    family: FontFamily,
    x: i32,
    y: i32,
    t: &TextGlyph,
    proportional: bool,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    let dot_size = match style {
        FontStyle::Matrix if scale > 1 => scale - 1,
        _ => scale,
    };
    let bold_extra = match style {
        FontStyle::Bold => (scale / 2).max(1),
        _ => 0,
    };
    let metrics = metrics_for(family);

    match family {
        FontFamily::FiveSeven => {
            let trim = if proportional { t.lsb as i32 } else { 0 };
            let dy = t.dy as i32;
            for (row_idx, &mask) in t.rows.iter().enumerate() {
                if mask == 0 {
                    continue;
                }
                let cell_row = row_idx as i32 + dy;
                let py = y + cell_row * scale;
                let shear_dx = match style {
                    FontStyle::Italic => ((6 - row_idx as i32) * scale) / 3,
                    _ => 0,
                };
                for col in 0..5 {
                    if (mask & (1 << (4 - col))) != 0 {
                        let px = x + (col - trim) * scale + shear_dx;
                        fb.fill(px, py, dot_size + bold_extra, dot_size, color);
                    }
                }
            }
        }
        FontFamily::TwelveSixteen => {
            let step = smooth::step_for_scale(scale);
            let w = smooth::cell_w(step);
            let h = smooth::cell_h(step);
            let mask = smooth::glyph(t.index, t.rows, t.dy, step);
            let trim = if proportional {
                smooth::left_bearing(t.index, t.rows, t.dy, step) as i32
            } else {
                0
            };
            let bold_extra = i32::from(style == FontStyle::Bold);
            for row in 0..h {
                let shear_dx = match style {
                    FontStyle::Italic => ((h - row) * 2) / 16,
                    _ => 0,
                };
                for col in 0..w {
                    let level = smooth::unpack(mask, step, col, row);
                    if level != 0 {
                        let px = x + col - trim + shear_dx;
                        let py = y + row;
                        fb.blend(px, py, 1 + bold_extra, 1, color, level as u32 * 85);
                    }
                }
            }
        }
    }

    if style == FontStyle::Underline {
        match family {
            FontFamily::FiveSeven => {
                let w = if proportional {
                    prop_advance_units(family, &Glyph::Text(*t))
                } else {
                    metrics.cell_w + 1
                };
                let ul_y = y + metrics.cell_h * scale + (scale / 2).max(1);
                fb.fill(x, ul_y, w * scale, scale.max(1), color);
            }
            FontFamily::TwelveSixteen => {
                let step = smooth::step_for_scale(scale);
                let w = if proportional {
                    smooth::ink_width(t.index, t.rows, t.dy, step) as i32 + 1
                } else {
                    smooth::cell_w(step) + 1
                };
                let ul_y = y + smooth::cell_h(step) + 1;
                fb.fill(x, ul_y, w, 1, color);
            }
        }
    }
}

/// Resolve a Unicode codepoint to FiveSeven rows (missing box when unavailable).
pub fn resolve_glyph(code: u32) -> [u8; 7] {
    glyph::lookup_fiveseven(code)
        .map(|t| *t.rows)
        .unwrap_or(MISSING_ROWS)
}

fn lookup_us_latin(code: u32) -> Option<TextGlyph> {
    glyph::lookup_fiveseven(code)
}

fn lookup_twelve(code: u32) -> Option<TextGlyph> {
    glyph::lookup_fiveseven(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_latin_fully_covered() {
        for cp in 0x20u32..=0x7E {
            assert!(glyph::lookup_fiveseven(cp).is_some(), "U+{:04X}", cp);
        }
    }

    #[test]
    fn extended_scripts_and_symbols_covered() {
        for cp in 0x00A0u32..=0x017F {
            assert!(glyph::lookup_fiveseven(cp).is_some(), "Latin U+{:04X}", cp);
        }
        for c in [
            'ñ', 'É', 'ß', 'ł', 'Ω', 'π', 'λ', 'Ж', 'Я', 'д', '€', '∞', '┼', '█',
        ] {
            assert!(glyph::lookup_fiveseven(c as u32).is_some(), "{}", c);
        }
    }

    #[test]
    fn descenders_are_bumped() {
        for (c, dy) in [
            ('g', 2),
            ('p', 2),
            ('q', 2),
            ('y', 2),
            ('j', 1),
            (',', 1),
            ('a', 0),
        ] {
            assert_eq!(glyph::lookup_fiveseven(c as u32).unwrap().dy, dy, "{}", c);
        }
    }

    #[test]
    fn proportional_trims_thin_glyphs() {
        glyph::set_family(FontFamily::FiveSeven);
        assert!(char_advance_prop('i', 1) < char_advance_prop('m', 1));
        assert_eq!(char_advance_prop('m', 1), 6);
        assert_eq!(char_advance_prop('!', 1), 2);
        assert!(text_width_prop("illicit", 1) < text_width("illicit", 1));
    }

    #[test]
    fn inline_icon_widths() {
        glyph::set_family(FontFamily::FiveSeven);
        let home = "\u{E000}";
        assert_eq!(text_width(home, 1), 2 * char_advance(1) - 1);
        assert_eq!(text_width_prop(home, 1), inline_icon(1));
        assert!(matches!(glyph::resolve(0x2302), Glyph::Icon(0)));
    }

    #[test]
    fn smooth_mask_uses_2bit_fill_levels() {
        let g = glyph::lookup_fiveseven('A' as u32).unwrap();
        for step in 0..smooth::STEPS {
            let mask = smooth::glyph(g.index, g.rows, g.dy, step);
            let mut solid = 0;
            let mut mid = 0;
            for y in 0..smooth::cell_h(step) {
                for x in 0..smooth::cell_w(step) {
                    match smooth::unpack(mask, step, x, y) {
                        3 => solid += 1,
                        1 | 2 => mid += 1,
                        _ => {}
                    }
                }
            }
            assert!(solid > 0, "step {}", step);
            assert!(mid > 0, "step {}", step);
        }
    }

    #[test]
    fn family_switch_changes_advance() {
        glyph::set_family(FontFamily::FiveSeven);
        assert_eq!(char_advance(1), 6);
        glyph::set_family(FontFamily::TwelveSixteen);
        assert_eq!(effective_scale(2), 1);
        assert_eq!(char_advance(2), 13);
        assert_eq!(glyph_height(2), 19);
        // Leave the process in the shipped default mode.
        glyph::set_family(FontFamily::TwelveSixteen);
    }
}
