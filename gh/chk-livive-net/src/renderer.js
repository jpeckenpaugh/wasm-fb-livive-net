/**
 * @file renderer.js
 * DOM rendering, board grid generation, piece presentation,
 * highlights, and zero-dependency Web Audio effects for English Draughts.
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
  indexToCoord,
  coordToIndex,
  indexToNotation,
  isDarkSquare
} from './state.js';

// SVG Crown icon markup for crowned King pieces
const CROWN_SVG = `
<svg class="crown-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
  <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .55-.45 1-1 1H6c-.55 0-1-.45-1-1v-1h14v1z"/>
</svg>`.trim();

/**
 * Audio Synthesizer using Web Audio API for zero-dependency sound effects.
 */
class SoundEffects {
  constructor() {
    this.ctx = null;
    this.muted = false;
  }

  _initContext() {
    if (!this.ctx && typeof window !== 'undefined' && (window.AudioContext || window.webkitAudioContext)) {
      const AudioCtx = window.AudioContext || window.webkitAudioContext;
      this.ctx = new AudioCtx();
    }
    if (this.ctx && this.ctx.state === 'suspended') {
      this.ctx.resume().catch(() => {});
    }
  }

  playMove() {
    if (this.muted) return;
    this._initContext();
    if (!this.ctx) return;

    try {
      const now = this.ctx.currentTime;
      const osc = this.ctx.createOscillator();
      const gain = this.ctx.createGain();

      osc.type = 'triangle';
      osc.frequency.setValueAtTime(220, now);
      osc.frequency.exponentialRampToValueAtTime(110, now + 0.05);

      gain.gain.setValueAtTime(0.3, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.05);

      osc.connect(gain);
      gain.connect(this.ctx.destination);

      osc.start(now);
      osc.stop(now + 0.05);
    } catch (e) {
      // Audio context may be restricted in non-user-gesture environment
    }
  }

  playCapture() {
    if (this.muted) return;
    this._initContext();
    if (!this.ctx) return;

    try {
      const now = this.ctx.currentTime;
      const osc = this.ctx.createOscillator();
      const gain = this.ctx.createGain();

      osc.type = 'sine';
      osc.frequency.setValueAtTime(150, now);
      osc.frequency.exponentialRampToValueAtTime(50, now + 0.09);

      gain.gain.setValueAtTime(0.5, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.09);

      osc.connect(gain);
      gain.connect(this.ctx.destination);

      osc.start(now);
      osc.stop(now + 0.09);
    } catch (e) {}
  }

  playCrown() {
    if (this.muted) return;
    this._initContext();
    if (!this.ctx) return;

    try {
      const now = this.ctx.currentTime;
      const notes = [587.33, 880]; // D5 -> A5 fanfare chime
      notes.forEach((freq, idx) => {
        const noteTime = now + idx * 0.08;
        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();

        osc.type = 'sine';
        osc.frequency.setValueAtTime(freq, noteTime);

        gain.gain.setValueAtTime(0.28, noteTime);
        gain.gain.exponentialRampToValueAtTime(0.001, noteTime + 0.18);

        osc.connect(gain);
        gain.connect(this.ctx.destination);

        osc.start(noteTime);
        osc.stop(noteTime + 0.18);
      });
    } catch (e) {}
  }

  playWin() {
    if (this.muted) return;
    this._initContext();
    if (!this.ctx) return;

    try {
      const now = this.ctx.currentTime;
      const chord = [523.25, 659.25, 783.99, 1046.5]; // C5, E5, G5, C6
      chord.forEach((freq, idx) => {
        const noteTime = now + idx * 0.09;
        const osc = this.ctx.createOscillator();
        const gain = this.ctx.createGain();

        osc.type = 'triangle';
        osc.frequency.setValueAtTime(freq, noteTime);

        gain.gain.setValueAtTime(0.3, noteTime);
        gain.gain.exponentialRampToValueAtTime(0.001, noteTime + 0.35);

        osc.connect(gain);
        gain.connect(this.ctx.destination);

        osc.start(noteTime);
        osc.stop(noteTime + 0.35);
      });
    } catch (e) {}
  }
}

/**
 * CheckersRenderer handles all UI updates, DOM construction, and visual state.
 */
