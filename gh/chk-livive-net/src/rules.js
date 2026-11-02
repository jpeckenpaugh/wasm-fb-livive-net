/**
 * @file rules.js
 * Pure rules engine for English Draughts (Checkers).
 * Enforces mandatory captures, multi-jump chains, king promotions,
 * and game-over conditions without external dependencies or DOM access.
 */

import {
  EMPTY,
  RED_MAN,
  RED_KING,
  BLACK_MAN,
  BLACK_KING,
  PLAYER_RED,
  PLAYER_BLACK,
  isRed,
  isBlack,
  isKing,
  isPlayerPiece,
  isOpponentPiece,
  indexToCoord,
  coordToIndex,
  cloneBoard
} from './state.js';

// Precomputed tables for 32 playable squares (0..31)
const STEP_TABLE = new Array(32);
const JUMP_TABLE = new Array(32);

(function precomputeTables() {
  for (let index = 0; index < 32; index++) {
    const { row, col } = indexToCoord(index);

    // Steps (1 diagonal step)
    const upSteps = [];
    const downSteps = [];

    // Up-Left and Up-Right
    const ulStep = coordToIndex(row - 1, col - 1);
    if (ulStep !== -1) upSteps.push(ulStep);
    const urStep = coordToIndex(row - 1, col + 1);
    if (urStep !== -1) upSteps.push(urStep);

    // Down-Left and Down-Right
    const dlStep = coordToIndex(row + 1, col - 1);
    if (dlStep !== -1) downSteps.push(dlStep);
    const drStep = coordToIndex(row + 1, col + 1);
    if (drStep !== -1) downSteps.push(drStep);

    STEP_TABLE[index] = { up: upSteps, down: downSteps };

    // Jumps (2 diagonal steps with piece in between)
    const upJumps = [];
    const downJumps = [];

    // Up-Left Jump
    const ulOver = coordToIndex(row - 1, col - 1);
    const ulTo = coordToIndex(row - 2, col - 2);
    if (ulOver !== -1 && ulTo !== -1) {
      upJumps.push({ over: ulOver, to: ulTo });
    }

    // Up-Right Jump
    const urOver = coordToIndex(row - 1, col + 1);
    const urTo = coordToIndex(row - 2, col + 2);
    if (urOver !== -1 && urTo !== -1) {
      upJumps.push({ over: urOver, to: urTo });
    }

    // Down-Left Jump
    const dlOver = coordToIndex(row + 1, col - 1);
    const dlTo = coordToIndex(row + 2, col - 2);
    if (dlOver !== -1 && dlTo !== -1) {
      downJumps.push({ over: dlOver, to: dlTo });
    }

    // Down-Right Jump
    const drOver = coordToIndex(row + 1, col + 1);
    const drTo = coordToIndex(row + 2, col + 2);
    if (drOver !== -1 && drTo !== -1) {
      downJumps.push({ over: drOver, to: drTo });
    }

    JUMP_TABLE[index] = { up: upJumps, down: downJumps };
  }
})();

/**
 * Returns all valid jump moves originating from a given piece at `fromIndex`.
 * @param {Int8Array} board
 * @param {number} fromIndex - 0..31
 * @param {number|null} [player=null] - optional player identifier override
 * @returns {Array<{ from: number, to: number, isJump: true, captured: number }>}
 */
export function getPieceJumps(board, fromIndex, player = null) {
  const piece = board[fromIndex];
  if (piece === EMPTY) return [];

  const piecePlayer = player !== null ? player : (isRed(piece) ? PLAYER_RED : PLAYER_BLACK);
  if (!isPlayerPiece(piece, piecePlayer)) return [];

  const jumps = [];
  const jumpVectors = [];

  if (piece === RED_MAN) {
    jumpVectors.push(...JUMP_TABLE[fromIndex].up);
  } else if (piece === BLACK_MAN) {
    jumpVectors.push(...JUMP_TABLE[fromIndex].down);
  } else if (isKing(piece)) {
    jumpVectors.push(...JUMP_TABLE[fromIndex].up, ...JUMP_TABLE[fromIndex].down);
  }

  for (let i = 0; i < jumpVectors.length; i++) {
    const { over, to } = jumpVectors[i];
    if (board[to] === EMPTY && isOpponentPiece(board[over], piecePlayer)) {
      jumps.push({
        from: fromIndex,
        to,
        isJump: true,
        captured: over
      });
    }
  }

  return jumps;
}

