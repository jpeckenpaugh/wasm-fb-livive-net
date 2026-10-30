//! Minimalist Calculator Application (`calc.rs`).
//!
//! Reference implementation for a `/v2/` appliance app:
//! - Pure `#![no_std]` state machine (`CalcApp`) with zero heap or OS dependencies.
//! - Draws directly into the maximized `1280 x 688` client surface (`draw_window`).
//! - Draws a live `128 x 72` miniature preview into the Left App Dock (`draw_dock_tile`).
//! - Accepts client-relative clicks (`click_window`) and key codes (`key_window`).

use crate::fb::{rgb, Framebuffer};
use crate::font::{draw_str, draw_str_centered, text_width};

pub const KEY_0: i32 = 10;
pub const KEY_PLUS: i32 = 20;
pub const KEY_MINUS: i32 = 21;
pub const KEY_MUL: i32 = 22;
pub const KEY_DIV: i32 = 23;
pub const KEY_EQ: i32 = 24;
pub const KEY_CLEAR: i32 = 25;
pub const KEY_BACK: i32 = 26;
pub const KEY_DOT: i32 = 27;

struct CalcGeom {
    pad_x: i32,
    bw: i32,
    bh: i32,
    gap: i32,
    grid_w: i32,
    disp_y: i32,
    disp_h: i32,
    key_y: i32,
}

fn calc_geom(client_w: i32, client_h: i32) -> CalcGeom {
    let gap = if client_w < 500 || client_h < 450 { 8 } else { 12 };
    let max_grid_w = (client_w - 32).clamp(240, 720);
    let bw = ((max_grid_w - 3 * gap) / 4).max(48);
    let grid_w = 4 * bw + 3 * gap;
    let pad_x = ((client_w - grid_w) / 2).max(8);

    let disp_h = if client_h < 450 {
        52
    } else if client_h < 600 {
        72
    } else {
        96
    };
    let avail_h = (client_h - disp_h - 32).max(200);
    let bh = ((avail_h - 4 * gap) / 5).clamp(32, 92);
    let total_h = disp_h + 20 + 5 * bh + 4 * gap;
    let disp_y = ((client_h - total_h) / 2).max(12);
    let key_y = disp_y + disp_h + 20;

    CalcGeom {
        pad_x,
        bw,
        bh,
        gap,
        grid_w,
        disp_y,
        disp_h,
        key_y,
    }
}

struct Btn {
    col: i32,
    row: i32,
    cspan: i32,
    rspan: i32,
    key: i32,
    label: &'static str,
}

const BUTTONS: [Btn; 18] = [
    Btn { col: 0, row: 0, cspan: 1, rspan: 1, key: KEY_CLEAR, label: "C" },
    Btn { col: 1, row: 0, cspan: 1, rspan: 1, key: KEY_BACK,  label: "<" },
    Btn { col: 2, row: 0, cspan: 1, rspan: 1, key: KEY_DIV,   label: "/" },
    Btn { col: 3, row: 0, cspan: 1, rspan: 1, key: KEY_MUL,   label: "*" },

    Btn { col: 0, row: 1, cspan: 1, rspan: 1, key: KEY_0 + 7, label: "7" },
    Btn { col: 1, row: 1, cspan: 1, rspan: 1, key: KEY_0 + 8, label: "8" },
    Btn { col: 2, row: 1, cspan: 1, rspan: 1, key: KEY_0 + 9, label: "9" },
    Btn { col: 3, row: 1, cspan: 1, rspan: 1, key: KEY_MINUS, label: "-" },

    Btn { col: 0, row: 2, cspan: 1, rspan: 1, key: KEY_0 + 4, label: "4" },
    Btn { col: 1, row: 2, cspan: 1, rspan: 1, key: KEY_0 + 5, label: "5" },
    Btn { col: 2, row: 2, cspan: 1, rspan: 1, key: KEY_0 + 6, label: "6" },
    Btn { col: 3, row: 2, cspan: 1, rspan: 1, key: KEY_PLUS,  label: "+" },

    Btn { col: 0, row: 3, cspan: 1, rspan: 1, key: KEY_0 + 1, label: "1" },
    Btn { col: 1, row: 3, cspan: 1, rspan: 1, key: KEY_0 + 2, label: "2" },
    Btn { col: 2, row: 3, cspan: 1, rspan: 1, key: KEY_0 + 3, label: "3" },
    Btn { col: 3, row: 3, cspan: 1, rspan: 2, key: KEY_EQ,    label: "=" },

    Btn { col: 0, row: 4, cspan: 2, rspan: 1, key: KEY_0,     label: "0" },
    Btn { col: 2, row: 4, cspan: 1, rspan: 1, key: KEY_DOT,   label: "." },
];