export class CheckersRenderer {
  constructor(boardElement) {
    this.boardElement = boardElement;
    this.sound = new SoundEffects();

    // Cache of 32 dark square DOM elements (indexed 0..31)
    this.darkSquareElements = new Array(32);
    // Cache of all 64 square elements
    this.allSquareElements = [];

    // DOM UI elements
    this.turnBadge = document.getElementById('turn-badge');
    this.turnDot = document.getElementById('turn-dot');
    this.turnText = document.getElementById('turn-text');
    this.turnMessage = document.getElementById('turn-message');

    this.playerRedCard = document.getElementById('player-red-card');
    this.playerBlackCard = document.getElementById('player-black-card');

    this.redPiecesCountEl = document.getElementById('red-pieces-count');
    this.redKingsCountEl = document.getElementById('red-kings-count');
    this.blackPiecesCountEl = document.getElementById('black-pieces-count');
    this.blackKingsCountEl = document.getElementById('black-kings-count');

    this.modal = document.getElementById('game-over-modal');
    this.modalTitle = document.getElementById('modal-title');
    this.modalMessage = document.getElementById('modal-message');
    this.modalTrophy = document.getElementById('modal-trophy');
    this.modalReasonBadge = document.getElementById('modal-reason-badge');
    this.modalStatTurns = document.getElementById('modal-stat-turns');
    this.modalStatTime = document.getElementById('modal-stat-time');
    this.modalStatRedCaptures = document.getElementById('modal-stat-red-captures');
    this.modalStatBlackCaptures = document.getElementById('modal-stat-black-captures');
    this.modalStatKings = document.getElementById('modal-stat-kings');
    this.btnModalReview = document.getElementById('btn-modal-review');
    this.btnReopenModal = document.getElementById('btn-reopen-modal');

    this.multiJumpBanner = document.getElementById('multijump-banner');
    this.multiJumpText = document.getElementById('multijump-text');

    this.soundOnIcon = document.getElementById('icon-sound-on');
    this.soundOffIcon = document.getElementById('icon-sound-off');
    this.soundLabel = document.getElementById('sound-label');

    // Arena and LLM Banter elements
    this.speechBubbleRed = document.getElementById('speech-bubble-red');
    this.speechBubbleBlack = document.getElementById('speech-bubble-black');
    this.playerRedNameEl = document.getElementById('player-red-name');
    this.playerBlackNameEl = document.getElementById('player-black-name');
    this.playerRedAvatarEl = document.getElementById('player-red-avatar');
    this.playerBlackAvatarEl = document.getElementById('player-black-avatar');

    this.arenaFeedSection = document.getElementById('arena-feed-section');
    this.arenaFeedList = document.getElementById('arena-feed-list');

    this.webLLMProgressContainer = document.getElementById('webllm-progress-container');
    this.webLLMProgressLabel = document.getElementById('webllm-progress-label');
    this.webLLMProgressPercent = document.getElementById('webllm-progress-percent');
    this.webLLMProgressFill = document.getElementById('webllm-progress-fill');

    this.spectatorControls = document.getElementById('spectator-controls');
    this.btnSpectatorPlay = document.getElementById('btn-spectator-play');
    this.spectatorPlayIcon = document.getElementById('spectator-play-icon');
    this.spectatorPlayText = document.getElementById('spectator-play-text');
    this.btnSpectatorStep = document.getElementById('btn-spectator-step');
    this.selectSpectatorSpeed = document.getElementById('select-spectator-speed');

    this._buildBoardGrid();
  }

  /**
   * Generates the 8x8 grid in the board container.
   * Dark squares are stored in `darkSquareElements` by 32-square index.
   */
  _buildBoardGrid() {
    this.boardElement.innerHTML = '';
    this.allSquareElements = [];

    for (let row = 0; row < 8; row++) {
      for (let col = 0; col < 8; col++) {
        const square = document.createElement('div');
        square.dataset.row = String(row);
        square.dataset.col = String(col);

        if (isDarkSquare(row, col)) {
          const index = coordToIndex(row, col);
          square.className = 'square square-dark';
          square.dataset.index = String(index);

          // Draughts notation badge (1..32)
          const notationBadge = document.createElement('span');
          notationBadge.className = 'square-notation';
          notationBadge.textContent = String(indexToNotation(index));
          square.appendChild(notationBadge);

          this.darkSquareElements[index] = square;
        } else {
          square.className = 'square square-light';
        }

        this.boardElement.appendChild(square);
        this.allSquareElements.push(square);
      }
    }
  }

