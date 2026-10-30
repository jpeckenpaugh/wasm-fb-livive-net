//! The "About" Appliance Application.
//!
//! Renders directly onto the maximized `1280 x 688` application surface (`fb`)
//! with zero nested boxes or redundant chrome. Every datum is shown once.

use crate::fb::{rgb, Framebuffer};
use crate::font::{draw_str, draw_str_centered};

pub const RUSTC_VER: &str = env!("APPLIANCE_RUSTC");
pub const TARGET_TRIPLE: &str = env!("APPLIANCE_TARGET");
pub const BUILD_PROFILE: &str = env!("APPLIANCE_PROFILE");
pub const SRC_HASH: &str = env!("APPLIANCE_SRC_HASH");
pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AboutApp {
    pub snapshots_taken: u32,
}

impl AboutApp {
    pub const fn new() -> Self {
        Self {
            snapshots_taken: 0,
        }
    }
}

/// Draw the maximized `About` window directly into the `1280 x 688` client surface.
pub fn draw_window(fb: &mut Framebuffer, app: &AboutApp) {
    // 1. Single flat window surface
    fb.clear(rgb(0x0b, 0x10, 0x16));

    let x0 = 160;
    let w = fb.w - 2 * x0; // 960px wide column
    let y0 = 96;

    // 2. Single header line + horizontal rule
    draw_str(fb, x0, y0, "WASM-FB APPLIANCE", 3, rgb(0xf0, 0xf6, 0xfc));
    fb.fill(x0, y0 + 36, w, 2, rgb(0x2d, 0xd4, 0xbf));

    // 3. Flat key-value attestation rows (no nested boxes, no repeated session/clock)
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

fn draw_row(fb: &mut Framebuffer, x: i32, y: i32, w: i32, key: &str, val: &str) {
    draw_str(fb, x, y, key, 2, rgb(0x7b, 0x8b, 0x98));

    let max_chars = ((w - 320) / 12).max(8) as usize;
    let v_clamped = if val.len() > max_chars {
        &val[..max_chars]
    } else {
        val
    };
    draw_str(fb, x + 320, y, v_clamped, 2, rgb(0xe6, 0xf1, 0xf8));

    // Subtle 1px divider line under each row
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

/// Draw the `128 x 72` App Icon in the Left Dock.
pub fn draw_dock_tile(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, active: bool) {
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

    // Top accent bar inside icon
    fb.fill(x + 2, y + 2, w - 4, 4, rgb(0x2d, 0xd4, 0xbf));

    draw_str_centered(fb, x, y + 14, w, 24, "ABOUT", 2, rgb(0xf0, 0xf6, 0xfc));
    draw_str_centered(
        fb,
        x,
        y + 42,
        w,
        16,
        PKG_VERSION,
        1,
        rgb(0x7b, 0x8b, 0x98),
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
