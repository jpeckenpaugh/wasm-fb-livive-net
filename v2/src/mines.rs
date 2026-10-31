//! Minimalist 8×8 Minesweeper Application (`mines.rs`) powered by `mine_sm`.
//!
//! Flat, single-surface layout inside the maximized client surface:
//! - Crisp 8×8 bitboard grid (`64` cells)
//! - Clean telemetry & controls column (or top/bottom strip in portrait viewports)
//! - Supports `ActionType::Reveal` and `ActionType::Flag`, seed cycling, and
//!   Classic (`max_strikes: 1`) vs. Shielded (`max_strikes: 3`) rules.

use crate::fb::Framebuffer;
use crate::fmt as f;
use crate::font::{draw_str, draw_str_centered, draw_str_prop};
use crate::icon::{draw_icon, Icon};
use crate::input::{InputResult, Key};
use crate::layout::{Metrics, Rect};
use crate::mine_sm::{Action, ActionType, BoardState, Rules};
use crate::theme as t;
use crate::ui;

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
    btn_mode: Rect,
    btn_seed: Rect,
    btn_rule: Rect,
}

fn mines_geom(client_w: i32, client_h: i32) -> MinesGeom {
    let m = Metrics::new(client_w, client_h);
    let landscape = client_w >= client_h && client_w >= 600;
    let gap = m.pick(3, 4, 4);

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
            btn_mode: Rect::new(info_x, btn_y0, info_w, btn_h),
            btn_seed: Rect::new(info_x, btn_y0 + btn_h + 10, info_w, btn_h),
            btn_rule: Rect::new(info_x, btn_y0 + 2 * (btn_h + 10), info_w, btn_h),
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
            btn_mode: Rect::new(info_x, btn_y0, half_w, btn_h),
            btn_seed: Rect::new(info_x + half_w + 8, btn_y0, info_w - half_w - 8, btn_h),
            btn_rule: Rect::new(info_x, btn_y0 + btn_h + 8, info_w, btn_h),
        }
    }
}

pub fn draw_window(fb: &mut Framebuffer, app: &MinesApp) {
    fb.clear(t::BG);
    let g = mines_geom(fb.w, fb.h);
    let b = &app.board;

    let accent = t::ACCENT;
    let label_col = t::TEXT_DIM;
    let text_col = t::TEXT_BODY;

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
            fb.fill(cx, cy, g.cell, g.cell, t::MINES_DETONATED);
            fb.border(cx, cy, g.cell, g.cell, 1, t::DANGER);
            draw_str_centered(fb, cx, cy, g.cell, g.cell, "*", glyph_sc, t::DANGER_SOFT);
        } else if is_rev {
            // Revealed safe cell
            fb.fill(cx, cy, g.cell, g.cell, t::GRID);
            fb.border(cx, cy, g.cell, g.cell, 1, t::MINES_REVEALED_BORDER);
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
                t::MINES_TERMINAL_FLAG
            } else {
                t::MINES_TERMINAL_MINE
            };
            fb.fill(cx, cy, g.cell, g.cell, bg);
            fb.border(cx, cy, g.cell, g.cell, 1, t::MINES_DETONATED_BORDER);
            draw_str_centered(fb, cx, cy, g.cell, g.cell, "*", glyph_sc, t::DANGER);
        } else if is_flag {
            // Flagged unrevealed cell
            fb.fill(cx, cy, g.cell, g.cell, t::MINES_FLAG);
            fb.border(cx, cy, g.cell, g.cell, 2, t::WARNING);
            draw_str_centered(fb, cx, cy, g.cell, g.cell, "F", glyph_sc, t::WARNING_BRIGHT);
        } else {
            // Hidden cell
            fb.fill(cx, cy, g.cell, g.cell, t::MINES_HIDDEN);
            fb.border(cx, cy, g.cell, g.cell, 1, t::BORDER_CONTROL);
        }
    }

    // --- 2. Draw Telemetry & Controls ---
    let compact_info = g.info_w < 260 || fb.h < 500;
    let head_sc = if compact_info { 2 } else { 3 };
    let row_sc = if compact_info { 1 } else { 2 };

    draw_str(fb, g.info_x, g.info_y, "MINES 8X8", head_sc, t::TEXT);
    let bar_col = if b.is_terminal {
        if b.is_won {
            t::SUCCESS
        } else {
            t::DANGER
        }
    } else {
        accent
    };
    fb.fill(g.info_x, g.info_y + 10 * head_sc + 2, g.info_w, 2, bar_col);

    let safe_total = 64u32.saturating_sub(b.num_bombs as u32);
    let safe_rev = (b.revealed_mask & !b.mine_mask).count_ones();
    let flags_placed = b.flagged_mask.count_ones();

    let mut y = g.info_y + 10 * head_sc + 14;
    let step_y = if row_sc == 1 { 16 } else { 28 };

    let row = |y: i32| Rect::new(g.info_x, y, g.info_w, step_y);
    ui::kv_row(
        fb,
        row(y),
        "STATE",
        app.status_str(),
        row_sc,
        label_col,
        bar_col,
    );
    y += step_y;

    let mut seed_buf = [b' '; 16];
    let seed_str = f::u32_str(&mut seed_buf, b.seed as u32);
    ui::kv_row(fb, row(y), "SEED", seed_str, row_sc, label_col, text_col);
    y += step_y;

    let mut safe_buf = [b' '; 16];
    let safe_str = f::ratio_str(&mut safe_buf, safe_rev, safe_total);
    ui::kv_row(fb, row(y), "CLEARED", safe_str, row_sc, label_col, text_col);
    y += step_y;

    let mut flag_buf = [b' '; 16];
    let flag_str = f::ratio_str(&mut flag_buf, flags_placed, b.num_bombs as u32);
    ui::kv_row(fb, row(y), "FLAGS", flag_str, row_sc, label_col, text_col);
    y += step_y;

    if y + step_y <= g.btn_mode.y {
        let mut strike_buf = [b' '; 16];
        let strike_str = f::ratio_str(
            &mut strike_buf,
            b.strikes as u32,
            b.rules.max_strikes as u32,
        );
        ui::kv_row(
            fb,
            row(y),
            "STRIKES",
            strike_str,
            row_sc,
            label_col,
            label_col,
        );
    }

    // --- 3. Interactive Action Buttons ---
    let mode_label = if app.flag_mode {
        "MODE: FLAG (F)"
    } else {
        "MODE: REVEAL"
    };
    ui::button(
        fb,
        g.btn_mode,
        mode_label,
        if app.flag_mode {
            ui::Tone::Warning
        } else {
            ui::Tone::Accent
        },
        row_sc,
    );
    ui::button(fb, g.btn_seed, "NEW SEED (N)", ui::Tone::Neutral, row_sc);

    let rule_lbl = if b.rules.max_strikes > 1 {
        "RULE: SHIELDED (3 STRIKES)"
    } else {
        "RULE: CLASSIC (1 STRIKE)"
    };
    ui::button(fb, g.btn_rule, rule_lbl, ui::Tone::Neutral, row_sc);
}

