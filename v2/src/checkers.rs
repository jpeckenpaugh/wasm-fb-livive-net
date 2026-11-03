//! 8×8 English Draughts (Checkers) Framebuffer Application (`checkers.rs`) powered by `checkers_sm`.
//!
//! Supports 3 play modes:
//! - `VsAi` (`0`, default): Human plays Red (P1) at bottom, local Minimax AI plays Black (P2)
//!   after a short host-paced delay (~260ms).
//! - `PassAndPlay` (`1`): Local 2-player human pass-and-play on the same guest.
//! - `Broker2P` (`2`): Two-guest match brokered via `window.__broker` (`local_seat = 1` Red or `2` Black).

use crate::checkers_sm::{
    coord_to_index, is_dark_square, is_king, is_player_piece, is_red,
    select_full_ai_turn, CheckersState, Difficulty, EMPTY, PLAYER_BLACK, PLAYER_RED,
};
use crate::fb::{rgb, Framebuffer};
use crate::fmt as f;
use crate::font::{draw_str, draw_str_centered, draw_str_prop, line_height};
use crate::icon::{draw_icon, Icon};
use crate::input::{InputResult, Key};
use crate::layout::{Metrics, Rect};
use crate::theme as t;
use crate::ui;

pub const TX_NONE: u8 = 0;
pub const TX_MOVE: u8 = 1;
pub const TX_NEW_GAME: u8 = 2;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckersMode {
    VsAi = 0,
    PassAndPlay = 1,
    Broker2P = 2,
}

pub struct CheckersApp {
    pub state: CheckersState,
    pub mode: CheckersMode,
    pub difficulty: Difficulty,
    pub flipped: bool,
    pub selected_sq: Option<u8>,
    pub last_from: Option<u8>,
    pub last_to: Option<u8>,
    pub last_move_buf: [u8; 32],
    pub last_move_len: usize,
    pub turn_hops: [u8; 8],
    pub turn_hop_count: u8,
    pub turn_is_jump: bool,
    pub ai_pending: bool,
    pub match_id: u32,
    pub local_seat: u8,
    pub mp_seq: u32,
    pub tx_pending: bool,
    pub tx_action: u8,
    pub tx_lo: u32,
    pub tx_hi: u32,
}

impl CheckersApp {
    pub const fn new_const() -> Self {
        Self {
            state: CheckersState::new_const(),
            mode: CheckersMode::VsAi,
            difficulty: Difficulty::Medium,
            flipped: false,
            selected_sq: None,
            last_from: None,
            last_to: None,
            last_move_buf: [b'-'; 32],
            last_move_len: 1,
            turn_hops: [0; 8],
            turn_hop_count: 0,
            turn_is_jump: false,
            ai_pending: false,
            match_id: 1,
            local_seat: 1,
            mp_seq: 0,
            tx_pending: false,
            tx_action: TX_NONE,
            tx_lo: 0,
            tx_hi: 0,
        }
    }

    pub fn init_default(&mut self) {
        *self = Self::new_const();
    }

    pub fn last_move_str(&self) -> &str {
        core::str::from_utf8(&self.last_move_buf[..self.last_move_len]).unwrap_or("-")
    }

    fn clear_turn_tracking(&mut self) {
        self.selected_sq = None;
        self.last_from = None;
        self.last_to = None;
        self.last_move_buf = [b'-'; 32];
        self.last_move_len = 1;
        self.turn_hops = [0; 8];
        self.turn_hop_count = 0;
        self.turn_is_jump = false;
        self.ai_pending = false;
    }

    pub fn new_game(&mut self) {
        self.state.reset();
        self.clear_turn_tracking();
        if self.mode == CheckersMode::Broker2P {
            self.mp_seq = self.mp_seq.wrapping_add(1);
            self.pack_tx_new_game();
        }
    }

    pub fn configure_mp(&mut self, enabled: bool, match_id: u32, seat: u8, difficulty: Difficulty) {
        self.state.reset();
        self.clear_turn_tracking();
        self.difficulty = difficulty;
        self.match_id = if match_id > 0 { match_id } else { 1 };
        self.mp_seq = 0;
        self.tx_pending = false;
        self.tx_action = TX_NONE;
        self.tx_lo = 0;
        self.tx_hi = 0;

        if enabled {
            self.mode = CheckersMode::Broker2P;
            self.local_seat = seat.clamp(1, 2);
            self.flipped = self.local_seat == 2;
        } else {
            self.mode = CheckersMode::VsAi;
            self.local_seat = 1;
            self.flipped = false;
        }
    }

    pub fn cycle_mode(&mut self) {
        match self.mode {
            CheckersMode::VsAi => {
                self.mode = CheckersMode::PassAndPlay;
                self.local_seat = 0;
                self.flipped = false;
            }
            CheckersMode::PassAndPlay => {
                self.mode = CheckersMode::Broker2P;
                self.local_seat = 1;
                self.flipped = false;
            }
            CheckersMode::Broker2P => {
                if self.local_seat == 1 {
                    self.local_seat = 2;
                    self.flipped = true;
                } else {
                    self.mode = CheckersMode::VsAi;
                    self.local_seat = 1;
                    self.flipped = false;
                }
            }
        }
        self.state.reset();
        self.clear_turn_tracking();
        if self.mode == CheckersMode::Broker2P {
            self.mp_seq = self.mp_seq.wrapping_add(1);
            self.pack_tx_new_game();
        } else {
            self.tx_pending = false;
        }
    }