pub struct CalcApp {
    pub acc: f64,
    pub pending: u8,
    pub entry: [u8; 12],
    pub entry_len: usize,
    pub error: bool,
}

impl CalcApp {
    pub const fn new() -> Self {
        Self {
            acc: 0.0,
            pending: 0,
            entry: [0; 12],
            entry_len: 0,
            error: false,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    fn clear_entry(&mut self) {
        self.entry_len = 0;
    }

    fn push_char(&mut self, ch: u8) -> bool {
        if self.entry_len >= 10 {
            return false;
        }
        if ch == b'.' {
            if self.entry[..self.entry_len].contains(&b'.') {
                return false;
            }
            if self.entry_len == 0 {
                self.entry[0] = b'0';
                self.entry_len = 1;
            }
        }
        self.entry[self.entry_len] = ch;
        self.entry_len += 1;
        true
    }

    fn entry_value(&self) -> f64 {
        if self.entry_len == 0 {
            return 0.0;
        }
        let mut int_part: f64 = 0.0;
        let mut frac_part: f64 = 0.0;
        let mut frac_div: f64 = 1.0;
        let mut in_frac = false;

        for &b in &self.entry[..self.entry_len] {
            if b == b'.' {
                in_frac = true;
            } else if b.is_ascii_digit() {
                let d = (b - b'0') as f64;
                if in_frac {
                    frac_div *= 10.0;
                    frac_part += d / frac_div;
                } else {
                    int_part = int_part * 10.0 + d;
                }
            }
        }
        int_part + frac_part
    }

    fn apply(&mut self, op: u8, rhs: f64) -> bool {
        let out = match op {
            b'+' => self.acc + rhs,
            b'-' => self.acc - rhs,
            b'*' => self.acc * rhs,
            b'/' => {
                if rhs == 0.0 {
                    return false;
                }
                self.acc / rhs
            }
            _ => rhs,
        };
        if !out.is_finite() || out.abs() >= 1_000_000_000.0 {
            return false;
        }
        self.acc = out;
        true
    }

    fn press_op(&mut self, op: u8) -> bool {
        if self.entry_len > 0 {
            let rhs = self.entry_value();
            if !self.apply(self.pending, rhs) {
                self.error = true;
                return true;
            }
            self.clear_entry();
        }
        self.pending = op;
        true
    }

    fn press_eq(&mut self) -> bool {
        if self.entry_len > 0 {
            let rhs = self.entry_value();
            if !self.apply(self.pending, rhs) {
                self.error = true;
                return true;
            }
            self.clear_entry();
        } else if self.pending != 0 && !self.apply(self.pending, self.acc) {
            self.error = true;
            return true;
        }
        self.pending = 0;
        true
    }

    /// Format current display value (with thousands separators) into `out`.
    pub fn format_display<'a>(&self, out: &'a mut [u8; 24]) -> &'a str {
        if self.error {
            out[0] = b'E';
            return core::str::from_utf8(&out[..1]).unwrap_or("E");
        }
        let mut raw = [0u8; 20];
        let raw_len = if self.entry_len > 0 {
            raw[..self.entry_len].copy_from_slice(&self.entry[..self.entry_len]);
            self.entry_len
        } else {
            format_f64(self.acc, &mut raw)
        };
        group_thousands(&raw[..raw_len], out)
    }
}