/**
 * Returns all valid simple step moves originating from a given piece at `fromIndex`.
 * @param {Int8Array} board
 * @param {number} fromIndex - 0..31
 * @param {number|null} [player=null] - optional player identifier override
 * @returns {Array<{ from: number, to: number, isJump: false, captured: null }>}
 */
export function getPieceSteps(board, fromIndex, player = null) {
  const piece = board[fromIndex];
  if (piece === EMPTY) return [];

  const piecePlayer = player !== null ? player : (isRed(piece) ? PLAYER_RED : PLAYER_BLACK);
  if (!isPlayerPiece(piece, piecePlayer)) return [];

  const steps = [];
  const stepDests = [];

  if (piece === RED_MAN) {
    stepDests.push(...STEP_TABLE[fromIndex].up);
  } else if (piece === BLACK_MAN) {
    stepDests.push(...STEP_TABLE[fromIndex].down);
  } else if (isKing(piece)) {
    stepDests.push(...STEP_TABLE[fromIndex].up, ...STEP_TABLE[fromIndex].down);
  }

  for (let i = 0; i < stepDests.length; i++) {
    const dest = stepDests[i];
    if (board[dest] === EMPTY) {
      steps.push({
        from: fromIndex,
        to: dest,
        isJump: false,
        captured: null
      });
    }
  }

  return steps;
}

/**
 * Generates all legal moves for a player on the current board.
 * Enforces:
 * - Mandatory capture (if ANY jump exists, only jump moves are returned).
 * - Multi-jump continuation (if `requiredPieceIndex` is set, only jumps from that square).
 *
 * @param {Int8Array} board
 * @param {number} player - PLAYER_RED (1) or PLAYER_BLACK (-1)
 * @param {number|null} [requiredPieceIndex=null] - square index if chained jump is required
 * @returns {Array<{ from: number, to: number, isJump: boolean, captured: number|null }>}
 */
export function getLegalMoves(board, player, requiredPieceIndex = null) {
  // If constrained to a piece in the middle of a multi-jump sequence:
  if (requiredPieceIndex !== null) {
    return getPieceJumps(board, requiredPieceIndex, player);
  }

  // Check for any available jumps on the board for the current player
  const allJumps = [];
  for (let i = 0; i < 32; i++) {
    if (isPlayerPiece(board[i], player)) {
      const pieceJumps = getPieceJumps(board, i, player);
      if (pieceJumps.length > 0) {
        allJumps.push(...pieceJumps);
      }
    }
  }

  // Mandatory capture rule: if any jumps exist, player MUST jump
  if (allJumps.length > 0) {
    return allJumps;
  }

  // Otherwise, collect all simple steps
  const allSteps = [];
  for (let i = 0; i < 32; i++) {
    if (isPlayerPiece(board[i], player)) {
      const pieceSteps = getPieceSteps(board, i, player);
      if (pieceSteps.length > 0) {
        allSteps.push(...pieceSteps);
      }
    }
  }

  return allSteps;
}

/**
 * Applies a move to the board (immutable).
 *
 * English Draughts crowning rule:
 * If a man reaches the king row during a move (even as part of a jump),
 * it is crowned immediately and the turn TERMINATES immediately.
 * No continuation jumps can be made by a newly crowned king on that turn.
 *
 * @param {Int8Array} board
 * @param {{ from: number, to: number, isJump: boolean, captured: number|null }} move
 * @returns {{
 *   nextBoard: Int8Array,
 *   crowned: boolean,
 *   canContinueJump: boolean
 * }}
 */
export function applyMove(board, move) {
  const nextBoard = cloneBoard(board);
  const piece = nextBoard[move.from];

  nextBoard[move.from] = EMPTY;

  if (move.isJump && move.captured !== null && move.captured !== undefined) {
    nextBoard[move.captured] = EMPTY;
  }

  let crowned = false;
  let finalPiece = piece;

  // Check king row promotion:
  // Red man reaching Row 0 (squares 0..3)
  if (piece === RED_MAN && move.to <= 3) {
    finalPiece = RED_KING;
    crowned = true;
  }
  // Black man reaching Row 7 (squares 28..31)
  else if (piece === BLACK_MAN && move.to >= 28) {
    finalPiece = BLACK_KING;
    crowned = true;
  }

  nextBoard[move.to] = finalPiece;

  // Check multi-jump continuation:
  // English Draughts rule: crowning immediately ends turn (canContinueJump = false).
  let canContinueJump = false;
  if (move.isJump && !crowned) {
    const player = isRed(finalPiece) ? PLAYER_RED : PLAYER_BLACK;
    const furtherJumps = getPieceJumps(nextBoard, move.to, player);
    canContinueJump = furtherJumps.length > 0;
  }

  return {
    nextBoard,
    crowned,
    canContinueJump
  };
}