    pub fn cycle_difficulty(&mut self) {
        self.difficulty = self.difficulty.next();
    }

    pub fn toggle_flip(&mut self) {
        self.flipped = !self.flipped;
    }

    pub fn active_turn_seat(&self) -> u8 {
        if self.state.turn == PLAYER_RED {
            1
        } else {
            2
        }
    }

    pub fn can_local_human_move(&self) -> bool {
        if self.state.winner != 0 {
            return false;
        }
        match self.mode {
            CheckersMode::VsAi => self.state.turn == PLAYER_RED && !self.ai_pending,
            CheckersMode::PassAndPlay => true,
            CheckersMode::Broker2P => self.active_turn_seat() == self.local_seat,
        }
    }

    fn pack_tx_move(&mut self, seat: u8) {
        let count = self.turn_hop_count.clamp(2, 7);
        let h0 = (self.turn_hops[0] & 31) as u32;
        let h1 = (self.turn_hops[1] & 31) as u32;
        let h2 = (self.turn_hops[2] & 31) as u32;
        let h3 = (self.turn_hops[3] & 31) as u32;
        let h4 = (self.turn_hops[4] & 31) as u32;
        let h5 = (self.turn_hops[5] & 31) as u32;
        let h6 = (self.turn_hops[6] & 31) as u32;

        self.tx_action = TX_MOVE;
        self.tx_lo = (TX_MOVE as u32)
            | ((seat as u32 & 0xF) << 4)
            | ((count as u32 & 0xF) << 8)
            | ((self.turn_is_jump as u32) << 12)
            | (h0 << 16)
            | (h1 << 21)
            | (h2 << 26);
        self.tx_hi = h3 | (h4 << 6) | (h5 << 12) | (h6 << 18);
        self.tx_pending = true;
    }

    fn pack_tx_new_game(&mut self) {
        let seat = self.local_seat.clamp(1, 2);
        self.tx_action = TX_NEW_GAME;
        self.tx_lo = (TX_NEW_GAME as u32) | ((seat as u32 & 0xF) << 4);
        self.tx_hi = 0;
        self.tx_pending = true;
    }

    /// Execute the pending AI turn (including any multi-jump chain).
    pub fn step_ai(&mut self) -> bool {
        if self.mode != CheckersMode::VsAi
            || self.state.turn != PLAYER_BLACK
            || self.state.winner != 0
        {
            self.ai_pending = false;
            return false;
        }
        let rec = select_full_ai_turn(&mut self.state, self.difficulty);
        self.ai_pending = false;
        if rec.hop_len >= 2 {
            self.last_from = Some(rec.hops[0]);
            self.last_to = Some(rec.hops[(rec.hop_len - 1) as usize]);
            self.last_move_len = f::format_pdn_hops(
                &mut self.last_move_buf,
                &rec.hops,
                rec.hop_len as usize,
                rec.is_jump,
            );
            self.selected_sq = None;
            return true;
        }
        false
    }

    /// Apply an inbound 2-player broker packet (`app_checkers_apply_rx`).
    pub fn apply_rx(&mut self, seq: u32, seat: u8, action: u8, lo: u32, hi: u32) -> bool {
        if action == TX_NEW_GAME {
            self.state.reset();
            self.clear_turn_tracking();
            self.mp_seq = seq;
            self.tx_pending = false;
            return true;
        }
        if seq <= self.mp_seq || action != TX_MOVE {
            return false;
        }
        let expected_seat = self.active_turn_seat();
        if seat != expected_seat {
            return false;
        }

        let hop_count = ((lo >> 8) & 0xF) as usize;
        if !(2..=7).contains(&hop_count) {
            return false;
        }
        let hops = [
            ((lo >> 16) & 0x1F) as u8,
            ((lo >> 21) & 0x1F) as u8,
            ((lo >> 26) & 0x1F) as u8,
            (hi & 0x1F) as u8,
            ((hi >> 6) & 0x1F) as u8,
            ((hi >> 12) & 0x1F) as u8,
            ((hi >> 18) & 0x1F) as u8,
            0,
        ];

        // Trial-validate the entire hop sequence on a copy of `state` before committing.
        let mut trial = self.state;
        let mut any_jump = false;
        for i in 0..(hop_count - 1) {
            let from = hops[i];
            let to = hops[i + 1];
            let res = trial.apply_hop(from, to);
            if !res.valid {
                return false;
            }
            if res.is_jump {
                any_jump = true;
            }
            if i + 2 < hop_count && res.turn_complete {
                return false;
            }
            if i + 2 == hop_count && !res.turn_complete {
                return false;
            }
        }

        self.state = trial;
        self.selected_sq = None;
        self.turn_hop_count = 0;
        self.last_from = Some(hops[0]);
        self.last_to = Some(hops[hop_count - 1]);
        self.last_move_len =
            f::format_pdn_hops(&mut self.last_move_buf, &hops, hop_count, any_jump);
        self.mp_seq = seq;
        self.tx_pending = false;
        true
    }

