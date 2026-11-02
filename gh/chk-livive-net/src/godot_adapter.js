/**
 * @file godot_adapter.js
 * Implements the CheckersRenderer interface for Godot 3D WebAssembly client.
 * Dispatches state updates, move animations, audio triggers, banter,
 * and player configurations across the bidirectional window.GodotCheckersBridge.
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
  isKing
} from './state.js';

/**
 * Procedural Audio Synthesizer using Web Audio API for zero-dependency sound effects.
 */
export class SoundEffects {
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
    } catch (e) {}
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

  playIllegal() {
    if (this.muted) return;
    this._initContext();
    if (!this.ctx) return;

    try {
      const now = this.ctx.currentTime;
      const osc = this.ctx.createOscillator();
      const gain = this.ctx.createGain();

      osc.type = 'sawtooth';
      osc.frequency.setValueAtTime(90, now);
      osc.frequency.setValueAtTime(70, now + 0.04);

      gain.gain.setValueAtTime(0.2, now);
      gain.gain.exponentialRampToValueAtTime(0.001, now + 0.08);

      osc.connect(gain);
      gain.connect(this.ctx.destination);

      osc.start(now);
      osc.stop(now + 0.08);
    } catch (e) {}
  }
}

/**
 * Creates a resilient dummy DOM element mock with safe chainable stubs.
 */
function createDummyDOMElement() {
  const listeners = new Map();
  return {
    innerHTML: '',
    textContent: '',
    value: '',
    scrollTop: 0,
    scrollHeight: 0,
    style: {},
    dataset: {},
    classList: {
      add: () => {},
      remove: () => {},
      toggle: () => {},
      contains: () => false
    },
    addEventListener: (event, handler) => {
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push(handler);
    },
    removeEventListener: (event, handler) => {
      if (!listeners.has(event)) return;
      const list = listeners.get(event).filter((h) => h !== handler);
      listeners.set(event, list);
    },
    dispatchEvent: (event) => {
      const list = listeners.get(event?.type || event);
      if (list) list.forEach((h) => h(event));
    },
    appendChild: () => ({}),
    removeChild: () => ({}),
    querySelector: () => null,
    querySelectorAll: () => [],
    closest: () => null,
    remove: () => {}
  };
}

/**
 * GodotAdapter bridges the JS CheckersController to Godot 3D Web client.
 */
export class GodotAdapter {
  constructor(boardElement = null) {
    this.sound = new SoundEffects();

    // DOM dummies to prevent null-dereference errors when controller accesses renderer elements
    this.boardElement = boardElement || createDummyDOMElement();
    this.turnMessage = createDummyDOMElement();
    this.turnBadge = createDummyDOMElement();
    this.turnDot = createDummyDOMElement();
    this.turnText = createDummyDOMElement();
    this.playerRedCard = createDummyDOMElement();
    this.playerBlackCard = createDummyDOMElement();
    this.redPiecesCountEl = createDummyDOMElement();
    this.redKingsCountEl = createDummyDOMElement();
    this.blackPiecesCountEl = createDummyDOMElement();
    this.blackKingsCountEl = createDummyDOMElement();
    this.modal = createDummyDOMElement();
    this.modalTitle = createDummyDOMElement();
    this.modalMessage = createDummyDOMElement();
    this.modalTrophy = createDummyDOMElement();
    this.modalReasonBadge = createDummyDOMElement();
    this.modalStatTurns = createDummyDOMElement();
    this.modalStatTime = createDummyDOMElement();
    this.modalStatRedCaptures = createDummyDOMElement();
    this.modalStatBlackCaptures = createDummyDOMElement();
    this.modalStatKings = createDummyDOMElement();
    this.btnModalReview = createDummyDOMElement();
    this.btnReopenModal = createDummyDOMElement();
    this.multiJumpBanner = createDummyDOMElement();
    this.multiJumpText = createDummyDOMElement();
    this.soundOnIcon = createDummyDOMElement();
    this.soundOffIcon = createDummyDOMElement();
    this.soundLabel = createDummyDOMElement();
    this.speechBubbleRed = createDummyDOMElement();
    this.speechBubbleBlack = createDummyDOMElement();
    this.playerRedNameEl = createDummyDOMElement();
    this.playerBlackNameEl = createDummyDOMElement();
    this.playerRedAvatarEl = createDummyDOMElement();
    this.playerBlackAvatarEl = createDummyDOMElement();
    this.arenaFeedSection = createDummyDOMElement();
    this.arenaFeedList = createDummyDOMElement();
    this.webLLMProgressContainer = createDummyDOMElement();
    this.webLLMProgressLabel = createDummyDOMElement();
    this.webLLMProgressPercent = createDummyDOMElement();
    this.webLLMProgressFill = createDummyDOMElement();
    this.spectatorControls = createDummyDOMElement();
    this.btnSpectatorPlay = createDummyDOMElement();
    this.spectatorPlayIcon = createDummyDOMElement();
    this.spectatorPlayText = createDummyDOMElement();
    this.btnSpectatorStep = createDummyDOMElement();
    this.selectSpectatorSpeed = createDummyDOMElement();

    // Move animation watchdog state
    this._animationResolve = null;
    this._animationWatchdogTimer = null;

    // Attach or initialize global bridge
    this.bridge = this._ensureBridge();
  }

