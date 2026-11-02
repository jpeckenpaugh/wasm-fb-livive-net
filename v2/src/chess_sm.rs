//! Pure `#![no_std]` Chess state machine and move generator (`chess_sm.rs`).
//!
//! Zero heap allocations, deterministic state hashing, full move generator
//! with castling, en passant, promotion, checkmate/stalemate detection,
//! and Alpha-Beta Minimax search.

pub const EMPTY: u8 = 0;

// Color bits
pub const COLOR_WHITE: u8 = 0x08;
pub const COLOR_BLACK: u8 = 0x10;

// Piece types
pub const PIECE_PAWN: u8 = 1;
pub const PIECE_KNIGHT: u8 = 2;
pub const PIECE_BISHOP: u8 = 3;
pub const PIECE_ROOK: u8 = 4;
pub const PIECE_QUEEN: u8 = 5;
pub const PIECE_KING: u8 = 6;

// Castling rights bitflags
pub const CASTLE_WHITE_KINGSIDE: u8 = 0x01;
pub const CASTLE_WHITE_QUEENSIDE: u8 = 0x02;
pub const CASTLE_BLACK_KINGSIDE: u8 = 0x04;
pub const CASTLE_BLACK_QUEENSIDE: u8 = 0x08;
pub const CASTLE_ALL: u8 = 0x0F;

// Board dimensions
pub const BOARD_SQUARES: usize = 64;
pub const MAX_LEGAL_MOVES: usize = 128;

#[inline]
pub const fn piece_color(piece: u8) -> u8 {
    piece & (COLOR_WHITE | COLOR_BLACK)
}

#[inline]
pub const fn piece_type(piece: u8) -> u8 {
    piece & 0x07
}

#[inline]
pub const fn is_white(piece: u8) -> bool {
    (piece & COLOR_WHITE) != 0
}

#[inline]
pub const fn is_black(piece: u8) -> bool {
    (piece & COLOR_BLACK) != 0
}

#[inline]
pub const fn is_piece(piece: u8) -> bool {
    piece_type(piece) != EMPTY
}

#[inline]
pub const fn opponent_color(color: u8) -> u8 {
    if color == COLOR_WHITE {
        COLOR_BLACK
    } else {
        COLOR_WHITE
    }
}

#[inline]
pub const fn coord_to_square(file: i8, rank: i8) -> Option<u8> {
    if file < 0 || file > 7 || rank < 0 || rank > 7 {
        None
    } else {
        Some((rank as u8) * 8 + (file as u8))
    }
}

#[inline]
pub const fn square_file(sq: u8) -> i8 {
    (sq & 7) as i8
}

