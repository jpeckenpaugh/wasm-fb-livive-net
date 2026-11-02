/**
 * @file state.js
 * Core state model and coordinate mapping for English Draughts (Checkers).
 * Zero external dependencies.
 */

// Square piece values
export const EMPTY = 0;
export const RED_MAN = 1;
export const RED_KING = 2;
export const BLACK_MAN = -1;
export const BLACK_KING = -2;

// Player identifiers
export const PLAYER_RED = 1;     // Human / Bottom player (moves first)
export const PLAYER_BLACK = -1;  // AI / Top player

/**
 * Checks if a piece belongs to Red (1 or 2).
 * @param {number} piece
 * @returns {boolean}
 */
export function isRed(piece) {
  return piece === RED_MAN || piece === RED_KING;
}

/**
 * Checks if a piece belongs to Black (-1 or -2).
 * @param {number} piece
 * @returns {boolean}
 */
export function isBlack(piece) {
  return piece === BLACK_MAN || piece === BLACK_KING;
}

/**
 * Checks if a piece is a King (2 or -2).
 * @param {number} piece
 * @returns {boolean}
 */
export function isKing(piece) {
  return piece === RED_KING || piece === BLACK_KING;
}

/**
 * Checks if a piece belongs to the given player.
 * @param {number} piece
 * @param {number} player - PLAYER_RED (1) or PLAYER_BLACK (-1)
 * @returns {boolean}
 */
export function isPlayerPiece(piece, player) {
  return player === PLAYER_RED ? isRed(piece) : isBlack(piece);
}

/**
 * Checks if a piece belongs to the opponent of the given player.
 * @param {number} piece
 * @param {number} player - PLAYER_RED (1) or PLAYER_BLACK (-1)
 * @returns {boolean}
 */
export function isOpponentPiece(piece, player) {
  return player === PLAYER_RED ? isBlack(piece) : isRed(piece);
}

/**
 * Determines whether an (row, col) position is a playable dark square on an 8x8 board.
 * In English Draughts, dark squares have (row + col) % 2 === 1.
 * @param {number} row - 0..7
 * @param {number} col - 0..7
 * @returns {boolean}
 */
export function isDarkSquare(row, col) {
  if (row < 0 || row > 7 || col < 0 || col > 7) return false;
  return (row + col) % 2 === 1;
}

/**
 * Maps a 32-square index (0..31) to an { row, col } coordinate on the 8x8 grid.
 * @param {number} index - 0..31
 * @returns {{ row: number, col: number }}
 */
export function indexToCoord(index) {
  if (index < 0 || index > 31) {
    throw new RangeError(`Square index out of bounds: ${index}`);
  }
  const row = Math.floor(index / 4);
  const col = (row % 2 === 0) ? (index % 4) * 2 + 1 : (index % 4) * 2;
  return { row, col };
}

/**
 * Maps an 8x8 grid coordinate { row, col } to a 32-square index (0..31).
 * Returns -1 if the coordinate is out of bounds or is a light (unplayable) square.
 * @param {number} row - 0..7
 * @param {number} col - 0..7
 * @returns {number} index (0..31) or -1
 */
export function coordToIndex(row, col) {
  if (!isDarkSquare(row, col)) {
    return -1;
  }
  return row * 4 + Math.floor(col / 2);
}

/**
 * Maps 32-square index (0..31) to standard English Draughts notation (1..32).
 * @param {number} index - 0..31
 * @returns {number} 1..32
 */
export function indexToNotation(index) {
  if (index < 0 || index > 31) {
    throw new RangeError(`Square index out of bounds: ${index}`);
  }
  return index + 1;
}

/**
 * Maps standard English Draughts notation (1..32) to 32-square index (0..31).
 * Returns -1 if notation is invalid.
 * @param {number} notationNumber - 1..32
 * @returns {number} 0..31 or -1
 */
export function notationToIndex(notationNumber) {
  if (notationNumber < 1 || notationNumber > 32 || !Number.isInteger(notationNumber)) {
    return -1;
  }
  return notationNumber - 1;
}

/**
 * Creates a new Int8Array(32) with the standard starting piece layout:
 * - 0..11: BLACK_MAN (-1) [Rows 0, 1, 2]
 * - 12..19: EMPTY (0)      [Rows 3, 4]
 * - 20..31: RED_MAN (1)   [Rows 5, 6, 7]
 * @returns {Int8Array}
 */
export function createInitialBoard() {
  const board = new Int8Array(32);
  for (let i = 0; i < 12; i++) {
    board[i] = BLACK_MAN;
  }
  for (let i = 12; i < 20; i++) {
    board[i] = EMPTY;
  }
  for (let i = 20; i < 32; i++) {
    board[i] = RED_MAN;
  }
  return board;
}

/**
 * Creates the initial game state.
 * @returns {{
 *   board: Int8Array,
 *   turn: number,
 *   selectedSquare: number|null,
 *   mustContinueJumpFrom: number|null,
 *   winner: number|'draw'|null
 * }}
 */
export function createInitialState() {
  return {
    board: createInitialBoard(),
    turn: PLAYER_RED,
    selectedSquare: null,
    mustContinueJumpFrom: null,
    winner: null
  };
}

/**
 * Creates a shallow clone of a 32-square Int8Array board.
 * @param {Int8Array} board
 * @returns {Int8Array}
 */
export function cloneBoard(board) {
  return new Int8Array(board);
}

/**
 * Creates a deep clone of the game state.
 * @param {Object} state
 * @returns {Object}
 */
export function cloneState(state) {
  return {
    board: cloneBoard(state.board),
    turn: state.turn,
    selectedSquare: state.selectedSquare,
    mustContinueJumpFrom: state.mustContinueJumpFrom,
    winner: state.winner
  };
}