  /**
   * Initializes or reuses window.GodotCheckersBridge
   * @private
   */
  _ensureBridge() {
    const root = typeof window !== 'undefined' ? window : globalThis;
    root.GodotCheckersBridge = root.GodotCheckersBridge || {};
    const b = root.GodotCheckersBridge;

    // JS -> Godot notifications (ALWAYS attach)
    b.notifyRender = (json) => {
      if (b._onGodotRender) {
        try {
          b._onGodotRender(json);
        } catch (e) {
          console.error('[GodotAdapter] Error in _onGodotRender:', e);
        }
      }
    };
    b.notifyAnimateMove = (json) => {
      if (b._onGodotAnimateMove) {
        try {
          b._onGodotAnimateMove(json);
        } catch (e) {
          console.error('[GodotAdapter] Error in _onGodotAnimateMove:', e);
        }
      }
    };
    b.notifyBanterStream = (color, text) => {
      if (b._onGodotBanterStream) {
        try {
          b._onGodotBanterStream(color, text);
        } catch (e) {
          console.error('[GodotAdapter] Error in _onGodotBanterStream:', e);
        }
      }
    };
    b.notifyPlayerInfo = (color, json) => {
      if (b._onGodotPlayerInfo) {
        try {
          b._onGodotPlayerInfo(color, json);
        } catch (e) {
          console.error('[GodotAdapter] Error in _onGodotPlayerInfo:', e);
        }
      }
    };
    b.notifySpectatorMode = (active, paused) => {
      if (b._onGodotSpectatorMode) {
        try {
          b._onGodotSpectatorMode(Boolean(active), Boolean(paused));
        } catch (e) {
          console.error('[GodotAdapter] Error in _onGodotSpectatorMode:', e);
        }
      }
    };
    b.notifyBoardFlipped = (flipped) => {
      if (b._onGodotBoardFlipped) {
        try {
          b._onGodotBoardFlipped(Boolean(flipped));
        } catch (e) {
          console.error('[GodotAdapter] Error in _onGodotBoardFlipped:', e);
        }
      }
    };
    b.notifyShowNotation = (show) => {
      if (b._onGodotShowNotation) {
        try {
          b._onGodotShowNotation(Boolean(show));
        } catch (e) {
          console.error('[GodotAdapter] Error in _onGodotShowNotation:', e);
        }
      }
    };
    b.notifyDownloadProgress = (json) => {
      if (b._onGodotDownloadProgress) {
        try {
          b._onGodotDownloadProgress(json);
        } catch (e) {
          console.error('[GodotAdapter] Error in _onGodotDownloadProgress:', e);
        }
      }
    };

    // Godot -> JS callbacks
    const prevOnSquareClicked = b.onSquareClicked;
    b.onSquareClicked = (index) => {
      this.triggerSquareClick(index);
      if (typeof prevOnSquareClicked === 'function') prevOnSquareClicked(index);
    };

    const prevOnAnimationComplete = b.onAnimationComplete;
    b.onAnimationComplete = () => {
      this._handleAnimationComplete();
      if (typeof prevOnAnimationComplete === 'function') prevOnAnimationComplete();
    };

    const prevOnLaunchMatch = b.onLaunchMatch;
    b.onLaunchMatch = (configJson) => {
      this._handleLaunchMatch(configJson);
      if (typeof prevOnLaunchMatch === 'function') prevOnLaunchMatch(configJson);
    };

    const prevOnResetGame = b.onResetGame;
    b.onResetGame = () => {
      this._handleResetGame();
      if (typeof prevOnResetGame === 'function') prevOnResetGame();
    };

    const prevOnSpectatorAction = b.onSpectatorAction;
    b.onSpectatorAction = (action, value) => {
      this._handleSpectatorAction(action, value);
      if (typeof prevOnSpectatorAction === 'function') prevOnSpectatorAction(action, value);
    };

    const prevOnGodotReady = b.onGodotReady;
    b.onGodotReady = () => {
      this._handleGodotReady();
      if (typeof prevOnGodotReady === 'function') prevOnGodotReady();
    };

    b.onBoardFlipped = (flipped) => {
      this.setBoardFlipped(flipped);
    };
    b.onShowNotation = (show) => {
      this.setShowNotation(show);
    };
    b.onMuted = (muted) => {
      this.setMuted(muted);
    };

    return b;

    return root.GodotCheckersBridge;
  }

