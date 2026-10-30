//! The "System" Appliance Application (`system.rs`).
//!
//! Consolidates environment inspection into three flat, non-nested sections
//! selectable from the `SYSTEM` Title Bar menu (or keys `1`, `2`, `3`):
//! - `0` (`About`): Build & binary attestation
//! - `1` (`Locale`): UTC clock, timezone offset, and language/encoding
//! - `2` (`Fonts`): `FiveSeven` glyph repertoire, scale ladder, and styles

use crate::fb::{rgb, Framebuffer};
use crate::font::{
    draw_str, draw_str_centered, draw_str_styled, FontStyle, FIVESEVEN_US_LATIN,
};

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
}

impl SystemSection {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => SystemSection::Locale,
            2 => SystemSection::Fonts,
            _ => SystemSection::About,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SystemSection::About => "ABOUT",
            SystemSection::Locale => "LOCALE",
            SystemSection::Fonts => "FONTS",
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

pub fn draw_window(fb: &mut Framebuffer, app: &SystemApp, session_w: i32, session_h: i32) {
    match app.section {
        SystemSection::About => draw_about(fb, app, session_w, session_h),
        SystemSection::Locale => draw_locale(fb, app),
        SystemSection::Fonts => draw_fonts(fb),
    }
}

fn layout_margins(fb_w: i32, fb_h: i32) -> (i32, i32, i32, i32, i32) {
    let x0 = if fb_w >= 1100 {
        120
    } else if fb_w >= 640 {
        40
    } else {
        16
    };
    let w = (fb_w - 2 * x0).max(200);
    let val_off = if fb_w >= 900 {
        280
    } else if fb_w >= 600 {
        200
    } else {
        124
    };
    let y0 = if fb_h >= 600 { 64 } else { 20 };
    let row_h = if fb_h >= 600 {
        52
    } else if fb_h >= 400 {
        38
    } else {
        30
    };
    (x0, w, val_off, y0, row_h)
}

fn draw_about(fb: &mut Framebuffer, app: &SystemApp, session_w: i32, session_h: i32) {
    fb.clear(rgb(0x0b, 0x10, 0x16));

    let (x0, w, val_off, y0, row_h) = layout_margins(fb.w, fb.h);
    let head_scale = if fb.w < 480 { 2 } else { 3 };

    draw_str(fb, x0, y0, "ABOUT", head_scale, rgb(0xf0, 0xf6, 0xfc));
    fb.fill(x0, y0 + 12 * head_scale, w, 2, rgb(0x2d, 0xd4, 0xbf));

    let rows_y = y0 + 12 * head_scale + 24;

    draw_row(fb, x0, rows_y, w, val_off, "VERSION", PKG_VERSION);
    draw_row(fb, x0, rows_y + row_h, w, val_off, "TARGET", TARGET_TRIPLE);
    draw_row(fb, x0, rows_y + row_h * 2, w, val_off, "PROFILE", BUILD_PROFILE);
    draw_row(fb, x0, rows_y + row_h * 3, w, val_off, "COMPILER", RUSTC_VER);
    draw_row(fb, x0, rows_y + row_h * 4, w, val_off, "SOURCE FNV1A", SRC_HASH);

    let mut fb_buf = [b' '; 48];
    let fb_str = fmt_fb_spec(&mut fb_buf, session_w as u32, session_h as u32);
    draw_row(fb, x0, rows_y + row_h * 5, w, val_off, "FRAMEBUFFER", fb_str);

    let mut snap_buf = [b' '; 16];
    let snap_str = fmt_u32(&mut snap_buf, app.snapshots_taken);
    draw_row(fb, x0, rows_y + row_h * 6, w, val_off, "SNAPSHOTS", snap_str);
}

fn draw_locale(fb: &mut Framebuffer, app: &SystemApp) {
    fb.clear(rgb(0x0b, 0x10, 0x16));

    let (x0, w, val_off, y0, row_h) = layout_margins(fb.w, fb.h);
    let head_scale = if fb.w < 480 { 2 } else { 3 };

    draw_str(fb, x0, y0, "LOCALE", head_scale, rgb(0xf0, 0xf6, 0xfc));
    fb.fill(x0, y0 + 12 * head_scale, w, 2, rgb(0x2d, 0xd4, 0xbf));

    let rows_y = y0 + 12 * head_scale + 24;

    draw_row(fb, x0, rows_y, w, val_off, "LANGUAGE", "en-US");
    draw_row(
        fb,
        x0,
        rows_y + row_h,
        w,
        val_off,
        "CHARSET",
        "ASCII / BASIC LATIN (U+0020..U+007E)",
    );

    let mut utc_buf = [b' '; 24];
    let utc_str = fmt_datetime(
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
    let tz_str = fmt_tz_offset(&mut tz_buf, app.tz_offset_min);
    draw_row(fb, x0, rows_y + row_h * 3, w, val_off, "UTC OFFSET", tz_str);
}

fn draw_fonts(fb: &mut Framebuffer) {
    fb.clear(rgb(0x0b, 0x10, 0x16));

    let (x0, w, _, _, _) = layout_margins(fb.w, fb.h);
    let val_off = if fb.w >= 800 {
        220
    } else if fb.w >= 540 {
        140
    } else {
        92
    };
    let label_col = rgb(0x7b, 0x8b, 0x98);
    let val_col = rgb(0xe6, 0xf1, 0xf8);
    let accent = rgb(0x2d, 0xd4, 0xbf);
    let rule_col = rgb(0x18, 0x22, 0x2d);
    let compact = fb.h < 520 || fb.w < 540;
    let body_scale = if fb.w < 480 { 1 } else { 2 };
    let head_scale = if compact { 2 } else { 3 };

    let y0 = if compact { 16 } else { 36 };
    draw_str(fb, x0, y0, FIVESEVEN_US_LATIN.id, head_scale, rgb(0xf0, 0xf6, 0xfc));
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
    let y_cs = y0 + 12 * head_scale + (if compact { 12 } else { 24 });
    let cs_step = if compact { 18 } else { 26 };
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

    draw_str(fb, x0, y_cs + cs_step * 2, "SYMBOLS", body_scale, label_col);
    draw_str(
        fb,
        x0 + val_off,
        y_cs + cs_step * 2,
        "0123456789 .,:-/()+=!?<>[]{}@#$%",
        body_scale,
        val_col,
    );
    let div1_y = y_cs + cs_step * 3;
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
        draw_str(fb, x0, y_scale + (row_h - 7 * body_scale) / 2, label, body_scale, label_col);
        draw_str(fb, x0 + val_off, y_scale, "FiveSeven 0123", sc, val_col);
        y_scale += row_h + scale_gap;
    }
    fb.fill(x0, y_scale - 2, w, 1, rule_col);

    // 3. Style Transforms (Regular, Bold, Italic, Matrix, Underline)
    let y_style = y_scale + (if compact { 8 } else { 12 });
    let style_sc = if compact { 2 } else { 3 };
    let style_step = if compact { 24 } else { 36 };
    let styles: [(FontStyle, &str); 5] = [
        (FontStyle::Regular,   "REGULAR"),
        (FontStyle::Bold,      "BOLD"),
        (FontStyle::Italic,    "ITALIC"),
        (FontStyle::Matrix,    "MATRIX"),
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
    draw_str(fb, x, y, key, scale, rgb(0x7b, 0x8b, 0x98));

    let max_chars = ((w - val_off) / char_w).max(6) as usize;
    let v_clamped = if val.len() > max_chars {
        &val[..max_chars]
    } else {
        val
    };
    draw_str(fb, x + val_off, y, v_clamped, scale, rgb(0xe6, 0xf1, 0xf8));
    fb.fill(x, y + 12 * scale + 6, w, 1, rgb(0x18, 0x22, 0x2d));
}

pub fn draw_desktop_backdrop(fb: &mut Framebuffer) {
    fb.clear(rgb(0x09, 0x0d, 0x12));

    let step = 32;
    let grid_col = rgb(0x0f, 0x16, 0x1f);
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
    let bg = if active {
        rgb(0x13, 0x22, 0x2b)
    } else {
        rgb(0x10, 0x17, 0x20)
    };
    let border = if active {
        rgb(0x2d, 0xd4, 0xbf)
    } else {
        rgb(0x2e, 0x3f, 0x50)
    };
    fb.fill(x, y, w, h, bg);
    fb.border(x, y, w, h, 2, border);
    fb.fill(x + 2, y + 2, w - 4, 4, rgb(0x2d, 0xd4, 0xbf));

    draw_str_centered(fb, x, y + 14, w, 24, "SYSTEM", 2, rgb(0xf0, 0xf6, 0xfc));
    draw_str_centered(
        fb,
        x,
        y + 42,
        w,
        16,
        app.section.label(),
        1,
        rgb(0x2d, 0xd4, 0xbf),
    );
}

fn fmt_fb_spec(buf: &mut [u8; 48], w: u32, h: u32) -> &str {
    let mut pos = 0;
    pos = write_u32(buf, pos, w);
    buf[pos] = b'X';
    pos += 1;
    pos = write_u32(buf, pos, h);
    for &b in b" RGBA8888 (" {
        buf[pos] = b;
        pos += 1;
    }
    let bytes = w.saturating_mul(h).saturating_mul(4);
    pos = write_u32(buf, pos, bytes);
    for &b in b" B)" {
        buf[pos] = b;
        pos += 1;
    }
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

fn write_u32(buf: &mut [u8], mut pos: usize, mut val: u32) -> usize {
    if val == 0 {
        if pos < buf.len() {
            buf[pos] = b'0';
            pos += 1;
        }
        return pos;
    }
    let mut tmp = [0u8; 10];
    let mut n = 0;
    while val > 0 {
        tmp[n] = b'0' + (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    while n > 0 && pos < buf.len() {
        n -= 1;
        buf[pos] = tmp[n];
        pos += 1;
    }
    pos
}

fn fmt_u32(buf: &mut [u8; 16], val: u32) -> &str {
    let pos = write_u32(buf, 0, val);
    core::str::from_utf8(&buf[..pos]).unwrap_or("0")
}

fn fmt_datetime(
    buf: &mut [u8; 24],
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    min: u8,
    sec: u8,
) -> &str {
    buf[0] = b'0' + ((year / 1000) % 10) as u8;
    buf[1] = b'0' + ((year / 100) % 10) as u8;
    buf[2] = b'0' + ((year / 10) % 10) as u8;
    buf[3] = b'0' + (year % 10) as u8;
    buf[4] = b'-';
    buf[5] = b'0' + (month / 10) % 10;
    buf[6] = b'0' + (month % 10);
    buf[7] = b'-';
    buf[8] = b'0' + (day / 10) % 10;
    buf[9] = b'0' + (day % 10);
    buf[10] = b' ';
    buf[11] = b'0' + (hour / 10) % 10;
    buf[12] = b'0' + (hour % 10);
    buf[13] = b':';
    buf[14] = b'0' + (min / 10) % 10;
    buf[15] = b'0' + (min % 10);
    buf[16] = b':';
    buf[17] = b'0' + (sec / 10) % 10;
    buf[18] = b'0' + (sec % 10);
    core::str::from_utf8(&buf[..19]).unwrap_or("")
}

fn fmt_tz_offset(buf: &mut [u8; 24], offset_min: i16) -> &str {
    let mut pos = 0;
    for &b in b"UTC" {
        buf[pos] = b;
        pos += 1;
    }
    let sign = if offset_min < 0 { b'-' } else { b'+' };
    let abs_min = offset_min.unsigned_abs();
    let hh = (abs_min / 60) as u8;
    let mm = (abs_min % 60) as u8;
    buf[pos] = sign;
    buf[pos + 1] = b'0' + (hh / 10) % 10;
    buf[pos + 2] = b'0' + (hh % 10);
    buf[pos + 3] = b':';
    buf[pos + 4] = b'0' + (mm / 10) % 10;
    buf[pos + 5] = b'0' + (mm % 10);
    pos += 6;
    core::str::from_utf8(&buf[..pos]).unwrap_or("UTC+00:00")
}
