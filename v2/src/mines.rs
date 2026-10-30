//! Minimalist 8×8 Minesweeper Application (`mines.rs`) powered by `mine_sm`.
//!
//! Flat, single-surface layout inside the maximized client surface:
//! - Crisp 8×8 bitboard grid (`64` cells)
//! - Clean telemetry & controls column (or top/bottom strip in portrait viewports)
//! - Supports `ActionType::Reveal` and `ActionType::Flag`, seed cycling, and
//!   Classic (`max_strikes: 1`) vs. Shielded (`max_strikes: 3`) rules.

use crate::fb::{rgb, Framebuffer};
use crate::font::{draw_str, draw_str_centered, text_width};
use crate::mine_sm::{Action, ActionType, BoardState, Rules};

pub const DEFAULT_SEED: u64 = 42;
pub const DEFAULT_BOMBS: u8 = 10;

pub struct MinesApp {
    pub board: BoardState,
    pub flag_mode: bool,
}

impl MinesApp {
    pub const fn new_const() -> Self {
        Self {
            board: BoardState {
                seed: DEFAULT_SEED,
                num_bombs: DEFAULT_BOMBS,
                move_count: 0,
                strikes: 0,
                rules: Rules { max_strikes: 1 },
                mine_mask: 0,
                adj_counts: [0; 64],
                revealed_mask: 0,
                flagged_mask: 0,
                detonated_mask: 0,
                is_terminal: false,
                is_won: false,
            },
            flag_mode: false,
        }
    }

    pub fn init_default(&mut self) {
        self.board = BoardState::new(DEFAULT_SEED, DEFAULT_BOMBS, Rules { max_strikes: 1 });
        self.flag_mode = false;
    }

    pub fn reset_seed(&mut self, seed: u64) {
        let rules = self.board.rules;
        let bombs = self.board.num_bombs;
        self.board = BoardState::new(seed, bombs, rules);
    }

    pub fn next_seed(&mut self) {
        let next = self.board.seed.wrapping_add(1);
        self.reset_seed(next);
    }

    pub fn toggle_mode(&mut self) {
        self.flag_mode = !self.flag_mode;
    }

    pub fn toggle_rules(&mut self) {
        let next_strikes = if self.board.rules.max_strikes == 1 {
            3
        } else {
            1
        };
        self.board = BoardState::new(
            self.board.seed,
            self.board.num_bombs,
            Rules {
                max_strikes: next_strikes,
            },
        );
    }

    pub fn status_str(&self) -> &'static str {
        if self.board.is_terminal {
            if self.board.is_won {
                "CLEARED (WON)"
            } else {
                "DETONATED"
            }
        } else if self.board.move_count == 0 {
            "READY"
        } else {
            "PLAYING"
        }
    }
}

struct MinesGeom {
    board_x: i32,
    board_y: i32,
    cell: i32,
    gap: i32,
    grid_px: i32,
    info_x: i32,
    info_y: i32,
    info_w: i32,
    btn_mode: (i32, i32, i32, i32),
    btn_seed: (i32, i32, i32, i32),
    btn_rule: (i32, i32, i32, i32),
}

fn mines_geom(client_w: i32, client_h: i32) -> MinesGeom {
    let landscape = client_w >= client_h && client_w >= 600;
    let gap = if client_w < 500 || client_h < 450 { 3 } else { 4 };

    if landscape {
        let max_board_h = client_h - 48;
        let max_board_w = (client_w * 3 / 5) - 48;
        let avail = max_board_h.min(max_board_w).max(200);
        let cell = ((avail - 7 * gap) / 8).clamp(24, 72);
        let grid_px = 8 * cell + 7 * gap;

        let info_w = (client_w - grid_px - 96).clamp(200, 360);
        let total_w = grid_px + 48 + info_w;
        let board_x = ((client_w - total_w) / 2).max(16);
        let board_y = ((client_h - grid_px) / 2).max(16);

        let info_x = board_x + grid_px + 48;
        let info_y = board_y;
        let btn_h = if cell < 40 { 34 } else { 42 };
        let btn_y0 = info_y + (grid_px - (3 * btn_h + 20)).max(160);

        MinesGeom {
            board_x,
            board_y,
            cell,
            gap,
            grid_px,
            info_x,
            info_y,
            info_w,
            btn_mode: (info_x, btn_y0, info_w, btn_h),
            btn_seed: (info_x, btn_y0 + btn_h + 10, info_w, btn_h),
            btn_rule: (info_x, btn_y0 + 2 * (btn_h + 10), info_w, btn_h),
        }
    } else {
        // Portrait / compact layout: board on top, telemetry + buttons below
        let avail_w = client_w - 32;
        let avail_h = (client_h * 58 / 100).max(200);
        let avail = avail_w.min(avail_h);
        let cell = ((avail - 7 * gap) / 8).clamp(24, 68);
        let grid_px = 8 * cell + 7 * gap;

        let board_x = ((client_w - grid_px) / 2).max(12);
        let board_y = 20;
        let info_x = board_x;
        let info_y = board_y + grid_px + 18;
        let info_w = grid_px;

        let btn_h = 34;
        let btn_y0 = (client_h - 2 * btn_h - 24).max(info_y + 96);
        let half_w = (info_w - 8) / 2;

        MinesGeom {
            board_x,
            board_y,
            cell,
            gap,
            grid_px,
            info_x,
            info_y,
            info_w,
            btn_mode: (info_x, btn_y0, half_w, btn_h),
            btn_seed: (info_x + half_w + 8, btn_y0, info_w - half_w - 8, btn_h),
            btn_rule: (info_x, btn_y0 + btn_h + 8, info_w, btn_h),
        }
    }
}