  /**
   * Binds the CheckersController instance to the adapter.
   * @param {import('./controller.js').CheckersController} controller
   */
  setController(controller) {
    this.controller = controller;
  }

  /**
   * Handles launch match request from Godot UI
   * @param {string|Object} configJson
   * @private
   */
  _handleLaunchMatch(configJson) {
    try {
      const config = typeof configJson === 'string' ? JSON.parse(configJson) : (configJson || {});
      if (this.controller && typeof this.controller.launchMatch === 'function') {
        this.controller.launchMatch(config);
      }
    } catch (e) {
      console.error('[GodotAdapter] Error parsing launch match config:', e);
    }
  }

  /**
   * Handles reset match request from Godot UI
   * @private
   */
  _handleResetGame() {
    if (this.controller && typeof this.controller.resetGame === 'function') {
      this.controller.resetGame();
    }
  }

  /**
   * Handles spectator action request from Godot UI
   * @param {string} action
   * @param {any} value
   * @private
   */
  _handleSpectatorAction(action, value) {
    if (!this.controller) return;
    if (action === 'play') {
      this.controller.spectatorPlay();
    } else if (action === 'pause') {
      this.controller.spectatorPause();
    } else if (action === 'step') {
      this.controller.spectatorStep();
    } else if (action === 'speed') {
      this.controller.spectatorSpeed = Number(value);
    }
  }

  /**
   * Handles Godot ready signal
   * @private
   */
  _handleGodotReady() {
    console.log('[GodotCheckersBridge] Godot ready received');
    if (this.controller) {
      if (!this.controller._godotInitialized) {
        this.controller._godotInitialized = true;
        this.controller.init();
      } else {
        this.controller.updateView();
      }
    }
  }

  /**
   * Simulates a square click event on boardElement to trigger controller.handleSquareClick
   * @param {number} index - 0..31 dark square index
   */
  triggerSquareClick(index) {
    const squareEl = {
      dataset: { index: String(index) },
      closest: (sel) => {
        if (sel === '.square-dark') return squareEl;
        return null;
      }
    };
    const event = {
      target: squareEl,
      type: 'click'
    };
    this.boardElement.dispatchEvent(event);
  }

  /**
   * Resolves active move animation Promise when Godot reports completion.
   * @private
   */
  _handleAnimationComplete() {
    if (this._animationWatchdogTimer) {
      clearTimeout(this._animationWatchdogTimer);
      this._animationWatchdogTimer = null;
    }
    if (this._animationResolve) {
      const resolve = this._animationResolve;
      this._animationResolve = null;
      resolve();
    }
  }

  /**
   * CheckersRenderer interface: Renders board state and highlights
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
    // Convert Sets to Arrays for clean JSON serialization
    const payload = {
      board: Array.from(state.board),
      turn: state.turn,
      selectedSquare: state.selectedSquare,
      mustContinueJumpFrom: state.mustContinueJumpFrom,
      winner: state.winner,
      legalMovesForSelected: legalMovesForSelected || [],
      selectableIndices: Array.from(selectableIndices || []),
      preyIndices: Array.from(preyIndices || []),
      lastMove: lastMove || null,
      mustJump: Boolean(mustJump),
      message: message || '',
      newlyCrownedIndex: newlyCrownedIndex !== undefined ? newlyCrownedIndex : null,
      stats: stats ? { ...stats } : null,
      winReason: winReason || null
    };

    const json = JSON.stringify(payload);
    if (this.bridge && typeof this.bridge.notifyRender === 'function') {
      this.bridge.notifyRender(json);
    }
  }

  /**
   * CheckersRenderer interface: Initiates 3D move animation in Godot.
   * Returns a Promise that resolves when Godot calls onAnimationComplete,
   * with a 1500ms watchdog safety timeout.
   * @param {{ from: number, to: number, isJump: boolean, captured: number|null }} move
   * @returns {Promise<void>}
   */
  animateMove(move) {
    // If a previous animation was still pending, resolve it first
    this._handleAnimationComplete();

    return new Promise((resolve) => {
      this._animationResolve = resolve;

      // 1500ms watchdog safety timeout
      this._animationWatchdogTimer = setTimeout(() => {
        this._handleAnimationComplete();
      }, 1500);

      const payload = {
        from: move.from,
        to: move.to,
        isJump: Boolean(move.isJump),
        captured: move.captured !== undefined ? move.captured : null
      };

      const json = JSON.stringify(payload);
      if (this.bridge && typeof this.bridge.notifyAnimateMove === 'function') {
        this.bridge.notifyAnimateMove(json);
      }
    });
  }