pub fn click_window(
    app: &mut MinesApp,
    cx: i32,
    cy: i32,
    client_w: i32,
    client_h: i32,
) -> InputResult {
    let g = mines_geom(client_w, client_h);

    // 1. Check 8×8 grid hit
    if Rect::new(g.board_x, g.board_y, g.grid_px, g.grid_px).contains(cx, cy) {
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
            return InputResult::from(res.valid_move);
        }
    }

    if g.btn_mode.contains(cx, cy) {
        app.toggle_mode();
        return InputResult::Changed;
    }
    if g.btn_seed.contains(cx, cy) {
        app.next_seed();
        return InputResult::Changed;
    }
    if g.btn_rule.contains(cx, cy) {
        app.toggle_rules();
        return InputResult::Changed;
    }

    InputResult::Noop
}

pub fn key_window(app: &mut MinesApp, key: Key) -> InputResult {
    match key {
        Key::Flag => {
            app.toggle_mode();
            InputResult::Changed
        }
        Key::NewSeed => {
            app.next_seed();
            InputResult::Changed
        }
        Key::Replay => {
            app.reset_seed(app.board.seed);
            InputResult::Changed
        }
        Key::Enter if app.board.is_terminal => {
            app.next_seed();
            InputResult::Changed
        }
        _ => InputResult::Noop,
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
    ui::dock_tile(fb, Rect::new(x, y, w, h), active);
    draw_icon(fb, x + 10, y + 24, Icon::Games, 32, t::ACCENT);
    draw_str_prop(fb, x + 48, y + 18, "MINES", 2, t::TEXT);

    let sub = if app.board.is_terminal {
        if app.board.is_won {
            "8X8 WON"
        } else {
            "8X8 BOOM"
        }
    } else {
        "8X8 SEEDED"
    };
    draw_str_prop(fb, x + 48, y + 42, sub, 1, t::ACCENT);
}

fn count_color(cnt: u8) -> u32 {
    match cnt {
        1 => t::INFO,           // Sky blue
        2 => t::SUCCESS,        // Emerald
        3 => t::DANGER,         // Coral red
        4 => t::PURPLE,         // Purple
        5 => t::WARNING_BRIGHT, // Amber
        _ => t::ACCENT,         // Teal
    }
}