#[inline]
pub const fn square_rank(sq: u8) -> i8 {
    (sq >> 3) as i8
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GameStatus {
    InPlay,
    Check,
    Checkmate,
    Stalemate,
    Draw50Move,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChessMove {
    pub from: u8,
    pub to: u8,
    pub promo: u8, // 0 or PIECE_QUEEN, PIECE_ROOK, PIECE_BISHOP, PIECE_KNIGHT
    pub is_capture: bool,
    pub is_castling: bool,
    pub is_en_passant: bool,
}

impl ChessMove {
    pub const fn new(from: u8, to: u8) -> Self {
        Self {
            from,
            to,
            promo: 0,
            is_capture: false,
            is_castling: false,
            is_en_passant: false,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MoveBuffer {
    pub moves: [ChessMove; MAX_LEGAL_MOVES],
    pub count: usize,
}

impl MoveBuffer {
    pub const fn new() -> Self {
        Self {
            moves: [ChessMove::new(0, 0); MAX_LEGAL_MOVES],
            count: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, m: ChessMove) -> bool {
        if self.count < MAX_LEGAL_MOVES {
            self.moves[self.count] = m;
            self.count += 1;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[ChessMove] {
        &self.moves[..self.count]
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ChessState {
    pub board: [u8; BOARD_SQUARES],
    pub turn: u8,
    pub castling: u8,
    pub en_passant: Option<u8>,
    pub halfmove_clock: u16,
    pub fullmove_number: u16,
    pub status: GameStatus,
}

impl ChessState {
    pub const fn new() -> Self {
        let mut board = [EMPTY; BOARD_SQUARES];

        // White pieces (rank 0 = 0..7, rank 1 = 8..15)
        board[0] = COLOR_WHITE | PIECE_ROOK;
        board[1] = COLOR_WHITE | PIECE_KNIGHT;
        board[2] = COLOR_WHITE | PIECE_BISHOP;
        board[3] = COLOR_WHITE | PIECE_QUEEN;
        board[4] = COLOR_WHITE | PIECE_KING;
        board[5] = COLOR_WHITE | PIECE_BISHOP;
        board[6] = COLOR_WHITE | PIECE_KNIGHT;
        board[7] = COLOR_WHITE | PIECE_ROOK;

        let mut i = 8;
        while i < 16 {
            board[i] = COLOR_WHITE | PIECE_PAWN;
            i += 1;
        }

        // Black pieces (rank 6 = 48..55, rank 7 = 56..63)
        let mut j = 48;
        while j < 56 {
            board[j] = COLOR_BLACK | PIECE_PAWN;
            j += 1;
        }

        board[56] = COLOR_BLACK | PIECE_ROOK;
        board[57] = COLOR_BLACK | PIECE_KNIGHT;
        board[58] = COLOR_BLACK | PIECE_BISHOP;
        board[59] = COLOR_BLACK | PIECE_QUEEN;
        board[60] = COLOR_BLACK | PIECE_KING;
        board[61] = COLOR_BLACK | PIECE_BISHOP;
        board[62] = COLOR_BLACK | PIECE_KNIGHT;
        board[63] = COLOR_BLACK | PIECE_ROOK;

        Self {
            board,
            turn: COLOR_WHITE,
            castling: CASTLE_ALL,
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            status: GameStatus::InPlay,
        }
    }

    /// FNV-1a 32-bit state hash.
    pub fn state_hash(&self) -> u32 {
        const FNV_OFFSET_BASIS: u32 = 2166136261;
        const FNV_PRIME: u32 = 16777619;

        let mut hash = FNV_OFFSET_BASIS;
        let mut i = 0;
        while i < BOARD_SQUARES {
            hash ^= self.board[i] as u32;
            hash = hash.wrapping_mul(FNV_PRIME);
            i += 1;
        }

        hash ^= self.turn as u32;
        hash = hash.wrapping_mul(FNV_PRIME);

        hash ^= self.castling as u32;
        hash = hash.wrapping_mul(FNV_PRIME);

        let ep_byte = self.en_passant.map_or(0xFF, |sq| sq);
        hash ^= ep_byte as u32;
        hash = hash.wrapping_mul(FNV_PRIME);

        hash
    }

    /// Finds the square of the king of the given color.
    pub fn find_king(&self, color: u8) -> Option<u8> {
        let king_val = color | PIECE_KING;
        for sq in 0..64 {
            if self.board[sq] == king_val {
                return Some(sq as u8);
            }
        }
        None
    }

    /// Checks if a square is attacked by pieces of `attacker_color`.
    pub fn is_square_attacked(&self, sq: u8, attacker_color: u8) -> bool {
        let f = square_file(sq);
        let r = square_rank(sq);

        // 1. Attacked by pawns
        // If attacker is White, attacking pawn comes from (f-1, r-1) or (f+1, r-1)
        // If attacker is Black, attacking pawn comes from (f-1, r+1) or (f+1, r+1)
        let pawn_p = attacker_color | PIECE_PAWN;
        let pawn_r = if attacker_color == COLOR_WHITE { r - 1 } else { r + 1 };
        if pawn_r >= 0 && pawn_r <= 7 {
            if f > 0 {
                let p_sq = (pawn_r as u8) * 8 + ((f - 1) as u8);
                if self.board[p_sq as usize] == pawn_p {
                    return true;
                }
            }
            if f < 7 {
                let p_sq = (pawn_r as u8) * 8 + ((f + 1) as u8);
                if self.board[p_sq as usize] == pawn_p {
                    return true;
                }
            }
        }

        // 2. Attacked by knights
        const KNIGHT_DELTAS: [(i8, i8); 8] = [
            (-2, -1), (-2, 1), (-1, -2), (-1, 2),
            (1, -2), (1, 2), (2, -1), (2, 1),
        ];
        let knight_p = attacker_color | PIECE_KNIGHT;
        for (df, dr) in KNIGHT_DELTAS {
            let nf = f + df;
            let nr = r + dr;
            if let Some(n_sq) = coord_to_square(nf, nr) {
                if self.board[n_sq as usize] == knight_p {
                    return true;
                }
            }
        }

        // 3. Attacked by king (1 step in 8 directions)
        const KING_DELTAS: [(i8, i8); 8] = [
            (-1, -1), (-1, 0), (-1, 1),
            (0, -1),           (0, 1),
            (1, -1),  (1, 0),  (1, 1),
        ];
        let king_p = attacker_color | PIECE_KING;
        for (df, dr) in KING_DELTAS {
            let kf = f + df;
            let kr = r + dr;
            if let Some(k_sq) = coord_to_square(kf, kr) {
                if self.board[k_sq as usize] == king_p {
                    return true;
                }
            }
        }

        // 4. Sliding: orthogonal (Rooks, Queens)
        const ORTHO_DIRS: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let rook_p = attacker_color | PIECE_ROOK;
        let queen_p = attacker_color | PIECE_QUEEN;
        for (df, dr) in ORTHO_DIRS {
            let mut step = 1;
            loop {
                let cur_f = f + df * step;
                let cur_r = r + dr * step;
                let Some(cur_sq) = coord_to_square(cur_f, cur_r) else {
                    break;
                };
                let piece = self.board[cur_sq as usize];
                if piece != EMPTY {
                    if piece == rook_p || piece == queen_p {
                        return true;
                    }
                    break;
                }
                step += 1;
            }
        }

        // 5. Sliding: diagonal (Bishops, Queens)
        const DIAG_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
        let bishop_p = attacker_color | PIECE_BISHOP;
        for (df, dr) in DIAG_DIRS {
            let mut step = 1;
            loop {
                let cur_f = f + df * step;
                let cur_r = r + dr * step;
                let Some(cur_sq) = coord_to_square(cur_f, cur_r) else {
                    break;
                };
                let piece = self.board[cur_sq as usize];
                if piece != EMPTY {
                    if piece == bishop_p || piece == queen_p {
                        return true;
                    }
                    break;
                }
                step += 1;
            }
        }

        false
    }

    /// Returns true if the king of the specified color is currently in check.
    pub fn is_in_check(&self, color: u8) -> bool {
        let Some(king_sq) = self.find_king(color) else {
            return false;
        };
        self.is_square_attacked(king_sq, opponent_color(color))
    }

    /// Generates pseudo-legal moves for the active turn.
    pub fn generate_pseudo_legal_moves(&self, buf: &mut MoveBuffer) {
        let turn = self.turn;
        let opp = opponent_color(turn);

        for sq in 0..64 {
            let piece = self.board[sq];
            if piece == EMPTY || piece_color(piece) != turn {
                continue;
            }

            let pt = piece_type(piece);
            let f = square_file(sq as u8);
            let r = square_rank(sq as u8);

            match pt {
                PIECE_PAWN => {
                    let (forward_r, start_r, promo_r) = if turn == COLOR_WHITE {
                        (r + 1, 1, 7)
                    } else {
                        (r - 1, 6, 0)
                    };

                    // 1-step forward
                    if let Some(fwd_sq) = coord_to_square(f, forward_r) {
                        if self.board[fwd_sq as usize] == EMPTY {
                            if forward_r == promo_r {
                                for promo in [PIECE_QUEEN, PIECE_ROOK, PIECE_BISHOP, PIECE_KNIGHT] {
                                    buf.push(ChessMove {
                                        from: sq as u8,
                                        to: fwd_sq,
                                        promo,
                                        is_capture: false,
                                        is_castling: false,
                                        is_en_passant: false,
                                    });
                                }
                            } else {
                                buf.push(ChessMove {
                                    from: sq as u8,
                                    to: fwd_sq,
                                    promo: 0,
                                    is_capture: false,
                                    is_castling: false,
                                    is_en_passant: false,
                                });

                                // 2-step initial jump
                                if r == start_r {
                                    let double_r = if turn == COLOR_WHITE { r + 2 } else { r - 2 };
                                    if let Some(dbl_sq) = coord_to_square(f, double_r) {
                                        if self.board[dbl_sq as usize] == EMPTY {
                                            buf.push(ChessMove {
                                                from: sq as u8,
                                                to: dbl_sq,
                                                promo: 0,
                                                is_capture: false,
                                                is_castling: false,
                                                is_en_passant: false,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Diagonal captures
                    for df in [-1, 1] {
                        let cap_f = f + df;
                        if let Some(cap_sq) = coord_to_square(cap_f, forward_r) {
                            let target_piece = self.board[cap_sq as usize];
                            if target_piece != EMPTY && piece_color(target_piece) == opp {
                                if forward_r == promo_r {
                                    for promo in [PIECE_QUEEN, PIECE_ROOK, PIECE_BISHOP, PIECE_KNIGHT] {
                                        buf.push(ChessMove {
                                            from: sq as u8,
                                            to: cap_sq,
                                            promo,
                                            is_capture: true,
                                            is_castling: false,
                                            is_en_passant: false,
                                        });
                                    }
                                } else {
                                    buf.push(ChessMove {
                                        from: sq as u8,
                                        to: cap_sq,
                                        promo: 0,
                                        is_capture: true,
                                        is_castling: false,
                                        is_en_passant: false,
                                    });
                                }
                            } else if Some(cap_sq) == self.en_passant {
                                // En passant capture!
                                buf.push(ChessMove {
                                    from: sq as u8,
                                    to: cap_sq,
                                    promo: 0,
                                    is_capture: true,
                                    is_castling: false,
                                    is_en_passant: true,
                                });
                            }
                        }
                    }
                }
                PIECE_KNIGHT => {
                    const KNIGHT_DELTAS: [(i8, i8); 8] = [
                        (-2, -1), (-2, 1), (-1, -2), (-1, 2),
                        (1, -2), (1, 2), (2, -1), (2, 1),
                    ];
                    for (df, dr) in KNIGHT_DELTAS {
                        if let Some(dest_sq) = coord_to_square(f + df, r + dr) {
                            let target = self.board[dest_sq as usize];
                            if target == EMPTY {
                                buf.push(ChessMove {
                                    from: sq as u8,
                                    to: dest_sq,
                                    promo: 0,
                                    is_capture: false,
                                    is_castling: false,
                                    is_en_passant: false,
                                });
                            } else if piece_color(target) == opp {
                                buf.push(ChessMove {
                                    from: sq as u8,
                                    to: dest_sq,
                                    promo: 0,
                                    is_capture: true,
                                    is_castling: false,
                                    is_en_passant: false,
                                });
                            }
                        }
                    }
                }
                PIECE_BISHOP | PIECE_ROOK | PIECE_QUEEN => {
                    let check_ortho = pt == PIECE_ROOK || pt == PIECE_QUEEN;
                    let check_diag = pt == PIECE_BISHOP || pt == PIECE_QUEEN;

                    const ORTHO_DIRS: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
                    const DIAG_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];

                    if check_ortho {
                        for (df, dr) in ORTHO_DIRS {
                            let mut step = 1;
                            loop {
                                let Some(dest_sq) = coord_to_square(f + df * step, r + dr * step) else {
                                    break;
                                };
                                let target = self.board[dest_sq as usize];
                                if target == EMPTY {
                                    buf.push(ChessMove {
                                        from: sq as u8,
                                        to: dest_sq,
                                        promo: 0,
                                        is_capture: false,
                                        is_castling: false,
                                        is_en_passant: false,
                                    });
                                } else {
                                    if piece_color(target) == opp {
                                        buf.push(ChessMove {
                                            from: sq as u8,
                                            to: dest_sq,
                                            promo: 0,
                                            is_capture: true,
                                            is_castling: false,
                                            is_en_passant: false,
                                        });
                                    }
                                    break;
                                }
                                step += 1;
                            }
                        }
                    }

                    if check_diag {
                        for (df, dr) in DIAG_DIRS {
                            let mut step = 1;
                            loop {
                                let Some(dest_sq) = coord_to_square(f + df * step, r + dr * step) else {
                                    break;
                                };
                                let target = self.board[dest_sq as usize];
                                if target == EMPTY {
                                    buf.push(ChessMove {
                                        from: sq as u8,
                                        to: dest_sq,
                                        promo: 0,
                                        is_capture: false,
                                        is_castling: false,
                                        is_en_passant: false,
                                    });
                                } else {
                                    if piece_color(target) == opp {
                                        buf.push(ChessMove {
                                            from: sq as u8,
                                            to: dest_sq,
                                            promo: 0,
                                            is_capture: true,
                                            is_castling: false,
                                            is_en_passant: false,
                                        });
                                    }
                                    break;
                                }
                                step += 1;
                            }
                        }
                    }
                }
                PIECE_KING => {
                    const KING_DELTAS: [(i8, i8); 8] = [
                        (-1, -1), (-1, 0), (-1, 1),
                        (0, -1),           (0, 1),
                        (1, -1),  (1, 0),  (1, 1),
                    ];
                    for (df, dr) in KING_DELTAS {
                        if let Some(dest_sq) = coord_to_square(f + df, r + dr) {
                            let target = self.board[dest_sq as usize];
                            if target == EMPTY {
                                buf.push(ChessMove {
                                    from: sq as u8,
                                    to: dest_sq,
                                    promo: 0,
                                    is_capture: false,
                                    is_castling: false,
                                    is_en_passant: false,
                                });
                            } else if piece_color(target) == opp {
                                buf.push(ChessMove {
                                    from: sq as u8,
                                    to: dest_sq,
                                    promo: 0,
                                    is_capture: true,
                                    is_castling: false,
                                    is_en_passant: false,
                                });
                            }
                        }
                    }

                    // Castling
                    // White: King at e1 (sq 4).
                    // Black: King at e8 (sq 60).
                    if turn == COLOR_WHITE && sq == 4 {
                        // Kingside (O-O): e1 -> g1 (sq 6). Requires f1 (5), g1 (6) empty.
                        // King must not be in check, and 5, 6 must not be attacked.
                        if (self.castling & CASTLE_WHITE_KINGSIDE) != 0
                            && self.board[5] == EMPTY
                            && self.board[6] == EMPTY
                            && self.board[7] == (COLOR_WHITE | PIECE_ROOK)
                            && !self.is_square_attacked(4, COLOR_BLACK)
                            && !self.is_square_attacked(5, COLOR_BLACK)
                            && !self.is_square_attacked(6, COLOR_BLACK)
                        {
                            buf.push(ChessMove {
                                from: 4,
                                to: 6,
                                promo: 0,
                                is_capture: false,
                                is_castling: true,
                                is_en_passant: false,
                            });
                        }

                        // Queenside (O-O-O): e1 -> c1 (sq 2). Requires d1 (3), c1 (2), b1 (1) empty.
                        // King must not be in check, and 3, 2 must not be attacked.
                        if (self.castling & CASTLE_WHITE_QUEENSIDE) != 0
                            && self.board[1] == EMPTY
                            && self.board[2] == EMPTY
                            && self.board[3] == EMPTY
                            && self.board[0] == (COLOR_WHITE | PIECE_ROOK)
                            && !self.is_square_attacked(4, COLOR_BLACK)
                            && !self.is_square_attacked(3, COLOR_BLACK)
                            && !self.is_square_attacked(2, COLOR_BLACK)
                        {
                            buf.push(ChessMove {
                                from: 4,
                                to: 2,
                                promo: 0,
                                is_capture: false,
                                is_castling: true,
                                is_en_passant: false,
                            });
                        }
                    } else if turn == COLOR_BLACK && sq == 60 {
                        // Kingside (O-O): e8 -> g8 (sq 62). Requires f8 (61), g8 (62) empty.
                        if (self.castling & CASTLE_BLACK_KINGSIDE) != 0
                            && self.board[61] == EMPTY
                            && self.board[62] == EMPTY
                            && self.board[63] == (COLOR_BLACK | PIECE_ROOK)
                            && !self.is_square_attacked(60, COLOR_WHITE)
                            && !self.is_square_attacked(61, COLOR_WHITE)
                            && !self.is_square_attacked(62, COLOR_WHITE)
                        {
                            buf.push(ChessMove {
                                from: 60,
                                to: 62,
                                promo: 0,
                                is_capture: false,
                                is_castling: true,
                                is_en_passant: false,
                            });
                        }

                        // Queenside (O-O-O): e8 -> c8 (sq 58). Requires d8 (59), c8 (58), b8 (57) empty.
                        if (self.castling & CASTLE_BLACK_QUEENSIDE) != 0
                            && self.board[57] == EMPTY
                            && self.board[58] == EMPTY
                            && self.board[59] == EMPTY
                            && self.board[56] == (COLOR_BLACK | PIECE_ROOK)
                            && !self.is_square_attacked(60, COLOR_WHITE)
                            && !self.is_square_attacked(59, COLOR_WHITE)
                            && !self.is_square_attacked(58, COLOR_WHITE)
                        {
                            buf.push(ChessMove {
                                from: 60,
                                to: 58,
                                promo: 0,
                                is_capture: false,
                                is_castling: true,
                                is_en_passant: false,
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Applies a move to the state without legality checks.
    pub fn apply_move_unchecked(&mut self, m: ChessMove) {
        let piece = self.board[m.from as usize];
        let p_type = piece_type(piece);
        let turn = self.turn;

        // Reset en passant target
        self.en_passant = None;

        // Halfmove clock: reset on pawn move or capture
        if p_type == PIECE_PAWN || m.is_capture {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        }

        // Fullmove count: increment after Black's move
        if turn == COLOR_BLACK {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }

        // Handle Castling moves
        if m.is_castling {
            self.board[m.from as usize] = EMPTY;
            self.board[m.to as usize] = piece;

            if m.to == 6 {
                // White Kingside: Rook from 7 to 5
                self.board[7] = EMPTY;
                self.board[5] = COLOR_WHITE | PIECE_ROOK;
            } else if m.to == 2 {
                // White Queenside: Rook from 0 to 3
                self.board[0] = EMPTY;
                self.board[3] = COLOR_WHITE | PIECE_ROOK;
            } else if m.to == 62 {
                // Black Kingside: Rook from 63 to 61
                self.board[63] = EMPTY;
                self.board[61] = COLOR_BLACK | PIECE_ROOK;
            } else if m.to == 58 {
                // Black Queenside: Rook from 56 to 59
                self.board[56] = EMPTY;
                self.board[59] = COLOR_BLACK | PIECE_ROOK;
            }
        } else if m.is_en_passant {
            // Remove captured pawn
            self.board[m.from as usize] = EMPTY;
            self.board[m.to as usize] = piece;

            let cap_sq = if turn == COLOR_WHITE {
                m.to - 8
            } else {
                m.to + 8
            };
            self.board[cap_sq as usize] = EMPTY;
        } else {
            // Normal move or promotion
            self.board[m.from as usize] = EMPTY;
            if m.promo != 0 {
                self.board[m.to as usize] = turn | m.promo;
            } else {
                self.board[m.to as usize] = piece;
            }

            // Set en passant target if pawn moved 2 squares
            if p_type == PIECE_PAWN {
                let from_r = square_rank(m.from);
                let to_r = square_rank(m.to);
                if (to_r - from_r).abs() == 2 {
                    let ep_sq = ((from_r + to_r) / 2) as u8 * 8 + square_file(m.from) as u8;
                    self.en_passant = Some(ep_sq);
                }
            }
        }

        // Update castling rights if king or rook moved or rook was captured
        if m.from == 4 || m.to == 4 {
            self.castling &= !(CASTLE_WHITE_KINGSIDE | CASTLE_WHITE_QUEENSIDE);
        }
        if m.from == 60 || m.to == 60 {
            self.castling &= !(CASTLE_BLACK_KINGSIDE | CASTLE_BLACK_QUEENSIDE);
        }

        if m.from == 0 || m.to == 0 {
            self.castling &= !CASTLE_WHITE_QUEENSIDE;
        }
        if m.from == 7 || m.to == 7 {
            self.castling &= !CASTLE_WHITE_KINGSIDE;
        }
        if m.from == 56 || m.to == 56 {
            self.castling &= !CASTLE_BLACK_QUEENSIDE;
        }
        if m.from == 63 || m.to == 63 {
            self.castling &= !CASTLE_BLACK_KINGSIDE;
        }

        // Switch turn
        self.turn = opponent_color(turn);
    }

    /// Generates all strictly legal moves for the active turn.
    pub fn legal_moves(&self, out: &mut MoveBuffer) {
        out.count = 0;
        let mut pseudo = MoveBuffer::new();
        self.generate_pseudo_legal_moves(&mut pseudo);

        for i in 0..pseudo.count {
            let m = pseudo.moves[i];
            let mut next_state = *self;
            next_state.apply_move_unchecked(m);
            // After applying move, ensure self.turn (the player who moved) is not left in check
            if !next_state.is_in_check(self.turn) {
                out.push(m);
            }
        }
    }

    /// Updates `self.status` based on legal moves and 50-move rule.
    pub fn update_status(&mut self) {
        let mut moves = MoveBuffer::new();
        self.legal_moves(&mut moves);

        let in_chk = self.is_in_check(self.turn);
        if moves.count == 0 {
            if in_chk {
                self.status = GameStatus::Checkmate;
            } else {
                self.status = GameStatus::Stalemate;
            }
        } else if self.halfmove_clock >= 100 {
            // 50 full moves without capture or pawn advance = 100 halfmoves
            self.status = GameStatus::Draw50Move;
        } else if in_chk {
            self.status = GameStatus::Check;
        } else {
            self.status = GameStatus::InPlay;
        }
    }

    /// Applies a legal move and updates game status. Returns false if move is illegal.
    pub fn apply_move(&mut self, m: ChessMove) -> bool {
        let mut moves = MoveBuffer::new();
        self.legal_moves(&mut moves);

        let mut legal = false;
        for i in 0..moves.count {
            let cand = moves.moves[i];
            if cand.from == m.from && cand.to == m.to && cand.promo == m.promo {
                legal = true;
                break;
            }
        }

        if !legal {
            return false;
        }

        self.apply_move_unchecked(m);
        self.update_status();
        true
    }

    /// Static evaluation function: positive favours White, negative favours Black.
    pub fn evaluate(&self) -> i32 {
        if self.status == GameStatus::Checkmate {
            return if self.turn == COLOR_WHITE { -999999 } else { 999999 };
        }
        if self.status == GameStatus::Stalemate || self.status == GameStatus::Draw50Move {
            return 0;
        }

        let mut score = 0i32;

        #[rustfmt::skip]
        const PAWN_TABLE: [i32; 64] = [
             0,  0,  0,  0,  0,  0,  0,  0,
             5, 10, 10,-20,-20, 10, 10,  5,
             5, -5,-10,  0,  0,-10, -5,  5,
             0,  0,  0, 20, 20,  0,  0,  0,
             5,  5, 10, 25, 25, 10,  5,  5,
            10, 10, 20, 30, 30, 20, 10, 10,
            50, 50, 50, 50, 50, 50, 50, 50,
             0,  0,  0,  0,  0,  0,  0,  0,
        ];

        #[rustfmt::skip]
        const KNIGHT_TABLE: [i32; 64] = [
            -50,-40,-30,-30,-30,-30,-40,-50,
            -40,-20,  0,  5,  5,  0,-20,-40,
            -30,  5, 10, 15, 15, 10,  5,-30,
            -30,  0, 15, 20, 20, 15,  0,-30,
            -30,  5, 15, 20, 20, 15,  5,-30,
            -30,  0, 10, 15, 15, 10,  0,-30,
            -40,-20,  0,  0,  0,  0,-20,-40,
            -50,-40,-30,-30,-30,-30,-40,-50,
        ];

        #[rustfmt::skip]
        const BISHOP_TABLE: [i32; 64] = [
            -20,-10,-10,-10,-10,-10,-10,-20,
            -10,  5,  0,  0,  0,  0,  5,-10,
            -10, 10, 10, 10, 10, 10, 10,-10,
            -10,  0, 10, 10, 10, 10,  0,-10,
            -10,  5,  5, 10, 10,  5,  5,-10,
            -10,  0,  5, 10, 10,  5,  0,-10,
            -10,  0,  0,  0,  0,  0,  0,-10,
            -20,-10,-10,-10,-10,-10,-10,-20,
        ];

        #[rustfmt::skip]
        const ROOK_TABLE: [i32; 64] = [
              0,  0,  0,  5,  5,  0,  0,  0,
             -5,  0,  0,  0,  0,  0,  0, -5,
             -5,  0,  0,  0,  0,  0,  0, -5,
             -5,  0,  0,  0,  0,  0,  0, -5,
             -5,  0,  0,  0,  0,  0,  0, -5,
             -5,  0,  0,  0,  0,  0,  0, -5,
              5, 10, 10, 10, 10, 10, 10,  5,
              0,  0,  0,  0,  0,  0,  0,  0,
        ];

        #[rustfmt::skip]
        const QUEEN_TABLE: [i32; 64] = [
            -20,-10,-10, -5, -5,-10,-10,-20,
            -10,  0,  5,  0,  0,  0,  0,-10,
            -10,  5,  5,  5,  5,  5,  0,-10,
              0,  0,  5,  5,  5,  5,  0, -5,
             -5,  0,  5,  5,  5,  5,  0, -5,
            -10,  0,  5,  5,  5,  5,  0,-10,
            -10,  0,  0,  0,  0,  0,  0,-10,
            -20,-10,-10, -5, -5,-10,-10,-20,
        ];

        #[rustfmt::skip]
        const KING_TABLE: [i32; 64] = [
             20, 30, 10,  0,  0, 10, 30, 20,
             20, 20,  0,  0,  0,  0, 20, 20,
            -10,-20,-20,-20,-20,-20,-20,-10,
            -20,-30,-30,-40,-40,-30,-30,-20,
            -30,-40,-40,-50,-50,-40,-40,-30,
            -30,-40,-40,-50,-50,-40,-40,-30,
            -30,-40,-40,-50,-50,-40,-40,-30,
            -30,-40,-40,-50,-50,-40,-40,-30,
        ];

        for sq in 0..64 {
            let p = self.board[sq];
            if p == EMPTY {
                continue;
            }

            let pt = piece_type(p);
            let is_w = is_white(p);
            let (val, pst_val) = match pt {
                PIECE_PAWN => {
                    let pst = if is_w { PAWN_TABLE[sq] } else { PAWN_TABLE[63 - sq] };
                    (100, pst)
                }
                PIECE_KNIGHT => {
                    let pst = if is_w { KNIGHT_TABLE[sq] } else { KNIGHT_TABLE[63 - sq] };
                    (320, pst)
                }
                PIECE_BISHOP => {
                    let pst = if is_w { BISHOP_TABLE[sq] } else { BISHOP_TABLE[63 - sq] };
                    (330, pst)
                }
                PIECE_ROOK => {
                    let pst = if is_w { ROOK_TABLE[sq] } else { ROOK_TABLE[63 - sq] };
                    (500, pst)
                }
                PIECE_QUEEN => {
                    let pst = if is_w { QUEEN_TABLE[sq] } else { QUEEN_TABLE[63 - sq] };
                    (900, pst)
                }
                PIECE_KING => {
                    let pst = if is_w { KING_TABLE[sq] } else { KING_TABLE[63 - sq] };
                    (20000, pst)
                }
                _ => (0, 0),
            };

            let piece_score = val + pst_val;
            if is_w {
                score += piece_score;
            } else {
                score -= piece_score;
            }
        }

        score
    }
}

/// Minimax search with Alpha-Beta pruning.
fn minimax(
    state: &ChessState,
    depth: usize,
    mut alpha: i32,
    mut beta: i32,
    is_maximizing: bool,
) -> i32 {
    let mut moves = MoveBuffer::new();
    state.legal_moves(&mut moves);

    if moves.count == 0 {
        if state.is_in_check(state.turn) {
            // Checkmate: prefer shorter mates
            return if is_maximizing {
                -900000 - (depth as i32) * 1000
            } else {
                900000 + (depth as i32) * 1000
            };
        }
        return 0; // Stalemate
    }

    if depth == 0 || state.halfmove_clock >= 100 {
        return state.evaluate();
    }

    if is_maximizing {
        let mut max_eval = -10000000;
        for i in 0..moves.count {
            let mut next = *state;
            next.apply_move_unchecked(moves.moves[i]);
            let eval = minimax(&next, depth - 1, alpha, beta, false);
            if eval > max_eval {
                max_eval = eval;
            }
            if eval > alpha {
                alpha = eval;
            }
            if beta <= alpha {
                break;
            }
        }
        max_eval
    } else {
        let mut min_eval = 10000000;
        for i in 0..moves.count {
            let mut next = *state;
            next.apply_move_unchecked(moves.moves[i]);
            let eval = minimax(&next, depth - 1, alpha, beta, true);
            if eval < min_eval {
                min_eval = eval;
            }
            if eval < beta {
                beta = eval;
            }
            if beta <= alpha {
                break;
            }
        }
        min_eval
    }
}

/// Selects the best move for the active player using Minimax with Alpha-Beta pruning.
pub fn select_ai_move(state: &ChessState, depth: usize) -> Option<ChessMove> {
    let mut moves = MoveBuffer::new();
    state.legal_moves(&mut moves);

    if moves.count == 0 {
        return None;
    }

    let is_maximizing = state.turn == COLOR_WHITE;
    let mut best_move = moves.moves[0];
    let d = if depth == 0 { 1 } else { depth };

    if is_maximizing {
        let mut best_val = -10000000;
        let mut alpha = -10000000;
        let beta = 10000000;

        for i in 0..moves.count {
            let m = moves.moves[i];
            let mut next = *state;
            next.apply_move_unchecked(m);
            let val = minimax(&next, d - 1, alpha, beta, false);
            if val > best_val {
                best_val = val;
                best_move = m;
            }
            if val > alpha {
                alpha = val;
            }
        }
    } else {
        let mut best_val = 10000000;
        let alpha = -10000000;
        let mut beta = 10000000;

        for i in 0..moves.count {
            let m = moves.moves[i];
            let mut next = *state;
            next.apply_move_unchecked(m);
            let val = minimax(&next, d - 1, alpha, beta, true);
            if val < best_val {
                best_val = val;
                best_move = m;
            }
            if val < beta {
                beta = val;
            }
        }
    }

    Some(best_move)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_setup() {
        let state = ChessState::new();
        assert_eq!(state.turn, COLOR_WHITE);
        assert_eq!(state.status, GameStatus::InPlay);
        assert_eq!(state.castling, CASTLE_ALL);
        assert_eq!(state.en_passant, None);

        // Count pieces
        let mut white_pieces = 0;
        let mut black_pieces = 0;
        for &sq in &state.board {
            if is_white(sq) {
                white_pieces += 1;
            } else if is_black(sq) {
                black_pieces += 1;
            }
        }
        assert_eq!(white_pieces, 16);
        assert_eq!(black_pieces, 16);

        // Standard 20 opening moves for White
        let mut moves = MoveBuffer::new();
        state.legal_moves(&mut moves);
        assert_eq!(moves.count, 20);
    }

    #[test]
    fn test_pawn_double_push_and_en_passant() {
        let mut state = ChessState::new();

        // White e2 -> e4 (12 -> 28)
        assert!(state.apply_move(ChessMove::new(12, 28)));
        assert_eq!(state.en_passant, Some(20)); // e3 is en passant square

        // Black a7 -> a6 (48 -> 40)
        assert!(state.apply_move(ChessMove::new(48, 40)));
        assert_eq!(state.en_passant, None);

        // White e4 -> e5 (28 -> 36)
        assert!(state.apply_move(ChessMove::new(28, 36)));

        // Black d7 -> d5 (51 -> 35)
        assert!(state.apply_move(ChessMove::new(51, 35)));
        assert_eq!(state.en_passant, Some(43)); // d6

        // White en passant capture: e5 x d6 (36 -> 43)
        let ep_move = ChessMove {
            from: 36,
            to: 43,
            promo: 0,
            is_capture: true,
            is_castling: false,
            is_en_passant: true,
        };
        assert!(state.apply_move(ep_move));

        // Black pawn at d5 (35) should now be gone!
        assert_eq!(state.board[35], EMPTY);
        // White pawn at d6 (43)
        assert_eq!(state.board[43], COLOR_WHITE | PIECE_PAWN);
    }

    #[test]
    fn test_sliding_blocked_by_friendly() {
        let state = ChessState::new();
        let mut moves = MoveBuffer::new();
        state.legal_moves(&mut moves);

        // In initial state, rooks (0, 7), bishops (2, 5), and queen (3) cannot move
        for i in 0..moves.count {
            let m = moves.moves[i];
            assert_ne!(m.from, 0, "Rook at a1 should not have legal moves");
            assert_ne!(m.from, 7, "Rook at h1 should not have legal moves");
            assert_ne!(m.from, 2, "Bishop at c1 should not have legal moves");
            assert_ne!(m.from, 5, "Bishop at f1 should not have legal moves");
            assert_ne!(m.from, 3, "Queen at d1 should not have legal moves");
        }
    }

    #[test]
    fn test_in_check_detection_and_evasion() {
        let mut state = ChessState::new();

        // 1. e4 e5
        assert!(state.apply_move(ChessMove::new(12, 28)));
        assert!(state.apply_move(ChessMove::new(52, 36)));

        // 2. Qh5 (3 -> 39) Nc6 (57 -> 42)
        assert!(state.apply_move(ChessMove::new(3, 39)));
        assert!(state.apply_move(ChessMove::new(57, 42)));

        // 3. Qxf7+ (39 -> 53)
        let mut qxf7 = ChessMove::new(39, 53);
        qxf7.is_capture = true;
        assert!(state.apply_move(qxf7));

        assert_eq!(state.status, GameStatus::Check);
        assert!(state.is_in_check(COLOR_BLACK));

        // Black must evade check: King takes Queen Kxf7 (60 -> 53) is legal
        let mut moves = MoveBuffer::new();
        state.legal_moves(&mut moves);
        assert_eq!(moves.count, 1);
        assert_eq!(moves.moves[0].from, 60);
        assert_eq!(moves.moves[0].to, 53);

        let mut kxf7 = ChessMove::new(60, 53);
        kxf7.is_capture = true;
        assert!(state.apply_move(kxf7));
        assert_eq!(state.status, GameStatus::InPlay);
    }

    #[test]
    fn test_kingside_and_queenside_castling() {
        let mut state = ChessState::new();

        // White moves to prepare Kingside castling:
        // 1. e4 (12->28) e5 (52->36)
        assert!(state.apply_move(ChessMove::new(12, 28)));
        assert!(state.apply_move(ChessMove::new(52, 36)));

        // 2. Nf3 (6->21) d6 (51->43)
        assert!(state.apply_move(ChessMove::new(6, 21)));
        assert!(state.apply_move(ChessMove::new(51, 43)));

        // 3. Be2 (5->12) Be6 (58->44)
        assert!(state.apply_move(ChessMove::new(5, 12)));
        assert!(state.apply_move(ChessMove::new(58, 44)));

        // 4. White castles Kingside O-O (4 -> 6)
        let mut castle_k = ChessMove::new(4, 6);
        castle_k.is_castling = true;
        assert!(state.apply_move(castle_k));

        assert_eq!(state.board[4], EMPTY);
        assert_eq!(state.board[6], COLOR_WHITE | PIECE_KING);
        assert_eq!(state.board[7], EMPTY);
        assert_eq!(state.board[5], COLOR_WHITE | PIECE_ROOK);

        // 4... Nc6 (57->42)
        assert!(state.apply_move(ChessMove::new(57, 42)));

        // 5. White a3 (8->16) Qd7 (59->51)
        assert!(state.apply_move(ChessMove::new(8, 16)));
        assert!(state.apply_move(ChessMove::new(59, 51)));

        // 6. White b3 (9->17)
        assert!(state.apply_move(ChessMove::new(9, 17)));

        // 6... Black castles Queenside O-O-O (60 -> 58)
        let mut castle_q = ChessMove::new(60, 58);
        castle_q.is_castling = true;
        assert!(state.apply_move(castle_q));

        assert_eq!(state.board[60], EMPTY);
        assert_eq!(state.board[58], COLOR_BLACK | PIECE_KING);
        assert_eq!(state.board[56], EMPTY);
        assert_eq!(state.board[59], COLOR_BLACK | PIECE_ROOK);
    }

    #[test]
    fn test_scholars_mate_and_fools_mate() {
        // Fool's Mate: 1. f3 e5 2. g4 Qh4#
        let mut state = ChessState::new();
        // 1. f3 (13 -> 21)
        assert!(state.apply_move(ChessMove::new(13, 21)));
        // 1... e5 (52 -> 36)
        assert!(state.apply_move(ChessMove::new(52, 36)));
        // 2. g4 (14 -> 30)
        assert!(state.apply_move(ChessMove::new(14, 30)));
        // 2... Qh4# (59 -> 31)
        assert!(state.apply_move(ChessMove::new(59, 31)));

        assert_eq!(state.status, GameStatus::Checkmate);
        let mut moves = MoveBuffer::new();
        state.legal_moves(&mut moves);
        assert_eq!(moves.count, 0);

        // Scholar's Mate: 1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6 4. Qxf7#
        let mut state2 = ChessState::new();
        assert!(state2.apply_move(ChessMove::new(12, 28))); // e4
        assert!(state2.apply_move(ChessMove::new(52, 36))); // e5
        assert!(state2.apply_move(ChessMove::new(5, 26))); // Bc4
        assert!(state2.apply_move(ChessMove::new(57, 42))); // Nc6
        assert!(state2.apply_move(ChessMove::new(3, 39))); // Qh5
        assert!(state2.apply_move(ChessMove::new(62, 45))); // Nf6
        let mut qxf7 = ChessMove::new(39, 53);
        qxf7.is_capture = true;
        assert!(state2.apply_move(qxf7));

        assert_eq!(state2.status, GameStatus::Checkmate);
    }

    #[test]
    fn test_minimax_ai_selects_legal_moves() {
        let state = ChessState::new();
        let ai_move = select_ai_move(&state, 2);
        assert!(ai_move.is_some());
        let m = ai_move.unwrap();

        let mut legals = MoveBuffer::new();
        state.legal_moves(&mut legals);
        let mut found = false;
        for i in 0..legals.count {
            if legals.moves[i].from == m.from && legals.moves[i].to == m.to {
                found = true;
                break;
            }
        }
        assert!(found, "AI move should be in legal moves buffer");
    }

    #[test]
    fn test_state_hash_determinism_and_uniqueness() {
        let state1 = ChessState::new();
        let state2 = ChessState::new();
        assert_eq!(state1.state_hash(), state2.state_hash());

        let mut state3 = ChessState::new();
        state3.apply_move(ChessMove::new(12, 28));
        assert_ne!(state1.state_hash(), state3.state_hash());
    }
}