pub fn draw_window(fb: &mut Framebuffer, app: &MinesApp) {
    fb.clear(rgb(0x0b, 0x10, 0x16));
    let g = mines_geom(fb.w, fb.h);
    let b = &app.board;

    let accent = rgb(0x2d, 0xd4, 0xbf);
    let label_col = rgb(0x7b, 0x8b, 0x98);
    let text_col = rgb(0xe6, 0xf1, 0xf8);

    // --- 1. Draw the 8×8 Minesweeper Bitboard Grid ---
    let glyph_sc = if g.cell >= 54 {
        3
    } else if g.cell >= 34 {
        2
    } else {
        1
    };

    for idx in 0..64u8 {
        let r = (idx / 8) as i32;
        let c = (idx % 8) as i32;
        let cx = g.board_x + c * (g.cell + g.gap);
        let cy = g.board_y + r * (g.cell + g.gap);

        let bit = 1u64 << idx;
        let is_rev = (b.revealed_mask & bit) != 0;
        let is_flag = (b.flagged_mask & bit) != 0;
        let is_det = (b.detonated_mask & bit) != 0;
        let is_mine = (b.mine_mask & bit) != 0;

        if is_det {
            // Detonated bomb cell
            fb.fill(cx, cy, g.cell, g.cell, rgb(0x4c, 0x14, 0x1d));
            fb.border(cx, cy, g.cell, g.cell, 1, rgb(0xf8, 0x71, 0x71));
            draw_str_centered(
                fb,
                cx,
                cy,
                g.cell,
                g.cell,
                "*",
                glyph_sc,
                rgb(0xff, 0xb4, 0xb4),
            );
        } else if is_rev {
            // Revealed safe cell
            fb.fill(cx, cy, g.cell, g.cell, rgb(0x0f, 0x16, 0x1f));
            fb.border(cx, cy, g.cell, g.cell, 1, rgb(0x1b, 0x26, 0x33));
            let cnt = b.adj_counts[idx as usize];
            if cnt > 0 {
                let digit_buf = [b'0' + cnt];
                let digit_str = core::str::from_utf8(&digit_buf).unwrap_or("");
                draw_str_centered(
                    fb,
                    cx,
                    cy,
                    g.cell,
                    g.cell,
                    digit_str,
                    glyph_sc,
                    count_color(cnt),
                );
            }
        } else if b.is_terminal && is_mine {
            // Reveal remaining unflagged/flagged mines when terminal
            let bg = if is_flag {
                rgb(0x14, 0x33, 0x2e)
            } else {
                rgb(0x28, 0x15, 0x1d)
            };
            fb.fill(cx, cy, g.cell, g.cell, bg);
            fb.border(cx, cy, g.cell, g.cell, 1, rgb(0x5c, 0x2b, 0x35));
            draw_str_centered(
                fb,
                cx,
                cy,
                g.cell,
                g.cell,
                "*",
                glyph_sc,
                rgb(0xf8, 0x71, 0x71),
            );
        } else if is_flag {
            // Flagged unrevealed cell
            fb.fill(cx, cy, g.cell, g.cell, rgb(0x16, 0x26, 0x34));
            fb.border(cx, cy, g.cell, g.cell, 2, rgb(0xf5, 0x9e, 0x0b));
            draw_str_centered(
                fb,
                cx,
                cy,
                g.cell,
                g.cell,
                "F",
                glyph_sc,
                rgb(0xfb, 0xbf, 0x24),
            );
        } else {
            // Hidden cell
            fb.fill(cx, cy, g.cell, g.cell, rgb(0x16, 0x22, 0x2f));
            fb.border(cx, cy, g.cell, g.cell, 1, rgb(0x26, 0x38, 0x4a));
        }
    }

    // --- 2. Draw Telemetry & Controls ---
    let compact_info = g.info_w < 260 || fb.h < 500;
    let head_sc = if compact_info { 2 } else { 3 };
    let row_sc = if compact_info { 1 } else { 2 };

    draw_str(fb, g.info_x, g.info_y, "MINES 8X8", head_sc, rgb(0xf0, 0xf6, 0xfc));
    let bar_col = if b.is_terminal {
        if b.is_won {
            rgb(0x34, 0xd3, 0x99)
        } else {
            rgb(0xf8, 0x71, 0x71)
        }
    } else {
        accent
    };
    fb.fill(
        g.info_x,
        g.info_y + 10 * head_sc + 2,
        g.info_w,
        2,
        bar_col,
    );

    let safe_total = 64u32.saturating_sub(b.num_bombs as u32);
    let safe_rev = (b.revealed_mask & !b.mine_mask).count_ones();
    let flags_placed = b.flagged_mask.count_ones();

    let mut y = g.info_y + 10 * head_sc + 14;
    let step_y = if row_sc == 1 { 16 } else { 28 };

    draw_kv(fb, g.info_x, y, g.info_w, row_sc, "STATE", app.status_str(), bar_col);
    y += step_y;

    let mut seed_buf = [b' '; 16];
    let seed_str = fmt_u32(&mut seed_buf, b.seed as u32);
    draw_kv(fb, g.info_x, y, g.info_w, row_sc, "SEED", seed_str, text_col);
    y += step_y;

    let mut safe_buf = [b' '; 16];
    let safe_str = fmt_ratio(&mut safe_buf, safe_rev, safe_total);
    draw_kv(fb, g.info_x, y, g.info_w, row_sc, "CLEARED", safe_str, text_col);
    y += step_y;

    let mut flag_buf = [b' '; 16];
    let flag_str = fmt_ratio(&mut flag_buf, flags_placed, b.num_bombs as u32);
    draw_kv(fb, g.info_x, y, g.info_w, row_sc, "FLAGS", flag_str, text_col);
    y += step_y;

    if y + step_y <= g.btn_mode.1 {
        let mut strike_buf = [b' '; 16];
        let strike_str = fmt_ratio(&mut strike_buf, b.strikes as u32, b.rules.max_strikes as u32);
        draw_kv(fb, g.info_x, y, g.info_w, row_sc, "STRIKES", strike_str, label_col);
    }

    // --- 3. Interactive Action Buttons ---
    let (mx, my, mw, mh) = g.btn_mode;
    let mode_bg = if app.flag_mode {
        rgb(0x36, 0x26, 0x10)
    } else {
        rgb(0x10, 0x2c, 0x2e)
    };
    let mode_border = if app.flag_mode {
        rgb(0xf5, 0x9e, 0x0b)
    } else {
        accent
    };
    let mode_label = if app.flag_mode {
        "MODE: FLAG (F)"
    } else {
        "MODE: REVEAL"
    };
    fb.fill(mx, my, mw, mh, mode_bg);
    fb.border(mx, my, mw, mh, 1, mode_border);
    draw_str_centered(fb, mx, my, mw, mh, mode_label, row_sc, rgb(0xf0, 0xf6, 0xfc));

    let (sx, sy, sw, sh) = g.btn_seed;
    fb.fill(sx, sy, sw, sh, rgb(0x14, 0x1d, 0x26));
    fb.border(sx, sy, sw, sh, 1, rgb(0x26, 0x38, 0x4a));
    draw_str_centered(
        fb,
        sx,
        sy,
        sw,
        sh,
        "NEW SEED (N)",
        row_sc,
        rgb(0xe6, 0xf1, 0xf8),
    );

    let (rx, ry, rw, rh) = g.btn_rule;
    let rule_lbl = if b.rules.max_strikes > 1 {
        "RULE: SHIELDED (3 STRIKES)"
    } else {
        "RULE: CLASSIC (1 STRIKE)"
    };
    fb.fill(rx, ry, rw, rh, rgb(0x14, 0x1d, 0x26));
    fb.border(rx, ry, rw, rh, 1, rgb(0x26, 0x38, 0x4a));
    draw_str_centered(fb, rx, ry, rw, rh, rule_lbl, row_sc, label_col);
}

