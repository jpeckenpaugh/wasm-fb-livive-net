/**
 * @file ai.js
 * High-performance Minimax search engine with Alpha-Beta pruning,
 * positional heuristic evaluation, move ordering, and quiescence search.
 * Zero external dependencies.
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
  indexToCoord
} from './state.js';

import {
  getLegalMoves,
  applyMoveInPlace,
  undoMoveInPlace,
  checkGameOver
} from './rules.js';

// Material weights
export const WEIGHT_MAN = 100;
export const WEIGHT_KING = 180;

// Positional bonuses
export const BONUS_BACK_RANK = 15;
export const BONUS_CENTER = 10;
export const BONUS_ADVANCED_RANK = 5;
export const PENALTY_EDGE = 8;

// Center squares: rows 2-5, central columns
// In 32-square index:
// Row 2: 9, 10
// Row 3: 13, 14
// Row 4: 17, 18
// Row 5: 21, 22
const CENTER_SQUARES = new Uint8Array([9, 10, 13, 14, 17, 18, 21, 22]);
const IS_CENTER = new Uint8Array(32);
for (let i = 0; i < CENTER_SQUARES.length; i++) {
  IS_CENTER[CENTER_SQUARES[i]] = 1;
}

// Side columns (col 0 or col 7)
const IS_EDGE = new Uint8Array(32);
// Squares on col 0 or 7:
// Row 0: index 0 (col 1), 1 (col 3), 2 (col 5), 3 (col 7) -> 3 is edge
// Row 1: index 4 (col 0), 5 (col 2), 6 (col 4), 7 (col 6) -> 4 is edge
// Row 2: 8, 9, 10, 11 -> 11 is edge
// Row 3: 12, 13, 14, 15 -> 12 is edge
// Row 4: 16, 17, 18, 19 -> 19 is edge
// Row 5: 20, 21, 22, 23 -> 20 is edge
// Row 6: 24, 25, 26, 27 -> 27 is edge
// Row 7: 28, 29, 30, 31 -> 28 is edge
for (let i = 0; i < 32; i++) {
  const { col } = indexToCoord(i);
  if (col === 0 || col === 7) {
    IS_EDGE[i] = 1;
  }
}

// Precomputed row coordinates for each index
const ROW_TABLE = new Uint8Array(32);
for (let i = 0; i < 32; i++) {
  ROW_TABLE[i] = indexToCoord(i).row;
}

/**
 * Difficulty settings configuration.
 */
export const DIFFICULTY_PRESETS = {
  easy: {
    depth: 2,
    quiescence: false,
    randomness: 6
  },
  medium: {
    depth: 4,
    quiescence: false,
    randomness: 0
  },
  hard: {
    depth: 6,
    quiescence: true,
    maxQuiescencePly: 2,
    randomness: 0
  }
};

/**
 * Evaluates the board position from the perspective of the given player.
 * A positive score favors the player; a negative score favors the opponent.
 *
 * @param {Int8Array} board
 * @param {number} player - PLAYER_RED (1) or PLAYER_BLACK (-1)
 * @returns {number} heuristic score
 */
export function evaluateBoard(board, player) {
  let redScore = 0;
  let blackScore = 0;

  for (let i = 0; i < 32; i++) {
    const piece = board[i];
    if (piece === EMPTY) continue;

    const row = ROW_TABLE[i];
    const isCenter = IS_CENTER[i];
    const isEdge = IS_EDGE[i];

    if (isRed(piece)) {
      if (piece === RED_MAN) {
        redScore += WEIGHT_MAN;
        // Back-rank protection (Row 7: squares 28..31)
        if (row === 7) {
          redScore += BONUS_BACK_RANK;
        }
        // Advanced rank progression (moving toward row 0)
        redScore += (7 - row) * BONUS_ADVANCED_RANK;
      } else {
        // Red King
        redScore += WEIGHT_KING;
      }

      // Center control bonus
      if (isCenter) {
        redScore += BONUS_CENTER;
      }

      // Edge penalty for non-kings (limits tactical mobility)
      if (isEdge && piece === RED_MAN) {
        redScore -= PENALTY_EDGE;
      }
    } else {
      // Black piece
      if (piece === BLACK_MAN) {
        blackScore += WEIGHT_MAN;
        // Back-rank protection (Row 0: squares 0..3)
        if (row === 0) {
          blackScore += BONUS_BACK_RANK;
        }
        // Advanced rank progression (moving toward row 7)
        blackScore += row * BONUS_ADVANCED_RANK;
      } else {
        // Black King
        blackScore += WEIGHT_KING;
      }

      // Center control bonus
      if (isCenter) {
        blackScore += BONUS_CENTER;
      }

      // Edge penalty for non-kings
      if (isEdge && piece === BLACK_MAN) {
        blackScore -= PENALTY_EDGE;
      }
    }
  }

  const diff = player === PLAYER_RED ? (redScore - blackScore) : (blackScore - redScore);
  return diff;
}

