//! Bitmap font engine (`font.rs`).
//!
//! One font, **FiveSeven** (5x7, 1 bit per pixel), with two render modes:
//!
//! - **Native**: the source drawn as `scale x scale` blocks.
//! - **Smooth**: a runtime-derived mask of the same physical size, with
//!   softened edges. Scale 1 uses the native path: a 1-bit source at 1:1 has no
//!   sub-pixel information to soften.
//!
//! `scale` is absolute. 1 is the 5x7 base, and both modes occupy the same
//! layout box at the same scale — render mode affects pixels, never layout:
//!
//! ```text
//! ink     = 5s x 7s
//! box     = (5s+2) x (7s+2)     // 1px bleed ring on every side
//! advance = 5s+3                // ink + ring + 1 gap
//! line    = 9s+2                // exactly the smooth mask height
//! ```
//!
//! The bleed ring is part of the layout box for both modes, so the soft edge
//! always has a pixel to land in and toggling modes never reflows. At 1x there
//! is no ring: ink 5x7, advance 6, line 10.
//!
//! Additional fonts plug into [`FONT_REGISTRY`] with their own glyph table,
//! supported [`RenderMode`]s, and default mode. The mode is a property of the
//! rasterizer, not of the font's identity.
//!
//! Two spacing modes:
//! - **Monospace** ([`draw_str`], [`text_width`]): one cell plus a gap.
//! - **Proportional** ([`draw_str_prop`], [`text_width_prop`]): blank columns
//!   are trimmed and inline icons take a square cell.

use core::sync::atomic::{AtomicU8, Ordering};

use crate::fb::Framebuffer;
use crate::glyph::{self, Glyph, TextGlyph, MISSING_ROWS};
use crate::smooth;
use crate::theme;

pub use crate::glyph::Font;

// FiveSeven base metrics, per scale unit.
pub const GLYPH_W: i32 = 5;
pub const GLYPH_H: i32 = 7;
pub const DESCENT: i32 = 2;
pub const LINE_H: i32 = 10;
pub const ADVANCE: i32 = 6;
pub const INLINE_ICON: i32 = 7;
pub const GLYPH_COUNT: usize = glyph::FIVESEVEN_GLYPH_COUNT;

/// How a font's glyph source is rasterized.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RenderMode {
    Native = 0,
    #[default]
    Smooth = 1,
}

impl RenderMode {
    pub const ALL: [RenderMode; 2] = [RenderMode::Native, RenderMode::Smooth];

    pub const fn name(self) -> &'static str {
        match self {
            RenderMode::Native => "Native",
            RenderMode::Smooth => "Smooth",
        }
    }
}

static RENDER_MODE: AtomicU8 = AtomicU8::new(RenderMode::Smooth as u8);

pub fn render_mode() -> RenderMode {
    match RENDER_MODE.load(Ordering::Relaxed) {
        0 => RenderMode::Native,
        _ => RenderMode::Smooth,
    }
}

pub fn set_render_mode(mode: RenderMode) -> bool {
    RENDER_MODE.store(mode as u8, Ordering::Relaxed);
    true
}

pub fn font() -> Font {
    glyph::font()
}

pub fn set_font(font: Font) -> bool {
    glyph::set_font(font)
}

/// Descriptor for one font. Future fonts add an entry here.
pub struct FontSpec {
    pub font: Font,
    pub id: &'static str,
    pub range_label: &'static str,
    /// Render modes this font supports. The first entry is the default.
    pub modes: &'static [RenderMode],
    pub lookup: fn(u32) -> Option<TextGlyph>,
}

impl FontSpec {
    pub fn default_mode(&self) -> RenderMode {
        self.modes[0]
    }

    pub fn supports(&self, mode: RenderMode) -> bool {
        self.modes.contains(&mode)
    }
}

pub const FIVESEVEN: FontSpec = FontSpec {
    font: Font::FiveSeven,
    id: "FiveSeven",
    range_label: "532 Glyphs + 32 Icons (UTF-8)",
    modes: &[RenderMode::Smooth, RenderMode::Native],
    lookup: lookup_us_latin,
};

pub const FONT_REGISTRY: &[FontSpec] = &[FIVESEVEN];

pub fn spec(font: Font) -> &'static FontSpec {
    &FONT_REGISTRY[font as usize]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontStyle {
    Regular = 0,
    Bold = 1,
    Italic = 2,
    Matrix = 3,
    Underline = 4,
}

/// Scale-normalised metrics. Identical for every render mode.
#[derive(Clone, Copy, Debug)]
pub struct FontMetrics {
    pub cell_w: i32,
    pub cell_h: i32,
    pub line_h: i32,
    pub descent: i32,
    pub inline_icon: i32,
}

