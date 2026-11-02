//! Pure `#![no_std]` English Draughts (Checkers) rules & Alpha-Beta Minimax engine (`checkers_sm.rs`).
//!
//! Ported from `gh://jpeckenpaugh/chk-livive-net` (`src/state.js`, `src/rules.js`, `src/ai.js`)
//! with zero external dependencies and zero heap allocations.

pub const EMPTY: i8 = 0;
pub const RED_MAN: i8 = 1;
pub const RED_KING: i8 = 2;
pub const BLACK_MAN: i8 = -1;
pub const BLACK_KING: i8 = -2;

pub const PLAYER_RED: i8 = 1; // Human / Seat P1 (starts at indices 20..31, moves UP toward row 0)
pub const PLAYER_BLACK: i8 = -1; // AI / Seat P2 (starts at indices 0..11, moves DOWN toward row 7)

pub const DRAW_RULE_HALF_MOVES: u16 = 80;

// Material weights (matches `ai.js`)
pub const WEIGHT_MAN: i32 = 100;
pub const WEIGHT_KING: i32 = 180;

// Positional bonuses (matches `ai.js`)
pub const BONUS_BACK_RANK: i32 = 15;
pub const BONUS_CENTER: i32 = 10;
pub const BONUS_ADVANCED_RANK: i32 = 5;
pub const PENALTY_EDGE: i32 = 8;

pub const MAX_LEGAL_MOVES: usize = 48;

#[inline]
pub const fn is_red(piece: i8) -> bool {
    piece == RED_MAN || piece == RED_KING
}

#[inline]
pub const fn is_black(piece: i8) -> bool {
    piece == BLACK_MAN || piece == BLACK_KING
}

#[inline]
pub const fn is_king(piece: i8) -> bool {
    piece == RED_KING || piece == BLACK_KING
}

#[inline]
pub const fn is_player_piece(piece: i8, player: i8) -> bool {
    if player == PLAYER_RED {
        is_red(piece)
    } else {
        is_black(piece)
    }
}

#[inline]
pub const fn is_opponent_piece(piece: i8, player: i8) -> bool {
    if player == PLAYER_RED {
        is_black(piece)
    } else {
        is_red(piece)
    }
}

#[inline]
pub const fn is_dark_square(row: i32, col: i32) -> bool {
    if row < 0 || row > 7 || col < 0 || col > 7 {
        return false;
    }
    (row + col) % 2 == 1
}

/// Maps a 32-square index (`0..31`) to `(row, col)` on the 8×8 grid.
#[inline]
pub const fn index_to_coord(index: u8) -> (i32, i32) {
    let idx = (index & 31) as i32;
    let row = idx / 4;
    let col = if row % 2 == 0 {
        (idx % 4) * 2 + 1
    } else {
        (idx % 4) * 2
    };
    (row, col)
}

/// Maps an 8×8 grid coordinate `(row, col)` to a 32-square index (`0..31`), or `None` if light/out-of-bounds.
#[inline]
pub const fn coord_to_index(row: i32, col: i32) -> Option<u8> {
    if !is_dark_square(row, col) {
        return None;
    }
    Some((row * 4 + (col / 2)) as u8)
}

/// Maps a 32-square index (`0..31`) to standard English Draughts PDN notation (`1..32`).
#[inline]
pub const fn index_to_notation(index: u8) -> u8 {
    (index & 31) + 1
}

/// Maps standard English Draughts PDN notation (`1..32`) to a 32-square index (`0..31`).
#[inline]
pub const fn notation_to_index(notation: u8) -> Option<u8> {
    if notation < 1 || notation > 32 {
        None
    } else {
        Some(notation - 1)
    }
}

#[inline]
pub const fn is_center_square(sq: u8) -> bool {
    matches!(sq, 9 | 10 | 13 | 14 | 17 | 18 | 21 | 22)
}

