//! Minimalist 8×8 Minesweeper Application (`mines.rs`) powered by `mine_sm`.
//!
//! Flat, single-surface layout inside the maximized client surface:
//! - Crisp 8×8 bitboard grid (`64` cells)
//! - Clean telemetry & controls column (or top/bottom strip in portrait viewports)
//! - Supports `ActionType::Reveal` and `ActionType::Flag`, seed cycling,
//!   Classic (`max_strikes: 1`) vs. Shielded (`max_strikes: 3`) rules, and
//!   a 2-Player Turn-Based multiplayer wrapper over `BoardState`.

use crate::fb::Framebuffer;
use crate::fmt as f;
use crate::font::{draw_str, draw_str_centered, draw_str_prop, line_height};
use crate::icon::{draw_icon, Icon};
use crate::input::{InputResult, Key};
use crate::layout::{Metrics, Rect};
use crate::mine_sm::{Action, ActionType, BoardState, Rules, StepResult};
use crate::theme as t;
use crate::ui;

pub const DEFAULT_SEED: u64 = 42;
pub const DEFAULT_BOMBS: u8 = 10;

pub const TX_REVEAL: u8 = 0;
pub const TX_FLAG: u8 = 1;
pub const TX_NEW_MATCH: u8 = 2;

pub struct MinesApp {
    pub board: BoardState,
    pub flag_mode: bool,
    pub mp_enabled: bool,
    pub match_id: u32,
    pub local_seat: u8,
    pub active_turn: u8,
    pub p1_score: i32,
    pub p2_score: i32,
    pub winner_seat: u8,
    pub mp_seq: u32,
    pub tx_pending: bool,
    pub tx_action: u8,
    pub tx_cell: u8,
    pub p1_avatar: [u32; 256],
    pub p2_avatar: [u32; 256],
    pub has_p1_avatar: bool,
    pub has_p2_avatar: bool,
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
            mp_enabled: false,
            match_id: 1,
            local_seat: 0,
            active_turn: 1,
            p1_score: 0,
            p2_score: 0,
            winner_seat: 0,
            mp_seq: 0,
            tx_pending: false,
            tx_action: 0,
            tx_cell: 0,
            p1_avatar: [0; 256],
            p2_avatar: [0; 256],
            has_p1_avatar: false,
            has_p2_avatar: false,
        }
    }

    pub fn init_default(&mut self) {
        self.board = BoardState::new(DEFAULT_SEED, DEFAULT_BOMBS, Rules { max_strikes: 1 });
        self.flag_mode = false;
        self.mp_enabled = false;
        self.match_id = 1;
        self.local_seat = 0;
        self.active_turn = 1;
        self.p1_score = 0;
        self.p2_score = 0;
        self.winner_seat = 0;
        self.mp_seq = 0;
        self.tx_pending = false;
        self.tx_action = 0;
        self.tx_cell = 0;
    }

    pub fn configure_mp(
        &mut self,
        enabled: bool,
        match_id: u32,
        seat: u8,
        seed: u64,
        max_strikes: u8,
    ) {
        let strikes = if max_strikes > 1 { 3 } else { 1 };
        self.board = BoardState::new(seed, DEFAULT_BOMBS, Rules { max_strikes: strikes });
        self.flag_mode = false;
        self.mp_enabled = enabled;
        self.match_id = if match_id > 0 { match_id } else { 1 };
        self.local_seat = if enabled { seat.clamp(1, 2) } else { 0 };
        self.active_turn = 1;
        self.p1_score = 0;
        self.p2_score = 0;
        self.winner_seat = 0;
        self.mp_seq = 0;
        self.tx_pending = false;
        self.tx_action = 0;
        self.tx_cell = 0;
    }

    pub fn reset_seed(&mut self, seed: u64) {
        let rules = self.board.rules;
        let bombs = self.board.num_bombs;
        self.board = BoardState::new(seed, bombs, rules);
        self.active_turn = 1;
        self.p1_score = 0;
        self.p2_score = 0;
        self.winner_seat = 0;
        if self.mp_enabled {
            self.mp_seq = self.mp_seq.wrapping_add(1);
            self.tx_pending = true;
            self.tx_action = TX_NEW_MATCH;
            self.tx_cell = 0;
        }
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
        self.active_turn = 1;
        self.p1_score = 0;
        self.p2_score = 0;
        self.winner_seat = 0;
        if self.mp_enabled {
            self.mp_seq = self.mp_seq.wrapping_add(1);
            self.tx_pending = true;
            self.tx_action = TX_NEW_MATCH;
            self.tx_cell = 0;
        }
    }

    pub fn toggle_mp(&mut self) {
        if !self.mp_enabled {
            self.mp_enabled = true;
            self.local_seat = 1;
        } else if self.local_seat == 1 {
            self.local_seat = 2;
        } else {
            self.mp_enabled = false;
            self.local_seat = 0;
        }
        self.board = BoardState::new(self.board.seed, self.board.num_bombs, self.board.rules);
        self.active_turn = 1;
        self.p1_score = 0;
        self.p2_score = 0;
        self.winner_seat = 0;
        if self.mp_enabled {
            self.mp_seq = self.mp_seq.wrapping_add(1);
            self.tx_pending = true;
            self.tx_action = TX_NEW_MATCH;
            self.tx_cell = 0;
        } else {
            self.tx_pending = false;
        }
    }

    pub fn apply_move(&mut self, seat: u8, action_type: ActionType, cell: u8) -> StepResult {
        let res = self.board.step(Action { action_type, cell });
        if !res.valid_move {
            return res;
        }

        if self.mp_enabled {
            let effective_seat = if seat == 2 { 2 } else { 1 };
            if action_type == ActionType::Reveal {
                if res.hit_bomb {
                    if self.board.rules.max_strikes > 1 {
                        if effective_seat == 1 {
                            self.p1_score -= 5;
                        } else {
                            self.p2_score -= 5;
                        }
                    }
                    if res.terminal {
                        self.winner_seat = if effective_seat == 1 { 2 } else { 1 };
                    }
                } else {
                    let pts = res.newly_revealed as i32;
                    if effective_seat == 1 {
                        self.p1_score += pts;
                    } else {
                        self.p2_score += pts;
                    }
                    if res.terminal && res.won {
                        self.winner_seat = if self.p1_score > self.p2_score {
                            1
                        } else if self.p2_score > self.p1_score {
                            2
                        } else {
                            3
                        };
                    }
                }

                if !self.board.is_terminal {
                    self.active_turn = if effective_seat == 1 { 2 } else { 1 };
                }
            }
            self.mp_seq = self.mp_seq.wrapping_add(1);
        }

        res
    }

    pub fn status_str(&self) -> &'static str {
        if self.mp_enabled {
            if self.board.is_terminal {
                match self.winner_seat {
                    1 => "P1 Wins!",
                    2 => "P2 Wins!",
                    3 => "Draw!",
                    _ => {
                        if self.board.is_won {
                            "Cleared (Won)"
                        } else {
                            "Detonated"
                        }
                    }
                }
            } else if self.local_seat == self.active_turn {
                if self.local_seat == 2 {
                    "Your Turn (P2)"
                } else {
                    "Your Turn (P1)"
                }
            } else if self.active_turn == 2 {
                "Wait for P2..."
            } else {
                "Wait for P1..."
            }
        } else if self.board.is_terminal {
            if self.board.is_won {
                "Cleared (Won)"
            } else {
                "Detonated"
            }
        } else if self.board.move_count == 0 {
            "Ready"
        } else {
            "Playing"
        }
    }

    pub fn mp_button_label(&self) -> &'static str {
        if !self.mp_enabled {
            "Match: Solo (1P)"
        } else {
            match (self.local_seat, self.active_turn) {
                (1, 1) => "2P · Seat P1 (Turn: P1)",
                (1, _) => "2P · Seat P1 (Turn: P2)",
                (2, 1) => "2P · Seat P2 (Turn: P1)",
                (2, _) => "2P · Seat P2 (Turn: P2)",
                (_, 1) => "2P · Turn: P1",
                _ => "2P · Turn: P2",
            }
        }
    }
}