  /**
   * Updates sound muted UI state.
   * @param {boolean} isMuted
   */
  setMuted(isMuted) {
    this.sound.muted = isMuted;
    if (this.soundOnIcon && this.soundOffIcon) {
      if (isMuted) {
        this.soundOnIcon.classList.add('hidden');
        this.soundOffIcon.classList.remove('hidden');
        if (this.soundLabel) this.soundLabel.textContent = 'Muted';
      } else {
        this.soundOnIcon.classList.remove('hidden');
        this.soundOffIcon.classList.add('hidden');
        if (this.soundLabel) this.soundLabel.textContent = 'Sound';
      }
    }
  }

  /**
   * Toggles display of square numbers (1..32).
   * @param {boolean} show
   */
  setShowNotation(show) {
    if (show) {
      this.boardElement.classList.add('show-notation');
    } else {
      this.boardElement.classList.remove('show-notation');
    }
  }

  /**
   * Toggles board rotation by 180 degrees.
   * @param {boolean} flipped
   */
  setBoardFlipped(flipped) {
    if (flipped) {
      this.boardElement.classList.add('flipped');
    } else {
      this.boardElement.classList.remove('flipped');
    }
  }

  /**
   * Performs slide (200ms) or hop (250ms) animation, and triggers
   * dissipation animation (200ms) on jumped piece.
   * Resolves Promise when animation completes.
   * @param {{ from: number, to: number, isJump: boolean, captured: number|null }} move
   * @returns {Promise<void>}
   */
  async animateMove(move) {
    if (typeof window === 'undefined' || !this.darkSquareElements) {
      return Promise.resolve();
    }

    const fromSquare = this.darkSquareElements[move.from];
    const toSquare = this.darkSquareElements[move.to];
    if (!fromSquare || !toSquare) {
      return Promise.resolve();
    }

    const pieceEl = fromSquare.querySelector('.piece');
    if (!pieceEl) {
      return Promise.resolve();
    }

    const fromCoord = indexToCoord(move.from);
    const toCoord = indexToCoord(move.to);
    const dCol = toCoord.col - fromCoord.col;
    const dRow = toCoord.row - fromCoord.row;

    // Animate capture dissipation on jumped piece
    if (move.isJump && move.captured !== null && move.captured !== undefined) {
      const capturedSquare = this.darkSquareElements[move.captured];
      if (capturedSquare) {
        const capturedPiece = capturedSquare.querySelector('.piece');
        if (capturedPiece) {
          capturedPiece.classList.add('anim-dissipating');
        }
      }
    }

    return new Promise((resolve) => {
      if (move.isJump) {
        pieceEl.style.setProperty('--move-dx', `${dCol * 100}%`);
        pieceEl.style.setProperty('--move-dy', `${dRow * 100}%`);
        pieceEl.classList.add('anim-hopping');
        setTimeout(resolve, 250);
      } else {
        pieceEl.classList.add('anim-sliding');
        pieceEl.style.transform = `translate(${dCol * 100}%, ${dRow * 100}%)`;
        setTimeout(resolve, 200);
      }
    });
  }