    pub fn handle_square_click(&mut self, sq: u8) -> bool {
        if sq >= 32 || !self.can_local_human_move() {
            return false;
        }

        let player = self.state.turn;
        let legal = self.state.legal_moves();

        // If we are locked into a multi-jump continuation from `must_sq`:
        if let Some(must_sq) = self.state.must_continue_from {
            self.selected_sq = Some(must_sq);
            for mv in legal.as_slice() {
                if mv.from == must_sq && mv.to == sq {
                    return self.execute_human_hop(must_sq, sq);
                }
            }
            return false;
        }

        // If a piece is currently selected and `sq` is a legal destination for it:
        if let Some(sel) = self.selected_sq {
            for mv in legal.as_slice() {
                if mv.from == sel && mv.to == sq {
                    return self.execute_human_hop(sel, sq);
                }
            }
        }

        // Otherwise, check if `sq` holds one of the active player's pieces:
        if is_player_piece(self.state.board[sq as usize], player) {
            // Prefer pieces that have at least one legal move, or allow selecting to inspect
            let has_legal = legal.as_slice().iter().any(|m| m.from == sq);
            if has_legal {
                if self.selected_sq == Some(sq) {
                    self.selected_sq = None;
                } else {
                    self.selected_sq = Some(sq);
                }
                return true;
            }
        }

        if self.selected_sq.is_some() {
            self.selected_sq = None;
            return true;
        }
        false
    }

    fn execute_human_hop(&mut self, from: u8, to: u8) -> bool {
        let was_continuing = self.state.must_continue_from.is_some();
        let active_seat = self.active_turn_seat();
        let res = self.state.apply_hop(from, to);
        if !res.valid {
            return false;
        }

        if !was_continuing {
            self.turn_hops[0] = from;
            self.turn_hops[1] = to;
            self.turn_hop_count = 2;
            self.turn_is_jump = res.is_jump;
        } else if (self.turn_hop_count as usize) < self.turn_hops.len() {
            self.turn_hops[self.turn_hop_count as usize] = to;
            self.turn_hop_count += 1;
        }

        self.last_from = Some(self.turn_hops[0]);
        self.last_to = Some(to);

        if res.can_continue_jump {
            self.selected_sq = Some(to);
        } else {
            self.selected_sq = None;
            let hop_len = self.turn_hop_count as usize;
            self.last_move_len = f::format_pdn_hops(
                &mut self.last_move_buf,
                &self.turn_hops,
                hop_len,
                self.turn_is_jump,
            );
            if self.mode == CheckersMode::Broker2P {
                self.mp_seq = self.mp_seq.wrapping_add(1);
                self.pack_tx_move(active_seat);
            } else if self.mode == CheckersMode::VsAi
                && self.state.turn == PLAYER_BLACK
                && self.state.winner == 0
            {
                self.ai_pending = true;
            }
        }
        true
    }

    pub fn status_str(&self) -> &'static str {
        match self.state.winner {
            1 => return "Red Wins!",
            2 => return "Black Wins!",
            3 => return "Draw!",
            _ => {}
        }
        if self.state.must_continue_from.is_some() {
            return "Jump Chain!";
        }
        match self.mode {
            CheckersMode::VsAi => {
                if self.state.turn == PLAYER_BLACK || self.ai_pending {
                    "AI Thinking..."
                } else {
                    "Your Turn (Red)"
                }
            }
            CheckersMode::PassAndPlay => {
                if self.state.turn == PLAYER_RED {
                    "Red's Turn"
                } else {
                    "Black's Turn"
                }
            }
            CheckersMode::Broker2P => {
                let turn_seat = self.active_turn_seat();
                if turn_seat == self.local_seat {
                    if self.local_seat == 2 {
                        "Your Turn (Blk)"
                    } else {
                        "Your Turn (Red)"
                    }
                } else if turn_seat == 2 {
                    "Wait for P2..."
                } else {
                    "Wait for P1..."
                }
            }
        }
    }

    pub fn mode_button_label(&self) -> &'static str {
        match self.mode {
            CheckersMode::VsAi => "Mode: vs AI",
            CheckersMode::PassAndPlay => "Mode: Pass & Play",
            CheckersMode::Broker2P => {
                if self.local_seat == 2 {
                    "2P · Seat P2 (Blk)"
                } else {
                    "2P · Seat P1 (Red)"
                }
            }
        }
    }

    pub fn flip_button_label(&self) -> &'static str {
        if self.flipped {
            "View: Blk Bottom"
        } else {
            "View: Red Bottom"
        }
    }
}

pub struct CheckersGeom {
    pub board_x: i32,
    pub board_y: i32,
    pub cell: i32,
    pub grid_px: i32,
    pub info_x: i32,
    pub info_y: i32,
    pub info_w: i32,
    pub btn_new: Rect,
    pub btn_mode: Rect,
    pub btn_diff: Rect,
    pub btn_flip: Rect,
}