pub fn click_window(
    app: &mut MinesApp,
    cx: i32,
    cy: i32,
    client_w: i32,
    client_h: i32,
) -> bool {
    let g = mines_geom(client_w, client_h);

    // 1. Check 8×8 grid hit
    if cx >= g.board_x
        && cx < g.board_x + g.grid_px
        && cy >= g.board_y
        && cy < g.board_y + g.grid_px
    {
        let rel_x = cx - g.board_x;
        let rel_y = cy - g.board_y;
        let stride = g.cell + g.gap;
        let col = rel_x / stride;
        let row = rel_y / stride;
        if (0..8).contains(&col)
            && (0..8).contains(&row)
            && (rel_x % stride) < g.cell
            && (rel_y % stride) < g.cell
        {
            let cell = (row * 8 + col) as u8;
            let action_type = if app.flag_mode {
                ActionType::Flag
            } else {
                ActionType::Reveal
            };
            let res = app.board.step(Action { action_type, cell });
            return res.valid_move;
        }
    }

    // 2. Check Mode button
    let (mx, my, mw, mh) = g.btn_mode;
    if cx >= mx && cx < mx + mw && cy >= my && cy < my + mh {
        app.toggle_mode();
        return true;
    }

    // 3. Check New Seed button
    let (sx, sy, sw, sh) = g.btn_seed;
    if cx >= sx && cx < sx + sw && cy >= sy && cy < sy + sh {
        app.next_seed();
        return true;
    }

    // 4. Check Rule button
    let (rx, ry, rw, rh) = g.btn_rule;
    if cx >= rx && cx < rx + rw && cy >= ry && cy < ry + rh {
        app.toggle_rules();
        return true;
    }

    false
}

