//! The "System" Appliance Application (`system.rs`).
//!
//! Consolidates environment inspection into five flat sections selectable from
//! the app menu, the bottom tab bar, or keys `1`..`5`:
//! - `0` (`About`): build & binary attestation
//! - `1` (`Locale`): UTC clock, timezone offset, and language/encoding
//! - `2` (`Fonts`): active family glyph repertoire, scale ladder, and styles
//! - `3` (`Icons`): `Icons32` gallery (32x32, 2-bit)
//! - `4` (`Theme`): global font family and ten color themes

use crate::fb::Framebuffer;
use crate::fmt as f;
use crate::font::{
    self, char_advance, draw_str, draw_str_centered, draw_str_native, draw_str_prop,
    draw_str_styled, glyph_height, line_height, text_width, text_width_prop, FontFamily, FontStyle,
    FIVESEVEN_US_LATIN, TWELVESIXTEEN_US_LATIN,
};
use crate::icon::{draw_icon, Icon};
use crate::input::InputResult;
use crate::layout::{Metrics, Rect};
use crate::theme as t;
use crate::ui;

pub const SECTION_BAR_H: i32 = 30;

pub const RUSTC_VER: &str = env!("APPLIANCE_RUSTC");
pub const TARGET_TRIPLE: &str = env!("APPLIANCE_TARGET");
pub const BUILD_PROFILE: &str = env!("APPLIANCE_PROFILE");
pub const SRC_HASH: &str = env!("APPLIANCE_SRC_HASH");
pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SystemSection {
    About = 0,
    Locale = 1,
    Fonts = 2,
    Icons = 3,
    Theme = 4,
}

impl SystemSection {
    pub const COUNT: usize = 5;

    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => SystemSection::Locale,
            2 => SystemSection::Fonts,
            3 => SystemSection::Icons,
            4 => SystemSection::Theme,
            _ => SystemSection::About,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SystemSection::About => "About",
            SystemSection::Locale => "Locale",
            SystemSection::Fonts => "Fonts",
            SystemSection::Icons => "Icons",
            SystemSection::Theme => "Theme",
        }
    }
}

pub struct SystemApp {
    pub section: SystemSection,
    pub snapshots_taken: u32,
    pub focus_visible: bool,
    pub hover_tab: Option<usize>,
    pub theme_index: usize,
    pub theme_focus: usize,
    pub font_family: FontFamily,
    pub family_focus: usize,
    pub theme_cursor_ready: bool,
    // UTC wall clock + timezone offset (minutes East of UTC, e.g. -240 for UTC-04:00)
    pub utc_year: u16,
    pub utc_month: u8,
    pub utc_day: u8,
    pub utc_hour: u8,
    pub utc_min: u8,
    pub utc_sec: u8,
    pub tz_offset_min: i16,
}

impl SystemApp {
    pub const fn new() -> Self {
        Self {
            section: SystemSection::About,
            snapshots_taken: 0,
            focus_visible: false,
            hover_tab: None,
            theme_index: 0,
            theme_focus: 0,
            font_family: FontFamily::TwelveSixteen,
            family_focus: 1,
            theme_cursor_ready: false,
            utc_year: 2026,
            utc_month: 10,
            utc_day: 30,
            utc_hour: 11,
            utc_min: 45,
            utc_sec: 0,
            tz_offset_min: -240,
        }
    }
}