pub fn checkers_geom(client_w: i32, client_h: i32) -> CheckersGeom {
    let _m = Metrics::new(client_w, client_h);
    let landscape = client_w >= client_h && client_w >= 600;

    if landscape {
        let max_board_h = client_h - 44;
        let max_board_w = (client_w * 3 / 5) - 44;
        let avail = max_board_h.min(max_board_w).max(200);
        let cell = (avail / 8).clamp(24, 74);
        let grid_px = 8 * cell;

        let info_w = (client_w - grid_px - 96).clamp(200, 360);
        let total_w = grid_px + 48 + info_w;
        let board_x = ((client_w - total_w) / 2).max(16);
        let board_y = ((client_h - grid_px) / 2).max(12);

        let info_x = board_x + grid_px + 48;
        let info_y = board_y;
        let btn_h = if cell < 42 { 30 } else { 38 };
        let btn_gap = if cell < 42 { 6 } else { 8 };
        let stack_h = 4 * btn_h + 3 * btn_gap;
        let btn_y0 = info_y + (grid_px - stack_h).max(140);

        CheckersGeom {
            board_x,
            board_y,
            cell,
            grid_px,
            info_x,
            info_y,
            info_w,
            btn_new: Rect::new(info_x, btn_y0, info_w, btn_h),
            btn_mode: Rect::new(info_x, btn_y0 + (btn_h + btn_gap), info_w, btn_h),
            btn_diff: Rect::new(info_x, btn_y0 + 2 * (btn_h + btn_gap), info_w, btn_h),
            btn_flip: Rect::new(info_x, btn_y0 + 3 * (btn_h + btn_gap), info_w, btn_h),
        }
    } else {
        // Portrait / compact layout: board on top, telemetry + 2×2 buttons below
        let avail_w = client_w - 32;
        let avail_h = (client_h * 58 / 100).max(200);
        let avail = avail_w.min(avail_h);
        let cell = (avail / 8).clamp(24, 68);
        let grid_px = 8 * cell;

        let board_x = ((client_w - grid_px) / 2).max(12);
        let board_y = 18;
        let info_x = board_x;
        let info_y = board_y + grid_px + 16;
        let info_w = grid_px;

        let btn_h = 34;
        let btn_y0 = (client_h - 2 * btn_h - 20).max(info_y + 92);
        let half_w = (info_w - 8) / 2;

        CheckersGeom {
            board_x,
            board_y,
            cell,
            grid_px,
            info_x,
            info_y,
            info_w,
            btn_new: Rect::new(info_x, btn_y0, half_w, btn_h),
            btn_mode: Rect::new(info_x + half_w + 8, btn_y0, info_w - half_w - 8, btn_h),
            btn_diff: Rect::new(info_x, btn_y0 + btn_h + 8, half_w, btn_h),
            btn_flip: Rect::new(
                info_x + half_w + 8,
                btn_y0 + btn_h + 8,
                info_w - half_w - 8,
                btn_h,
            ),
        }
    }
}

fn draw_disc(
    fb: &mut Framebuffer,
    cx: i32,
    cy: i32,
    radius: i32,
    fill_col: u32,
    rim_col: u32,
    spec_col: u32,
    shadow_col: u32,
) {
    let r2 = radius * radius;
    let inner_r = (radius - 2).max(1);
    let inner_r2 = inner_r * inner_r;
    let ring_r = (radius * 64 / 100).max(2);
    let ring_outer2 = ring_r * ring_r;
    let ring_inner2 = (ring_r - 1).max(1) * (ring_r - 1).max(1);

    // Specular highlight center (top-left offset)
    let spec_ox = -radius / 3;
    let spec_oy = -radius / 3;
    let spec_r = (radius / 3).max(2);
    let spec_r2 = spec_r * spec_r;

    for dy in -radius..=radius {
        let py = cy + dy;
        for dx in -radius..=radius {
            let d2 = dx * dx + dy * dy;
            if d2 <= r2 {
                let px = cx + dx;
                let sdx = dx - spec_ox;
                let sdy = dy - spec_oy;
                let sd2 = sdx * sdx + sdy * sdy;

                let col = if d2 > inner_r2 {
                    if dy > radius / 4 {
                        shadow_col
                    } else {
                        rim_col
                    }
                } else if sd2 <= spec_r2 {
                    spec_col
                } else if d2 >= ring_inner2 && d2 <= ring_outer2 {
                    rim_col
                } else {
                    fill_col
                };
                fb.fill(px, py, 1, 1, col);
            }
        }
    }
}

fn draw_target_ring(fb: &mut Framebuffer, cx: i32, cy: i32, radius: i32, color: u32, is_jump: bool) {
    let outer2 = radius * radius;
    let inner = (radius - 3).max(1);
    let inner2 = inner * inner;
    let dot_r = if is_jump { (radius / 2).max(2) } else { 2 };
    let dot_r2 = dot_r * dot_r;

    for dy in -radius..=radius {
        let py = cy + dy;
        for dx in -radius..=radius {
            let d2 = dx * dx + dy * dy;
            if (d2 <= outer2 && d2 >= inner2) || d2 <= dot_r2 {
                fb.fill(cx + dx, py, 1, 1, color);
            }
        }
    }
}