pub fn metrics() -> FontMetrics {
    FontMetrics {
        cell_w: GLYPH_W,
        cell_h: GLYPH_H,
        line_h: LINE_H,
        descent: DESCENT,
        inline_icon: INLINE_ICON,
    }
}

fn scale_of(scale: i32) -> i32 {
    scale.max(1)
}

/// One pixel of bleed on every side of the ink. The smooth mask draws its
/// antialiased ring into this space; native leaves it blank so both modes keep
/// the same footprint. At 1x there is no ring (Native == Smooth), so no pad.
fn bleed(scale: i32) -> i32 {
    if scale_of(scale) >= 2 {
        1
    } else {
        0
    }
}

/// Ink box plus its bleed ring, used for centring and row allocation.
pub fn glyph_height(scale: i32) -> i32 {
    GLYPH_H * scale_of(scale) + 2 * bleed(scale)
}

/// Full line box: ink + descent room + the bleed ring. Above 1x it is sized to
/// the smooth mask exactly, so the ring is never clipped.
pub fn line_height(scale: i32) -> i32 {
    let s = scale_of(scale);
    if s >= 2 {
        9 * s + 2
    } else {
        LINE_H
    }
}

/// One monospace cell: ink + ring + one pixel of gap.
pub fn char_advance(scale: i32) -> i32 {
    let s = scale_of(scale);
    if s >= 2 {
        5 * s + 3
    } else {
        ADVANCE
    }
}

pub fn inline_icon(scale: i32) -> i32 {
    INLINE_ICON * scale_of(scale)
}

pub fn descent(scale: i32) -> i32 {
    DESCENT * scale_of(scale)
}

/// Native cell size (width, height) at an absolute scale.
pub fn native_cell(scale: i32) -> (i32, i32) {
    let s = scale_of(scale);
    (GLYPH_W * s, GLYPH_H * s)
}

/// Monospace advance per character cell.
pub fn mono_cells(cp: u32) -> i32 {
    match glyph::resolve(cp) {
        Glyph::Icon(_) => 2,
        _ => 1,
    }
}

/// Monospace string width in pixels (no trailing gap).
pub fn text_width(s: &str, scale: i32) -> i32 {
    let scale = scale_of(scale);
    let cells: i32 = s
        .chars()
        .map(|c| match glyph::resolve(c as u32) {
            Glyph::Icon(_) => 2,
            _ => 1,
        })
        .sum();
    if cells <= 0 {
        0
    } else {
        cells * char_advance(scale) - 1
    }
}

/// Proportional advance in pixels of a resolved glyph at `scale`.
fn prop_advance(g: &Glyph, scale: i32) -> i32 {
    let s = scale_of(scale);
    let ring = 2 * bleed(s);
    match g {
        Glyph::Text(t) if t.ink_w == 0 => 3 * s + ring,
        Glyph::Text(t) => t.ink_w as i32 * s + 1 + ring,
        Glyph::Icon(_) => INLINE_ICON * s + 1 + ring,
        Glyph::Missing => GLYPH_W * s + 1 + ring,
    }
}

/// Proportional advance of a single character at `scale`.
pub fn char_advance_prop(ch: char, scale: i32) -> i32 {
    prop_advance(&glyph::resolve(ch as u32), scale)
}

/// Proportional string width in pixels (no trailing gap).
pub fn text_width_prop(s: &str, scale: i32) -> i32 {
    let scale = scale_of(scale);
    let sum: i32 = s
        .chars()
        .map(|c| prop_advance(&glyph::resolve(c as u32), scale))
        .sum();
    if sum <= 0 {
        0
    } else {
        sum - 1 - 2 * bleed(scale)
    }
}

pub fn draw_str(fb: &mut Framebuffer, x: i32, y: i32, s: &str, scale: i32, color: u32) {
    draw_str_styled_mode(
        fb,
        render_mode(),
        x,
        y,
        s,
        scale,
        FontStyle::Regular,
        color,
    );
}

/// Draw with an explicit render mode (used by the Fonts gallery).
pub fn draw_str_mode(
    fb: &mut Framebuffer,
    mode: RenderMode,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    color: u32,
) {
    draw_str_styled_mode(fb, mode, x, y, s, scale, FontStyle::Regular, color);
}

/// Monospace styled string in the active render mode.
pub fn draw_str_styled(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    draw_str_styled_mode(fb, render_mode(), x, y, s, scale, style, color);
}