pub fn key_window(app: &mut CalcApp, key: i32) -> bool {
    if app.error {
        app.clear();
        if key == KEY_CLEAR {
            return true;
        }
    }
    if (KEY_0..=KEY_0 + 9).contains(&key) {
        return app.push_char(b'0' + (key - KEY_0) as u8);
    }
    match key {
        KEY_DOT => app.push_char(b'.'),
        KEY_PLUS => app.press_op(b'+'),
        KEY_MINUS => app.press_op(b'-'),
        KEY_MUL => app.press_op(b'*'),
        KEY_DIV => app.press_op(b'/'),
        KEY_EQ => app.press_eq(),
        KEY_BACK => {
            if app.entry_len == 0 {
                false
            } else {
                app.entry_len -= 1;
                true
            }
        }
        KEY_CLEAR => {
            app.clear();
            true
        }
        _ => false,
    }
}

pub fn click_window(app: &mut CalcApp, cx: i32, cy: i32, client_w: i32, client_h: i32) -> bool {
    let g = calc_geom(client_w, client_h);
    for b in &BUTTONS {
        let (bx, by, bw, bh) = btn_rect(&g, b);
        if cx >= bx && cx < bx + bw && cy >= by && cy < by + bh {
            return key_window(app, b.key);
        }
    }
    false
}

pub fn draw_window(fb: &mut Framebuffer, app: &CalcApp) {
    // 1. Single flat window surface
    fb.clear(rgb(0x0b, 0x10, 0x16));
    let g = calc_geom(fb.w, fb.h);

    // 2. Readout line (pending operator on left, value right-aligned, 2px underline)
    let op_scale = if g.disp_h < 64 { 3 } else { 4 };
    if app.pending != 0 && !app.error {
        let op_buf = [app.pending];
        let op_str = core::str::from_utf8(&op_buf).unwrap_or("");
        draw_str(
            fb,
            g.pad_x + 8,
            g.disp_y + (g.disp_h - 7 * op_scale) / 2,
            op_str,
            op_scale,
            rgb(0x2d, 0xd4, 0xbf),
        );
    }

    let mut disp_buf = [0u8; 24];
    let disp_str = app.format_display(&mut disp_buf);
    let scale = if g.grid_w < 360 || g.disp_h < 64 {
        4
    } else if g.grid_w < 520 {
        5
    } else {
        6
    };
    let tw = text_width(disp_str, scale);
    draw_str(
        fb,
        g.pad_x + g.grid_w - tw,
        g.disp_y + (g.disp_h - 7 * scale) / 2,
        disp_str,
        scale,
        rgb(0xf0, 0xf6, 0xfc),
    );
    fb.fill(
        g.pad_x,
        g.disp_y + g.disp_h - 2,
        g.grid_w,
        2,
        rgb(0x2d, 0xd4, 0xbf),
    );

    // 3. Flat 18-key grid
    let btn_scale = if g.bh < 48 { 2 } else { 3 };
    for b in &BUTTONS {
        let (bx, by, bw, bh) = btn_rect(&g, b);
        let bg = match b.key {
            KEY_CLEAR => rgb(0x2e, 0x17, 0x1d),
            KEY_EQ => rgb(0x10, 0x34, 0x37),
            KEY_PLUS | KEY_MINUS | KEY_MUL | KEY_DIV => rgb(0x12, 0x26, 0x2c),
            _ => rgb(0x14, 0x1d, 0x26),
        };
        fb.fill(bx, by, bw, bh, bg);
        fb.border(bx, by, bw, bh, 1, rgb(0x24, 0x32, 0x40));
        draw_str_centered(fb, bx, by, bw, bh, b.label, btn_scale, rgb(0xe6, 0xf1, 0xf8));
    }
}