pub fn draw_window(fb: &mut Framebuffer, app: &CheckersApp) {
    fb.clear(t::BG);
    let g = checkers_geom(fb.w, fb.h);
    let s = &app.state;

    // Outer board frame with elevation bevel
    fb.fill(
        g.board_x - 5,
        g.board_y - 5,
        g.grid_px + 10,
        g.grid_px + 10,
        t::SURFACE_RAISED,
    );
    fb.border(
        g.board_x - 5,
        g.board_y - 5,
        g.grid_px + 10,
        g.grid_px + 10,
        2,
        t::BORDER_STRONG,
    );

    let legal = s.legal_moves();
    let active_sel = s.must_continue_from.or(app.selected_sq);

    // --- 1. Draw the 8×8 Checkerboard Grid & Pieces ---
    // Warm slate / cream for light squares, deep walnut / obsidian for playable dark squares
    let light_sq_col = rgb(0x2d, 0x37, 0x48);
    let light_hi_col = rgb(0x3a, 0x48, 0x5d);
    let light_sh_col = rgb(0x21, 0x29, 0x36);

    let dark_sq_col = rgb(0x0f, 0x17, 0x24);
    let dark_hi_col = rgb(0x19, 0x25, 0x39);
    let dark_sh_col = rgb(0x09, 0x0e, 0x16);

    let last_sq_col = rgb(0x26, 0x23, 0x4b); // Deep indigo
    let sel_sq_col = rgb(0x15, 0x36, 0x45);  // Deep slate cyan

    for v_row in 0..8i32 {
        for v_col in 0..8i32 {
            let row = if app.flipped { 7 - v_row } else { v_row };
            let col = if app.flipped { 7 - v_col } else { v_col };
            let x = g.board_x + v_col * g.cell;
            let y = g.board_y + v_row * g.cell;

            if !is_dark_square(row, col) {
                // Light square with subtle inset bevel
                fb.fill(x, y, g.cell, g.cell, light_sq_col);
                fb.fill(x, y, g.cell, 1, light_hi_col);
                fb.fill(x, y, 1, g.cell, light_hi_col);
                fb.fill(x, y + g.cell - 1, g.cell, 1, light_sh_col);
                fb.fill(x + g.cell - 1, y, 1, g.cell, light_sh_col);
                continue;
            }

            let sq = coord_to_index(row, col).unwrap_or(0);
            let is_selected = active_sel == Some(sq);
            let is_last = app.last_from == Some(sq) || app.last_to == Some(sq);
            
            // Check if this square is a legal destination from active_sel, and whether it's a jump
            let mut is_dest = false;
            let mut is_dest_jump = false;
            if let Some(sel) = active_sel {
                for m in legal.as_slice().iter() {
                    if m.from == sel && m.to == sq {
                        is_dest = true;
                        if m.is_jump {
                            is_dest_jump = true;
                        }
                    }
                }
            }

            let bg = if is_selected {
                sel_sq_col
            } else if is_last {
                last_sq_col
            } else {
                dark_sq_col
            };
            fb.fill(x, y, g.cell, g.cell, bg);

            // 1px beveled edge on dark playable squares
            if !is_selected && !is_last {
                fb.fill(x, y, g.cell, 1, dark_hi_col);
                fb.fill(x, y, 1, g.cell, dark_hi_col);
                fb.fill(x, y + g.cell - 1, g.cell, 1, dark_sh_col);
                fb.fill(x + g.cell - 1, y, 1, g.cell, dark_sh_col);
            }

            if is_selected {
                // Vibrant amber / cyan glow border
                fb.border(x, y, g.cell, g.cell, 2, rgb(0x38, 0xbd, 0xf8));
            } else if is_last {
                // Indigo / lilac last-move border
                fb.border(x, y, g.cell, g.cell, 2, rgb(0x63, 0x66, 0xf1));
            }

            // Subtle PDN square number (1..32) in top-left of dark square
            if g.cell >= 44 {
                let mut pdn_buf = [0u8; 8];
                let pdn_str = f::line_no(&mut pdn_buf, (sq as u32) + 1);
                draw_str(fb, x + 3, y + 3, pdn_str, 1, rgb(0x64, 0x74, 0x8b));
            }

            // Legal destination highlight: Emerald for slide, Ruby/Amber for capture
            let center_x = x + g.cell / 2;
            let center_y = y + g.cell / 2;
            if is_dest {
                let ring_r = (g.cell / 4).max(5);
                let target_col = if is_dest_jump {
                    rgb(0xf4, 0x3f, 0x5e) // Ruby / Rose for capture jump
                } else {
                    rgb(0x10, 0xb9, 0x81) // Emerald for normal slide
                };
                draw_target_ring(fb, center_x, center_y, ring_r, target_col, is_dest_jump);
            }

            // Draughts piece with 3D specular highlight, shaded rim, and gleaming King crown
            let piece = s.board[sq as usize];
            if piece != EMPTY {
                let radius = (g.cell * 38 / 100).max(8);
                let (fill_col, rim_col, spec_col, shadow_col) = if is_red(piece) {
                    (
                        rgb(0xbe, 0x12, 0x3c), // Rich crimson fill
                        rgb(0xfb, 0x71, 0x85), // Warm coral rim
                        rgb(0xfd, 0xba, 0x74), // Sunlit highlight dot
                        rgb(0x88, 0x0e, 0x27), // Deep shadow edge
                    )
                } else {
                    (
                        rgb(0x18, 0x1e, 0x24), // Obsidian dark charcoal fill
                        rgb(0x38, 0xbd, 0xf8), // Electric cyan / ice rim
                        rgb(0x93, 0xc5, 0xfd), // Specular blue-white dot
                        rgb(0x0a, 0x0d, 0x10), // Pitch dark shadow edge
                    )
                };
                draw_disc(
                    fb, center_x, center_y, radius, fill_col, rim_col, spec_col,
                    shadow_col,
                );

                if is_king(piece) {
                    // Double gold ring crown marker + "K" glyph
                    let gold = rgb(0xfb, 0xbf, 0x24);
                    let gold_hi = rgb(0xfe, 0xf0, 0x8a);
                    let k_sc = if g.cell >= 52 { 2 } else { 1 };

                    // Inner crown gold ring
                    let crown_r = (radius * 50 / 100).max(4);
                    let c_r2 = crown_r * crown_r;
                    let c_in2 = (crown_r - 2).max(1) * (crown_r - 2).max(1);
                    for dy in -crown_r..=crown_r {
                        let py = center_y + dy;
                        for dx in -crown_r..=crown_r {
                            let d2 = dx * dx + dy * dy;
                            if d2 <= c_r2 && d2 >= c_in2 {
                                fb.fill(center_x + dx, py, 1, 1, gold_hi);
                            }
                        }
                    }

                    draw_str_centered(
                        fb,
                        x,
                        y,
                        g.cell,
                        g.cell,
                        "K",
                        k_sc,
                        gold,
                    );
                }
            }
        }
    }

    // --- 2. Draw Telemetry & Elevated Status HUD Panel ---
    let compact_info = g.info_w < 260 || fb.h < 500;
    let head_sc = if compact_info { 2 } else { 3 };
    let row_sc = 2;

    // Outer card container for telemetry
    let card_top = g.info_y - 4;
    let card_h = (g.btn_new.y - 10 - card_top).max(150);
    let card_rect = Rect::new(g.info_x - 6, card_top, g.info_w + 12, card_h);
    ui::panel(fb, card_rect, t::SURFACE_RAISED, t::BORDER_STRONG);

    let title_str = match app.mode {
        CheckersMode::VsAi => "Checkers vs AI",
        CheckersMode::PassAndPlay => "Checkers 8x8",
        CheckersMode::Broker2P => "Checkers 2P Match",
    };
    draw_str(fb, g.info_x, g.info_y + 2, title_str, head_sc, t::TEXT);

    // Turn indicator pill / badge
    let turn_pill_h = 24;
    let turn_pill_y = g.info_y + line_height(head_sc) + 6;
    let (pill_bg, pill_border, pill_text, pill_text_col) = match s.winner {
        1 => (rgb(0x88, 0x13, 0x37), rgb(0xf4, 0x3f, 0x5e), "VICTORY: RED WINS!", rgb(0xff, 0xff, 0xff)),
        2 => (rgb(0x0f, 0x17, 0x2a), rgb(0x38, 0xbd, 0xf8), "VICTORY: BLACK WINS!", rgb(0x38, 0xbd, 0xf8)),
        3 => (rgb(0x45, 0x1a, 0x03), rgb(0xf5, 0x9e, 0x0b), "DRAW GAME", rgb(0xfb, 0xbf, 0x24)),
        _ => {
            if s.turn == PLAYER_RED {
                (rgb(0x4c, 0x05, 0x19), rgb(0xf4, 0x3f, 0x5e), "[*] RED'S TURN", rgb(0xfd, 0xba, 0x74))
            } else {
                if app.mode == CheckersMode::VsAi {
                    (rgb(0x0b, 0x1b, 0x2b), rgb(0x38, 0xbd, 0xf8), "[AI] THINKING...", rgb(0x7d, 0xd3, 0xfc))
                } else {
                    (rgb(0x0b, 0x1b, 0x2b), rgb(0x38, 0xbd, 0xf8), "[*] BLACK'S TURN", rgb(0x7d, 0xd3, 0xfc))
                }
            }
        }
    };
    fb.fill(g.info_x, turn_pill_y, g.info_w, turn_pill_h, pill_bg);
    fb.border(g.info_x, turn_pill_y, g.info_w, turn_pill_h, 1, pill_border);
    draw_str_centered(
        fb,
        g.info_x,
        turn_pill_y,
        g.info_w,
        turn_pill_h,
        pill_text,
        1,
        pill_text_col,
    );

    let (red_cnt, blk_cnt) = s.piece_counts();
    let (red_kings, blk_kings) = s.king_counts();
    let red_caps = (12 - blk_cnt as i32).max(0);
    let blk_caps = (12 - red_cnt as i32).max(0);

    // 2-Column Piece Scoreboard
    let score_y = turn_pill_y + turn_pill_h + 8;
    let col_w = (g.info_w - 6) / 2;
    let col2_x = g.info_x + col_w + 6;

    // Column 1: Red Header
    fb.fill(g.info_x, score_y, col_w, 16, rgb(0x3b, 0x07, 0x12));
    draw_str(fb, g.info_x + 4, score_y + 3, "RED", 1, rgb(0xfb, 0x71, 0x85));

    // Column 2: Black Header
    let blk_hdr_label = if app.mode == CheckersMode::VsAi { "BLACK (AI)" } else { "BLACK" };
    fb.fill(col2_x, score_y, col_w, 16, rgb(0x0a, 0x1e, 0x2f));
    draw_str(fb, col2_x + 4, score_y + 3, blk_hdr_label, 1, rgb(0x38, 0xbd, 0xf8));

    // Row: Discs & Kings count
    let mut num_buf = [0u8; 32];
    let row1_y = score_y + 19;
    
    // Red stats: "D: {red_cnt}  K: {red_kings}"
    let r_discs_str = f::checkers_pieces_str(&mut num_buf, red_cnt, red_kings);
    draw_str(fb, g.info_x + 4, row1_y, r_discs_str, 1, t::TEXT);

    // Black stats: "D: {blk_cnt}  K: {blk_kings}"
    let mut num_buf2 = [0u8; 32];
    let b_discs_str = f::checkers_pieces_str(&mut num_buf2, blk_cnt, blk_kings);
    draw_str(fb, col2_x + 4, row1_y, b_discs_str, 1, t::TEXT);

    // Row: Captures
    let row2_y = row1_y + 13;
    let mut cap_buf1 = [0u8; 8];
    let r_cap_str = f::line_no(&mut cap_buf1, red_caps as u32);
    draw_str(fb, g.info_x + 4, row2_y, "Captures:", 1, t::TEXT_DIM);
    draw_str(fb, g.info_x + 60, row2_y, r_cap_str, 1, rgb(0xfd, 0xba, 0x74));

    let mut cap_buf2 = [0u8; 8];
    let b_cap_str = f::line_no(&mut cap_buf2, blk_caps as u32);
    draw_str(fb, col2_x + 4, row2_y, "Captures:", 1, t::TEXT_DIM);
    draw_str(fb, col2_x + 60, row2_y, b_cap_str, 1, rgb(0x93, 0xc5, 0xfd));

    // Divider line
    let mut y = row2_y + 18;
    fb.fill(g.info_x, y, g.info_w, 1, t::BORDER);
    y += 6;

    let step_y = if compact_info { 18 } else { 22 };
    let row = |ry: i32| Rect::new(g.info_x, ry, g.info_w, step_y);

    if y + step_y <= g.btn_new.y {
        ui::kv_row(
            fb,
            row(y),
            "Last",
            app.last_move_str(),
            row_sc,
            t::TEXT_DIM,
            rgb(0x81, 0x8c, 0xf8),
        );
        y += step_y;
    }

    if y + step_y <= g.btn_new.y {
        let mut mh_buf = [0u8; 32];
        let mh_str = f::checkers_moves_hash_str(&mut mh_buf, s.move_count, s.state_hash());
        ui::kv_row(
            fb,
            row(y),
            "Moves",
            mh_str,
            row_sc,
            t::TEXT_DIM,
            t::TEXT_BODY,
        );
    }

    // --- 3. Interactive Action Buttons ---
    ui::button(fb, g.btn_new, "New Game (N)", ui::Tone::Neutral, row_sc);

    let mode_tone = match app.mode {
        CheckersMode::VsAi => ui::Tone::Accent,
        CheckersMode::PassAndPlay => ui::Tone::Neutral,
        CheckersMode::Broker2P => {
            if app.active_turn_seat() == app.local_seat {
                ui::Tone::Accent
            } else {
                ui::Tone::Warning
            }
        }
    };
    ui::button(fb, g.btn_mode, app.mode_button_label(), mode_tone, row_sc);
    ui::button(
        fb,
        g.btn_diff,
        app.difficulty.label(),
        ui::Tone::Neutral,
        row_sc,
    );
    ui::button(
        fb,
        g.btn_flip,
        app.flip_button_label(),
        ui::Tone::Neutral,
        row_sc,
    );
}

