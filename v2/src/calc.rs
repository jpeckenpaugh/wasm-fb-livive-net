//! Minimalist Calculator Application (`calc.rs`).
//!
//! Reference implementation for a `/v2/` appliance app:
//! - Pure `#![no_std]` state machine (`CalcApp`) with zero heap or OS dependencies.
//! - Draws directly into the maximized client surface (`draw_window`).
//! - Draws a live `128 x 72` miniature preview into the Left App Dock (`draw_dock_tile`).
//! - Accepts client-relative clicks (`click_window`) and logical keys (`key_window`).

use crate::fb::Framebuffer;
use crate::font::{draw_str, draw_str_centered, draw_str_prop, text_width};
use crate::icon::{draw_icon, Icon};
use crate::input::{InputResult, Key};
use crate::layout::{Metrics, Rect};
use crate::theme as t;
use crate::ui;

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
    let m = Metrics::new(client_w, client_h);
    let gap = m.pick(8, 10, 12);
    let max_grid_w = (client_w - 32).clamp(240, 720);
    let bw = ((max_grid_w - 3 * gap) / 4).max(48);
    let grid_w = 4 * bw + 3 * gap;
    let pad_x = ((client_w - grid_w) / 2).max(8);

    let disp_h = m.pick(52, 72, 96);
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
    key: Key,
    label: &'static str,
}

