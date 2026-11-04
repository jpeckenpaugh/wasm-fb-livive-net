//! 8×8 Chess Framebuffer Application (`chess.rs`) powered by `chess_sm`.
//!
//! Supports 3 play modes:
//! - `VsAi` (`0`, default): Human plays White (P1) at bottom, local Minimax AI plays Black (P2)
//!   after a host-paced step (`app_chess_step_ai`).
//! - `PassAndPlay` (`1`): Local 2-player human pass-and-play on the same guest.
//! - `Broker2P` (`2`): Two-guest match brokered via host mailbox (`local_seat = 1` White or `2` Black).

use crate::checkers_sm::Difficulty;
use crate::chess_sm::{
    coord_to_square, is_black, is_piece, is_white, piece_color, piece_type, select_ai_move,
    square_file, square_rank, ChessMove, ChessState, GameStatus, MoveBuffer, COLOR_BLACK,
    COLOR_WHITE, PIECE_BISHOP, PIECE_KING, PIECE_KNIGHT, PIECE_PAWN, PIECE_QUEEN,
    PIECE_ROOK,
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
pub enum ChessMode {
    VsAi = 0,
    PassAndPlay = 1,
    Broker2P = 2,
}

pub struct ChessApp {
    pub state: ChessState,
    pub mode: ChessMode,
    pub difficulty: Difficulty,
    pub flipped: bool,
    pub selected_sq: Option<u8>,
    pub last_from: Option<u8>,
    pub last_to: Option<u8>,
    pub last_move_buf: [u8; 16],
    pub last_move_len: usize,
    pub ai_pending: bool,
    pub match_id: u32,
    pub local_seat: u8, // 1: White, 2: Black
    pub mp_seq: u32,
    pub tx_pending: bool,
    pub tx_action: u8,
    pub tx_from: u8,
    pub tx_to: u8,
    pub tx_promo: u8,
}

impl ChessApp {
    pub const fn new_const() -> Self {
        Self {
            state: ChessState::new(),
            mode: ChessMode::VsAi,
            difficulty: Difficulty::Medium,
            flipped: false,
            selected_sq: None,
            last_from: None,
            last_to: None,
            last_move_buf: [b'-'; 16],
            last_move_len: 1,
            ai_pending: false,
            match_id: 1,
            local_seat: 1,
            mp_seq: 0,
            tx_pending: false,
            tx_action: TX_NONE,
            tx_from: 0,
            tx_to: 0,
            tx_promo: 0,
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
        self.last_move_buf = [b'-'; 16];
        self.last_move_len = 1;
        self.ai_pending = false;
    }

    pub fn new_game(&mut self) {
        self.state = ChessState::new();
        self.clear_turn_tracking();
        if self.mode == ChessMode::Broker2P {
            self.mp_seq = self.mp_seq.wrapping_add(1);
            self.pack_tx_new_game();
        }
    }

    pub fn configure_mp(&mut self, enabled: bool, match_id: u32, seat: u8, difficulty: Difficulty) {
        self.state = ChessState::new();
        self.clear_turn_tracking();
        self.difficulty = difficulty;
        self.match_id = if match_id > 0 { match_id } else { 1 };
        self.mp_seq = 0;
        self.tx_pending = false;
        self.tx_action = TX_NONE;
        self.tx_from = 0;
        self.tx_to = 0;
        self.tx_promo = 0;

        if enabled {
            self.mode = ChessMode::Broker2P;
            self.local_seat = seat.clamp(1, 2);
            self.flipped = self.local_seat == 2;
        } else {
            self.mode = ChessMode::VsAi;
            self.local_seat = 1;
            self.flipped = false;
        }
    }

    pub fn cycle_mode(&mut self) {
        match self.mode {
            ChessMode::VsAi => {
                self.mode = ChessMode::PassAndPlay;
                self.local_seat = 0;
                self.flipped = false;
            }
            ChessMode::PassAndPlay => {
                self.mode = ChessMode::Broker2P;
                self.local_seat = 1;
                self.flipped = false;
            }
            ChessMode::Broker2P => {
                if self.local_seat == 1 {
                    self.local_seat = 2;
                    self.flipped = true;
                } else {
                    self.mode = ChessMode::VsAi;
                    self.local_seat = 1;
                    self.flipped = false;
                }
            }
        }
        self.state = ChessState::new();
        self.clear_turn_tracking();
        if self.mode == ChessMode::Broker2P {
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
        if self.state.turn == COLOR_WHITE {
            1
        } else {
            2
        }
    }

    pub fn can_local_human_move(&self) -> bool {
        if matches!(
            self.state.status,
            GameStatus::Checkmate | GameStatus::Stalemate | GameStatus::Draw50Move
        ) {
            return false;
        }
        match self.mode {
            ChessMode::VsAi => self.state.turn == COLOR_WHITE && !self.ai_pending,
            ChessMode::PassAndPlay => true,
            ChessMode::Broker2P => self.active_turn_seat() == self.local_seat,
        }
    }

    fn pack_tx_move(&mut self, m: ChessMove) {
        self.tx_action = TX_MOVE;
        self.tx_from = m.from;
        self.tx_to = m.to;
        self.tx_promo = m.promo;
        self.tx_pending = true;
    }

    fn pack_tx_new_game(&mut self) {
        self.tx_action = TX_NEW_GAME;
        self.tx_from = 0;
        self.tx_to = 0;
        self.tx_promo = 0;
        self.tx_pending = true;
    }

    /// Pack TX into a single u32 for `app_chess_take_tx`.
    pub fn take_tx_word(&mut self) -> u32 {
        self.tx_pending = false;
        (self.tx_action as u32)
            | ((self.local_seat as u32) << 8)
            | ((self.tx_from as u32) << 16)
            | ((self.tx_to as u32) << 24)
            | (((self.tx_promo & 0x7) as u32) << 4)
    }

    /// Execute the pending AI move.
    pub fn step_ai(&mut self) -> bool {
        if self.mode != ChessMode::VsAi
            || self.state.turn != COLOR_BLACK
            || matches!(
                self.state.status,
                GameStatus::Checkmate | GameStatus::Stalemate | GameStatus::Draw50Move
            )
        {
            self.ai_pending = false;
            return false;
        }
        let depth = match self.difficulty {
            Difficulty::Easy => 1,
            Difficulty::Medium => 2,
            Difficulty::Hard => 3,
        };
        if let Some(mv) = select_ai_move(&self.state, depth) {
            self.ai_pending = false;
            if self.state.apply_move(mv) {
                self.last_from = Some(mv.from);
                self.last_to = Some(mv.to);
                self.record_last_move(mv);
                self.selected_sq = None;
                return true;
            }
        }
        self.ai_pending = false;
        false
    }

    /// Apply an inbound 2-player broker packet.
    pub fn apply_rx(
        &mut self,
        seq: u32,
        seat: u8,
        action: u8,
        from: u8,
        to: u8,
        promo: u8,
    ) -> bool {
        if action == TX_NEW_GAME {
            self.state = ChessState::new();
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

        let mut moves = MoveBuffer::new();
        self.state.legal_moves(&mut moves);

        let mut matched: Option<ChessMove> = None;
        for i in 0..moves.count {
            let m = moves.moves[i];
            if m.from == from && m.to == to && m.promo == promo {
                matched = Some(m);
                break;
            }
        }

        if let Some(mv) = matched {
            if self.state.apply_move(mv) {
                self.selected_sq = None;
                self.last_from = Some(mv.from);
                self.last_to = Some(mv.to);
                self.record_last_move(mv);
                self.mp_seq = seq;
                self.tx_pending = false;
                return true;
            }
        }
        false
    }

    fn record_last_move(&mut self, m: ChessMove) {
        let f1 = square_file(m.from) as u8;
        let r1 = square_rank(m.from) as u8;
        let f2 = square_file(m.to) as u8;
        let r2 = square_rank(m.to) as u8;

        let mut pos = 0;
        self.last_move_buf[pos] = b'a' + f1;
        pos += 1;
        self.last_move_buf[pos] = b'1' + r1;
        pos += 1;
        self.last_move_buf[pos] = if m.is_capture { b'x' } else { b'-' };
        pos += 1;
        self.last_move_buf[pos] = b'a' + f2;
        pos += 1;
        self.last_move_buf[pos] = b'1' + r2;
        pos += 1;

        if m.promo != 0 {
            self.last_move_buf[pos] = b'=';
            pos += 1;
            self.last_move_buf[pos] = match m.promo {
                PIECE_QUEEN => b'Q',
                PIECE_ROOK => b'R',
                PIECE_BISHOP => b'B',
                PIECE_KNIGHT => b'N',
                _ => b'Q',
            };
            pos += 1;
        }
        self.last_move_len = pos;
    }

    pub fn handle_square_click(&mut self, sq: u8) -> bool {
        if sq >= 64 || !self.can_local_human_move() {
            return false;
        }

        let mut legal = MoveBuffer::new();
        self.state.legal_moves(&mut legal);

        // If a piece was selected and sq is a legal target:
        if let Some(sel) = self.selected_sq {
            let mut found_move: Option<ChessMove> = None;
            for i in 0..legal.count {
                let m = legal.moves[i];
                if m.from == sel && m.to == sq {
                    // Default promotion to Queen for tap interface
                    if m.promo == 0 || m.promo == PIECE_QUEEN {
                        found_move = Some(m);
                        break;
                    }
                }
            }

            if let Some(m) = found_move {
                return self.execute_human_move(m);
            }
        }

        // Check if square holds active player piece
        let piece = self.state.board[sq as usize];
        if is_piece(piece) && piece_color(piece) == self.state.turn {
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

    fn execute_human_move(&mut self, m: ChessMove) -> bool {
        if !self.state.apply_move(m) {
            return false;
        }

        self.last_from = Some(m.from);
        self.last_to = Some(m.to);
        self.record_last_move(m);
        self.selected_sq = None;

        if self.mode == ChessMode::Broker2P {
            self.mp_seq = self.mp_seq.wrapping_add(1);
            self.pack_tx_move(m);
        } else if self.mode == ChessMode::VsAi
            && self.state.turn == COLOR_BLACK
            && !matches!(
                self.state.status,
                GameStatus::Checkmate | GameStatus::Stalemate | GameStatus::Draw50Move
            )
        {
            self.ai_pending = true;
        }

        true
    }

    pub fn piece_counts(&self) -> (usize, usize) {
        let mut w = 0;
        let mut b = 0;
        for &p in &self.state.board {
            if is_white(p) {
                w += 1;
            } else if is_black(p) {
                b += 1;
            }
        }
        (w, b)
    }

    pub fn mode_button_label(&self) -> &'static str {
        match self.mode {
            ChessMode::VsAi => "Mode: vs AI",
            ChessMode::PassAndPlay => "Mode: Pass & Play",
            ChessMode::Broker2P => {
                if self.local_seat == 2 {
                    "2P · Seat P2 (Blk)"
                } else {
                    "2P · Seat P1 (Wht)"
                }
            }
        }
    }

    pub fn flip_button_label(&self) -> &'static str {
        if self.flipped {
            "View: Blk Bottom"
        } else {
            "View: Wht Bottom"
        }
    }
}

pub struct ChessGeom {
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

pub fn chess_geom(client_w: i32, client_h: i32) -> ChessGeom {
    let _m = Metrics::new(client_w, client_h);
    let landscape = client_w >= client_h && client_w >= 600;

    if landscape {
        let max_board_h = client_h - 44;
        let max_board_w = (client_w * 3 / 5) - 44;
        let avail = max_board_h.min(max_board_w).max(200);
        let max_cell = if client_w >= 1440 || client_h >= 900 { 116 } else { 74 };
        let cell = (avail / 8).clamp(24, max_cell);
        let grid_px = 8 * cell;

        let max_info = if client_w >= 1440 { 420 } else { 360 };
        let info_w = (client_w - grid_px - 96).clamp(200, max_info);
        let total_w = grid_px + 48 + info_w;
        let board_x = ((client_w - total_w) / 2).max(16);
        let board_y = ((client_h - grid_px) / 2).max(12);

        let info_x = board_x + grid_px + 48;
        let info_y = board_y;
        let btn_h = if cell < 42 { 30 } else { 38 };
        let btn_gap = if cell < 42 { 6 } else { 8 };
        let stack_h = 4 * btn_h + 3 * btn_gap;
        let btn_y0 = info_y + (grid_px - stack_h).max(140);

        ChessGeom {
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
        let avail_w = client_w - 32;
        let avail_h = (client_h * 58 / 100).max(200);
        let avail = avail_w.min(avail_h);
        let max_cell = if client_h >= 900 { 96 } else { 68 };
        let cell = (avail / 8).clamp(24, max_cell);
        let grid_px = 8 * cell;

        let board_x = ((client_w - grid_px) / 2).max(12);
        let board_y = 18;
        let info_x = board_x;
        let info_y = board_y + grid_px + 16;
        let info_w = grid_px;

        let btn_h = 34;
        let btn_y0 = (client_h - 2 * btn_h - 20).max(info_y + 92);
        let half_w = (info_w - 8) / 2;

        ChessGeom {
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

fn draw_target_ring(
    fb: &mut Framebuffer,
    cx: i32,
    cy: i32,
    radius: i32,
    color: u32,
    is_capture: bool,
) {
    let outer2 = radius * radius;
    let inner = (radius - 3).max(1);
    let inner2 = inner * inner;
    let dot_r = if is_capture { (radius / 2).max(2) } else { 3 };
    let dot_r2 = dot_r * dot_r;

    for dy in -radius..=radius {
        let py = cy + dy;
        for dx in -radius..=radius {
            let d2 = dx * dx + dy * dy;
            if (d2 <= outer2 && d2 >= inner2) || (!is_capture && d2 <= dot_r2) {
                fb.fill(cx + dx, py, 1, 1, color);
            }
        }
    }
}

fn piece_to_icon(piece: u8) -> Option<Icon> {
    match piece_type(piece) {
        PIECE_PAWN => Some(Icon::Pawn),
        PIECE_KNIGHT => Some(Icon::Knight),
        PIECE_BISHOP => Some(Icon::Bishop),
        PIECE_ROOK => Some(Icon::Rook),
        PIECE_QUEEN => Some(Icon::Queen),
        PIECE_KING => Some(Icon::King),
        _ => None,
    }
}

pub fn draw_window(
    fb: &mut Framebuffer,
    app: &ChessApp,
    anim_tick: u32,
    p1_avatar: Option<&[u32]>,
    p2_avatar: Option<&[u32]>,
    p1_name: Option<&str>,
    p2_name: Option<&str>,
) {
    fb.clear(t::BG);
    let g = chess_geom(fb.w, fb.h);
    let s = &app.state;

    // Board bevel & frame
    fb.fill(
        g.board_x - 6,
        g.board_y - 6,
        g.grid_px + 12,
        g.grid_px + 12,
        t::SURFACE_RAISED,
    );
    fb.border(
        g.board_x - 6,
        g.board_y - 6,
        g.grid_px + 12,
        g.grid_px + 12,
        2,
        t::BORDER_STRONG,
    );

    let mut legal = MoveBuffer::new();
    s.legal_moves(&mut legal);
    let active_sel = app.selected_sq;

    // Palette for chessboard
    let light_sq_col = rgb(0x2d, 0x37, 0x48);
    let light_hi_col = rgb(0x3a, 0x48, 0x5d);
    let light_sh_col = rgb(0x21, 0x29, 0x36);

    let dark_sq_col = rgb(0x0f, 0x17, 0x24);
    let dark_hi_col = rgb(0x19, 0x25, 0x39);
    let dark_sh_col = rgb(0x09, 0x0e, 0x16);

    let last_sq_col = rgb(0x26, 0x23, 0x4b); // Deep indigo
    let sel_sq_col = rgb(0x15, 0x36, 0x45);  // Deep slate cyan

    // Piece colors
    // White pieces: warm pearl / alabaster with warm gold shadow
    let w_fg = rgb(0xff, 0xfd, 0xf5);
    let w_shadow = rgb(0xd9, 0x77, 0x06); // Amber / warm gold shadow

    // Black pieces: obsidian charcoal with electric-cyan rim / shadow
    let b_fg = rgb(0x18, 0x1e, 0x24);
    let b_rim = rgb(0x38, 0xbd, 0xf8); // Electric cyan

    // 1. Draw 8x8 Board & Coordinates
    for v_row in 0..8i32 {
        for v_col in 0..8i32 {
            let f = if app.flipped { 7 - v_col } else { v_col };
            let r = if app.flipped { v_row } else { 7 - v_row };

            let sq = coord_to_square(f as i8, r as i8).unwrap_or(0);
            let is_dark = ((f + r) % 2) == 0; // a1 (0,0) is dark

            let x = g.board_x + v_col * g.cell;
            let y = g.board_y + v_row * g.cell;

            let is_selected = active_sel == Some(sq);
            let is_last = app.last_from == Some(sq) || app.last_to == Some(sq);

            let mut is_dest = false;
            let mut is_dest_capture = false;
            if let Some(sel) = active_sel {
                for m in legal.as_slice() {
                    if m.from == sel && m.to == sq {
                        is_dest = true;
                        if m.is_capture {
                            is_dest_capture = true;
                        }
                    }
                }
            }

            let bg = if is_selected {
                sel_sq_col
            } else if is_last {
                last_sq_col
            } else if is_dark {
                dark_sq_col
            } else {
                light_sq_col
            };

            fb.fill(x, y, g.cell, g.cell, bg);

            // Inset shading / bevel
            if !is_selected && !is_last {
                let (hi, sh) = if is_dark {
                    (dark_hi_col, dark_sh_col)
                } else {
                    (light_hi_col, light_sh_col)
                };
                fb.fill(x, y, g.cell, 1, hi);
                fb.fill(x, y, 1, g.cell, hi);
                fb.fill(x, y + g.cell - 1, g.cell, 1, sh);
                fb.fill(x + g.cell - 1, y, 1, g.cell, sh);
            }

            if is_selected {
                fb.border(x, y, g.cell, g.cell, 2, rgb(0x38, 0xbd, 0xf8));
            } else if is_last {
                fb.border(x, y, g.cell, g.cell, 2, rgb(0x63, 0x66, 0xf1));
            }

            // Board coordinates on outer edges
            let coord_col = rgb(0x64, 0x74, 0x8b);
            if v_col == 0 && g.cell >= 32 {
                let rank_char = [b'1' + r as u8];
                let rank_str = core::str::from_utf8(&rank_char).unwrap_or("");
                draw_str(fb, x + 3, y + 3, rank_str, 1, coord_col);
            }
            if v_row == 7 && g.cell >= 32 {
                let file_char = [b'a' + f as u8];
                let file_str = core::str::from_utf8(&file_char).unwrap_or("");
                draw_str(fb, x + g.cell - 9, y + g.cell - 11, file_str, 1, coord_col);
            }

            // Move target dot or capture ring
            let center_x = x + g.cell / 2;
            let center_y = y + g.cell / 2;
            if is_dest {
                let ring_r = (g.cell / 4).max(5);
                let target_col = if is_dest_capture {
                    rgb(0xf4, 0x3f, 0x5e) // Ruby
                } else {
                    rgb(0x10, 0xb9, 0x81) // Emerald
                };
                draw_target_ring(fb, center_x, center_y, ring_r, target_col, is_dest_capture);
            }

            // Piece rendering
            let piece = s.board[sq as usize];
            if let Some(icon) = piece_to_icon(piece) {
                let icon_sz = (g.cell * 72 / 100).clamp(16, 44);
                let px = center_x - icon_sz / 2;
                let py = center_y - icon_sz / 2;

                if is_white(piece) {
                    // Warm gold shadow offset by +1,+1
                    draw_icon(fb, px + 1, py + 1, icon, icon_sz, w_shadow);
                    // Pearl / alabaster fill
                    draw_icon(fb, px, py, icon, icon_sz, w_fg);
                } else {
                    // Electric cyan rim / halo
                    draw_icon(fb, px - 1, py, icon, icon_sz, b_rim);
                    draw_icon(fb, px + 1, py, icon, icon_sz, b_rim);
                    draw_icon(fb, px, py - 1, icon, icon_sz, b_rim);
                    draw_icon(fb, px, py + 1, icon, icon_sz, b_rim);
                    // Obsidian fill
                    draw_icon(fb, px, py, icon, icon_sz, b_fg);
                }
            }
        }
    }

    // 2. Telemetry & Side HUD Panel
    let compact_info = g.info_w < 260 || fb.h < 500;
    let head_sc = if compact_info { 2 } else { 3 };
    let row_sc = 2;

    let card_top = g.info_y - 4;
    let card_h = (g.btn_new.y - 10 - card_top).max(150);
    let card_rect = Rect::new(g.info_x - 6, card_top, g.info_w + 12, card_h);
    ui::panel(fb, card_rect, t::SURFACE_RAISED, t::BORDER_STRONG);

    let title_str = match app.mode {
        ChessMode::VsAi => "Chess vs AI",
        ChessMode::PassAndPlay => "Chess 8x8",
        ChessMode::Broker2P => "Chess 2P Match",
    };
    draw_str(fb, g.info_x, g.info_y + 2, title_str, head_sc, t::TEXT);

    // Turn / Status banner
    let banner_h = 24;
    let banner_y = g.info_y + line_height(head_sc) + 6;
    let (ban_bg, ban_border, ban_text, ban_text_col) = match s.status {
        GameStatus::Checkmate => {
            if s.turn == COLOR_WHITE {
                (
                    rgb(0x0f, 0x17, 0x2a),
                    rgb(0x38, 0xbd, 0xf8),
                    "CHECKMATE! BLACK WINS",
                    rgb(0x38, 0xbd, 0xf8),
                )
            } else {
                (
                    rgb(0x45, 0x1a, 0x03),
                    rgb(0xfb, 0xbf, 0x24),
                    "CHECKMATE! WHITE WINS",
                    rgb(0xfb, 0xbf, 0x24),
                )
            }
        }
        GameStatus::Stalemate => (
            rgb(0x33, 0x41, 0x55),
            rgb(0x94, 0xa3, 0xb8),
            "STALEMATE (DRAW)",
            rgb(0xf1, 0xf5, 0xf9),
        ),
        GameStatus::Draw50Move => (
            rgb(0x33, 0x41, 0x55),
            rgb(0x94, 0xa3, 0xb8),
            "50-MOVE RULE DRAW",
            rgb(0xf1, 0xf5, 0xf9),
        ),
        GameStatus::Check => {
            if s.turn == COLOR_WHITE {
                (
                    rgb(0x88, 0x13, 0x37),
                    rgb(0xf4, 0x3f, 0x5e),
                    "CHECK! WHITE'S TURN",
                    rgb(0xff, 0xff, 0xff),
                )
            } else {
                (
                    rgb(0x88, 0x13, 0x37),
                    rgb(0xf4, 0x3f, 0x5e),
                    "CHECK! BLACK'S TURN",
                    rgb(0xff, 0xff, 0xff),
                )
            }
        }
        GameStatus::InPlay => {
            if s.turn == COLOR_WHITE {
                (
                    rgb(0x45, 0x1a, 0x03),
                    rgb(0xfb, 0xbf, 0x24),
                    "WHITE'S TURN",
                    rgb(0xfd, 0xba, 0x74),
                )
            } else if app.mode == ChessMode::VsAi {
                (
                    rgb(0x0b, 0x1b, 0x2b),
                    rgb(0x38, 0xbd, 0xf8),
                    "AI THINKING...",
                    rgb(0x7d, 0xd3, 0xfc),
                )
            } else {
                (
                    rgb(0x0b, 0x1b, 0x2b),
                    rgb(0x38, 0xbd, 0xf8),
                    "BLACK'S TURN",
                    rgb(0x7d, 0xd3, 0xfc),
                )
            }
        }
    };
    fb.fill(g.info_x, banner_y, g.info_w, banner_h, ban_bg);
    fb.border(g.info_x, banner_y, g.info_w, banner_h, 1, ban_border);
    draw_str_centered(
        fb,
        g.info_x,
        banner_y,
        g.info_w,
        banner_h,
        ban_text,
        1,
        ban_text_col,
    );

    // 2-Column Player Avatar & Captured Pieces Scoreboard
    let score_y = banner_y + banner_h + 8;
    let col_w = (g.info_w - 6) / 2;
    let col2_x = g.info_x + col_w + 6;
    let (avatar_sz, header_h) = if compact_info {
        (32, 32)
    } else {
        (64, 64)
    };

    // Column 1: White Player Card
    let white_label = p1_name.unwrap_or("WHITE");
    fb.fill(g.info_x, score_y, col_w, header_h, rgb(0x45, 0x1a, 0x03));
    fb.border(g.info_x, score_y, col_w, header_h, 1, rgb(0xb4, 0x53, 0x09));
    let text_x1 = if let Some(avatar) = p1_avatar {
        let frame = match (anim_tick / 2) % 15 {
            12 => 1, // blink
            14 => 2, // shift
            _ => 0,  // rest
        };
        let frame_offset = (frame as usize) * 16384;
        let avatar_slice = &avatar[frame_offset..frame_offset + 16384];
        fb.blit_scaled(g.info_x, score_y, avatar_sz, avatar_sz, avatar_slice, 128, 128);
        g.info_x + avatar_sz + 8
    } else {
        g.info_x + 6
    };
    draw_str(fb, text_x1, score_y + 4, white_label, if compact_info { 1 } else { 2 }, rgb(0xfd, 0xba, 0x74));

    // Column 2: Black Player Card
    let blk_hdr = if let Some(name) = p2_name {
        name
    } else if app.mode == ChessMode::VsAi {
        "BLACK (AI)"
    } else {
        "BLACK"
    };
    fb.fill(col2_x, score_y, col_w, header_h, rgb(0x0a, 0x1e, 0x2f));
    fb.border(col2_x, score_y, col_w, header_h, 1, rgb(0x02, 0x84, 0xc7));
    let text_x2 = if let Some(avatar) = p2_avatar {
        let frame = match ((anim_tick + 7) / 2) % 15 {
            12 => 1, // blink (staggered offset)
            14 => 2, // shift
            _ => 0,  // rest
        };
        let frame_offset = (frame as usize) * 16384;
        let avatar_slice = &avatar[frame_offset..frame_offset + 16384];
        fb.blit_scaled(col2_x, score_y, avatar_sz, avatar_sz, avatar_slice, 128, 128);
        col2_x + avatar_sz + 8
    } else {
        col2_x + 6
    };
    draw_str(fb, text_x2, score_y + 4, blk_hdr, 1, rgb(0x38, 0xbd, 0xf8));

    let (w_count, b_count) = app.piece_counts();
    let row1_y = score_y + header_h + 6;

    let mut w_buf = [0u8; 8];
    let mut b_buf = [0u8; 8];
    draw_str(
        fb,
        g.info_x + 4,
        row1_y,
        "Pieces: ",
        1,
        t::TEXT_DIM,
    );
    draw_str(
        fb,
        g.info_x + 50,
        row1_y,
        f::line_no(&mut w_buf, w_count as u32),
        1,
        t::TEXT,
    );

    draw_str(
        fb,
        col2_x + 4,
        row1_y,
        "Pieces: ",
        1,
        t::TEXT_DIM,
    );
    draw_str(
        fb,
        col2_x + 50,
        row1_y,
        f::line_no(&mut b_buf, b_count as u32),
        1,
        t::TEXT,
    );

    // Mini captured icons row
    let mut w_cap_types = [0u8; 5]; // P, N, B, R, Q captured by White (missing from Black)
    let mut b_cap_types = [0u8; 5]; // P, N, B, R, Q captured by Black (missing from White)

    let mut cur_w_p = 0;
    let mut cur_w_n = 0;
    let mut cur_w_b = 0;
    let mut cur_w_r = 0;
    let mut cur_w_q = 0;

    let mut cur_b_p = 0;
    let mut cur_b_n = 0;
    let mut cur_b_b = 0;
    let mut cur_b_r = 0;
    let mut cur_b_q = 0;

    for &p in &s.board {
        match p {
            p if p == (COLOR_WHITE | PIECE_PAWN) => cur_w_p += 1,
            p if p == (COLOR_WHITE | PIECE_KNIGHT) => cur_w_n += 1,
            p if p == (COLOR_WHITE | PIECE_BISHOP) => cur_w_b += 1,
            p if p == (COLOR_WHITE | PIECE_ROOK) => cur_w_r += 1,
            p if p == (COLOR_WHITE | PIECE_QUEEN) => cur_w_q += 1,
            p if p == (COLOR_BLACK | PIECE_PAWN) => cur_b_p += 1,
            p if p == (COLOR_BLACK | PIECE_KNIGHT) => cur_b_n += 1,
            p if p == (COLOR_BLACK | PIECE_BISHOP) => cur_b_b += 1,
            p if p == (COLOR_BLACK | PIECE_ROOK) => cur_b_r += 1,
            p if p == (COLOR_BLACK | PIECE_QUEEN) => cur_b_q += 1,
            _ => {}
        }
    }

    w_cap_types[0] = 8usize.saturating_sub(cur_b_p) as u8;
    w_cap_types[1] = 2usize.saturating_sub(cur_b_n) as u8;
    w_cap_types[2] = 2usize.saturating_sub(cur_b_b) as u8;
    w_cap_types[3] = 2usize.saturating_sub(cur_b_r) as u8;
    w_cap_types[4] = 1usize.saturating_sub(cur_b_q) as u8;

    b_cap_types[0] = 8usize.saturating_sub(cur_w_p) as u8;
    b_cap_types[1] = 2usize.saturating_sub(cur_w_n) as u8;
    b_cap_types[2] = 2usize.saturating_sub(cur_w_b) as u8;
    b_cap_types[3] = 2usize.saturating_sub(cur_w_r) as u8;
    b_cap_types[4] = 1usize.saturating_sub(cur_w_q) as u8;

    let mini_y = row1_y + 14;
    let mut draw_caps = |ox: i32, caps: &[u8; 5], piece_color: u32| {
        let icons = [
            Icon::Pawn,
            Icon::Knight,
            Icon::Bishop,
            Icon::Rook,
            Icon::Queen,
        ];
        let mut cx = ox + 4;
        for i in 0..5 {
            if caps[i] > 0 {
                draw_icon(fb, cx, mini_y, icons[i], 12, piece_color);
                cx += 13;
                if caps[i] > 1 {
                    let mut b = [0u8; 8];
                    draw_str(fb, cx, mini_y + 2, f::line_no(&mut b, caps[i] as u32), 1, piece_color);
                    cx += 8;
                }
            }
        }
    };
    draw_caps(g.info_x, &w_cap_types, rgb(0xfd, 0xba, 0x74));
    draw_caps(col2_x, &b_cap_types, rgb(0x93, 0xc5, 0xfd));

    // Divider line
    let mut y = mini_y + 16;
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
        let mh_str = f::checkers_moves_hash_str(
            &mut mh_buf,
            s.fullmove_number as u32,
            s.state_hash(),
        );
        ui::kv_row(
            fb,
            row(y),
            "Move/Hash",
            mh_str,
            row_sc,
            t::TEXT_DIM,
            t::TEXT_BODY,
        );
    }

    // 3. Action Buttons
    ui::button(fb, g.btn_new, "New Game (N)", ui::Tone::Neutral, row_sc);

    let mode_tone = match app.mode {
        ChessMode::VsAi => ui::Tone::Accent,
        ChessMode::PassAndPlay => ui::Tone::Neutral,
        ChessMode::Broker2P => {
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
    app: &mut ChessApp,
    cx: i32,
    cy: i32,
    client_w: i32,
    client_h: i32,
) -> InputResult {
    let g = chess_geom(client_w, client_h);

    if Rect::new(g.board_x, g.board_y, g.grid_px, g.grid_px).contains(cx, cy) {
        let v_col = (cx - g.board_x) / g.cell;
        let v_row = (cy - g.board_y) / g.cell;
        if (0..8).contains(&v_col) && (0..8).contains(&v_row) {
            let f = if app.flipped { 7 - v_col } else { v_col };
            let r = if app.flipped { v_row } else { 7 - v_row };
            if let Some(sq) = coord_to_square(f as i8, r as i8) {
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

pub fn key_window(app: &mut ChessApp, key: Key) -> InputResult {
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
        Key::Enter
            if matches!(
                app.state.status,
                GameStatus::Checkmate | GameStatus::Stalemate | GameStatus::Draw50Move
            ) =>
        {
            app.new_game();
            InputResult::Changed
        }
        _ => InputResult::Noop,
    }
}

/// Draw the `128 x h` live icon in the Left App Home Drawer.
pub fn draw_dock_tile(
    fb: &mut Framebuffer,
    app: &ChessApp,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    active: bool,
) {
    ui::dock_tile(fb, Rect::new(x, y, w, h), active);
    let icon_sz = 20;
    let icon_y = y + (h - icon_sz) / 2;
    draw_icon(fb, x + 6, icon_y, Icon::King, icon_sz, t::ACCENT);

    let text_x = x + 10 + icon_sz;
    if h < 56 {
        draw_str_prop(fb, text_x, y + (h - 16) / 2, "Chess", 2, t::TEXT);
    } else {
        draw_str_prop(fb, text_x, y + 16, "Chess", 2, t::TEXT);
        let sub = match app.mode {
            ChessMode::VsAi => "vs AI",
            ChessMode::PassAndPlay => "2P Local",
            ChessMode::Broker2P => "2P Match",
        };
        draw_str_prop(fb, text_x, y + 40, sub, 2, t::ACCENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chess_app_basic_flow_and_ai_step() {
        let mut app = ChessApp::new_const();
        assert_eq!(app.mode, ChessMode::VsAi);
        assert_eq!(app.state.turn, COLOR_WHITE);
        assert!(!app.ai_pending);

        // e2 -> e4: e2 is sq 12, e4 is sq 28
        assert!(app.handle_square_click(12));
        assert_eq!(app.selected_sq, Some(12));
        assert!(app.handle_square_click(28));
        assert_eq!(app.selected_sq, None);
        assert_eq!(app.last_move_str(), "e2-e4");
        assert_eq!(app.state.turn, COLOR_BLACK);
        assert!(app.ai_pending);

        // Step AI
        assert!(app.step_ai());
        assert!(!app.ai_pending);
        assert_eq!(app.state.turn, COLOR_WHITE);
        assert_ne!(app.last_move_str(), "e2-e4");
    }

    #[test]
    fn test_chess_broker_tx_rx() {
        let mut p1 = ChessApp::new_const();
        let mut p2 = ChessApp::new_const();
        p1.configure_mp(true, 10, 1, Difficulty::Medium);
        p2.configure_mp(true, 10, 2, Difficulty::Medium);

        // P2 cannot move on P1's turn
        assert!(!p2.handle_square_click(52)); // e7

        // P1 plays e2 -> e4
        assert!(p1.handle_square_click(12));
        assert!(p1.handle_square_click(28));
        assert!(p1.tx_pending);
        assert_eq!(p1.last_move_str(), "e2-e4");

        let word = p1.take_tx_word();
        let action = (word & 0xFF) as u8;
        let seat = ((word >> 8) & 0xFF) as u8;
        let from = ((word >> 16) & 0xFF) as u8;
        let to = ((word >> 24) & 0xFF) as u8;
        let promo = ((word >> 4) & 0x07) as u8;

        assert_eq!(action, TX_MOVE);
        assert_eq!(seat, 1);
        assert_eq!(from, 12);
        assert_eq!(to, 28);

        // Apply to P2
        assert!(p2.apply_rx(1, seat, action, from, to, promo));
        assert_eq!(p2.last_move_str(), "e2-e4");
        assert_eq!(p1.state.state_hash(), p2.state.state_hash());
        assert_eq!(p2.state.turn, COLOR_BLACK);
    }
}