pub fn click_window(
    app: &mut CheckersApp,
    cx: i32,
    cy: i32,
    client_w: i32,
    client_h: i32,
) -> InputResult {
    let g = checkers_geom(client_w, client_h);

    // 1. Check 8×8 checkerboard hit
    if Rect::new(g.board_x, g.board_y, g.grid_px, g.grid_px).contains(cx, cy) {
        let v_col = (cx - g.board_x) / g.cell;
        let v_row = (cy - g.board_y) / g.cell;
        if (0..8).contains(&v_col) && (0..8).contains(&v_row) {
            let row = if app.flipped { 7 - v_row } else { v_row };
            let col = if app.flipped { 7 - v_col } else { v_col };
            if let Some(sq) = coord_to_index(row, col) {
                return InputResult::from(app.handle_square_click(sq));
            }
        }
        return InputResult::Noop;
    }

    if g.btn_new.contains(cx, cy) {
        app.new_game();
        return InputResult::Changed;
    }
    if g.btn_mode.contains(cx, cy) {
        app.cycle_mode();
        return InputResult::Changed;
    }
    if g.btn_diff.contains(cx, cy) {
        app.cycle_difficulty();
        return InputResult::Changed;
    }
    if g.btn_flip.contains(cx, cy) {
        app.toggle_flip();
        return InputResult::Changed;
    }

    InputResult::Noop
}