#[inline]
pub const fn is_edge_square(sq: u8) -> bool {
    let (_, col) = index_to_coord(sq);
    col == 0 || col == 7
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CheckersMove {
    pub from: u8,
    pub to: u8,
    pub is_jump: bool,
    pub captured: u8, // 0..31 when is_jump == true, 0xFF when is_jump == false
}

impl CheckersMove {
    pub const NONE: Self = Self {
        from: 0,
        to: 0,
        is_jump: false,
        captured: 0xFF,
    };
}

#[derive(Clone, Copy, Debug)]
pub struct MoveList {
    pub moves: [CheckersMove; MAX_LEGAL_MOVES],
    pub len: usize,
}

impl MoveList {
    pub const fn new() -> Self {
        Self {
            moves: [CheckersMove::NONE; MAX_LEGAL_MOVES],
            len: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, m: CheckersMove) {
        if self.len < MAX_LEGAL_MOVES {
            self.moves[self.len] = m;
            self.len += 1;
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn as_slice(&self) -> &[CheckersMove] {
        &self.moves[..self.len]
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [CheckersMove] {
        &mut self.moves[..self.len]
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UndoRecord {
    pub prev_piece: i8,
    pub captured_piece: i8,
    pub crowned: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ApplyResult {
    pub valid: bool,
    pub is_jump: bool,
    pub crowned: bool,
    pub can_continue_jump: bool,
    pub turn_complete: bool,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Difficulty {
    Easy = 0,
    Medium = 1,
    Hard = 2,
}

impl Difficulty {
    pub const fn from_i32(v: i32) -> Self {
        match v {
            0 => Difficulty::Easy,
            2 => Difficulty::Hard,
            _ => Difficulty::Medium,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Difficulty::Easy => Difficulty::Medium,
            Difficulty::Medium => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Easy,
        }
    }

    pub const fn depth(self) -> i32 {
        match self {
            Difficulty::Easy => 2,
            Difficulty::Medium => 4,
            Difficulty::Hard => 6,
        }
    }

    pub const fn quiescence(self) -> bool {
        matches!(self, Difficulty::Hard)
    }

    pub const fn max_q_ply(self) -> i32 {
        2
    }

    pub const fn label(self) -> &'static str {
        match self {
            Difficulty::Easy => "AI: Easy (2-Ply)",
            Difficulty::Medium => "AI: Medium (4-Ply)",
            Difficulty::Hard => "AI: Hard (6-Ply)",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TurnRecord {
    pub hops: [u8; 8],
    pub hop_len: u8,
    pub is_jump: bool,
}

impl TurnRecord {
    pub const fn empty() -> Self {
        Self {
            hops: [0; 8],
            hop_len: 0,
            is_jump: false,
        }
    }
}

/// Core 32-square English Draughts state machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CheckersState {
    pub board: [i8; 32],
    pub turn: i8,
    pub must_continue_from: Option<u8>,
    pub half_moves_no_progress: u16,
    pub move_count: u32,
    /// `0` = ongoing, `1` = Red/P1 wins, `2` = Black/P2 wins, `3` = Draw.
    pub winner: i8,
}

impl CheckersState {
    pub const fn new_const() -> Self {
        let mut board = [EMPTY; 32];
        let mut i = 0;
        while i < 12 {
            board[i] = BLACK_MAN;
            i += 1;
        }
        let mut j = 20;
        while j < 32 {
            board[j] = RED_MAN;
            j += 1;
        }
        Self {
            board,
            turn: PLAYER_RED,
            must_continue_from: None,
            half_moves_no_progress: 0,
            move_count: 0,
            winner: 0,
        }
    }

    pub fn new() -> Self {
        Self::new_const()
    }

    pub fn reset(&mut self) {
        *self = Self::new_const();
    }

    /// Count `(red_pieces, black_pieces)` on the board.
    pub fn piece_counts(&self) -> (u8, u8) {
        let mut r = 0u8;
        let mut b = 0u8;
        let mut i = 0;
        while i < 32 {
            let p = self.board[i];
            if is_red(p) {
                r += 1;
            } else if is_black(p) {
                b += 1;
            }
            i += 1;
        }
        (r, b)
    }

    /// Count `(red_kings, black_kings)` on the board.
    pub fn king_counts(&self) -> (u8, u8) {
        let mut rk = 0u8;
        let mut bk = 0u8;
        let mut i = 0;
        while i < 32 {
            match self.board[i] {
                RED_KING => rk += 1,
                BLACK_KING => bk += 1,
                _ => {}
            }
            i += 1;
        }
        (rk, bk)
    }

    /// Deterministic 32-bit FNV-1a hash over `board[0..32]`, `turn`, `must_continue_from`, and `winner`.
    pub fn state_hash(&self) -> u32 {
        let mut h: u32 = 0x811c_9dc5;
        let mut i = 0;
        while i < 32 {
            h ^= (self.board[i] as u8) as u32;
            h = h.wrapping_mul(0x0100_0193);
            i += 1;
        }
        h ^= (self.turn as u8) as u32;
        h = h.wrapping_mul(0x0100_0193);
        let cont = match self.must_continue_from {
            Some(sq) => sq,
            None => 0xFF,
        };
        h ^= cont as u32;
        h = h.wrapping_mul(0x0100_0193);
        h ^= (self.winner as u8) as u32;
        h = h.wrapping_mul(0x0100_0193);
        h
    }

    pub fn legal_moves(&self) -> MoveList {
        get_legal_moves(&self.board, self.turn, self.must_continue_from)
    }

    /// Apply a single hop (`from -> to`) if legal for the current active player.
    pub fn apply_hop(&mut self, from: u8, to: u8) -> ApplyResult {
        if self.winner != 0 {
            return ApplyResult {
                valid: false,
                is_jump: false,
                crowned: false,
                can_continue_jump: false,
                turn_complete: false,
            };
        }
        let legal = self.legal_moves();
        let mut chosen = None;
        for m in legal.as_slice() {
            if m.from == from && m.to == to {
                chosen = Some(*m);
                break;
            }
        }
        let Some(mv) = chosen else {
            return ApplyResult {
                valid: false,
                is_jump: false,
                crowned: false,
                can_continue_jump: false,
                turn_complete: false,
            };
        };

        let prev_piece = self.board[mv.from as usize];
        let is_man_advance = prev_piece == RED_MAN || prev_piece == BLACK_MAN;
        let rec = apply_move_in_place(&mut self.board, mv);

        // English Draughts crowning rule: crowning immediately terminates the turn.
        let mut can_continue = false;
        if mv.is_jump && !rec.crowned {
            let further = get_piece_jumps(&self.board, mv.to, self.turn);
            if !further.is_empty() {
                can_continue = true;
            }
        }

        if can_continue {
            self.must_continue_from = Some(mv.to);
        } else {
            self.must_continue_from = None;
            self.move_count = self.move_count.wrapping_add(1);
            if mv.is_jump || is_man_advance {
                self.half_moves_no_progress = 0;
            } else {
                self.half_moves_no_progress = self.half_moves_no_progress.saturating_add(1);
            }
            self.turn = -self.turn;
            self.winner = check_game_over(&self.board, self.turn, self.half_moves_no_progress);
        }

        ApplyResult {
            valid: true,
            is_jump: mv.is_jump,
            crowned: rec.crowned,
            can_continue_jump: can_continue,
            turn_complete: !can_continue,
        }
    }
}

impl Default for CheckersState {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns all valid jump moves originating from `from_index` for `player`.
pub fn get_piece_jumps(board: &[i8; 32], from_index: u8, player: i8) -> MoveList {
    let mut out = MoveList::new();
    if from_index >= 32 {
        return out;
    }
    let piece = board[from_index as usize];
    if piece == EMPTY || !is_player_piece(piece, player) {
        return out;
    }

    let (row, col) = index_to_coord(from_index);
    let can_up = piece == RED_MAN || is_king(piece);
    let can_down = piece == BLACK_MAN || is_king(piece);

    if can_up {
        // Up-Left (-1, -1) -> (-2, -2)
        if let (Some(over), Some(to)) = (
            coord_to_index(row - 1, col - 1),
            coord_to_index(row - 2, col - 2),
        ) {
            if board[to as usize] == EMPTY && is_opponent_piece(board[over as usize], player) {
                out.push(CheckersMove {
                    from: from_index,
                    to,
                    is_jump: true,
                    captured: over,
                });
            }
        }
        // Up-Right (-1, +1) -> (-2, +2)
        if let (Some(over), Some(to)) = (
            coord_to_index(row - 1, col + 1),
            coord_to_index(row - 2, col + 2),
        ) {
            if board[to as usize] == EMPTY && is_opponent_piece(board[over as usize], player) {
                out.push(CheckersMove {
                    from: from_index,
                    to,
                    is_jump: true,
                    captured: over,
                });
            }
        }
    }

    if can_down {
        // Down-Left (+1, -1) -> (+2, -2)
        if let (Some(over), Some(to)) = (
            coord_to_index(row + 1, col - 1),
            coord_to_index(row + 2, col - 2),
        ) {
            if board[to as usize] == EMPTY && is_opponent_piece(board[over as usize], player) {
                out.push(CheckersMove {
                    from: from_index,
                    to,
                    is_jump: true,
                    captured: over,
                });
            }
        }
        // Down-Right (+1, +1) -> (+2, +2)
        if let (Some(over), Some(to)) = (
            coord_to_index(row + 1, col + 1),
            coord_to_index(row + 2, col + 2),
        ) {
            if board[to as usize] == EMPTY && is_opponent_piece(board[over as usize], player) {
                out.push(CheckersMove {
                    from: from_index,
                    to,
                    is_jump: true,
                    captured: over,
                });
            }
        }
    }

    out
}

/// Returns all valid simple step moves originating from `from_index` for `player`.
pub fn get_piece_steps(board: &[i8; 32], from_index: u8, player: i8) -> MoveList {
    let mut out = MoveList::new();
    if from_index >= 32 {
        return out;
    }
    let piece = board[from_index as usize];
    if piece == EMPTY || !is_player_piece(piece, player) {
        return out;
    }

    let (row, col) = index_to_coord(from_index);
    let can_up = piece == RED_MAN || is_king(piece);
    let can_down = piece == BLACK_MAN || is_king(piece);

    if can_up {
        if let Some(to) = coord_to_index(row - 1, col - 1) {
            if board[to as usize] == EMPTY {
                out.push(CheckersMove {
                    from: from_index,
                    to,
                    is_jump: false,
                    captured: 0xFF,
                });
            }
        }
        if let Some(to) = coord_to_index(row - 1, col + 1) {
            if board[to as usize] == EMPTY {
                out.push(CheckersMove {
                    from: from_index,
                    to,
                    is_jump: false,
                    captured: 0xFF,
                });
            }
        }
    }

    if can_down {
        if let Some(to) = coord_to_index(row + 1, col - 1) {
            if board[to as usize] == EMPTY {
                out.push(CheckersMove {
                    from: from_index,
                    to,
                    is_jump: false,
                    captured: 0xFF,
                });
            }
        }
        if let Some(to) = coord_to_index(row + 1, col + 1) {
            if board[to as usize] == EMPTY {
                out.push(CheckersMove {
                    from: from_index,
                    to,
                    is_jump: false,
                    captured: 0xFF,
                });
            }
        }
    }

    out
}

/// Generates all legal moves for `player` on `board`.
/// Enforces mandatory capture and `required_piece` multi-jump continuation lock.
pub fn get_legal_moves(board: &[i8; 32], player: i8, required_piece: Option<u8>) -> MoveList {
    if let Some(sq) = required_piece {
        return get_piece_jumps(board, sq, player);
    }

    let mut all_jumps = MoveList::new();
    for sq in 0..32u8 {
        if is_player_piece(board[sq as usize], player) {
            let pj = get_piece_jumps(board, sq, player);
            for m in pj.as_slice() {
                all_jumps.push(*m);
            }
        }
    }

    if !all_jumps.is_empty() {
        return all_jumps;
    }

    let mut all_steps = MoveList::new();
    for sq in 0..32u8 {
        if is_player_piece(board[sq as usize], player) {
            let ps = get_piece_steps(board, sq, player);
            for m in ps.as_slice() {
                all_steps.push(*m);
            }
        }
    }

    all_steps
}

#[inline]
pub fn apply_move_in_place(board: &mut [i8; 32], mv: CheckersMove) -> UndoRecord {
    let prev_piece = board[mv.from as usize];
    board[mv.from as usize] = EMPTY;

    let mut captured_piece = EMPTY;
    if mv.is_jump && mv.captured < 32 {
        captured_piece = board[mv.captured as usize];
        board[mv.captured as usize] = EMPTY;
    }

    let mut crowned = false;
    let mut final_piece = prev_piece;
    if prev_piece == RED_MAN && mv.to <= 3 {
        final_piece = RED_KING;
        crowned = true;
    } else if prev_piece == BLACK_MAN && mv.to >= 28 {
        final_piece = BLACK_KING;
        crowned = true;
    }

    board[mv.to as usize] = final_piece;

    UndoRecord {
        prev_piece,
        captured_piece,
        crowned,
    }
}

#[inline]
pub fn undo_move_in_place(board: &mut [i8; 32], mv: CheckersMove, rec: UndoRecord) {
    board[mv.to as usize] = EMPTY;
    board[mv.from as usize] = rec.prev_piece;
    if mv.is_jump && mv.captured < 32 {
        board[mv.captured as usize] = rec.captured_piece;
    }
}

/// Evaluates whether the game is over and returns the winner code:
/// `0` = ongoing, `1` = Red/P1 wins, `2` = Black/P2 wins, `3` = Draw.
pub fn check_game_over(board: &[i8; 32], current_player: i8, half_moves_no_progress: u16) -> i8 {
    if half_moves_no_progress >= DRAW_RULE_HALF_MOVES {
        return 3;
    }

    let mut red_count = 0u8;
    let mut black_count = 0u8;
    for &p in board.iter() {
        if is_red(p) {
            red_count += 1;
        } else if is_black(p) {
            black_count += 1;
        }
    }

    if red_count == 0 {
        return 2; // Black wins
    }
    if black_count == 0 {
        return 1; // Red wins
    }

    let legal = get_legal_moves(board, current_player, None);
    if legal.is_empty() {
        return if current_player == PLAYER_RED { 2 } else { 1 };
    }

    0
}

/// Evaluates the board position from the perspective of `player` (`PLAYER_RED` or `PLAYER_BLACK`).
/// Matches `evaluateBoard` in `ai.js`.
pub fn evaluate_board(board: &[i8; 32], player: i8) -> i32 {
    let mut red_score: i32 = 0;
    let mut black_score: i32 = 0;

    for sq in 0..32u8 {
        let piece = board[sq as usize];
        if piece == EMPTY {
            continue;
        }

        let (row, _) = index_to_coord(sq);
        let center = is_center_square(sq);
        let edge = is_edge_square(sq);

        if is_red(piece) {
            if piece == RED_MAN {
                red_score += WEIGHT_MAN;
                if row == 7 {
                    red_score += BONUS_BACK_RANK;
                }
                red_score += (7 - row) * BONUS_ADVANCED_RANK;
            } else {
                red_score += WEIGHT_KING;
            }
            if center {
                red_score += BONUS_CENTER;
            }
            if edge && piece == RED_MAN {
                red_score -= PENALTY_EDGE;
            }
        } else {
            if piece == BLACK_MAN {
                black_score += WEIGHT_MAN;
                if row == 0 {
                    black_score += BONUS_BACK_RANK;
                }
                black_score += row * BONUS_ADVANCED_RANK;
            } else {
                black_score += WEIGHT_KING;
            }
            if center {
                black_score += BONUS_CENTER;
            }
            if edge && piece == BLACK_MAN {
                black_score -= PENALTY_EDGE;
            }
        }
    }

    if player == PLAYER_RED {
        red_score - black_score
    } else {
        black_score - red_score
    }
}

#[inline]
fn move_order_score(mv: CheckersMove) -> i32 {
    let center = if is_center_square(mv.to) { 10 } else { 0 };
    let edge = if is_edge_square(mv.to) { 5 } else { 0 };
    center - edge
}

fn order_step_moves(list: &mut MoveList) {
    if list.len <= 1 || list.moves[0].is_jump {
        return;
    }
    let slice = list.as_mut_slice();
    let mut i = 1;
    while i < slice.len() {
        let key = slice[i];
        let key_score = move_order_score(key);
        let mut j = i;
        while j > 0 && move_order_score(slice[j - 1]) < key_score {
            slice[j] = slice[j - 1];
            j -= 1;
        }
        slice[j] = key;
        i += 1;
    }
}

/// Quiescence search to mitigate the horizon effect on capture tactical lines.
pub fn quiescence_search(
    board: &mut [i8; 32],
    mut alpha: i32,
    beta: i32,
    player: i8,
    current_turn_player: i8,
    remaining_q_depth: i32,
    required_piece: Option<u8>,
) -> i32 {
    let sign = if current_turn_player == player { 1 } else { -1 };
    let stand_pat = evaluate_board(board, player) * sign;

    if stand_pat >= beta {
        return beta;
    }
    if alpha < stand_pat {
        alpha = stand_pat;
    }
    if remaining_q_depth <= 0 {
        return stand_pat;
    }

    let moves = get_legal_moves(board, current_turn_player, required_piece);
    if moves.is_empty() || !moves.moves[0].is_jump {
        return stand_pat;
    }

    for i in 0..moves.len {
        let mv = moves.moves[i];
        let rec = apply_move_in_place(board, mv);

        let mut next_turn_player = -current_turn_player;
        let mut next_required = None;
        let mut continue_jump = false;

        if !rec.crowned {
            let further = get_piece_jumps(board, mv.to, current_turn_player);
            if !further.is_empty() {
                continue_jump = true;
                next_turn_player = current_turn_player;
                next_required = Some(mv.to);
            }
        }

        let score = if continue_jump {
            quiescence_search(
                board,
                alpha,
                beta,
                player,
                next_turn_player,
                remaining_q_depth,
                next_required,
            )
        } else {
            -quiescence_search(
                board,
                -beta,
                -alpha,
                player,
                next_turn_player,
                remaining_q_depth - 1,
                None,
            )
        };

        undo_move_in_place(board, mv, rec);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}

/// Alpha-Beta Minimax search (matches `minimax` in `ai.js`).
#[allow(clippy::too_many_arguments)]
pub fn minimax(
    board: &mut [i8; 32],
    depth: i32,
    mut alpha: i32,
    mut beta: i32,
    player: i8,
    current_turn_player: i8,
    ply: i32,
    required_piece: Option<u8>,
    use_quiescence: bool,
    max_q_depth: i32,
) -> i32 {
    let mut moves = get_legal_moves(board, current_turn_player, required_piece);
    if moves.is_empty() {
        let loss_score = -100_000 + ply;
        return if current_turn_player == player {
            loss_score
        } else {
            -loss_score
        };
    }

    if depth <= 0 {
        if use_quiescence && moves.moves[0].is_jump {
            return quiescence_search(
                board,
                alpha,
                beta,
                player,
                current_turn_player,
                max_q_depth,
                required_piece,
            );
        }
        let eval_score = evaluate_board(board, player);
        return if current_turn_player == player {
            eval_score
        } else {
            -eval_score
        };
    }

    order_step_moves(&mut moves);

    let is_maximizing = current_turn_player == player;
    let mut best_score = if is_maximizing {
        i32::MIN / 2
    } else {
        i32::MAX / 2
    };

    for i in 0..moves.len {
        let mv = moves.moves[i];
        let rec = apply_move_in_place(board, mv);

        let mut continue_jump = false;
        let mut next_turn_player = -current_turn_player;
        let mut next_required = None;

        if mv.is_jump && !rec.crowned {
            let further = get_piece_jumps(board, mv.to, current_turn_player);
            if !further.is_empty() {
                continue_jump = true;
                next_turn_player = current_turn_player;
                next_required = Some(mv.to);
            }
        }

        let score = if continue_jump {
            minimax(
                board,
                depth,
                alpha,
                beta,
                player,
                next_turn_player,
                ply + 1,
                next_required,
                use_quiescence,
                max_q_depth,
            )
        } else {
            minimax(
                board,
                depth - 1,
                alpha,
                beta,
                player,
                next_turn_player,
                ply + 1,
                None,
                use_quiescence,
                max_q_depth,
            )
        };

        undo_move_in_place(board, mv, rec);

        if is_maximizing {
            if score > best_score {
                best_score = score;
            }
            if best_score > alpha {
                alpha = best_score;
            }
        } else {
            if score < best_score {
                best_score = score;
            }
            if best_score < beta {
                beta = best_score;
            }
        }

        if beta <= alpha {
            break;
        }
    }

    best_score
}

/// Selects the best single hop (or next hop in a multi-jump chain) for `state.turn`.
pub fn select_best_move(
    state: &CheckersState,
    difficulty: Difficulty,
    rng_seed: u32,
) -> Option<CheckersMove> {
    let player = state.turn;
    let mut legal = get_legal_moves(&state.board, player, state.must_continue_from);
    if legal.is_empty() {
        return None;
    }
    if legal.len == 1 {
        return Some(legal.moves[0]);
    }

    order_step_moves(&mut legal);

    let depth = difficulty.depth();
    let use_q = difficulty.quiescence();
    let max_q = difficulty.max_q_ply();

    let mut board = state.board;
    let mut best_move = legal.moves[0];
    let mut best_score = i32::MIN / 2;
    let mut alpha = i32::MIN / 2;
    let beta = i32::MAX / 2;

    let mut rng = if rng_seed == 0 {
        state.state_hash() ^ 0x9E37_79B9
    } else {
        rng_seed
    };

    for i in 0..legal.len {
        let mv = legal.moves[i];
        let rec = apply_move_in_place(&mut board, mv);

        let mut continue_jump = false;
        let mut next_turn_player = -player;
        let mut next_required = None;

        if mv.is_jump && !rec.crowned {
            let further = get_piece_jumps(&board, mv.to, player);
            if !further.is_empty() {
                continue_jump = true;
                next_turn_player = player;
                next_required = Some(mv.to);
            }
        }

        let score = if continue_jump {
            minimax(
                &mut board,
                depth,
                alpha,
                beta,
                player,
                next_turn_player,
                1,
                next_required,
                use_q,
                max_q,
            )
        } else {
            minimax(
                &mut board,
                depth - 1,
                alpha,
                beta,
                player,
                next_turn_player,
                1,
                None,
                use_q,
                max_q,
            )
        };

        undo_move_in_place(&mut board, mv, rec);

        // Xorshift32 tie-breaker / Easy-mode jitter
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        let jitter = if difficulty == Difficulty::Easy {
            ((rng % 13) as i32) - 6
        } else {
            0
        };
        let adjusted = score.saturating_add(jitter);

        if adjusted > best_score {
            best_score = adjusted;
            best_move = mv;
        }
        if best_score > alpha {
            alpha = best_score;
        }
    }

    Some(best_move)
}

/// Executes a complete AI turn (including any multi-jump continuation chain) on `state`
/// and returns the full hop path (`hops: [u8; 8]`, `hop_len`, `is_jump`).
pub fn select_full_ai_turn(state: &mut CheckersState, difficulty: Difficulty) -> TurnRecord {
    let mut record = TurnRecord::empty();
    if state.winner != 0 {
        return record;
    }

    let starting_player = state.turn;
    let mut step_idx = 0usize;

    while state.winner == 0 && state.turn == starting_player && step_idx < 7 {
        let seed = state.state_hash().wrapping_add(step_idx as u32);
        let Some(mv) = select_best_move(state, difficulty, seed) else {
            break;
        };
        if step_idx == 0 {
            record.hops[0] = mv.from;
            record.hops[1] = mv.to;
            record.hop_len = 2;
            record.is_jump = mv.is_jump;
        } else if (record.hop_len as usize) < record.hops.len() {
            record.hops[record.hop_len as usize] = mv.to;
            record.hop_len += 1;
        }

        let res = state.apply_hop(mv.from, mv.to);
        step_idx += 1;
        if !res.valid || res.turn_complete {
            break;
        }
    }

    record
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_board_and_coordinate_mapping() {
        let s = CheckersState::new();
        assert_eq!(s.turn, PLAYER_RED);
        assert_eq!(s.piece_counts(), (12, 12));
        assert_eq!(s.king_counts(), (0, 0));
        assert_eq!(s.winner, 0);

        for sq in 0..32u8 {
            let (r, c) = index_to_coord(sq);
            assert!(is_dark_square(r, c));
            assert_eq!(coord_to_index(r, c), Some(sq));
            assert_eq!(notation_to_index(index_to_notation(sq)), Some(sq));
        }

        // Red has 7 opening legal steps from row 5 (squares 20..23)
        let moves = s.legal_moves();
        assert_eq!(moves.len, 7);
        for m in moves.as_slice() {
            assert!(!m.is_jump);
            assert!((20..=23).contains(&m.from));
            assert!((16..=19).contains(&m.to));
        }
    }

    #[test]
    fn mandatory_capture_and_multi_jump_and_crowning_terminates_turn() {
        let mut s = CheckersState::new();
        s.board = [EMPTY; 32];
        // Red man at 22 (row 5, col 4); Black men at 17 (row 4, col 3) and 9 (row 2, col 3)
        // Jump 1: 22 -> 13 (over 17)
        // Jump 2: 13 -> 6 (over 9)
        s.board[22] = RED_MAN;
        s.board[27] = RED_MAN; // Another Red man that would have normal steps if capture wasn't mandatory
        s.board[17] = BLACK_MAN;
        s.board[9] = BLACK_MAN;
        s.turn = PLAYER_RED;

        let legal = s.legal_moves();
        // Mandatory capture: only the jump from 22 -> 13 is legal; 27's steps are excluded!
        assert_eq!(legal.len, 1);
        assert!(legal.moves[0].is_jump);
        assert_eq!(legal.moves[0].from, 22);
        assert_eq!(legal.moves[0].to, 13);
        assert_eq!(legal.moves[0].captured, 17);

        let r1 = s.apply_hop(22, 13);
        assert!(r1.valid);
        assert!(r1.is_jump);
        assert!(r1.can_continue_jump);
        assert!(!r1.turn_complete);
        assert_eq!(s.must_continue_from, Some(13));
        assert_eq!(s.turn, PLAYER_RED);

        let r2 = s.apply_hop(13, 6);
        assert!(r2.valid);
        assert!(r2.is_jump);
        assert!(!r2.can_continue_jump);
        assert!(r2.turn_complete);
        assert_eq!(s.must_continue_from, None);
        // Both Black pieces captured -> Red wins immediately (winner == 1)
        assert_eq!(s.winner, 1);

        // Now verify English Draughts crowning rule: jumping onto king row crowns and terminates turn immediately
        let mut s2 = CheckersState::new();
        s2.board = [EMPTY; 32];
        // Red man at 9 (row 2, col 3), Black man at 5 (row 1, col 2) -> jumps to 0 (row 0, col 1)
        // Even if another Black piece is at 4 (row 1, col 0) with 8 (row 2, col 1) empty,
        // crowning at 0 MUST terminate the turn immediately!
        s2.board[9] = RED_MAN;
        s2.board[5] = BLACK_MAN;
        s2.board[4] = BLACK_MAN;
        s2.turn = PLAYER_RED;

        let rc = s2.apply_hop(9, 0);
        assert!(rc.valid);
        assert!(rc.crowned);
        assert!(!rc.can_continue_jump);
        assert!(rc.turn_complete);
        assert_eq!(s2.board[0], RED_KING);
        assert_eq!(s2.turn, PLAYER_BLACK);
    }

    #[test]
    fn minimax_ai_selects_legal_moves_across_all_difficulties() {
        for diff in [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard] {
            let mut s = CheckersState::new();
            // Red plays 20 -> 16
            assert!(s.apply_hop(20, 16).valid);
            assert_eq!(s.turn, PLAYER_BLACK);

            let turn_rec = select_full_ai_turn(&mut s, diff);
            assert!(turn_rec.hop_len >= 2);
            assert_eq!(s.turn, PLAYER_RED);
        }

        // Verify AI completes a multi-jump chain in one turn
        let mut s = CheckersState::new();
        s.board = [EMPTY; 32];
        // Black man at 6 (row 1, col 4); Red men at 10 (row 2, col 5) and 18 (row 4, col 5) and 31
        // Black jumps 6 -> 15 (over 10) -> 22 (over 18)
        s.board[6] = BLACK_MAN;
        s.board[10] = RED_MAN;
        s.board[18] = RED_MAN;
        s.board[31] = RED_MAN;
        s.turn = PLAYER_BLACK;

        let rec = select_full_ai_turn(&mut s, Difficulty::Medium);
        assert!(rec.is_jump);
        assert_eq!(rec.hop_len, 3);
        assert_eq!(&rec.hops[..3], &[6, 15, 22]);
        assert_eq!(s.turn, PLAYER_RED);
    }
}
