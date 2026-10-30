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

pub fn draw_window(fb: &mut Framebuffer, app: &SystemApp) {
    match app.section {
        SystemSection::About => draw_about(fb, app),
        SystemSection::Locale => draw_locale(fb, app),
        SystemSection::Fonts => draw_fonts(fb),
    }
}

fn draw_about(fb: &mut Framebuffer, app: &SystemApp) {
    fb.clear(rgb(0x0b, 0x10, 0x16));

    let x0 = 160;
    let w = fb.w - 2 * x0;
    let y0 = 96;

    draw_str(fb, x0, y0, "ABOUT", 3, rgb(0xf0, 0xf6, 0xfc));
    fb.fill(x0, y0 + 36, w, 2, rgb(0x2d, 0xd4, 0xbf));

    let rows_y = y0 + 68;
    let row_h = 56;

    draw_row(fb, x0, rows_y, w, "VERSION", PKG_VERSION);
    draw_row(fb, x0, rows_y + row_h, w, "TARGET", TARGET_TRIPLE);
    draw_row(fb, x0, rows_y + row_h * 2, w, "PROFILE", BUILD_PROFILE);
    draw_row(fb, x0, rows_y + row_h * 3, w, "COMPILER", RUSTC_VER);
    draw_row(fb, x0, rows_y + row_h * 4, w, "SOURCE FNV1A-64", SRC_HASH);
    draw_row(
        fb,
        x0,
        rows_y + row_h * 5,
        w,
        "FRAMEBUFFER",
        "1280X720 RGBA8888 (3,686,400 B)",
    );

    let mut snap_buf = [b' '; 16];
    let snap_str = fmt_u32(&mut snap_buf, app.snapshots_taken);
    draw_row(fb, x0, rows_y + row_h * 6, w, "SNAPSHOTS", snap_str);
}

fn draw_locale(fb: &mut Framebuffer, app: &SystemApp) {
    fb.clear(rgb(0x0b, 0x10, 0x16));

    let x0 = 160;
    let w = fb.w - 2 * x0;
    let y0 = 96;

    draw_str(fb, x0, y0, "LOCALE", 3, rgb(0xf0, 0xf6, 0xfc));
    fb.fill(x0, y0 + 36, w, 2, rgb(0x2d, 0xd4, 0xbf));

    let rows_y = y0 + 68;
    let row_h = 56;

    draw_row(fb, x0, rows_y, w, "LANGUAGE", "en-US");
    draw_row(fb, x0, rows_y + row_h, w, "CHARACTER SET", "ASCII / BASIC LATIN (U+0020..U+007E)");

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
    draw_row(fb, x0, rows_y + row_h * 2, w, "UTC TIME", utc_str);

    let mut tz_buf = [b' '; 24];
    let tz_str = fmt_tz_offset(&mut tz_buf, app.tz_offset_min);
    draw_row(fb, x0, rows_y + row_h * 3, w, "UTC OFFSET", tz_str);
}

fn draw_fonts(fb: &mut Framebuffer) {
    fb.clear(rgb(0x0b, 0x10, 0x16));

    let x0 = 160;
    let w = fb.w - 2 * x0;
    let label_col = rgb(0x7b, 0x8b, 0x98);
    let val_col = rgb(0xe6, 0xf1, 0xf8);
    let accent = rgb(0x2d, 0xd4, 0xbf);
    let rule_col = rgb(0x18, 0x22, 0x2d);

    let y0 = 44;
    draw_str(fb, x0, y0, FIVESEVEN_US_LATIN.id, 3, rgb(0xf0, 0xf6, 0xfc));
    draw_str(fb, x0 + 320, y0 + 7, FIVESEVEN_US_LATIN.range_label, 2, label_col);
    fb.fill(x0, y0 + 36, w, 2, accent);

    // 1. Character Set (all 95 printable glyphs at 2x)
    let y_cs = 100;
    draw_str(fb, x0, y_cs, "UPPER", 2, label_col);
    draw_str(fb, x0 + 220, y_cs, "ABCDEFGHIJKLMNOPQRSTUVWXYZ", 2, val_col);

    draw_str(fb, x0, y_cs + 28, "LOWER", 2, label_col);
    draw_str(fb, x0 + 220, y_cs + 28, "abcdefghijklmnopqrstuvwxyz", 2, val_col);

    draw_str(fb, x0, y_cs + 56, "SYMBOLS", 2, label_col);
    draw_str(
        fb,
        x0 + 220,
        y_cs + 56,
        "0123456789 .,:-/()+=!?<>[]{}@#$%^&*~_|\\;\"'`",
        2,
        val_col,
    );
    fb.fill(x0, y_cs + 84, w, 1, rule_col);

    // 2. Scale Ladder (1x .. 5x)
    let mut y_scale = 202;
    let scales: [(i32, &str); 5] = [
        (1, "1X (5X7)"),
        (2, "2X (10X14)"),
        (3, "3X (15X21)"),
        (4, "4X (20X28)"),
        (5, "5X (25X35)"),
    ];
    for &(sc, label) in &scales {
        let row_h = (7 * sc).max(14);
        draw_str(fb, x0, y_scale + (row_h - 14) / 2, label, 2, label_col);
        draw_str(fb, x0 + 220, y_scale, "FiveSeven 0123456789", sc, val_col);
        y_scale += row_h + 16;
    }
    fb.fill(x0, y_scale - 4, w, 1, rule_col);

    // 3. Style Transforms (Regular, Bold, Italic, Matrix, Underline) at 3x
    let y_style = y_scale + 12;
    let styles: [(FontStyle, &str); 5] = [
        (FontStyle::Regular,   "REGULAR"),
        (FontStyle::Bold,      "BOLD"),
        (FontStyle::Italic,    "ITALIC"),
        (FontStyle::Matrix,    "MATRIX"),
        (FontStyle::Underline, "UNDERLINE"),
    ];
    for (idx, &(style, name)) in styles.iter().enumerate() {
        let ry = y_style + (idx as i32) * 38;
        draw_str(fb, x0, ry + 4, name, 2, label_col);
        draw_str_styled(
            fb,
            x0 + 220,
            ry,
            "Pack my box with five dozen jugs",
            3,
            style,
            val_col,
        );
    }
}

fn draw_row(fb: &mut Framebuffer, x: i32, y: i32, w: i32, key: &str, val: &str) {
    draw_str(fb, x, y, key, 2, rgb(0x7b, 0x8b, 0x98));

    let max_chars = ((w - 320) / 12).max(8) as usize;
    let v_clamped = if val.len() > max_chars {
        &val[..max_chars]
    } else {
        val
    };
    draw_str(fb, x + 320, y, v_clamped, 2, rgb(0xe6, 0xf1, 0xf8));
    fb.fill(x, y + 32, w, 1, rgb(0x18, 0x22, 0x2d));
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

fn fmt_u32(buf: &mut [u8; 16], mut val: u32) -> &str {
    if val == 0 {
        buf[0] = b'0';
        return core::str::from_utf8(&buf[..1]).unwrap_or("0");
    }
    let mut tmp = [0u8; 10];
    let mut n = 0;
    while val > 0 {
        tmp[n] = b'0' + (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    let mut pos = 0;
    while n > 0 {
        n -= 1;
        buf[pos] = tmp[n];
        pos += 1;
    }
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