pub fn key_window(app: &mut CheckersApp, key: Key) -> InputResult {
    match key {
        Key::NewSeed | Key::Replay => {
            app.new_game();
            InputResult::Changed
        }
        Key::ToggleMp => {
            app.cycle_mode();
            InputResult::Changed
        }
        Key::Difficulty => {
            app.cycle_difficulty();
            InputResult::Changed
        }
        Key::Flag => {
            app.toggle_flip();
            InputResult::Changed
        }
        Key::Enter if app.state.winner != 0 => {
            app.new_game();
            InputResult::Changed
        }
        _ => InputResult::Noop,
    }
}

/// Draw the `128 x h` live icon in the Left App Home Drawer.
pub fn draw_dock_tile(
    fb: &mut Framebuffer,
    app: &CheckersApp,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    active: bool,
) {
    ui::dock_tile(fb, Rect::new(x, y, w, h), active);
    let icon_sz = 20;
    let icon_y = y + (h - icon_sz) / 2;
    draw_icon(fb, x + 6, icon_y, Icon::Games, icon_sz, t::ACCENT);

    let text_x = x + 10 + icon_sz;
    if h < 56 {
        draw_str_prop(fb, text_x, y + (h - 16) / 2, "Checkers", 2, t::TEXT);
    } else {
        draw_str_prop(fb, text_x, y + 16, "Checkers", 2, t::TEXT);
        let sub = match app.mode {
            CheckersMode::VsAi => "vs AI",
            CheckersMode::PassAndPlay => "2P Local",
            CheckersMode::Broker2P => "2P Match",
        };
        draw_str_prop(fb, text_x, y + 40, sub, 2, t::ACCENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkers_sm::{BLACK_MAN, RED_MAN};

    #[test]
    fn vs_ai_human_move_triggers_ai_pending_and_step_ai_replies() {
        let mut app = CheckersApp::new_const();
        assert_eq!(app.mode, CheckersMode::VsAi);
        assert_eq!(app.state.turn, PLAYER_RED);
        assert!(!app.ai_pending);

        // Human clicks piece at square 20 (PDN 21) and moves to square 16 (PDN 17)
        assert!(app.handle_square_click(20));
        assert_eq!(app.selected_sq, Some(20));
        assert!(app.handle_square_click(16));
        assert_eq!(app.selected_sq, None);
        assert_eq!(app.last_move_str(), "21-17");
        assert!(app.ai_pending);
        assert_eq!(app.state.turn, PLAYER_BLACK);

        // AI replies
        assert!(app.step_ai());
        assert!(!app.ai_pending);
        assert_eq!(app.state.turn, PLAYER_RED);
        assert_ne!(app.last_move_str(), "21-17");
    }

    #[test]
    fn broker_2p_multi_jump_tx_and_rx_convergence() {
        let mut p1 = CheckersApp::new_const();
        let mut p2 = CheckersApp::new_const();
        p1.configure_mp(true, 5, 1, Difficulty::Medium);
        p2.configure_mp(true, 5, 2, Difficulty::Medium);

        // Set up a double-jump board on both P1 and P2:
        // Red at 22 (PDN 23), Black at 17 (PDN 18) and 9 (PDN 10), plus extra Black at 0
        let mut custom = [EMPTY; 32];
        custom[22] = RED_MAN;
        custom[17] = BLACK_MAN;
        custom[9] = BLACK_MAN;
        custom[0] = BLACK_MAN;
        p1.state.board = custom;
        p2.state.board = custom;

        // P2 cannot move on P1's turn
        assert!(!p2.handle_square_click(0));

        // P1 selects 22 and jumps to 13 (first hop of multi-jump)
        assert!(p1.handle_square_click(22));
        assert!(p1.handle_square_click(13));
        assert!(!p1.tx_pending); // Not transmitted until jump chain completes!
        assert_eq!(p1.state.must_continue_from, Some(13));

        // P1 completes jump chain: 13 -> 6
        assert!(p1.handle_square_click(6));
        assert!(p1.tx_pending);
        assert_eq!(p1.last_move_str(), "23x14x7");

        // Apply P1's TX packet to P2 via apply_rx
        assert!(p2.apply_rx(1, 1, p1.tx_action, p1.tx_lo, p1.tx_hi));
        assert_eq!(p2.last_move_str(), "23x14x7");
        assert_eq!(p1.state.state_hash(), p2.state.state_hash());
        assert_eq!(p2.state.turn, PLAYER_BLACK);
    }
}