  /**
   * Full re-render of the board and scoreboard based on current game state.
   * @param {Object} renderData
   * @param {Object} renderData.state - current state object
   * @param {Array} renderData.legalMovesForSelected - legal moves for selected piece
   * @param {Set<number>} renderData.selectableIndices - set of square indices player can pick
   * @param {Object|null} renderData.lastMove - { from, to }
   * @param {boolean} renderData.mustJump - whether mandatory jump is active
   * @param {string} renderData.message - status message to display
   * @param {number|null} [renderData.newlyCrownedIndex] - index of piece crowned on this move
   * @param {Object|null} [renderData.stats] - match statistics
   * @param {string|null} [renderData.winReason] - victory reason
   * @param {Set<number>} [renderData.preyIndices] - indices of pieces about to be jumped
   */
  render({
    state,
    legalMovesForSelected = [],
    selectableIndices = new Set(),
    lastMove = null,
    mustJump = false,
    message = '',
    newlyCrownedIndex = null,
    stats = null,
    winReason = null,
    preyIndices = new Set()
  }) {
    // 1. Clear highlights on all dark squares
    for (let i = 0; i < 32; i++) {
      const square = this.darkSquareElements[i];
      if (!square) continue;

      square.classList.remove('valid-dest', 'is-jump-dest', 'last-move-from', 'last-move-to', 'is-prey-square');

      // Clear existing piece element (retaining notation badge)
      const existingPiece = square.querySelector('.piece');
      if (existingPiece) {
        existingPiece.remove();
      }

      // Check board piece value
      const pieceVal = state.board[i];
      if (pieceVal !== EMPTY) {
        const pieceEl = document.createElement('div');
        pieceEl.className = 'piece';

        if (isRed(pieceVal)) {
          pieceEl.classList.add('red');
        } else if (isBlack(pieceVal)) {
          pieceEl.classList.add('black');
        }

        if (isKing(pieceVal)) {
          pieceEl.classList.add('king');
          pieceEl.innerHTML = CROWN_SVG;
        }

        if (selectableIndices.has(i)) {
          pieceEl.classList.add('selectable');
        }

        if (state.selectedSquare === i) {
          pieceEl.classList.add('selected');
        }

        if (state.mustContinueJumpFrom === i) {
          pieceEl.classList.add('multi-jump-active');
        }

        if (preyIndices && preyIndices.has(i)) {
          pieceEl.classList.add('target-prey');
          square.classList.add('is-prey-square');
        }

        if (newlyCrownedIndex === i) {
          pieceEl.classList.add('newly-crowned');
        }

        square.appendChild(pieceEl);
      }
    }

    // 2. Highlight legal move destinations for selected piece
    if (legalMovesForSelected && legalMovesForSelected.length > 0) {
      for (let j = 0; j < legalMovesForSelected.length; j++) {
        const move = legalMovesForSelected[j];
        const destSquare = this.darkSquareElements[move.to];
        if (destSquare) {
          destSquare.classList.add('valid-dest');
          if (move.isJump) {
            destSquare.classList.add('is-jump-dest');
          }
        }
      }
    }

    // 3. Highlight last move (from & to)
    if (lastMove) {
      const fromSquare = this.darkSquareElements[lastMove.from];
      const toSquare = this.darkSquareElements[lastMove.to];
      if (fromSquare) fromSquare.classList.add('last-move-from');
      if (toSquare) toSquare.classList.add('last-move-to');
    }

    // 4. Update Scoreboard and Stats
    this._updateStats(state);

    // 5. Update Status Banner and Multi-Jump Callout
    this._updateStatus(state, mustJump, message);

    // 6. Game Over Modal
    if (state.winner !== null) {
      this._showGameOver(state, stats, winReason);
    } else {
      this._hideGameOver();
    }
  }

  _updateStats(state) {
    let redCount = 0;
    let redKings = 0;
    let blackCount = 0;
    let blackKings = 0;

    for (let i = 0; i < 32; i++) {
      const piece = state.board[i];
      if (piece === RED_MAN) redCount++;
      else if (piece === RED_KING) { redCount++; redKings++; }
      else if (piece === BLACK_MAN) blackCount++;
      else if (piece === BLACK_KING) { blackCount++; blackKings++; }
    }

    if (this.redPiecesCountEl) this.redPiecesCountEl.textContent = String(redCount);
    if (this.redKingsCountEl) this.redKingsCountEl.textContent = String(redKings);
    if (this.blackPiecesCountEl) this.blackPiecesCountEl.textContent = String(blackCount);
    if (this.blackKingsCountEl) this.blackKingsCountEl.textContent = String(blackKings);

    // Active card highlighting
    if (this.playerRedCard && this.playerBlackCard) {
      if (state.turn === PLAYER_RED) {
        this.playerRedCard.classList.add('active-turn');
        this.playerBlackCard.classList.remove('active-turn');
      } else {
        this.playerBlackCard.classList.add('active-turn');
        this.playerRedCard.classList.remove('active-turn');
      }
    }
  }