#[allow(clippy::too_many_arguments)]
fn draw_str_styled_mode(
    fb: &mut Framebuffer,
    mode: RenderMode,
    mut x: i32,
    y: i32,
    s: &str,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    let scale = scale_of(scale);
    let color = theme::resolve(color);
    let adv = char_advance(scale);
    for ch in s.chars() {
        match glyph::resolve(ch as u32) {
            Glyph::Icon(i) => {
                let slot = 2 * adv - scale;
                let sz = INLINE_ICON * scale;
                glyph::draw_icon_index(fb, x + (slot - sz) / 2, y, i, sz, color);
                x += 2 * adv;
            }
            Glyph::Text(t) => {
                draw_text_glyph(fb, mode, x, y, &t, false, scale, style, color);
                x += adv;
            }
            Glyph::Missing => {
                // The fallback box is a native 5x7 bitmap at every scale.
                draw_text_glyph(fb, RenderMode::Native, x, y, &missing(), false, scale, style, color);
                x += adv;
            }
        }
    }
}

pub fn draw_str_prop(fb: &mut Framebuffer, x: i32, y: i32, s: &str, scale: i32, color: u32) {
    draw_str_prop_styled(fb, x, y, s, scale, FontStyle::Regular, color);
}

/// Proportional string in an explicit render mode.
pub fn draw_str_prop_mode(
    fb: &mut Framebuffer,
    mode: RenderMode,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    color: u32,
) {
    draw_str_prop_styled_mode(fb, mode, x, y, s, scale, FontStyle::Regular, color);
}

/// Proportional styled string in the active render mode.
pub fn draw_str_prop_styled(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    s: &str,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    draw_str_prop_styled_mode(fb, render_mode(), x, y, s, scale, style, color);
}

#[allow(clippy::too_many_arguments)]
fn draw_str_prop_styled_mode(
    fb: &mut Framebuffer,
    mode: RenderMode,
    mut x: i32,
    y: i32,
    s: &str,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    let scale = scale_of(scale);
    let color = theme::resolve(color);
    for ch in s.chars() {
        let g = glyph::resolve(ch as u32);
        match g {
            Glyph::Icon(i) => {
                let sz = INLINE_ICON * scale;
                glyph::draw_icon_index(fb, x, y, i, sz, color);
            }
            Glyph::Text(t) => {
                draw_text_glyph(fb, mode, x, y, &t, true, scale, style, color);
            }
            Glyph::Missing => {
                draw_text_glyph(fb, RenderMode::Native, x, y, &missing(), true, scale, style, color);
            }
        }
        x += prop_advance(&g, scale);
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
    draw_str(fb, rx + (rw - tw) / 2, ry + (rh - th) / 2, s, scale, color);
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
        rows: MISSING_ROWS,
        dy: 0,
        lsb: 0,
        ink_w: 5,
        index: u16::MAX,
    }
}

/// Rasterize a text glyph. In proportional mode blank leading columns are
/// trimmed; `t.dy` bumps rows down (descenders).
///
/// The smooth mask is anchored so its ink core starts exactly where the native
/// blocks would: the 1px ring hangs into the gap and line leading.
#[allow(clippy::too_many_arguments)]
fn draw_text_glyph(
    fb: &mut Framebuffer,
    mode: RenderMode,
    x: i32,
    y: i32,
    t: &TextGlyph,
    proportional: bool,
    scale: i32,
    style: FontStyle,
    color: u32,
) {
    let scale = scale_of(scale);
    let trim = if proportional { t.lsb as i32 * scale } else { 0 };
    let dy = t.dy as i32;

    if mode == RenderMode::Smooth && scale >= 2 {
        let step = smooth::step_for_scale(scale).unwrap_or(0);
        let w = smooth::mask_w(step);
        let h = smooth::mask_h(step);
        let mask = smooth::glyph(glyph::font() as u8, t.index, &t.rows, t.dy, step);
        let bold_extra = i32::from(style == FontStyle::Bold);
        for row in 0..h {
            let shear_dx = match style {
                FontStyle::Italic => {
                    let src_row = ((row - 1) / scale).clamp(0, 6);
                    ((6 - src_row) * scale) / 3
                }
                _ => 0,
            };
            for col in 0..w {
                let level = smooth::unpack(mask, step, col, row);
                if level == 0 {
                    continue;
                }
                let px = x + col - 1 - trim + shear_dx;
                let py = y + row;
                fb.blend(px, py, 1 + bold_extra, 1, color, level as u32);
            }
        }
    } else {
        let dot_size = match style {
            FontStyle::Matrix if scale > 1 => scale - 1,
            _ => scale,
        };
        let bold_extra = match style {
            FontStyle::Bold => (scale / 2).max(1),
            _ => 0,
        };
        let pad = bleed(scale);
        for (row_idx, &rows) in t.rows.iter().enumerate() {
            if rows == 0 {
                continue;
            }
            let py = y + pad + (row_idx as i32 + dy) * scale;
            let shear_dx = match style {
                FontStyle::Italic => ((6 - row_idx as i32) * scale) / 3,
                _ => 0,
            };
            for col in 0..5 {
                if (rows & (1 << (4 - col))) != 0 {
                    let px = x + col * scale - trim + shear_dx;
                    fb.fill(px, py, dot_size + bold_extra, dot_size, color);
                }
            }
        }
    }

    if style == FontStyle::Underline {
        let w = if proportional {
            prop_advance(&Glyph::Text(*t), scale)
        } else {
            ADVANCE * scale
        };
        let ul_y = y + bleed(scale) + GLYPH_H * scale + (scale / 2).max(1);
        fb.fill(x, ul_y, w, (scale / 2).max(1), color);
    }
}