const BUTTONS: [Btn; 18] = [
    Btn {
        col: 0,
        row: 0,
        cspan: 1,
        rspan: 1,
        key: Key::Escape,
        label: "C",
    },
    Btn {
        col: 1,
        row: 0,
        cspan: 1,
        rspan: 1,
        key: Key::Backspace,
        label: "<",
    },
    Btn {
        col: 2,
        row: 0,
        cspan: 1,
        rspan: 1,
        key: Key::Slash,
        label: "/",
    },
    Btn {
        col: 3,
        row: 0,
        cspan: 1,
        rspan: 1,
        key: Key::Star,
        label: "*",
    },
    Btn {
        col: 0,
        row: 1,
        cspan: 1,
        rspan: 1,
        key: Key::Digit7,
        label: "7",
    },
    Btn {
        col: 1,
        row: 1,
        cspan: 1,
        rspan: 1,
        key: Key::Digit8,
        label: "8",
    },
    Btn {
        col: 2,
        row: 1,
        cspan: 1,
        rspan: 1,
        key: Key::Digit9,
        label: "9",
    },
    Btn {
        col: 3,
        row: 1,
        cspan: 1,
        rspan: 1,
        key: Key::Minus,
        label: "-",
    },
    Btn {
        col: 0,
        row: 2,
        cspan: 1,
        rspan: 1,
        key: Key::Digit4,
        label: "4",
    },
    Btn {
        col: 1,
        row: 2,
        cspan: 1,
        rspan: 1,
        key: Key::Digit5,
        label: "5",
    },
    Btn {
        col: 2,
        row: 2,
        cspan: 1,
        rspan: 1,
        key: Key::Digit6,
        label: "6",
    },
    Btn {
        col: 3,
        row: 2,
        cspan: 1,
        rspan: 1,
        key: Key::Plus,
        label: "+",
    },
    Btn {
        col: 0,
        row: 3,
        cspan: 1,
        rspan: 1,
        key: Key::Digit1,
        label: "1",
    },
    Btn {
        col: 1,
        row: 3,
        cspan: 1,
        rspan: 1,
        key: Key::Digit2,
        label: "2",
    },
    Btn {
        col: 2,
        row: 3,
        cspan: 1,
        rspan: 1,
        key: Key::Digit3,
        label: "3",
    },
    Btn {
        col: 3,
        row: 3,
        cspan: 1,
        rspan: 2,
        key: Key::Enter,
        label: "=",
    },
    Btn {
        col: 0,
        row: 4,
        cspan: 2,
        rspan: 1,
        key: Key::Digit0,
        label: "0",
    },
    Btn {
        col: 2,
        row: 4,
        cspan: 1,
        rspan: 1,
        key: Key::Dot,
        label: ".",
    },
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

impl Default for CalcApp {
    fn default() -> Self {
        Self::new()
    }
}

pub fn key_window(app: &mut CalcApp, key: Key) -> InputResult {
    if app.error {
        app.clear();
        if key == Key::Escape {
            return InputResult::Changed;
        }
    }
    match key {
        Key::Digit0
        | Key::Digit1
        | Key::Digit2
        | Key::Digit3
        | Key::Digit4
        | Key::Digit5
        | Key::Digit6
        | Key::Digit7
        | Key::Digit8
        | Key::Digit9 => {
            let digit = key as i32 - Key::Digit0 as i32;
            InputResult::from(app.push_char(b'0' + digit as u8))
        }
        Key::Dot => InputResult::from(app.push_char(b'.')),
        Key::Plus => InputResult::from(app.press_op(b'+')),
        Key::Minus => InputResult::from(app.press_op(b'-')),
        Key::Star => InputResult::from(app.press_op(b'*')),
        Key::Slash => InputResult::from(app.press_op(b'/')),
        Key::Enter => InputResult::from(app.press_eq()),
        Key::Backspace => {
            if app.entry_len == 0 {
                InputResult::Noop
            } else {
                app.entry_len -= 1;
                InputResult::Changed
            }
        }
        Key::Escape => {
            app.clear();
            InputResult::Changed
        }
        _ => InputResult::Noop,
    }
}

pub fn click_window(
    app: &mut CalcApp,
    cx: i32,
    cy: i32,
    client_w: i32,
    client_h: i32,
) -> InputResult {
    let g = calc_geom(client_w, client_h);
    for b in &BUTTONS {
        if btn_rect(&g, b).contains(cx, cy) {
            return key_window(app, b.key);
        }
    }
    InputResult::Noop
}

pub fn draw_window(fb: &mut Framebuffer, app: &CalcApp) {
    // 1. Single flat window surface
    fb.clear(t::BG);
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
            t::ACCENT,
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
        t::TEXT,
    );
    fb.fill(g.pad_x, g.disp_y + g.disp_h - 2, g.grid_w, 2, t::ACCENT);

    // 3. Flat 18-key grid
    let btn_scale = if g.bh < 48 { 2 } else { 3 };
    for b in &BUTTONS {
        let r = btn_rect(&g, b);
        let bg = match b.key {
            Key::Escape => t::CALC_CLEAR,
            Key::Enter => t::CALC_EQ,
            Key::Plus | Key::Minus | Key::Star | Key::Slash => t::CALC_OP,
            _ => t::SURFACE_RAISED,
        };
        ui::panel(fb, r, bg, t::CALC_KEY_BORDER);
        draw_str_centered(fb, r.x, r.y, r.w, r.h, b.label, btn_scale, t::TEXT_BODY);
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
    ui::dock_tile(fb, Rect::new(x, y, w, h), active);
    draw_icon(fb, x + 10, y + 24, Icon::Calculator, 32, t::ACCENT);
    draw_str_prop(fb, x + 48, y + 18, "CALC", 2, t::TEXT);

    let mut disp_buf = [0u8; 24];
    let disp_str = app.format_display(&mut disp_buf);
    let max_chars = 12;
    let clamped = if disp_str.len() > max_chars {
        &disp_str[..max_chars]
    } else {
        disp_str
    };
    draw_str(fb, x + 48, y + 42, clamped, 1, t::ACCENT);
}

fn btn_rect(g: &CalcGeom, b: &Btn) -> Rect {
    let x = g.pad_x + b.col * (g.bw + g.gap);
    let y = g.key_y + b.row * (g.bh + g.gap);
    let w = b.cspan * g.bw + (b.cspan - 1) * g.gap;
    let h = b.rspan * g.bh + (b.rspan - 1) * g.gap;
    Rect::new(x, y, w, h)
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