  _updateStatus(state, mustJump, customMessage) {
    const isRedTurn = state.turn === PLAYER_RED;

    if (this.turnText) {
      this.turnText.textContent = isRedTurn ? "Red's Turn" : "Black's Turn";
    }

    if (this.turnDot) {
      if (isRedTurn) {
        this.turnDot.classList.remove('black-turn');
      } else {
        this.turnDot.classList.add('black-turn');
      }
    }

    if (this.turnMessage) {
      if (customMessage) {
        this.turnMessage.textContent = customMessage;
        if (/thinking/i.test(customMessage)) {
          this.turnMessage.classList.add('thinking');
          this.turnMessage.classList.remove('alert');
        } else {
          this.turnMessage.classList.add('alert');
          this.turnMessage.classList.remove('thinking');
        }
      } else if (state.mustContinueJumpFrom !== null) {
        this.turnMessage.textContent = 'Multi-jump! You must continue the capture chain.';
        this.turnMessage.classList.add('alert');
        this.turnMessage.classList.remove('thinking');
      } else if (mustJump) {
        this.turnMessage.textContent = 'Capture available! Mandatory jump required.';
        this.turnMessage.classList.add('alert');
        this.turnMessage.classList.remove('thinking');
      } else {
        this.turnMessage.textContent = 'Select a piece to move.';
        this.turnMessage.classList.remove('alert');
        this.turnMessage.classList.remove('thinking');
      }
    }

    // Dynamic Multi-Jump Callout Banner
    if (this.multiJumpBanner) {
      if (state.mustContinueJumpFrom !== null) {
        this.multiJumpBanner.classList.remove('hidden');
        if (this.multiJumpText) {
          this.multiJumpText.textContent = 'Capture chain in progress! Complete the remaining jump.';
        }
      } else {
        this.multiJumpBanner.classList.add('hidden');
      }
    }
  }

  _showGameOver(state, stats = null, winReason = null) {
    if (!this.modal) return;

    let title = 'Victory!';
    let trophy = '🏆';
    let badge = 'Victory';
    let msg = '';

    const winnerIsRed = state.winner === PLAYER_RED;
    const winnerName = winnerIsRed ? 'Red' : 'Black';
    const loserName = winnerIsRed ? 'Black' : 'Red';

    if (state.winner === 'draw') {
      title = 'Draw Game';
      trophy = '🤝';
      badge = '40-Move Rule';
      msg = '40 full moves (80 turns) were played without a capture or piece advance.';
    } else if (winReason === 'elimination') {
      title = `${winnerName} Victory!`;
      trophy = '🏆';
      badge = 'Elimination';
      msg = `Outstanding match! ${winnerName} captured all opposing pieces.`;
    } else if (winReason === 'trapped') {
      title = `${winnerName} Victory!`;
      trophy = '👑';
      badge = 'No Legal Moves';
      msg = `${loserName} was completely trapped with no legal moves remaining.`;
    } else {
      title = `${winnerName} Wins!`;
      trophy = '🏆';
      badge = 'Victory';
      msg = `Congratulations! ${winnerName} has won the match!`;
    }

    if (this.modalTitle) this.modalTitle.textContent = title;
    if (this.modalTrophy) this.modalTrophy.textContent = trophy;
    if (this.modalReasonBadge) this.modalReasonBadge.textContent = badge;
    if (this.modalMessage) this.modalMessage.textContent = msg;

    if (stats) {
      if (this.modalStatTurns) this.modalStatTurns.textContent = String(stats.turnCount || 0);
      if (this.modalStatRedCaptures) this.modalStatRedCaptures.textContent = String(stats.redCaptures || 0);
      if (this.modalStatBlackCaptures) this.modalStatBlackCaptures.textContent = String(stats.blackCaptures || 0);
      if (this.modalStatKings) {
        this.modalStatKings.textContent = `${(stats.redKingsCrowned || 0) + (stats.blackKingsCrowned || 0)} (${stats.redKingsCrowned || 0} Red, ${stats.blackKingsCrowned || 0} Black)`;
      }
      if (this.modalStatTime) {
        const elapsed = stats.startTime ? Math.max(0, Math.floor((Date.now() - stats.startTime) / 1000)) : 0;
        const mins = Math.floor(elapsed / 60);
        const secs = elapsed % 60;
        this.modalStatTime.textContent = `${mins}:${secs < 10 ? '0' : ''}${secs}`;
      }
    }

    this.modal.classList.remove('hidden');
    if (this.btnReopenModal) {
      this.btnReopenModal.classList.add('hidden');
    }
  }

  hideModalForReview() {
    if (this.modal) {
      this.modal.classList.add('hidden');
    }
    if (this.btnReopenModal) {
      this.btnReopenModal.classList.remove('hidden');
    }
  }

