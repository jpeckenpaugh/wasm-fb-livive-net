//! The "System" Appliance Application (`system.rs`).
//!
//! Consolidates environment inspection into five flat sections selectable from
//! the app menu, the bottom tab bar, or keys `1`..`5`:
//! - `0` (`About`): build & binary attestation
//! - `1` (`Locale`): UTC clock, timezone offset, and language/encoding
//! - `2` (`Fonts`): render mode, glyph repertoire, scale ladder, and styles
//! - `3` (`Icons`): `Icons32` gallery (32x32, 2-bit)
//! - `4` (`Theme`): ten color themes

use crate::fb::Framebuffer;
use crate::fmt as f;
use crate::font::{
    self, char_advance, draw_str, draw_str_centered, draw_str_mode, draw_str_prop,
    draw_str_styled, glyph_height, line_height, text_width, text_width_prop, FontStyle, RenderMode,
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
    pub render_focus: usize,
    pub preview_idx: usize,
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
            render_focus: 1,
            preview_idx: 3,
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
        app.theme_cursor_ready = true;
    }
    {
        let mut content = fb.sub_rows(0, content_h);
        match app.section {
            SystemSection::About => draw_about(&mut content, app, session_w, session_h),
            SystemSection::Locale => draw_locale(&mut content, app),
            SystemSection::Fonts => draw_fonts(&mut content, app),
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
    if app.section == SystemSection::Fonts {
        return click_fonts(app, cx, cy, client_w, client_h);
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
        let lw = text_width_prop(lbl, 2);
        draw_str_prop(fb, cx + (cell_w - lw) / 2, cy + 36, lbl, 2, label_col);
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
    y_list: i32,
    row_h: i32,
    row_gap: i32,
}

fn theme_geom(fb_w: i32, fb_h: i32) -> ThemeGeom {
    let (x0, w, _, y0, _) = layout_margins(fb_w, fb_h);
    let m = Metrics::new(fb_w, fb_h);
    let head_h = glyph_height(m.head_scale());
    let label_h = glyph_height(m.body_scale());
    let y_list = y0 + head_h + if m.is_compact() { 14 } else { 22 } + label_h + 4;
    let available = (fb_h - y_list - 8).max(10);
    let row_gap = 4;
    let row_h = ((available - 9 * row_gap) / 10).clamp(20, 34);
    ThemeGeom {
        x0,
        w,
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

pub fn apply_render_mode(app: &mut SystemApp, mode: RenderMode) -> InputResult {
    if font::render_mode() == mode {
        app.render_focus = mode as usize;
        return InputResult::Handled;
    }
    app.render_focus = mode as usize;
    font::set_render_mode(mode);
    InputResult::Changed
}

fn click_theme(app: &mut SystemApp, cx: i32, cy: i32, client_w: i32, client_h: i32) -> InputResult {
    let g = theme_geom(client_w, (client_h - SECTION_BAR_H).max(0));
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

/// Geometry shared by the Fonts page drawing and hit testing.
struct FontsGeom {
    x0: i32,
    w: i32,
    val_off: i32,
    y0: i32,
    rule_y: i32,
    y_toggle: i32,
    toggle_x: i32,
    toggle_h: i32,
    toggle_w: i32,
    body_scale: i32,
    head_scale: i32,
    compact: bool,
    y_cs: i32,
    cs_step: i32,
    y_bottom: i32,
    ladder_w: i32,
    native_x: i32,
    smooth_x: i32,
    right_x: i32,
    right_w: i32,
    ladder_rows: [Rect; 8],
}

fn fonts_geom(fb_w: i32, fb_h: i32) -> FontsGeom {
    let (x0, w, val_off, y0, _) = layout_margins(fb_w, fb_h);
    let m = Metrics::new(fb_w, fb_h);
    let head_scale = m.head_scale();
    let body_scale = m.body_scale();
    let compact = m.is_compact();
    let rule_y = y0 + line_height(head_scale) + 2 * head_scale;

    let (toggle_x, y_toggle, toggle_w, toggle_h) = if compact {
        (
            x0,
            rule_y + 8 + glyph_height(body_scale) + 4,
            ((w - 8) / 2).max(80),
            m.pick(24, 26, 28),
        )
    } else {
        let tw = 120;
        let th = 28;
        (x0 + w - 2 * tw - 6, y0 + 3, tw, th)
    };

    let y_cs = rule_y + 10;
    let cs_step = if compact { 16 } else { 20 };
    let divider = y_cs + cs_step * 5;
    let y_bottom = divider + 12;

    let ladder_w = (w * 52 / 100).max(320);
    let ladder_label_w = m.pick(100, 124, 140);
    let col_w = ((ladder_w - ladder_label_w - 10) / 2).max(60);
    let native_x = x0 + ladder_label_w + 10;
    let smooth_x = native_x + col_w;
    let right_x = x0 + ladder_w + 24;
    let right_w = (w - ladder_w - 24).max(160);

    let mut ladder_rows = [Rect::new(0, 0, 0, 0); 8];
    if !compact {
        let mut y = y_bottom + 24;
        for (idx, &(_, sc)) in FONTS_LADDER.iter().enumerate() {
            let h = line_height(sc);
            ladder_rows[idx] = Rect::new(x0, y, ladder_w, h);
            y += h + 8;
        }
    }

    FontsGeom {
        x0,
        w,
        val_off,
        y0,
        rule_y,
        y_toggle,
        toggle_x,
        toggle_h,
        toggle_w,
        body_scale,
        head_scale,
        compact,
        y_cs,
        cs_step,
        y_bottom,
        ladder_w,
        native_x,
        smooth_x,
        right_x,
        right_w,
        ladder_rows,
    }
}

fn render_toggle_rects(g: &FontsGeom) -> [Rect; 2] {
    [
        Rect::new(g.toggle_x, g.y_toggle, g.toggle_w, g.toggle_h),
        Rect::new(g.toggle_x + g.toggle_w + 6, g.y_toggle, g.toggle_w, g.toggle_h),
    ]
}

/// `Rendering: [ Native | Smooth ]` — the mode is global to the guest.
fn draw_render_toggle(fb: &mut Framebuffer, app: &SystemApp, g: &FontsGeom) {
    let label = "Rendering";
    let (label_x, label_y) = if g.compact {
        (g.x0, g.y_toggle - glyph_height(g.body_scale) - 4)
    } else {
        (
            g.toggle_x - text_width_prop(label, g.body_scale) - 10,
            g.y_toggle + (g.toggle_h - glyph_height(g.body_scale)) / 2,
        )
    };
    draw_str_prop(fb, label_x, label_y, label, g.body_scale, t::TEXT_DIM);

    let active = font::render_mode();
    for (idx, mode) in RenderMode::ALL.iter().enumerate() {
        let r = render_toggle_rects(g)[idx];
        ui::button(
            fb,
            r,
            mode.name(),
            if active == *mode {
                ui::Tone::Accent
            } else {
                ui::Tone::Neutral
            },
            g.body_scale,
        );
        if app.focus_visible && app.render_focus == idx {
            ui::focus_ring(fb, r);
        }
    }
}

fn click_fonts(app: &mut SystemApp, cx: i32, cy: i32, client_w: i32, client_h: i32) -> InputResult {
    let g = fonts_geom(client_w, (client_h - SECTION_BAR_H).max(0));
    for (idx, mode) in RenderMode::ALL.iter().enumerate() {
        if render_toggle_rects(&g)[idx].contains(cx, cy) {
            return apply_render_mode(app, *mode);
        }
    }
    if !g.compact {
        for (idx, r) in g.ladder_rows.iter().enumerate() {
            if r.contains(cx, cy) {
                return select_preview(app, idx);
            }
        }
    }
    InputResult::Noop
}

/// Select the size shown in the Specimen panel. Page-local; does not change
/// app text or the global scale.
pub fn select_preview(app: &mut SystemApp, idx: usize) -> InputResult {
    let idx = idx.min(FONTS_LADDER.len() - 1);
    if app.preview_idx == idx {
        return InputResult::Handled;
    }
    app.preview_idx = idx;
    InputResult::Changed
}

/// Absolute scale ladder. 1x is the 5x7 base in both modes; the smooth mask is
/// only generated from 2x up.
pub const FONTS_LADDER: [(&str, i32); 8] = [
    ("1x (5x7)", 1),
    ("2x (10x14)", 2),
    ("3x (15x21)", 3),
    ("4x (20x28)", 4),
    ("5x (25x35)", 5),
    ("6x (30x42)", 6),
    ("7x (35x49)", 7),
    ("8x (40x56)", 8),
];

fn draw_compact_fonts(fb: &mut Framebuffer, app: &SystemApp) {
    let g = fonts_geom(fb.w, fb.h);
    let x0 = g.x0;
    let body = g.body_scale;
    let head = g.head_scale;

    draw_str(fb, x0, g.y0, font::spec(font::font()).id, head, t::TEXT);
    fb.fill(x0, g.rule_y, (fb.w - 2 * x0).max(80), 2, t::ACCENT);

    draw_render_toggle(fb, app, &g);

    let mut y = g.y_toggle + g.toggle_h + 12;
    draw_str_mode(
        fb,
        font::render_mode(),
        x0,
        y,
        "AaBbCc 0123",
        2,
        t::TEXT_BODY,
    );
    y += glyph_height(2) + 12;

    let sample_x = x0 + 136;
    for &(label, sc) in FONTS_LADDER.iter().take(3) {
        let row_h = line_height(sc).max(glyph_height(body));
        draw_str_prop(
            fb,
            x0,
            y + (row_h - glyph_height(body)) / 2,
            label,
            body,
            t::TEXT_DIM,
        );
        draw_str_mode(
            fb,
            font::render_mode(),
            sample_x,
            y + (row_h - glyph_height(sc)) / 2,
            "Aa 0123",
            sc,
            t::TEXT_BODY,
        );
        y += row_h + 8;
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
        if y + glyph_height(2) > fb.h {
            break;
        }
        draw_str_prop(fb, x0, y, name, body, t::TEXT_DIM);
        draw_str_styled(fb, sample_x, y, "Aa 0123", 2, style, t::TEXT_BODY);
        y += glyph_height(body) + 8;
    }
}

fn draw_fonts(fb: &mut Framebuffer, app: &SystemApp) {
    let sheet = font::spec(font::font());
    if fb.w < 560 {
        fb.clear(t::BG);
        draw_compact_fonts(fb, app);
        return;
    }

    fb.clear(t::BG);
    let g = fonts_geom(fb.w, fb.h);
    let (x0, w) = (g.x0, g.w);
    let label_col = t::TEXT_DIM;
    let val_col = t::TEXT_BODY;

    draw_str(fb, x0, g.y0, sheet.id, g.head_scale, t::TEXT);
    let range_x = x0 + text_width(sheet.id, g.head_scale) + 24;
    if range_x + text_width(sheet.range_label, 2) < g.toggle_x - 12 {
        draw_str(fb, range_x, g.y0 + 7, sheet.range_label, 2, label_col);
    }
    fb.fill(x0, g.rule_y, w, 2, t::ACCENT);

    draw_render_toggle(fb, app, &g);

    // Character repertoire, full width.
    let cs: [(&str, &str); 5] = [
        ("Upper", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        ("Lower", "abcdefghijklmnopqrstuvwxyz"),
        (
            "Latin+",
            "¡¿ÀÁÂÃÄÅÆÇÈÉÊËÑÒÓÔÕÖØÙÚÛÜÝßàáéíñóúüąćęłńśźż",
        ),
        (
            "Greek/Cyr",
            "ΑΒΓΔΕΖΗΘΛΞΠΣΦΨΩ αβγδεζηθλμπσφω · АБГДЖЗИЙЛПФЦЧШЩЯ",
        ),
        (
            "Symbols",
            "0123456789 ±×÷≠≤≥√∞∑∫€£¥ •…–— ┌─┬─┐│┼│└─┴─┘░▒▓█★♥",
        ),
    ];
    for (i, (label, sample)) in cs.iter().enumerate() {
        let y = g.y_cs + i as i32 * g.cs_step;
        draw_str(fb, x0, y, label, g.body_scale, label_col);
        draw_sample(
            fb,
            x0 + g.val_off,
            y,
            sample,
            g.body_scale,
            w - g.val_off - 8,
            val_col,
        );
    }
    let divider = g.y_cs + g.cs_step * 5;
    fb.fill(x0, divider, w, 1, t::BORDER_DARK);

    // Size ladder (left) with a selectable row.
    draw_str(fb, x0, g.y_bottom, "Size", g.body_scale, label_col);
    draw_str(
        fb,
        g.native_x,
        g.y_bottom,
        "Native",
        g.body_scale,
        label_col,
    );
    draw_str(
        fb,
        g.smooth_x,
        g.y_bottom,
        "Smooth",
        g.body_scale,
        label_col,
    );
    let sel = app.preview_idx.min(FONTS_LADDER.len() - 1);
    let ladder_sample = if g.ladder_w >= 480 { "Aa0" } else { "Aa" };
    for (idx, &(label, sc)) in FONTS_LADDER.iter().enumerate() {
        let r = g.ladder_rows[idx];
        if r.bottom() > fb.h - 8 {
            break;
        }
        if idx == sel {
            fb.fill(r.x, r.y, r.w, r.h, t::SURFACE_SELECTED);
            fb.fill(r.x, r.y, 4, r.h, t::ACCENT);
        }
        let ly = r.y + (r.h - glyph_height(g.body_scale)).max(0) / 2;
        draw_str(
            fb,
            x0 + 8,
            ly,
            label,
            g.body_scale,
            if idx == sel { t::TEXT } else { label_col },
        );
        let sy = r.y + (r.h - glyph_height(sc)).max(0) / 2;
        draw_str_mode(
            fb,
            RenderMode::Native,
            g.native_x,
            sy,
            ladder_sample,
            sc,
            val_col,
        );
        draw_str_mode(
            fb,
            RenderMode::Smooth,
            g.smooth_x,
            sy,
            ladder_sample,
            sc,
            val_col,
        );
        if app.focus_visible && idx == sel {
            ui::focus_ring(fb, r);
        }
    }

    // Specimen panel (right), driven by the selected ladder row.
    let sel_scale = FONTS_LADDER[sel].1;
    let sel_line = line_height(sel_scale);
    let quote_sc = if g.right_w >= 420 { 3 } else { 2 };
    let quote_h = line_height(quote_sc);
    let panel_h = 24 + sel_line + 8 + quote_h + 12;
    let panel = Rect::new(g.right_x, g.y_bottom, g.right_w, panel_h);
    ui::panel(fb, panel, t::SURFACE, t::BORDER);
    draw_str(
        fb,
        g.right_x + 12,
        g.y_bottom + 6,
        "Specimen",
        g.body_scale,
        label_col,
    );
    let spec_label = FONTS_LADDER[sel].0;
    draw_str_prop(
        fb,
        g.right_x + g.right_w - 12 - text_width_prop(spec_label, g.body_scale),
        g.y_bottom + 6,
        spec_label,
        g.body_scale,
        t::ACCENT,
    );
    draw_sample(
        fb,
        g.right_x + 12,
        g.y_bottom + 24,
        "Aa 0123",
        sel_scale,
        g.right_w - 24,
        t::TEXT,
    );
    let quote = if sel_scale == 1 {
        "1x is the 5x7 base"
    } else {
        "Sphinx of black quartz"
    };
    draw_sample(
        fb,
        g.right_x + 12,
        g.y_bottom + 24 + sel_line + 8,
        quote,
        quote_sc,
        g.right_w - 24,
        t::TEXT_DIM,
    );

    // Styles (right, below the specimen).
    let styles_y = g.y_bottom + panel_h + 14;
    draw_str(
        fb,
        g.right_x,
        styles_y,
        "Styles",
        g.body_scale,
        label_col,
    );
    let style_sc = if g.right_w >= 420 { 3 } else { 2 };
    let style_label_w = 112;
    let style_sample = "Pack my box";
    let max_chars = ((g.right_w - style_label_w) / char_advance(style_sc)).max(4);
    let style_sample = ui::truncate_chars(style_sample, max_chars as usize);
    let styles: [(FontStyle, &str); 5] = [
        (FontStyle::Regular, "Regular"),
        (FontStyle::Bold, "Bold"),
        (FontStyle::Italic, "Italic"),
        (FontStyle::Matrix, "Matrix"),
        (FontStyle::Underline, "Underline"),
    ];
    for (idx, &(style, name)) in styles.iter().enumerate() {
        let ry = styles_y + 20 + idx as i32 * (glyph_height(style_sc) + 8);
        if ry + glyph_height(style_sc) > fb.h - 8 {
            break;
        }
        draw_str_prop(fb, g.right_x, ry, name, g.body_scale, label_col);
        draw_str_styled(
            fb,
            g.right_x + style_label_w,
            ry,
            style_sample,
            style_sc,
            style,
            val_col,
        );
    }

    let notes_y = styles_y + 20 + 5 * (glyph_height(style_sc) + 8) + 8;
    if notes_y + glyph_height(g.body_scale) * 2 + 4 < fb.h - 8 {
        draw_str(
            fb,
            g.right_x,
            notes_y,
            "Preview size is page-local.",
            g.body_scale,
            label_col,
        );
        draw_str(
            fb,
            g.right_x,
            notes_y + glyph_height(g.body_scale) + 4,
            "App text keeps its 2x / 3x scales.",
            g.body_scale,
            label_col,
        );
    }
}

fn draw_row(fb: &mut Framebuffer, x: i32, y: i32, w: i32, val_off: i32, key: &str, val: &str) {
    if y + 20 > fb.h {
        return;
    }
    let scale = 2;
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
    draw_str_prop(fb, x + 48, y + 42, app.section.label(), 2, t::ACCENT);
}