pub struct MinesGeom {
    pub board_x: i32,
    pub board_y: i32,
    pub cell: i32,
    pub gap: i32,
    pub grid_px: i32,
    pub info_x: i32,
    pub info_y: i32,
    pub info_w: i32,
    pub btn_mode: Rect,
    pub btn_seed: Rect,
    pub btn_rule: Rect,
    pub btn_mp: Rect,
}

pub fn mines_geom(client_w: i32, client_h: i32) -> MinesGeom {
    let m = Metrics::new(client_w, client_h);
    let landscape = client_w >= client_h && client_w >= 600;
    let gap = m.pick(3, 4, 4);

    if landscape {
        let max_board_h = client_h - 48;
        let max_board_w = (client_w * 3 / 5) - 48;
        let avail = max_board_h.min(max_board_w).max(200);
        let max_cell = if client_w >= 1440 || client_h >= 900 { 112 } else { 72 };
        let cell = ((avail - 7 * gap) / 8).clamp(24, max_cell);
        let grid_px = 8 * cell + 7 * gap;

        let max_info = if client_w >= 1440 { 420 } else { 360 };
        let info_w = (client_w - grid_px - 96).clamp(200, max_info);
        let total_w = grid_px + 48 + info_w;
        let board_x = ((client_w - total_w) / 2).max(16);
        let board_y = ((client_h - grid_px) / 2).max(16);

        let info_x = board_x + grid_px + 48;
        let info_y = board_y;
        let btn_h = if cell < 42 { 32 } else { 38 };
        let btn_gap = 8;
        let stack_h = 4 * btn_h + 3 * btn_gap;
        let btn_y0 = info_y + (grid_px - stack_h).max(164);

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
            btn_seed: Rect::new(info_x, btn_y0 + (btn_h + btn_gap), info_w, btn_h),
            btn_rule: Rect::new(info_x, btn_y0 + 2 * (btn_h + btn_gap), info_w, btn_h),
            btn_mp: Rect::new(info_x, btn_y0 + 3 * (btn_h + btn_gap), info_w, btn_h),
        }
    } else {
        // Portrait / compact layout: board on top, telemetry + 2×2 buttons below
        let avail_w = client_w - 32;
        let avail_h = (client_h * 58 / 100).max(200);
        let avail = avail_w.min(avail_h);
        let max_cell = if client_h >= 900 { 96 } else { 68 };
        let cell = ((avail - 7 * gap) / 8).clamp(24, max_cell);
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
            btn_rule: Rect::new(info_x, btn_y0 + btn_h + 8, half_w, btn_h),
            btn_mp: Rect::new(
                info_x + half_w + 8,
                btn_y0 + btn_h + 8,
                info_w - half_w - 8,
                btn_h,
            ),
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
    let glyph_sc = if g.cell >= 54 { 3 } else { 2 };

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
    let row_sc = 2;

    let title_str = if app.mp_enabled {
        "Mines 2P Match"
    } else {
        "Mines 8x8"
    };
    draw_str(fb, g.info_x, g.info_y, title_str, head_sc, t::TEXT);
    let bar_col = if b.is_terminal {
        if app.mp_enabled {
            if app.winner_seat == 3 {
                t::WARNING
            } else if app.winner_seat == app.local_seat || app.local_seat == 0 {
                t::SUCCESS
            } else {
                t::DANGER
            }
        } else if b.is_won {
            t::SUCCESS
        } else {
            t::DANGER
        }
    } else if app.mp_enabled && app.local_seat != 0 && app.local_seat != app.active_turn {
        t::WARNING
    } else {
        accent
    };
    fb.fill(
        g.info_x,
        g.info_y + line_height(head_sc) + 2,
        g.info_w,
        2,
        bar_col,
    );

    let safe_total = 64u32.saturating_sub(b.num_bombs as u32);
    let safe_rev = (b.revealed_mask & !b.mine_mask).count_ones();
    let flags_placed = b.flagged_mask.count_ones();

    let mut y = g.info_y + line_height(head_sc) + 14;
    let step_y = if compact_info { 20 } else { 26 };

    let row = |y: i32| Rect::new(g.info_x, y, g.info_w, step_y);
    ui::kv_row(
        fb,
        row(y),
        "State",
        app.status_str(),
        row_sc,
        label_col,
        bar_col,
    );
    y += step_y;

    if app.mp_enabled && y + step_y <= g.btn_mode.y {
        let mut score_buf = [b' '; 32];
        let score_str = f::mp_score_str(&mut score_buf, app.p1_score, app.p2_score);
        ui::kv_row(fb, row(y), "Score", score_str, row_sc, label_col, t::ACCENT);
        if app.has_p1_avatar {
            fb.blit(g.info_x + 52, y + (step_y - 16) / 2, &app.p1_avatar, 16, 16);
        }
        if app.has_p2_avatar {
            fb.blit(g.info_x + g.info_w - 20, y + (step_y - 16) / 2, &app.p2_avatar, 16, 16);
        }
        y += step_y;
    }

    if y + step_y <= g.btn_mode.y {
        let mut seed_buf = [b' '; 16];
        let seed_str = f::u32_str(&mut seed_buf, b.seed as u32);
        ui::kv_row(fb, row(y), "Seed", seed_str, row_sc, label_col, text_col);
        y += step_y;
    }

    if y + step_y <= g.btn_mode.y {
        let mut safe_buf = [b' '; 16];
        let safe_str = f::ratio_str(&mut safe_buf, safe_rev, safe_total);
        ui::kv_row(fb, row(y), "Cleared", safe_str, row_sc, label_col, text_col);
        y += step_y;
    }

    if y + step_y <= g.btn_mode.y {
        let mut flag_buf = [b' '; 16];
        let flag_str = f::ratio_str(&mut flag_buf, flags_placed, b.num_bombs as u32);
        ui::kv_row(fb, row(y), "Flags", flag_str, row_sc, label_col, text_col);
        y += step_y;
    }

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
            "Strikes",
            strike_str,
            row_sc,
            label_col,
            label_col,
        );
    }

    // --- 3. Interactive Action Buttons ---
    let mode_label = if app.flag_mode {
        "Mode: Flag (F)"
    } else {
        "Mode: Reveal"
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
    ui::button(fb, g.btn_seed, "New Seed (N)", ui::Tone::Neutral, row_sc);

    let rule_lbl = if b.rules.max_strikes > 1 {
        "Rule: Shielded (3)"
    } else {
        "Rule: Classic (1)"
    };
    ui::button(fb, g.btn_rule, rule_lbl, ui::Tone::Neutral, row_sc);

    let mp_tone = if !app.mp_enabled {
        ui::Tone::Neutral
    } else if app.local_seat == app.active_turn {
        ui::Tone::Accent
    } else {
        ui::Tone::Warning
    };
    ui::button(fb, g.btn_mp, app.mp_button_label(), mp_tone, row_sc);
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
            if app.mp_enabled
                && app.local_seat != 0
                && app.active_turn != app.local_seat
                && !app.board.is_terminal
            {
                return InputResult::Noop;
            }
            let cell = (row * 8 + col) as u8;
            let action_type = if app.flag_mode {
                ActionType::Flag
            } else {
                ActionType::Reveal
            };
            let seat = if app.local_seat != 0 {
                app.local_seat
            } else {
                app.active_turn
            };
            let res = app.apply_move(seat, action_type, cell);
            if res.valid_move && app.mp_enabled {
                app.tx_pending = true;
                app.tx_action = if action_type == ActionType::Flag {
                    TX_FLAG
                } else {
                    TX_REVEAL
                };
                app.tx_cell = cell;
            }
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
    if g.btn_mp.contains(cx, cy) {
        app.toggle_mp();
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
        Key::ToggleMp => {
            app.toggle_mp();
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
    draw_str_prop(fb, x + 48, y + 18, "Mines", 2, t::TEXT);

    let sub = if app.board.is_terminal {
        if app.mp_enabled {
            match app.winner_seat {
                1 => "P1 Win",
                2 => "P2 Win",
                3 => "Draw",
                _ => "Done",
            }
        } else if app.board.is_won {
            "Won"
        } else {
            "Boom"
        }
    } else if app.mp_enabled {
        "2P Turn"
    } else {
        "Seeded"
    };
    draw_str_prop(fb, x + 48, y + 42, sub, 2, t::ACCENT);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turn_enforcement_and_scoring_in_2p_mode() {
        let mut app = MinesApp::new_const();
        app.configure_mp(true, 7, 1, 42, 1);
        assert!(app.mp_enabled);
        assert_eq!(app.match_id, 7);
        assert_eq!(app.local_seat, 1);
        assert_eq!(app.active_turn, 1);

        let g = mines_geom(1280, 688);
        let cell0_x = g.board_x + g.cell / 2;
        let cell0_y = g.board_y + g.cell / 2;

        // P1 can click cell 0 on turn 1
        assert_eq!(
            click_window(&mut app, cell0_x, cell0_y, 1280, 688),
            InputResult::Changed
        );
        assert!(app.tx_pending);
        assert_eq!(app.tx_action, TX_REVEAL);
        assert_eq!(app.tx_cell, 0);
        assert!(app.p1_score > 0);
        assert_eq!(app.active_turn, 2);

        // Find an unrevealed safe cell for next turn
        let next_cell = (0..64u8)
            .find(|&c| {
                (app.board.revealed_mask & (1u64 << c)) == 0
                    && (app.board.mine_mask & (1u64 << c)) == 0
            })
            .unwrap();
        let nc_x = g.board_x + (next_cell as i32 % 8) * (g.cell + g.gap) + g.cell / 2;
        let nc_y = g.board_y + (next_cell as i32 / 8) * (g.cell + g.gap) + g.cell / 2;

        // P1 cannot move when active_turn == 2
        assert_eq!(
            click_window(&mut app, nc_x, nc_y, 1280, 688),
            InputResult::Noop
        );

        // Apply P2's move via apply_move
        let res = app.apply_move(2, ActionType::Reveal, next_cell);
        assert!(res.valid_move);
        assert!(app.p2_score > 0);
        assert_eq!(app.active_turn, 1);
    }

    #[test]
    fn sudden_death_and_shielded_bomb_scoring() {
        // 1. Sudden death (max_strikes = 1): P1 reveals safe cell, P2 hits a bomb -> P1 wins
        let mut app = MinesApp::new_const();
        app.configure_mp(true, 1, 1, 42, 1);
        let safe_cell = (0..64u8)
            .find(|&c| (app.board.mine_mask & (1u64 << c)) == 0)
            .unwrap();
        app.apply_move(1, ActionType::Reveal, safe_cell);
        assert_eq!(app.active_turn, 2);

        let bomb_cell = (0..64u8)
            .find(|&c| (app.board.mine_mask & (1u64 << c)) != 0)
            .unwrap();
        let res = app.apply_move(2, ActionType::Reveal, bomb_cell);
        assert!(res.valid_move && res.hit_bomb && res.terminal);
        assert_eq!(app.winner_seat, 1);
        assert_eq!(app.status_str(), "P1 Wins!");

        // 2. Shielded mode (max_strikes = 3): hitting a bomb deducts 5 points and passes turn
        let mut shielded = MinesApp::new_const();
        shielded.configure_mp(true, 2, 1, 42, 3);
        shielded.apply_move(1, ActionType::Reveal, safe_cell);
        let res2 = shielded.apply_move(2, ActionType::Reveal, bomb_cell);
        assert!(res2.valid_move && res2.hit_bomb && !res2.terminal);
        assert_eq!(shielded.p2_score, -5);
        assert_eq!(shielded.active_turn, 1);
    }
}