/// Draw the `128 x 72` live icon in the Left App Dock.
pub fn draw_dock_tile(
    fb: &mut Framebuffer,
    app: &CalcApp,
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

    draw_str_centered(fb, x, y + 14, w, 24, "CALC", 2, rgb(0xf0, 0xf6, 0xfc));

    let mut disp_buf = [0u8; 24];
    let disp_str = app.format_display(&mut disp_buf);
    let max_chars = 16;
    let clamped = if disp_str.len() > max_chars {
        &disp_str[..max_chars]
    } else {
        disp_str
    };
    draw_str_centered(fb, x, y + 42, w, 16, clamped, 1, rgb(0x2d, 0xd4, 0xbf));
}

fn btn_rect(g: &CalcGeom, b: &Btn) -> (i32, i32, i32, i32) {
    let x = g.pad_x + b.col * (g.bw + g.gap);
    let y = g.key_y + b.row * (g.bh + g.gap);
    let w = b.cspan * g.bw + (b.cspan - 1) * g.gap;
    let h = b.rspan * g.bh + (b.rspan - 1) * g.gap;
    (x, y, w, h)
}

/// Format a finite `f64` with up to 3 decimal places, stripping trailing zeros.
fn format_f64(val: f64, out: &mut [u8; 20]) -> usize {
    if !val.is_finite() || val.abs() >= 1_000_000_000.0 {
        out[0] = b'E';
        return 1;
    }
    let neg = val < -0.000_05;
    let abs_scaled = ((if val < 0.0 { -val } else { val }) * 1000.0 + 0.5) as u64;
    if abs_scaled == 0 {
        out[0] = b'0';
        return 1;
    }

    let int_part = abs_scaled / 1000;
    let frac_part = (abs_scaled % 1000) as u32;

    let mut pos = 0;
    if neg {
        out[pos] = b'-';
        pos += 1;
    }

    if int_part == 0 {
        out[pos] = b'0';
        pos += 1;
    } else {
        let mut tmp = [0u8; 12];
        let mut n = 0;
        let mut v = int_part;
        while v > 0 {
            tmp[n] = b'0' + (v % 10) as u8;
            v /= 10;
            n += 1;
        }
        while n > 0 {
            n -= 1;
            out[pos] = tmp[n];
            pos += 1;
        }
    }

    if frac_part > 0 {
        out[pos] = b'.';
        pos += 1;
        out[pos] = b'0' + ((frac_part / 100) % 10) as u8;
        out[pos + 1] = b'0' + ((frac_part / 10) % 10) as u8;
        out[pos + 2] = b'0' + (frac_part % 10) as u8;
        pos += 3;
        while pos > 0 && out[pos - 1] == b'0' {
            pos -= 1;
        }
        if pos > 0 && out[pos - 1] == b'.' {
            pos -= 1;
        }
    }
    pos
}

/// Insert commas every 3 integer digits ("1234567.89" -> "1,234,567.89").
fn group_thousands<'a>(raw: &[u8], out: &'a mut [u8; 24]) -> &'a str {
    if raw.is_empty() {
        out[0] = b'0';
        return core::str::from_utf8(&out[..1]).unwrap_or("0");
    }
    let sign = usize::from(raw[0] == b'-');
    let dot_idx = raw.iter().position(|&b| b == b'.').unwrap_or(raw.len());
    let digits = dot_idx.saturating_sub(sign);

    let mut pos = 0;
    if sign == 1 {
        out[pos] = b'-';
        pos += 1;
    }
    for i in 0..digits {
        if i > 0 && (digits - i) % 3 == 0 && pos < out.len() {
            out[pos] = b',';
            pos += 1;
        }
        if pos < out.len() {
            out[pos] = raw[sign + i];
            pos += 1;
        }
    }
    for &b in &raw[dot_idx..] {
        if pos < out.len() {
            out[pos] = b;
            pos += 1;
        }
    }
    core::str::from_utf8(&out[..pos]).unwrap_or("0")
}