/**
 * Quiescence search to mitigate the horizon effect on capture tactical lines.
 *
 * @param {Int8Array} board
 * @param {number} alpha
 * @param {number} beta
 * @param {number} player - perspective player
 * @param {number} currentTurnPlayer - whose turn it is to move
 * @param {number} remainingQDepth - max remaining capture ply
 * @param {number|null} requiredPieceIndex - locked square for multi-jump
 * @returns {number}
 */
function quiescenceSearch(board, alpha, beta, player, currentTurnPlayer, remainingQDepth, requiredPieceIndex = null) {
  const standPat = evaluateBoard(board, player) * (currentTurnPlayer === player ? 1 : -1);

  if (standPat >= beta) {
    return beta;
  }
  if (alpha < standPat) {
    alpha = standPat;
  }

  if (remainingQDepth <= 0) {
    return standPat;
  }

  const moves = getLegalMoves(board, currentTurnPlayer, requiredPieceIndex);
  // Check if any captures are available (if the first move is a jump, all are jumps)
  if (moves.length === 0 || !moves[0].isJump) {
    return standPat;
  }

  for (let i = 0; i < moves.length; i++) {
    const move = moves[i];
    const { prevPiece, capturedPiece, crowned } = applyMoveInPlace(board, move);

    // Multi-jump check (unless crowned, which immediately terminates turn)
    let nextTurnPlayer = -currentTurnPlayer;
    let nextRequiredPiece = null;
    let continueJump = false;

    if (!crowned) {
      // Check if continuing jump from destination
      const furtherJumps = getLegalMoves(board, currentTurnPlayer, move.to);
      if (furtherJumps.length > 0) {
        continueJump = true;
        nextTurnPlayer = currentTurnPlayer;
        nextRequiredPiece = move.to;
      }
    }

    let score;
    if (continueJump) {
      // Same player continues jump at same depth or decremented q-depth
      score = quiescenceSearch(board, alpha, beta, player, nextTurnPlayer, remainingQDepth, nextRequiredPiece);
    } else {
      score = -quiescenceSearch(board, -beta, -alpha, player, nextTurnPlayer, remainingQDepth - 1, null);
    }

    undoMoveInPlace(board, move, prevPiece, capturedPiece);

    if (score >= beta) {
      return beta;
    }
    if (score > alpha) {
      alpha = score;
    }
  }

  return alpha;
}

/**
 * Minimax search with Alpha-Beta pruning.
 *
 * @param {Int8Array} board
 * @param {number} depth - remaining search depth
 * @param {number} alpha
 * @param {number} beta
 * @param {number} player - root player we are maximizing for
 * @param {number} currentTurnPlayer - whose turn it is currently
 * @param {number} ply - ply depth from root
 * @param {number|null} requiredPieceIndex - locked square for multi-jump
 * @param {boolean} useQuiescence - whether to extend captures at depth 0
 * @param {number} maxQDepth - maximum quiescence extension
 * @returns {number} score
 */
export function minimax(
  board,
  depth,
  alpha,
  beta,
  player,
  currentTurnPlayer,
  ply,
  requiredPieceIndex = null,
  useQuiescence = false,
  maxQDepth = 2
) {
  // Check terminal win/loss
  const moves = getLegalMoves(board, currentTurnPlayer, requiredPieceIndex);
  if (moves.length === 0) {
    // Current player has lost (trapped or no pieces left)
    // AI favors fast wins and delayed losses
    const lossScore = -100000 + ply;
    return currentTurnPlayer === player ? lossScore : -lossScore;
  }

  // Depth cutoff
  if (depth <= 0) {
    if (useQuiescence && moves[0].isJump) {
      const qScore = quiescenceSearch(board, alpha, beta, player, currentTurnPlayer, maxQDepth, requiredPieceIndex);
      return qScore;
    }
    const evalScore = evaluateBoard(board, player);
    return currentTurnPlayer === player ? evalScore : -evalScore;
  }

  // Move ordering: jumps are already prioritized by mandatory capture rule.
  // When steps exist, prefer moves towards center.
  if (!moves[0].isJump && moves.length > 1) {
    moves.sort((a, b) => {
      const scoreA = IS_CENTER[a.to] * 10 - IS_EDGE[a.to] * 5;
      const scoreB = IS_CENTER[b.to] * 10 - IS_EDGE[b.to] * 5;
      return scoreB - scoreA;
    });
  }

  const isMaximizing = currentTurnPlayer === player;
  let bestScore = isMaximizing ? -Infinity : Infinity;

  for (let i = 0; i < moves.length; i++) {
    const move = moves[i];
    const { prevPiece, capturedPiece, crowned } = applyMoveInPlace(board, move);

    // Multi-jump continuation check
    let continueJump = false;
    let nextTurnPlayer = -currentTurnPlayer;
    let nextRequiredPiece = null;

    if (move.isJump && !crowned) {
      const furtherJumps = getLegalMoves(board, currentTurnPlayer, move.to);
      if (furtherJumps.length > 0) {
        continueJump = true;
        nextTurnPlayer = currentTurnPlayer;
        nextRequiredPiece = move.to;
      }
    }

    let score;
    if (continueJump) {
      // Continuation of jump sequence for same player:
      // Does not decrement depth so full tactical sequence completes
      score = minimax(
        board,
        depth,
        alpha,
        beta,
        player,
        nextTurnPlayer,
        ply + 1,
        nextRequiredPiece,
        useQuiescence,
        maxQDepth
      );
    } else {
      score = minimax(
        board,
        depth - 1,
        alpha,
        beta,
        player,
        nextTurnPlayer,
        ply + 1,
        null,
        useQuiescence,
        maxQDepth
      );
    }

    undoMoveInPlace(board, move, prevPiece, capturedPiece);

    if (isMaximizing) {
      if (score > bestScore) {
        bestScore = score;
      }
      if (bestScore > alpha) {
        alpha = bestScore;
      }
    } else {
      if (score < bestScore) {
        bestScore = score;
      }
      if (bestScore < beta) {
        beta = bestScore;
      }
    }

    if (beta <= alpha) {
      break; // Alpha-beta cutoff
    }
  }

  return bestScore;
}