pub fn key_window(app: &mut MinesApp, code: i32) -> bool {
    match code {
        // 'f' / 'F' (30) toggles Flag/Reveal mode
        30 => {
            app.toggle_mode();
            true
        }
        // 'n' / 'N' (31) or Enter (24) when terminal advances to next seed
        31 => {
            app.next_seed();
            true
        }
        // 'r' / 'R' (32) replays current seed from scratch
        32 => {
            app.reset_seed(app.board.seed);
            true
        }
        24 if app.board.is_terminal => {
            app.next_seed();
            true
        }
        _ => false,
    }
}

/// Draw the `128 x 72` live icon in the Left App Dock.
pub fn draw_dock_tile(
    fb: &mut Framebuffer,
    app: &MinesApp,
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

    draw_str_centered(fb, x, y + 14, w, 24, "MINES", 2, rgb(0xf0, 0xf6, 0xfc));

    let sub = if app.board.is_terminal {
        if app.board.is_won {
            "8X8 WON"
        } else {
            "8X8 BOOM"
        }
    } else {
        "8X8 SEEDED"
    };
    draw_str_centered(fb, x, y + 42, w, 16, sub, 1, rgb(0x2d, 0xd4, 0xbf));
}

fn count_color(cnt: u8) -> u32 {
    match cnt {
        1 => rgb(0x38, 0xbd, 0xf8), // Sky blue
        2 => rgb(0x34, 0xd3, 0x99), // Emerald
        3 => rgb(0xf8, 0x71, 0x71), // Coral red
        4 => rgb(0xc0, 0x84, 0xfc), // Purple
        5 => rgb(0xfb, 0xbf, 0x24), // Amber
        _ => rgb(0x2d, 0xd4, 0xbf), // Teal
    }
}

fn draw_kv(
    fb: &mut Framebuffer,
    x: i32,
    y: i32,
    w: i32,
    scale: i32,
    key: &str,
    val: &str,
    val_col: u32,
) {
    draw_str(fb, x, y, key, scale, rgb(0x7b, 0x8b, 0x98));
    let vw = text_width(val, scale);
    draw_str(fb, x + w - vw, y, val, scale, val_col);
}

fn fmt_u32(buf: &mut [u8; 16], val: u32) -> &str {
    let pos = write_u32(buf, 0, val);
    core::str::from_utf8(&buf[..pos]).unwrap_or("0")
}

fn fmt_ratio(buf: &mut [u8; 16], a: u32, b: u32) -> &str {
    let mut pos = write_u32(buf, 0, a);
    if pos < buf.len() {
        buf[pos] = b'/';
        pos += 1;
    }
    pos = write_u32(buf, pos, b);
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
