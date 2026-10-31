//! The "System" Appliance Application (`system.rs`).
//!
//! Consolidates environment inspection into four flat, non-nested sections
//! selectable from the `SYSTEM` title menu, the bottom section tabs, or keys `1`..`4`:
//! - `0` (`About`): Build & binary attestation
//! - `1` (`Locale`): UTC clock, timezone offset, and language/encoding
//! - `2` (`Fonts`): `FiveSeven` glyph repertoire, scale ladder, and styles
//! - `3` (`Icons`): `Icons32` gallery (32x32, 2-bit), size ladder, inline-in-text demo

use crate::fb::Framebuffer;
use crate::fmt as f;
use crate::font::{
    draw_str, draw_str_centered, draw_str_prop, draw_str_styled, text_width_prop, FontStyle,
    FIVESEVEN_US_LATIN,
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
}

impl SystemSection {
    pub const COUNT: usize = 4;

    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => SystemSection::Locale,
            2 => SystemSection::Fonts,
            3 => SystemSection::Icons,
            _ => SystemSection::About,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SystemSection::About => "ABOUT",
            SystemSection::Locale => "LOCALE",
            SystemSection::Fonts => "FONTS",
            SystemSection::Icons => "ICONS",
        }
    }
}

pub struct SystemApp {
    pub section: SystemSection,
    pub snapshots_taken: u32,
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

pub fn draw_window(fb: &mut Framebuffer, app: &SystemApp, session_w: i32, session_h: i32) {
    let content_h = (fb.h - SECTION_BAR_H).max(0);
    {
        let mut content = fb.sub_rows(0, content_h);
        match app.section {
            SystemSection::About => draw_about(&mut content, app, session_w, session_h),
            SystemSection::Locale => draw_locale(&mut content, app),
            SystemSection::Fonts => draw_fonts(&mut content),
            SystemSection::Icons => draw_icons(&mut content),
        }
    }
    draw_section_bar(fb, content_h, app.section);
}

fn draw_section_bar(fb: &mut Framebuffer, y: i32, active: SystemSection) {
    fb.fill(0, y, fb.w, SECTION_BAR_H, t::BAR);
    ui::separator(fb, 0, y, fb.w);
    let tab_w = (fb.w / SystemSection::COUNT as i32).max(1);
    for idx in 0..SystemSection::COUNT {
        let x = idx as i32 * tab_w;
        let selected = SystemSection::from_index(idx) == active;
        if selected {
            fb.fill(x, y + 1, tab_w, SECTION_BAR_H - 1, t::SURFACE);
            fb.fill(x, y + SECTION_BAR_H - 2, tab_w, 2, t::ACCENT);
        }
        draw_str_centered(
            fb,
            x,
            y + 1,
            tab_w,
            SECTION_BAR_H - 1,
            SystemSection::from_index(idx).label(),
            1,
            if selected { t::TEXT } else { t::TEXT_DIM },
        );
    }
}

/// Click the System section tabs. Content itself is read-only.
pub fn click_window(
    app: &mut SystemApp,
    cx: i32,
    cy: i32,
    client_w: i32,
    client_h: i32,
) -> InputResult {
    if cy < client_h - SECTION_BAR_H {
        return InputResult::Noop;
    }
    let tab_w = (client_w / SystemSection::COUNT as i32).max(1);
    let idx = (cx / tab_w).clamp(0, SystemSection::COUNT as i32 - 1) as usize;
    let target = SystemSection::from_index(idx);
    if app.section == target {
        InputResult::Noop
    } else {
        app.section = target;
        InputResult::Changed
    }
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
            "32X32 2-BIT  U+E000..U+E01F",
            1,
            label_col,
        );
    }
    y += 7 * head_scale + if compact { 10 } else { 18 };

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
        let mono = "\u{E000} HOME  \u{E001} BACK  \u{E004} CANCEL  \u{E015} GAMES";
        draw_str(fb, x0, y, mono, body_scale, ink);
        let y2 = y + 7 * body_scale + 12;
        if y2 + 7 * body_scale < fb.h {
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

fn draw_about(fb: &mut Framebuffer, app: &SystemApp, session_w: i32, session_h: i32) {
    fb.clear(t::BG);

    let (x0, w, val_off, y0, row_h) = layout_margins(fb.w, fb.h);
    let head_scale = Metrics::new(fb.w, fb.h).head_scale();

    draw_str(fb, x0, y0, "ABOUT", head_scale, t::TEXT);
    fb.fill(x0, y0 + 12 * head_scale, w, 2, t::ACCENT);

    let rows_y = y0 + 12 * head_scale + 24;

    draw_row(fb, x0, rows_y, w, val_off, "VERSION", PKG_VERSION);
    draw_row(fb, x0, rows_y + row_h, w, val_off, "TARGET", TARGET_TRIPLE);
    draw_row(
        fb,
        x0,
        rows_y + row_h * 2,
        w,
        val_off,
        "PROFILE",
        BUILD_PROFILE,
    );
    draw_row(
        fb,
        x0,
        rows_y + row_h * 3,
        w,
        val_off,
        "COMPILER",
        RUSTC_VER,
    );
    draw_row(
        fb,
        x0,
        rows_y + row_h * 4,
        w,
        val_off,
        "SOURCE FNV1A",
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
        "FRAMEBUFFER",
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
        "SNAPSHOTS",
        snap_str,
    );
}

fn draw_locale(fb: &mut Framebuffer, app: &SystemApp) {
    fb.clear(t::BG);

    let (x0, w, val_off, y0, row_h) = layout_margins(fb.w, fb.h);
    let head_scale = Metrics::new(fb.w, fb.h).head_scale();

    draw_str(fb, x0, y0, "LOCALE", head_scale, t::TEXT);
    fb.fill(x0, y0 + 12 * head_scale, w, 2, t::ACCENT);

    let rows_y = y0 + 12 * head_scale + 24;

    draw_row(fb, x0, rows_y, w, val_off, "LANGUAGE", "en-US");
    draw_row(
        fb,
        x0,
        rows_y + row_h,
        w,
        val_off,
        "CHARSET",
        "UTF-8 (LATIN / GREEK / CYRILLIC / SYMBOLS)",
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
    draw_row(fb, x0, rows_y + row_h * 2, w, val_off, "UTC TIME", utc_str);

    let mut tz_buf = [b' '; 24];
    let tz_str = f::tz_offset(&mut tz_buf, app.tz_offset_min);
    draw_row(fb, x0, rows_y + row_h * 3, w, val_off, "UTC OFFSET", tz_str);
    draw_row(
        fb,
        x0,
        rows_y + row_h * 4,
        w,
        val_off,
        "SCRIPTS",
        "¡Hola España! · Γεια σας · Привет мир · ┌─┼─┐",
    );
}

fn draw_fonts(fb: &mut Framebuffer) {
    fb.clear(t::BG);

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
    draw_str(fb, x0, y0, FIVESEVEN_US_LATIN.id, head_scale, t::TEXT);
    if fb.w >= 640 {
        draw_str(
            fb,
            x0 + 320,
            y0 + 7,
            FIVESEVEN_US_LATIN.range_label,
            2,
            label_col,
        );
    }
    fb.fill(x0, y0 + 12 * head_scale, w, 2, accent);

    // 1. Character Set
    let y_cs = y0 + 12 * head_scale + (if compact { 10 } else { 18 });
    let cs_step = if compact { 16 } else { 22 };
    draw_str(fb, x0, y_cs, "UPPER", body_scale, label_col);
    draw_str(
        fb,
        x0 + val_off,
        y_cs,
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        body_scale,
        val_col,
    );

    draw_str(fb, x0, y_cs + cs_step, "LOWER", body_scale, label_col);
    draw_str(
        fb,
        x0 + val_off,
        y_cs + cs_step,
        "abcdefghijklmnopqrstuvwxyz",
        body_scale,
        val_col,
    );

    draw_str(fb, x0, y_cs + cs_step * 2, "LATIN+", body_scale, label_col);
    draw_str(
        fb,
        x0 + val_off,
        y_cs + cs_step * 2,
        "¡¿ÀÁÂÃÄÅÆÇÈÉÊËÑÒÓÔÕÖØÙÚÛÜÝßàáéíñóúüąćęłńśźż",
        body_scale,
        val_col,
    );

    draw_str(
        fb,
        x0,
        y_cs + cs_step * 3,
        "GREEK/CYR",
        body_scale,
        label_col,
    );
    draw_str(
        fb,
        x0 + val_off,
        y_cs + cs_step * 3,
        "ΑΒΓΔΕΖΗΘΛΞΠΣΦΨΩ αβγδεζηθλμπσφω · АБГДЖЗИЙЛПФЦЧШЩЯ",
        body_scale,
        val_col,
    );

    draw_str(fb, x0, y_cs + cs_step * 4, "SYMBOLS", body_scale, label_col);
    draw_str(
        fb,
        x0 + val_off,
        y_cs + cs_step * 4,
        "0123456789 ±×÷≠≤≥√∞∑∫€£¥ •…–— ┌─┬─┐│┼│└─┴─┘░▒▓█★♥",
        body_scale,
        val_col,
    );
    let div1_y = y_cs + cs_step * 5;
    fb.fill(x0, div1_y, w, 1, rule_col);

    // 2. Scale Ladder (1x .. 5x)
    let mut y_scale = div1_y + (if compact { 10 } else { 18 });
    let scales: [(i32, &str); 5] = [
        (1, "1X (5X7)"),
        (2, "2X (10X14)"),
        (3, "3X (15X21)"),
        (4, "4X (20X28)"),
        (5, "5X (25X35)"),
    ];
    let scale_gap = if compact { 8 } else { 14 };
    for &(sc, label) in &scales {
        if y_scale + 7 * sc > fb.h - 80 && compact && sc > 3 {
            break;
        }
        let row_h = (7 * sc).max(14);
        draw_str(
            fb,
            x0,
            y_scale + (row_h - 7 * body_scale) / 2,
            label,
            body_scale,
            label_col,
        );
        draw_str(fb, x0 + val_off, y_scale, "FiveSeven 0123", sc, val_col);
        y_scale += row_h + scale_gap;
    }
    fb.fill(x0, y_scale - 2, w, 1, rule_col);

    // 3. Style Transforms (Regular, Bold, Italic, Matrix, Underline)
    let y_style = y_scale + (if compact { 8 } else { 12 });
    let style_sc = if compact { 2 } else { 3 };
    let style_step = if compact { 24 } else { 36 };
    let styles: [(FontStyle, &str); 5] = [
        (FontStyle::Regular, "REGULAR"),
        (FontStyle::Bold, "BOLD"),
        (FontStyle::Italic, "ITALIC"),
        (FontStyle::Matrix, "MATRIX"),
        (FontStyle::Underline, "UNDERLINE"),
    ];
    for (idx, &(style, name)) in styles.iter().enumerate() {
        let ry = y_style + (idx as i32) * style_step;
        if ry + 7 * style_sc > fb.h {
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
    let char_w = 6 * scale;
    let max_chars = ((w - val_off) / char_w).max(6) as usize;
    let v_clamped = ui::truncate_chars(val, max_chars);
    ui::kv_row(
        fb,
        Rect::new(x, y, w, 12 * scale + 6),
        key,
        v_clamped,
        scale,
        t::TEXT_DIM,
        t::TEXT_BODY,
    );
    ui::separator(fb, x, y + 12 * scale + 6, w);
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
    draw_str_prop(fb, x + 48, y + 18, "SYSTEM", 2, t::TEXT);
    draw_str_prop(fb, x + 48, y + 42, app.section.label(), 1, t::ACCENT);
}