  /**
   * CheckersRenderer interface: Audio mute toggle
   * @param {boolean} isMuted
   */
  setMuted(isMuted) {
    this.sound.muted = Boolean(isMuted);
  }

  /**
   * CheckersRenderer interface: Notation numbers toggle
   * @param {boolean} show
   */
  setShowNotation(show) {
    if (this.bridge && typeof this.bridge.notifyShowNotation === 'function') {
      this.bridge.notifyShowNotation(Boolean(show));
    }
  }

  /**
   * CheckersRenderer interface: 180° board flip toggle
   * @param {boolean} flipped
   */
  setBoardFlipped(flipped) {
    if (this.bridge && typeof this.bridge.notifyBoardFlipped === 'function') {
      this.bridge.notifyBoardFlipped(Boolean(flipped));
    }
  }

  /**
   * CheckersRenderer interface: Updates player name and avatar
   * @param {number} color
   * @param {{ name: string, avatar: string }} info
   */
  setPlayerInfo(color, info) {
    if (this.bridge && typeof this.bridge.notifyPlayerInfo === 'function') {
      const json = JSON.stringify(info || {});
      this.bridge.notifyPlayerInfo(color, json);
    }
  }

  /**
   * CheckersRenderer interface: Streams real-time banter
   * @param {number} color
   * @param {string} text
   */
  streamBanter(color, text) {
    if (this.bridge && typeof this.bridge.notifyBanterStream === 'function') {
      this.bridge.notifyBanterStream(color, text || '');
    }
  }

  /**
   * CheckersRenderer interface: Hides speech bubble
   * @param {number} color
   */
  hideBanter(color) {
    if (this.bridge && typeof this.bridge.notifyBanterStream === 'function') {
      this.bridge.notifyBanterStream(color, '');
    }
  }

  /**
   * CheckersRenderer interface: Appends banter to feed
   * @param {Object} entry
   */
  addBanterToFeed(entry) {
    // In Godot Hybrid UI, feed items can also be handled in Godot or HTML feed
    if (this.arenaFeedList) {
      const isRedPlayer = entry.color === PLAYER_RED;
      const safeAvatar = entry.avatar || (isRedPlayer ? '🔴' : '⚫');
      const safeName = entry.name || (isRedPlayer ? 'Red' : 'Black');
      const moveTag = entry.moveDesc ? ` [${entry.moveDesc}]` : '';
      const text = `${safeAvatar} ${safeName}${moveTag}: ${entry.text || ''}`;
      this.arenaFeedList.textContent += `${text}\n`;
    }
  }

  /**
   * CheckersRenderer interface: Progress bar reporting for WebLLM
   * @param {{ text: string, progress: number }} report
   */
  updateDownloadProgress(report) {
    if (this.bridge && typeof this.bridge.notifyDownloadProgress === 'function') {
      const json = JSON.stringify(report || {});
      this.bridge.notifyDownloadProgress(json);
    }
  }

  /**
   * CheckersRenderer interface: Controls spectator UI state
   * @param {boolean} isActive
   * @param {boolean} isPaused
   */
  setSpectatorMode(isActive, isPaused) {
    if (this.bridge && typeof this.bridge.notifySpectatorMode === 'function') {
      this.bridge.notifySpectatorMode(Boolean(isActive), Boolean(isPaused));
    }
  }

  /**
   * CheckersRenderer interface: Modal review helpers
   */
  hideModalForReview() {}
  showReviewRecap() {}
}