/**
 * In-place board mutation for Minimax AI search performance.
 * @param {Int8Array} board
 * @param {{ from: number, to: number, isJump: boolean, captured: number|null }} move
 * @returns {{ prevPiece: number, capturedPiece: number, crowned: boolean }}
 */
export function applyMoveInPlace(board, move) {
  const prevPiece = board[move.from];
  board[move.from] = EMPTY;

  let capturedPiece = EMPTY;
  if (move.isJump && move.captured !== null && move.captured !== undefined) {
    capturedPiece = board[move.captured];
    board[move.captured] = EMPTY;
  }

  let crowned = false;
  let finalPiece = prevPiece;

  if (prevPiece === RED_MAN && move.to <= 3) {
    finalPiece = RED_KING;
    crowned = true;
  } else if (prevPiece === BLACK_MAN && move.to >= 28) {
    finalPiece = BLACK_KING;
    crowned = true;
  }

  board[move.to] = finalPiece;

  return { prevPiece, capturedPiece, crowned };
}

/**
 * In-place undo for Minimax AI search backtracking.
 * @param {Int8Array} board
 * @param {{ from: number, to: number, isJump: boolean, captured: number|null }} move
 * @param {number} prevPiece - original piece value at move.from
 * @param {number} capturedPiece - piece value at move.captured before jump
 */
export function undoMoveInPlace(board, move, prevPiece, capturedPiece) {
  board[move.to] = EMPTY;
  board[move.from] = prevPiece;
  if (move.isJump && move.captured !== null && move.captured !== undefined) {
    board[move.captured] = capturedPiece;
  }
}

export const DRAW_RULE_HALF_MOVES = 80;

/**
 * Evaluates whether the game is over and returns the winner.
 * A player wins if the opponent has 0 pieces remaining or 0 legal moves available.
 * A draw is declared if 40 full moves (80 half-moves) occur without a capture or man advance.
 *
 * @param {Int8Array} board
 * @param {number} player - current player whose turn it is to move
 * @param {number} [halfMovesWithoutProgress=0] - turns without capture or man advance
 * @returns {number|'draw'|null} winner (1, -1, 'draw', or null if ongoing)
 */
export function checkGameOver(board, player, halfMovesWithoutProgress = 0) {
  if (halfMovesWithoutProgress >= DRAW_RULE_HALF_MOVES) {
    return 'draw';
  }

  let redCount = 0;
  let blackCount = 0;

  for (let i = 0; i < 32; i++) {
    const piece = board[i];
    if (isRed(piece)) redCount++;
    else if (isBlack(piece)) blackCount++;
  }

  // Piece elimination
  if (redCount === 0) return PLAYER_BLACK;
  if (blackCount === 0) return PLAYER_RED;

  // Move exhaustion / trapped pieces
  const legalMoves = getLegalMoves(board, player);
  if (legalMoves.length === 0) {
    return -player; // Current player cannot move and loses
  }

  return null;
}

/**
 * Returns detailed victory / game-over reason.
 * @param {Int8Array} board
 * @param {number} player
 * @param {number} [halfMovesWithoutProgress=0]
 * @returns {'elimination'|'trapped'|'draw'|null}
 */
export function getGameOverReason(board, player, halfMovesWithoutProgress = 0) {
  if (halfMovesWithoutProgress >= DRAW_RULE_HALF_MOVES) {
    return 'draw';
  }

  let redCount = 0;
  let blackCount = 0;

  for (let i = 0; i < 32; i++) {
    const piece = board[i];
    if (isRed(piece)) redCount++;
    else if (isBlack(piece)) blackCount++;
  }

  if (redCount === 0 || blackCount === 0) {
    return 'elimination';
  }

  const legalMoves = getLegalMoves(board, player);
  if (legalMoves.length === 0) {
    return 'trapped';
  }

  return null;
}