  showReviewRecap() {
    if (this.modal) {
      this.modal.classList.remove('hidden');
    }
    if (this.btnReopenModal) {
      this.btnReopenModal.classList.add('hidden');
    }
  }

  _hideGameOver() {
    if (this.modal) {
      this.modal.classList.add('hidden');
    }
    if (this.btnReopenModal) {
      this.btnReopenModal.classList.add('hidden');
    }
  }

  /**
   * Updates player badge with custom name, avatar, and persona title.
   * @param {number} color
   * @param {Object} info
   */
  setPlayerInfo(color, { name, avatar }) {
    const isRed = color === PLAYER_RED;
    const nameEl = isRed ? this.playerRedNameEl : this.playerBlackNameEl;
    const avatarEl = isRed ? this.playerRedAvatarEl : this.playerBlackAvatarEl;
    if (nameEl && name) nameEl.textContent = name;
    if (avatarEl && avatar) avatarEl.textContent = avatar;
  }

  /**
   * Streams banter into the player's active speech bubble.
   * @param {number} color
   * @param {string} text
   */
  streamBanter(color, text) {
    const isRed = color === PLAYER_RED;
    const bubble = isRed ? this.speechBubbleRed : this.speechBubbleBlack;
    if (bubble) {
      const textEl = bubble.querySelector('.bubble-text') || bubble;
      textEl.textContent = text;
      bubble.classList.remove('hidden');
    }
  }

  /**
   * Hides speech bubble for a player.
   * @param {number} color
   */
  hideBanter(color) {
    const isRed = color === PLAYER_RED;
    const bubble = isRed ? this.speechBubbleRed : this.speechBubbleBlack;
    if (bubble) {
      bubble.classList.add('hidden');
    }
  }

  /**
   * Appends a message card to the Arena Live Banter feed.
   * @param {Object} entry
   */
  addBanterToFeed({ color, name, avatar, text, moveDesc }) {
    if (!this.arenaFeedList) return;
    const item = document.createElement('div');
    const isRed = color === PLAYER_RED;
    item.className = `arena-msg ${isRed ? 'msg-red' : 'msg-black'}`;

    const safeAvatar = avatar || (isRed ? '🔴' : '⚫');
    const safeName = name || (isRed ? 'Red' : 'Black');
    const moveTag = moveDesc ? `<span class="msg-move">${moveDesc}</span>` : '';

    item.innerHTML = `
      <div class="msg-header">
        <span class="msg-avatar">${safeAvatar}</span>
        <span class="msg-author">${safeName}</span>
        ${moveTag}
      </div>
      <div class="msg-body">${text}</div>
    `;

    this.arenaFeedList.appendChild(item);
    this.arenaFeedList.scrollTop = this.arenaFeedList.scrollHeight;
  }

  /**
   * Updates WebLLM model loading progress bar.
   * @param {Object} report - { text: string, progress: number }
   */
  updateDownloadProgress({ text, progress }) {
    if (!this.webLLMProgressContainer) return;

    if (progress > 0 && progress < 1) {
      this.webLLMProgressContainer.classList.remove('hidden');
    } else if (progress >= 1) {
      setTimeout(() => {
        if (this.webLLMProgressContainer) {
          this.webLLMProgressContainer.classList.add('hidden');
        }
      }, 1500);
    }

    const percent = Math.min(100, Math.max(0, Math.round(progress * 100)));
    if (this.webLLMProgressFill) this.webLLMProgressFill.style.width = `${percent}%`;
    if (this.webLLMProgressPercent) this.webLLMProgressPercent.textContent = `${percent}%`;
    if (this.webLLMProgressLabel && text) this.webLLMProgressLabel.textContent = text;
  }

  /**
   * Controls spectator bar visibility and play/pause button state.
   * @param {boolean} isActive
   * @param {boolean} isPaused
   */
  setSpectatorMode(isActive, isPaused) {
    if (this.spectatorControls) {
      if (isActive) {
        this.spectatorControls.classList.remove('hidden');
      } else {
        this.spectatorControls.classList.add('hidden');
      }
    }

    if (this.spectatorPlayIcon && this.spectatorPlayText) {
      if (isPaused) {
        this.spectatorPlayIcon.textContent = '▶';
        this.spectatorPlayText.textContent = 'Play';
      } else {
        this.spectatorPlayIcon.textContent = '❚❚';
        this.spectatorPlayText.textContent = 'Pause';
      }
    }
  }
}

