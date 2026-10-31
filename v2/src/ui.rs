//! Shared chrome widgets (`ui.rs`).
//!
//! Keep panel frames, separators, dock tiles, buttons and key/value rows here
//! so every app has the same baseline geometry and palette behavior.

use crate::fb::Framebuffer;
use crate::font::{draw_str, draw_str_centered, text_width};
use crate::layout::Rect;
use crate::theme as t;

/// Button emphasis, mapped to a small set of theme tokens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Neutral,
    Accent,
    Warning,
    Danger,
    Success,
}

/// One-pixel rule.
pub fn separator(fb: &mut Framebuffer, x: i32, y: i32, w: i32) {
    fb.fill(x, y, w, 1, t::BORDER_DARK);
}

/// Keyboard focus ring: two-pixel amber border with a dark inner separation.
pub fn focus_ring(fb: &mut Framebuffer, r: Rect) {
    if r.w <= 0 || r.h <= 0 {
        return;
    }
    fb.border(r.x, r.y, r.w, r.h, 2, t::FOCUS);
    fb.border(r.x + 2, r.y + 2, r.w - 4, r.h - 4, 1, t::OVERLAY);
}

/// Mouse hover outline, intentionally weaker than the focus ring.
pub fn hover_ring(fb: &mut Framebuffer, r: Rect) {
    if r.w <= 0 || r.h <= 0 {
        return;
    }
    fb.border(r.x, r.y, r.w, r.h, 1, t::TEXT_SOFT);
}

/// Two-pixel accent rule used under section headings.
pub fn accent_rule(fb: &mut Framebuffer, x: i32, y: i32, w: i32) {
    fb.fill(x, y, w, 2, t::ACCENT);
}

/// Filled, bordered panel.
pub fn panel(fb: &mut Framebuffer, r: Rect, bg: u32, border: u32) {
    fb.fill(r.x, r.y, r.w, r.h, bg);
    fb.border(r.x, r.y, r.w, r.h, 1, border);
}

/// Shared left-dock tile frame: shadow, surface, active accent border and the
/// four-pixel accent stripe. Apps draw only their icon and labels on top.
pub fn dock_tile(fb: &mut Framebuffer, r: Rect, active: bool) {
    fb.shade(r.x - 4, r.y - 4, r.w + 8, r.h + 8, 64);
    fb.fill(
        r.x,
        r.y,
        r.w,
        r.h,
        if active {
            t::SURFACE_ACTIVE
        } else {
            t::SURFACE
        },
    );
    fb.border(
        r.x,
        r.y,
        r.w,
        r.h,
        2,
        if active { t::ACCENT } else { t::BORDER_STRONG },
    );
    fb.fill(r.x + 2, r.y + 2, r.w - 4, 4, t::ACCENT);
}

/// Toolbar strip with a bottom separator.
pub fn toolbar(fb: &mut Framebuffer, r: Rect) {
    fb.fill(r.x, r.y, r.w, r.h, t::BAR);
    separator(fb, r.x, r.bottom() - 1, r.w);
}

/// Standard button. `label` is centered in the rect.
pub fn button(fb: &mut Framebuffer, r: Rect, label: &str, tone: Tone, scale: i32) {
    let (bg, border, fg) = match tone {
        Tone::Neutral => (t::SURFACE_ALT, t::BORDER_STRONG, t::TEXT_BODY),
        Tone::Accent => (t::ACCENT_DIM, t::ACCENT, t::TEXT),
        Tone::Warning => (t::MINES_FLAG_MODE, t::WARNING, t::WARNING_BRIGHT),
        Tone::Danger => (t::CALC_CLEAR, t::DANGER, t::DANGER_SOFT),
        Tone::Success => (t::TTS_ACTIVE, t::ACCENT, t::SUCCESS),
    };
    panel(fb, r, bg, border);
    draw_str_centered(fb, r.x, r.y, r.w, r.h, label, scale, fg);
}

/// Left label / right value row.
pub fn kv_row(
    fb: &mut Framebuffer,
    r: Rect,
    key: &str,
    val: &str,
    scale: i32,
    key_col: u32,
    val_col: u32,
) {
    draw_str(fb, r.x, r.y, key, scale, key_col);
    let vw = text_width(val, scale);
    draw_str(fb, r.right() - vw, r.y, val, scale, val_col);
}

/// Clamp `s` to at most `max_chars` Unicode scalar values.
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}