impl Default for SystemApp {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_window(fb: &mut Framebuffer, app: &mut SystemApp, session_w: i32, session_h: i32) {
    let content_h = (fb.h - SECTION_BAR_H).max(0);
    if app.section == SystemSection::Theme && !app.theme_cursor_ready {
        app.theme_index = t::theme_index();
        app.theme_focus = app.theme_index;
        app.font_family = font::family();
        app.family_focus = match app.font_family {
            FontFamily::FiveSeven => 0,
            FontFamily::TwelveSixteen => 1,
        };
        app.theme_cursor_ready = true;
    }
    {
        let mut content = fb.sub_rows(0, content_h);
        match app.section {
            SystemSection::About => draw_about(&mut content, app, session_w, session_h),
            SystemSection::Locale => draw_locale(&mut content, app),
            SystemSection::Fonts => draw_fonts(&mut content),
            SystemSection::Icons => draw_icons(&mut content),
            SystemSection::Theme => draw_theme(&mut content, app),
        }
    }
    draw_section_bar(fb, content_h, app.section, app.focus_visible, app.hover_tab);
}

fn draw_section_bar(
    fb: &mut Framebuffer,
    y: i32,
    active: SystemSection,
    focus_visible: bool,
    hover_tab: Option<usize>,
) {
    fb.fill(0, y, fb.w, SECTION_BAR_H, t::BAR);
    ui::separator(fb, 0, y, fb.w);
    let tab_w = (fb.w / SystemSection::COUNT as i32).max(1);
    for idx in 0..SystemSection::COUNT {
        let x = idx as i32 * tab_w;
        let selected = SystemSection::from_index(idx) == active;
        let hovered = hover_tab == Some(idx);
        if selected {
            fb.fill(x, y + 1, tab_w, SECTION_BAR_H - 1, t::SURFACE);
            fb.fill(x, y + SECTION_BAR_H - 2, tab_w, 2, t::ACCENT);
        } else if hovered {
            fb.fill(x, y + 1, tab_w, SECTION_BAR_H - 1, t::SURFACE_HOVER);
        }
        draw_str_centered(
            fb,
            x,
            y + 1,
            tab_w,
            SECTION_BAR_H - 1,
            SystemSection::from_index(idx).label(),
            1,
            if selected {
                t::TEXT
            } else if hovered {
                t::TEXT_BODY
            } else {
                t::TEXT_DIM
            },
        );
        if focus_visible && selected {
            ui::focus_ring(fb, Rect::new(x + 1, y + 1, tab_w - 2, SECTION_BAR_H - 2));
        }
    }
}

pub fn pointer_move(app: &mut SystemApp, cx: i32, cy: i32, client_w: i32, client_h: i32) -> bool {
    let next = if cy >= client_h - SECTION_BAR_H {
        let tab_w = (client_w / SystemSection::COUNT as i32).max(1);
        let idx = (cx / tab_w).clamp(0, SystemSection::COUNT as i32 - 1) as usize;
        Some(idx)
    } else {
        None
    };
    if app.hover_tab == next {
        false
    } else {
        app.hover_tab = next;
        true
    }
}

pub fn pointer_leave(app: &mut SystemApp) -> bool {
    let changed = app.hover_tab.is_some();
    app.hover_tab = None;
    changed
}

/// Click the System section tabs. Content itself is read-only.
pub fn click_window(
    app: &mut SystemApp,
    cx: i32,
    cy: i32,
    client_w: i32,
    client_h: i32,
) -> InputResult {
    if cy >= client_h - SECTION_BAR_H {
        let tab_w = (client_w / SystemSection::COUNT as i32).max(1);
        let idx = (cx / tab_w).clamp(0, SystemSection::COUNT as i32 - 1) as usize;
        let target = SystemSection::from_index(idx);
        if app.section == target {
            return InputResult::Noop;
        }
        app.section = target;
        if target == SystemSection::Theme {
            app.theme_cursor_ready = false;
        }
        return InputResult::Changed;
    }
    if app.section == SystemSection::Theme {
        return click_theme(app, cx, cy, client_w, client_h);
    }
    InputResult::Noop
}

/// `Icons32` gallery: all icons at native 32px with labels, a size ladder showing
/// box-filtered downscaling / integer upscaling, two-tone backgrounds, and icons
/// embedded inline in text.
fn draw_icons(fb: &mut Framebuffer) {
    fb.clear(t::BG);
    let (x0, w, _, _, _) = layout_margins(fb.w, fb.h);
    let m = Metrics::new(fb.w, fb.h);
    let label_col = t::TEXT_DIM;
    let ink = t::TEXT_BODY;
    let rule_col = t::BORDER_DARK;
    let compact = m.is_compact();
    let head_scale = m.head_scale();
    let body_scale = m.body_scale();

    let mut y = if compact { 16 } else { 36 };
    draw_str(fb, x0, y, "Icons32", head_scale, t::TEXT);
    if fb.w >= 640 {
        draw_str(
            fb,
            x0 + 320,
            y + 7,
            "32x32 2-bit  U+E000..U+E01F",
            1,
            label_col,
        );
    }
    y += glyph_height(head_scale) + if compact { 10 } else { 18 };

    // 1. Gallery grid at native size.
    let cell_w = if compact { 64 } else { 96 };
    let cell_h = if compact { 48 } else { 58 };
    let cols = (w / cell_w).max(1);
    for (i, icon) in Icon::ALL.iter().enumerate() {
        let cx = x0 + (i as i32 % cols) * cell_w;
        let cy = y + (i as i32 / cols) * cell_h;
        if cy + cell_h > fb.h {
            break;
        }
        draw_icon(fb, cx + (cell_w - 32) / 2, cy, *icon, 32, ink);
        let lbl = icon.label();
        let lw = text_width_prop(lbl, 1);
        draw_str_prop(fb, cx + (cell_w - lw) / 2, cy + 36, lbl, 1, label_col);
    }
    let rows = (Icon::ALL.len() as i32 + cols - 1) / cols;
    y += rows * cell_h + 4;
    fb.fill(x0, y, w, 1, rule_col);
    y += if compact { 8 } else { 14 };

    // 2. Size ladder on dark and light backgrounds (4-step coverage blending).
    let sizes = [8, 12, 16, 24, 32, 48, 64];
    let ladder_h = 64 + 8;
    if y + ladder_h + 20 < fb.h {
        let mut lx = x0;
        for &sz in &sizes {
            draw_icon(fb, lx, y + (64 - sz), Icon::Home, sz, ink);
            lx += sz + 12;
        }
        let light_x = lx + 12;
        let light_w = (x0 + w - light_x).min(sizes.iter().sum::<i32>() + 12 * 7 + 16);
        if light_w > 80 {
            fb.fill(light_x, y - 4, light_w, 72, t::WHITE);
            let mut lx2 = light_x + 8;
            for &sz in &sizes {
                if lx2 + sz > light_x + light_w {
                    break;
                }
                draw_icon(fb, lx2, y + (64 - sz), Icon::Games, sz, t::BLACK);
                lx2 += sz + 12;
            }
        }
        y += ladder_h + 4;
        fb.fill(x0, y, w, 1, rule_col);
        y += if compact { 8 } else { 14 };
    }

    // 3. Inline icons in text (monospace + proportional).
    if y + 40 < fb.h {
        let mono = "\u{E000} Home  \u{E001} Back  \u{E004} Cancel  \u{E015} Games";
        draw_str(fb, x0, y, mono, body_scale, ink);
        let y2 = y + glyph_height(body_scale) + 12;
        if y2 + glyph_height(body_scale) < fb.h {
            let prop = "\u{2302} Home \u{2190} Back \u{2715} Cancel \u{1F3AE} Games (proportional)";
            draw_str_prop(fb, x0, y2, prop, body_scale, ink);
        }
    }
}

fn layout_margins(fb_w: i32, fb_h: i32) -> (i32, i32, i32, i32, i32) {
    let m = Metrics::new(fb_w, fb_h);
    let x0 = m.pick(16, 40, 120);
    let w = (fb_w - 2 * x0).max(200);
    let val_off = m.pick(124, 200, 280);
    let y0 = m.pick(20, 40, 64);
    let row_h = m.pick(30, 38, 52);
    (x0, w, val_off, y0, row_h)
}

struct ThemeGeom {
    x0: i32,
    w: i32,
    y_family: i32,
    family_h: i32,
    family_w: i32,
    y_list: i32,
    row_h: i32,
    row_gap: i32,
}

fn theme_geom(fb_w: i32, fb_h: i32) -> ThemeGeom {
    let (x0, w, _, y0, _) = layout_margins(fb_w, fb_h);
    let m = Metrics::new(fb_w, fb_h);
    let head_h = glyph_height(m.head_scale());
    let label_h = glyph_height(m.body_scale());
    let y_family = y0 + head_h + if m.is_compact() { 14 } else { 22 } + label_h + 4;
    let family_h = m.pick(24, 26, 28);
    let family_w = ((w - 8) / 2).max(80);
    let y_list = y_family + family_h + if m.is_compact() { 12 } else { 22 } + label_h + 4;
    let available = (fb_h - y_list - 8).max(10);
    let row_gap = 4;
    let row_h = ((available - 9 * row_gap) / 10).clamp(20, 30);
    ThemeGeom {
        x0,
        w,
        y_family,
        family_h,
        family_w,
        y_list,
        row_h,
        row_gap,
    }
}

fn draw_theme(fb: &mut Framebuffer, app: &SystemApp) {
    fb.clear(t::BG);
    let g = theme_geom(fb.w, fb.h);
    let m = Metrics::new(fb.w, fb.h);
    let head_scale = m.head_scale();
    let body_scale = m.body_scale();
    let accent = t::ACCENT;
    let label = t::TEXT_DIM;
    let text = t::TEXT_BODY;

    draw_str(fb, g.x0, 16, "Theme", head_scale, t::TEXT);
    fb.fill(
        g.x0,
        16 + line_height(head_scale) + 2 * head_scale,
        g.w,
        2,
        accent,
    );

    draw_str_prop(
        fb,
        g.x0,
        g.y_family - glyph_height(body_scale) - 4,
        "Rendering",
        body_scale,
        label,
    );

    let families = [FontFamily::FiveSeven, FontFamily::TwelveSixteen];
    let labels = ["Native", "Smooth 12x16"];
    for (idx, family) in families.iter().enumerate() {
        let r = Rect::new(
            g.x0 + idx as i32 * (g.family_w + 8),
            g.y_family,
            g.family_w,
            g.family_h,
        );
        let selected = app.font_family == *family;
        ui::button(
            fb,
            r,
            labels[idx],
            if selected {
                ui::Tone::Accent
            } else {
                ui::Tone::Neutral
            },
            body_scale,
        );
        if app.focus_visible && app.family_focus == idx {
            ui::focus_ring(fb, r);
        }
    }

    draw_str_prop(
        fb,
        g.x0,
        g.y_list - glyph_height(body_scale) - 4,
        "Color Theme",
        body_scale,
        label,
    );

    let current = t::theme_index();
    for idx in 0..t::THEME_COUNT {
        let r = Rect::new(
            g.x0,
            g.y_list + idx as i32 * (g.row_h + g.row_gap),
            g.w,
            g.row_h,
        );
        let selected = idx == current;
        let cursor = idx == app.theme_focus;
        if selected {
            fb.fill(r.x, r.y, r.w, r.h, t::SURFACE_SELECTED);
            fb.fill(r.x, r.y, 4, r.h, accent);
        } else if cursor && app.focus_visible {
            fb.fill(r.x, r.y, r.w, r.h, t::SURFACE_HOVER);
        }
        let swatch_w = 8;
        let swatch_y = r.y + 5;
        let swatch_h = (r.h - 10).max(4);
        fb.fill(
            r.x + 12,
            swatch_y,
            swatch_w,
            swatch_h,
            t::color(idx, t::ACCENT),
        );
        fb.fill(
            r.x + 22,
            swatch_y,
            swatch_w,
            swatch_h,
            t::color(idx, t::SURFACE),
        );
        fb.fill(
            r.x + 32,
            swatch_y,
            swatch_w,
            swatch_h,
            t::color(idx, t::TEXT_DIM),
        );
        draw_str_prop(
            fb,
            r.x + 52,
            r.y + (r.h - glyph_height(body_scale)) / 2,
            t::theme_name(idx),
            body_scale,
            if selected { t::TEXT } else { text },
        );
        if selected {
            draw_str_prop(
                fb,
                r.right() - text_width_prop("Current", body_scale) - 12,
                r.y + (r.h - glyph_height(body_scale)) / 2,
                "Current",
                body_scale,
                accent,
            );
        }
        if app.focus_visible && cursor {
            ui::focus_ring(fb, r);
        }
    }
}

pub fn apply_theme(app: &mut SystemApp, idx: usize) -> InputResult {
    if idx >= t::THEME_COUNT {
        return InputResult::Noop;
    }
    app.theme_index = idx;
    app.theme_focus = idx;
    t::set_theme(idx);
    InputResult::Changed
}

pub fn apply_family(app: &mut SystemApp, family: FontFamily) -> InputResult {
    app.font_family = family;
    app.family_focus = match family {
        FontFamily::FiveSeven => 0,
        FontFamily::TwelveSixteen => 1,
    };
    font::set_family(family);
    InputResult::Changed
}

fn click_theme(app: &mut SystemApp, cx: i32, cy: i32, client_w: i32, client_h: i32) -> InputResult {
    let g = theme_geom(client_w, (client_h - SECTION_BAR_H).max(0));
    let families = [FontFamily::FiveSeven, FontFamily::TwelveSixteen];
    for (idx, family) in families.iter().enumerate() {
        let r = Rect::new(
            g.x0 + idx as i32 * (g.family_w + 8),
            g.y_family,
            g.family_w,
            g.family_h,
        );
        if r.contains(cx, cy) {
            return apply_family(app, *family);
        }
    }
    for idx in 0..t::THEME_COUNT {
        let r = Rect::new(
            g.x0,
            g.y_list + idx as i32 * (g.row_h + g.row_gap),
            g.w,
            g.row_h,
        );
        if r.contains(cx, cy) {
            return apply_theme(app, idx);
        }
    }
    InputResult::Noop
}

fn draw_about(fb: &mut Framebuffer, app: &SystemApp, session_w: i32, session_h: i32) {
    fb.clear(t::BG);

    let (x0, w, val_off, y0, row_h) = layout_margins(fb.w, fb.h);
    let head_scale = Metrics::new(fb.w, fb.h).head_scale();

    draw_str(fb, x0, y0, "About", head_scale, t::TEXT);
    fb.fill(
        x0,
        y0 + line_height(head_scale) + 2 * head_scale,
        w,
        2,
        t::ACCENT,
    );

    let rows_y = y0 + line_height(head_scale) + 2 * head_scale + 24;

    draw_row(fb, x0, rows_y, w, val_off, "Version", PKG_VERSION);
    draw_row(fb, x0, rows_y + row_h, w, val_off, "Target", TARGET_TRIPLE);
    draw_row(
        fb,
        x0,
        rows_y + row_h * 2,
        w,
        val_off,
        "Profile",
        BUILD_PROFILE,
    );
    draw_row(
        fb,
        x0,
        rows_y + row_h * 3,
        w,
        val_off,
        "Compiler",
        RUSTC_VER,
    );
    draw_row(
        fb,
        x0,
        rows_y + row_h * 4,
        w,
        val_off,
        "Source FNV-1a",
        SRC_HASH,
    );

    let mut fb_buf = [b' '; 48];
    let fb_str = f::fb_spec(&mut fb_buf, session_w as u32, session_h as u32);
    draw_row(
        fb,
        x0,
        rows_y + row_h * 5,
        w,
        val_off,
        "Framebuffer",
        fb_str,
    );

    let mut snap_buf = [b' '; 16];
    let snap_str = f::u32_str(&mut snap_buf, app.snapshots_taken);
    draw_row(
        fb,
        x0,
        rows_y + row_h * 6,
        w,
        val_off,
        "Snapshots",
        snap_str,
    );
}

fn draw_locale(fb: &mut Framebuffer, app: &SystemApp) {
    fb.clear(t::BG);

    let (x0, w, val_off, y0, row_h) = layout_margins(fb.w, fb.h);
    let head_scale = Metrics::new(fb.w, fb.h).head_scale();

    draw_str(fb, x0, y0, "Locale", head_scale, t::TEXT);
    fb.fill(
        x0,
        y0 + line_height(head_scale) + 2 * head_scale,
        w,
        2,
        t::ACCENT,
    );

    let rows_y = y0 + line_height(head_scale) + 2 * head_scale + 24;

    draw_row(fb, x0, rows_y, w, val_off, "Language", "en-US");
    draw_row(
        fb,
        x0,
        rows_y + row_h,
        w,
        val_off,
        "Charset",
        "UTF-8 (Latin / Greek / Cyrillic / Symbols)",
    );

    let mut utc_buf = [b' '; 24];
    let utc_str = f::datetime24(
        &mut utc_buf,
        app.utc_year,
        app.utc_month,
        app.utc_day,
        app.utc_hour,
        app.utc_min,
        app.utc_sec,
    );
    draw_row(fb, x0, rows_y + row_h * 2, w, val_off, "UTC Time", utc_str);

    let mut tz_buf = [b' '; 24];
    let tz_str = f::tz_offset(&mut tz_buf, app.tz_offset_min);
    draw_row(fb, x0, rows_y + row_h * 3, w, val_off, "UTC Offset", tz_str);
    draw_row(
        fb,
        x0,
        rows_y + row_h * 4,
        w,
        val_off,
        "Scripts",
        "¡Hola España! · Γεια σας · Привет мир · ┌─┼─┐",
    );
}

fn draw_sample(fb: &mut Framebuffer, x: i32, y: i32, s: &str, scale: i32, max_w: i32, color: u32) {
    if max_w <= 0 {
        return;
    }
    let max_chars = (max_w / char_advance(scale)).max(4) as usize;
    draw_str(fb, x, y, ui::truncate_chars(s, max_chars), scale, color);
}

fn draw_compact_fonts(fb: &mut Framebuffer, sheet: &crate::font::FontSheet) {
    let x0 = 16;
    let m = Metrics::new(fb.w, fb.h);
    let body = m.body_scale();
    let head = m.head_scale();
    let mut y = 16;
    draw_str(fb, x0, y, sheet.id, head, t::TEXT);
    y += line_height(head) + 2 * head;
    fb.fill(x0, y, (fb.w - 2 * x0).max(80), 2, t::ACCENT);
    y += 10;

    draw_str_native(fb, font::family(), x0, y, "AaBbCc 0123", 1, t::TEXT_BODY);
    y += font::metrics().cell_h + 12;

    for (sc, label) in [(1, "1x"), (2, "2x")] {
        draw_str_prop(
            fb,
            x0,
            y + (font::metrics().cell_h * sc - glyph_height(body)) / 2,
            label,
            body,
            t::TEXT_DIM,
        );
        draw_str_native(fb, font::family(), x0 + 40, y, "Aa 0123", sc, t::TEXT_BODY);
        y += font::metrics().cell_h * sc + 8;
    }
    y += 4;
    fb.fill(x0, y, (fb.w - 2 * x0).max(80), 1, t::BORDER_DARK);
    y += 10;

    let styles: [(FontStyle, &str); 5] = [
        (FontStyle::Regular, "Regular"),
        (FontStyle::Bold, "Bold"),
        (FontStyle::Italic, "Italic"),
        (FontStyle::Matrix, "Matrix"),
        (FontStyle::Underline, "Underline"),
    ];
    let sample_x = x0 + text_width_prop("Underline", body) + 12;
    for (style, name) in styles {
        draw_str_prop(fb, x0, y, name, body, t::TEXT_DIM);
        draw_str_styled(fb, sample_x, y, "Aa 0123", 1, style, t::TEXT_BODY);
        y += glyph_height(body) + 8;
    }
}

fn draw_fonts(fb: &mut Framebuffer) {
    fb.clear(t::BG);
    let sheet = match font::family() {
        FontFamily::FiveSeven => &FIVESEVEN_US_LATIN,
        FontFamily::TwelveSixteen => &TWELVESIXTEEN_US_LATIN,
    };
    if fb.w < 560 {
        draw_compact_fonts(fb, sheet);
        return;
    }

    let (x0, w, _, _, _) = layout_margins(fb.w, fb.h);
    let m = Metrics::new(fb.w, fb.h);
    let val_off = m.pick(92, 140, 220);
    let label_col = t::TEXT_DIM;
    let val_col = t::TEXT_BODY;
    let accent = t::ACCENT;
    let rule_col = t::BORDER_DARK;
    let compact = m.is_compact();
    let body_scale = m.body_scale();
    let head_scale = m.head_scale();

    let y0 = if compact { 16 } else { 36 };
    draw_str(fb, x0, y0, sheet.id, head_scale, t::TEXT);
    if fb.w >= 640 {
        let sheet_x = x0 + text_width(sheet.id, head_scale) + 24;
        if sheet_x + text_width(sheet.range_label, 2) < x0 + w {
            draw_str(fb, sheet_x, y0 + 7, sheet.range_label, 2, label_col);
        }
    }
    fb.fill(
        x0,
        y0 + line_height(head_scale) + 2 * head_scale,
        w,
        2,
        accent,
    );

    // 1. Character Set
    let y_cs = y0 + line_height(head_scale) + 2 * head_scale + (if compact { 10 } else { 18 });
    let cs_step = if compact { 16 } else { 22 };
    draw_str(fb, x0, y_cs, "Upper", body_scale, label_col);
    draw_sample(
        fb,
        x0 + val_off,
        y_cs,
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        body_scale,
        w - val_off - 8,
        val_col,
    );

    draw_str(fb, x0, y_cs + cs_step, "Lower", body_scale, label_col);
    draw_sample(
        fb,
        x0 + val_off,
        y_cs + cs_step,
        "abcdefghijklmnopqrstuvwxyz",
        body_scale,
        w - val_off - 8,
        val_col,
    );

    draw_str(fb, x0, y_cs + cs_step * 2, "Latin+", body_scale, label_col);
    draw_sample(
        fb,
        x0 + val_off,
        y_cs + cs_step * 2,
        "¡¿ÀÁÂÃÄÅÆÇÈÉÊËÑÒÓÔÕÖØÙÚÛÜÝßàáéíñóúüąćęłńśźż",
        body_scale,
        w - val_off - 8,
        val_col,
    );

    draw_str(
        fb,
        x0,
        y_cs + cs_step * 3,
        "Greek/Cyr",
        body_scale,
        label_col,
    );
    draw_sample(
        fb,
        x0 + val_off,
        y_cs + cs_step * 3,
        "ΑΒΓΔΕΖΗΘΛΞΠΣΦΨΩ αβγδεζηθλμπσφω · АБГДЖЗИЙЛПФЦЧШЩЯ",
        body_scale,
        w - val_off - 8,
        val_col,
    );

    draw_str(fb, x0, y_cs + cs_step * 4, "Symbols", body_scale, label_col);
    draw_sample(
        fb,
        x0 + val_off,
        y_cs + cs_step * 4,
        "0123456789 ±×÷≠≤≥√∞∑∫€£¥ •…–— ┌─┬─┐│┼│└─┴─┘░▒▓█★♥",
        body_scale,
        w - val_off - 8,
        val_col,
    );
    let div1_y = y_cs + cs_step * 5;
    fb.fill(x0, div1_y, w, 1, rule_col);

    // 2. Native scale ladder for the active family.
    let mut y_scale = div1_y + (if compact { 10 } else { 18 });
    let scales: &[(i32, &str)] = match font::family() {
        FontFamily::FiveSeven => &[
            (1, "1x (5x7)"),
            (2, "2x (10x14)"),
            (3, "3x (15x21)"),
            (4, "4x (20x28)"),
            (5, "5x (25x35)"),
        ],
        FontFamily::TwelveSixteen => &[(1, "1x (12x19)"), (2, "2x (17x28)"), (3, "3x (22x37)")],
    };
    let scale_gap = if compact { 8 } else { 14 };
    for &(sc, label) in scales {
        let row_h = font::native_cell(font::family(), sc).1;
        if y_scale + row_h > fb.h - 80 && sc > 1 {
            break;
        }
        draw_str(
            fb,
            x0,
            y_scale + (row_h - glyph_height(body_scale)) / 2,
            label,
            body_scale,
            label_col,
        );
        draw_str_native(
            fb,
            font::family(),
            x0 + val_off,
            y_scale,
            "Aa 0123",
            sc,
            val_col,
        );
        y_scale += row_h + scale_gap;
    }
    fb.fill(x0, y_scale - 2, w, 1, rule_col);

    // 3. Style Transforms (Regular, Bold, Italic, Matrix, Underline)
    let y_style = y_scale + (if compact { 8 } else { 12 });
    let style_sc = if font::family() == FontFamily::TwelveSixteen {
        1
    } else if compact {
        2
    } else {
        3
    };
    let style_step = glyph_height(style_sc) + 12;
    let styles: [(FontStyle, &str); 5] = [
        (FontStyle::Regular, "Regular"),
        (FontStyle::Bold, "Bold"),
        (FontStyle::Italic, "Italic"),
        (FontStyle::Matrix, "Matrix"),
        (FontStyle::Underline, "Underline"),
    ];
    for (idx, &(style, name)) in styles.iter().enumerate() {
        let ry = y_style + (idx as i32) * style_step;
        if ry + glyph_height(style_sc) > fb.h {
            break;
        }
        draw_str(fb, x0, ry + 2, name, body_scale, label_col);
        draw_str_styled(
            fb,
            x0 + val_off,
            ry,
            "Pack my box with five dozen jugs",
            style_sc,
            style,
            val_col,
        );
    }
}

fn draw_row(fb: &mut Framebuffer, x: i32, y: i32, w: i32, val_off: i32, key: &str, val: &str) {
    if y + 20 > fb.h {
        return;
    }
    let scale = if w < 420 { 1 } else { 2 };
    let char_w = char_advance(scale);
    let max_chars = ((w - val_off) / char_w).max(6) as usize;
    let v_clamped = ui::truncate_chars(val, max_chars);
    ui::kv_row(
        fb,
        Rect::new(x, y, w, line_height(scale) + 2 * scale + 6),
        key,
        v_clamped,
        scale,
        t::TEXT_DIM,
        t::TEXT_BODY,
    );
    ui::separator(fb, x, y + line_height(scale) + 2 * scale + 6, w);
}

pub fn draw_desktop_backdrop(fb: &mut Framebuffer) {
    fb.clear(t::BG_DEEP);

    let step = 32;
    let grid_col = t::GRID;
    let mut x = 0;
    while x < fb.w {
        fb.fill(x, 0, 1, fb.h, grid_col);
        x += step;
    }
    let mut y = 0;
    while y < fb.h {
        fb.fill(0, y, fb.w, 1, grid_col);
        y += step;
    }
}

/// Draw the `128 x 72` System App Icon in the Left Dock.
pub fn draw_dock_tile(
    fb: &mut Framebuffer,
    app: &SystemApp,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    active: bool,
) {
    ui::dock_tile(fb, Rect::new(x, y, w, h), active);
    draw_icon(fb, x + 10, y + 24, Icon::System, 32, t::ACCENT);
    draw_str_prop(fb, x + 48, y + 18, "System", 2, t::TEXT);
    draw_str_prop(fb, x + 48, y + 42, app.section.label(), 1, t::ACCENT);
}