/**
 * AI Engine class providing move decision-making for any difficulty setting.
 */
export class CheckersAI {
  /**
   * @param {string} [difficulty='medium'] - 'easy', 'medium', or 'hard'
   */
  constructor(difficulty = 'medium') {
    this.setDifficulty(difficulty);
  }

  /**
   * Sets the engine difficulty.
   * @param {string} difficulty - 'easy', 'medium', or 'hard'
   */
  setDifficulty(difficulty) {
    this.difficulty = difficulty;
    this.config = DIFFICULTY_PRESETS[difficulty] || DIFFICULTY_PRESETS.medium;
  }

  /**
   * Selects the best move for the active player from the current game state.
   *
   * @param {{ board: Int8Array, turn: number, mustContinueJumpFrom: number|null }} state
   * @returns {{ from: number, to: number, isJump: boolean, captured: number|null } | null}
   */
  getBestMove(state) {
    const player = state.turn;
    const legalMoves = getLegalMoves(state.board, player, state.mustContinueJumpFrom);

    if (legalMoves.length === 0) {
      return null;
    }

    // Only one legal move (mandatory jump or forced single step) -> immediate return
    if (legalMoves.length === 1) {
      return legalMoves[0];
    }

    const { depth, quiescence, maxQuiescencePly, randomness } = this.config;

    let bestMove = legalMoves[0];
    let bestScore = -Infinity;
    let alpha = -Infinity;
    const beta = Infinity;

    // Shuffle moves slightly to avoid deterministic predictability among equally scored moves
    const candidateMoves = legalMoves.slice();
    for (let i = candidateMoves.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      const temp = candidateMoves[i];
      candidateMoves[i] = candidateMoves[j];
      candidateMoves[j] = temp;
    }

    for (let i = 0; i < candidateMoves.length; i++) {
      const move = candidateMoves[i];
      const { prevPiece, capturedPiece, crowned } = applyMoveInPlace(state.board, move);

      // Multi-jump check
      let continueJump = false;
      let nextTurnPlayer = -player;
      let nextRequiredPiece = null;

      if (move.isJump && !crowned) {
        const furtherJumps = getLegalMoves(state.board, player, move.to);
        if (furtherJumps.length > 0) {
          continueJump = true;
          nextTurnPlayer = player;
          nextRequiredPiece = move.to;
        }
      }

      let score;
      if (continueJump) {
        score = minimax(
          state.board,
          depth,
          alpha,
          beta,
          player,
          nextTurnPlayer,
          1,
          nextRequiredPiece,
          quiescence,
          maxQuiescencePly
        );
      } else {
        score = minimax(
          state.board,
          depth - 1,
          alpha,
          beta,
          player,
          nextTurnPlayer,
          1,
          null,
          quiescence,
          maxQuiescencePly
        );
      }

      undoMoveInPlace(state.board, move, prevPiece, capturedPiece);

      // Add small randomness if configured (e.g. Easy mode)
      const adjustedScore = randomness > 0
        ? score + (Math.random() * randomness * 2 - randomness)
        : score;

      if (adjustedScore > bestScore) {
        bestScore = adjustedScore;
        bestMove = move;
      }

      if (bestScore > alpha) {
        alpha = bestScore;
      }
    }

    return bestMove;
  }
}