/// Resolve a Unicode codepoint to FiveSeven rows (missing box when unavailable).
pub fn resolve_glyph(code: u32) -> [u8; 7] {
    glyph::lookup_fiveseven(code)
        .map(|t| t.rows)
        .unwrap_or(MISSING_ROWS)
}

fn lookup_us_latin(code: u32) -> Option<TextGlyph> {
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
        assert!(char_advance_prop('i', 1) < char_advance_prop('m', 1));
        assert_eq!(char_advance_prop('m', 1), 6);
        assert_eq!(char_advance_prop('!', 1), 2);
        assert!(text_width_prop("illicit", 1) < text_width("illicit", 1));
    }

    #[test]
    fn inline_icon_widths() {
        let home = "\u{E000}";
        assert_eq!(text_width(home, 1), 2 * char_advance(1) - 1);
        assert_eq!(text_width_prop(home, 1), inline_icon(1));
        assert!(matches!(glyph::resolve(0x2302), Glyph::Icon(0)));
    }

    #[test]
    fn smooth_mask_has_full_and_partial_coverage() {
        let g = glyph::lookup_fiveseven('A' as u32).unwrap();
        for step in 0..smooth::STEPS {
            let mask = smooth::glyph(0, g.index, &g.rows, g.dy, step);
            let (mut solid, mut mid) = (0, 0);
            for y in 0..smooth::mask_h(step) {
                for x in 0..smooth::mask_w(step) {
                    match smooth::unpack(mask, step, x, y) {
                        255 => solid += 1,
                        0 => {}
                        _ => mid += 1,
                    }
                }
            }
            assert!(solid > 0, "step {}", step);
            assert!(mid > 0, "step {}", step);
        }
    }

    /// The whole point of the mode split: at the same scale, both modes occupy
    /// the same layout box.
    #[test]
    fn render_modes_share_layout() {
        assert_eq!(glyph_height(1), 7);
        assert_eq!(line_height(1), 10);
        assert_eq!(char_advance(1), 6);
        for scale in 2..=8 {
            let s = scale;
            assert_eq!(glyph_height(s), 7 * s + 2);
            assert_eq!(line_height(s), 9 * s + 2);
            assert_eq!(char_advance(s), 5 * s + 3);
            // The smooth core is exactly the native ink footprint.
            if let Some(step) = smooth::step_for_scale(s) {
                assert_eq!(smooth::ink_w(step), 5 * s, "scale {}", s);
                assert_eq!(smooth::ink_h(step), 7 * s, "scale {}", s);
            }
        }
    }

    /// Smooth at 1x must be pixel-identical to Native at 1x.
    #[test]
    fn scale_one_smooth_equals_native() {
        let w = 40;
        let h = 20;
        let mut a = std::vec![0u32; (w * h) as usize];
        let mut b = std::vec![0u32; (w * h) as usize];
        {
            let mut fa = Framebuffer::new(&mut a, w, h);
            draw_str_mode(&mut fa, RenderMode::Native, 1, 2, "Ag", 1, 0xFFFFFFFF);
        }
        {
            let mut fb = Framebuffer::new(&mut b, w, h);
            draw_str_mode(&mut fb, RenderMode::Smooth, 1, 2, "Ag", 1, 0xFFFFFFFF);
        }
        assert_eq!(a, b);
    }

    #[test]
    fn registry_declares_modes() {
        let f = spec(Font::FiveSeven);
        assert_eq!(f.id, "FiveSeven");
        assert!(f.supports(RenderMode::Native));
        assert!(f.supports(RenderMode::Smooth));
        assert_eq!(f.default_mode(), RenderMode::Smooth);
    }
}
