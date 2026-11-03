/* Deterministic Trajectory Recording, Playback, and Export/Import Engine */

/**
 * Trajectory format schema:
 * {
 *   version: 1,
 *   metadata: {
 *     guestId: string|number,
 *     buildId: string,
 *     resolution: { width: number, height: number },
 *     startTime: number,
 *     endTime: number,
 *     durationMs: number,
 *     actionCount: number,
 *     initialHash: number,
 *     finalHash: number
 *   },
 *   events: [
 *     { dt: number, type: 'click', x: number, y: number, hash: number },
 *     { dt: number, type: 'key', code: number, mods: number, hash: number },
 *     { dt: number, type: 'openApp', slot: string|number, hash: number },
 *     { dt: number, type: 'seedMines', seed: number, maxStrikes: number, hash: number }
 *   ]
 * }
 */

export class TrajectoryRecorder {
  constructor() {
    this.guest = null;
    this.guestId = null;
    this.buildId = null;
    this.resolution = { width: 1280, height: 720 };
    this.startTime = 0;
    this.lastEventTime = 0;
    this.events = [];
    this._recording = false;
    this.initialHash = 0;
    this.initialClientHash = 0;
    this.initialClock = null;
  }

  isRecording() {
    return this._recording;
  }

  getDuration() {
    if (!this._recording && this.events.length === 0) return 0;
    const now = this._recording ? performance.now() : this.lastEventTime;
    return Math.max(0, Math.round(now - this.startTime));
  }

  getActionCount() {
    return this.events.length;
  }

  start(guest) {
    if (!guest) {
      throw new Error('TrajectoryRecorder: Guest is required to start recording');
    }
    this.guest = guest;
    this.guestId = guest.id;
    this.buildId = guest.buildId || 'build-03';
    this.resolution = {
      width: guest.width || 1280,
      height: guest.height || 720,
    };
    this.startTime = performance.now();
    this.lastEventTime = this.startTime;
    this.events = [];

    const now = new Date();
    this.initialClock = {
      year: now.getFullYear(),
      month: now.getMonth() + 1,
      day: now.getDate(),
      hours: now.getHours(),
      minutes: now.getMinutes(),
      seconds: now.getSeconds(),
      ms: now.getMilliseconds(),
    };

    this.initialHash = guest.exports?.app_fb_hash ? (guest.exports.app_fb_hash() >>> 0) : (guest.fbHash || 0);
    this.initialClientHash = guest.exports?.app_client_fb_hash ? (guest.exports.app_client_fb_hash() >>> 0) : 0;
    this._recording = true;
  }

  _getGameHash() {
    if (!this.guest?.exports) return null;
    const exp = this.guest.exports;
    const sel = exp.app_selected ? exp.app_selected() : 0;
    if (sel === 6 && exp.app_chess_state_hash) {
      return exp.app_chess_state_hash() >>> 0;
    }
    if (sel === 5 && exp.app_checkers_state_hash) {
      return exp.app_checkers_state_hash() >>> 0;
    }
    return null;
  }

  recordEvent(type, payload = {}, hash = null) {
    if (!this._recording) return null;

    const now = performance.now();
    const dt = Math.max(0, Math.round(now - this.lastEventTime));
    this.lastEventTime = now;

    let computedHash = hash;
    if (computedHash == null && this.guest?.exports?.app_fb_hash) {
      computedHash = this.guest.exports.app_fb_hash() >>> 0;
    }

    const clientHash = this.guest?.exports?.app_client_fb_hash
      ? (this.guest.exports.app_client_fb_hash() >>> 0)
      : null;

    const gameHash = this._getGameHash();

    const event = {
      dt,
      type,
      ...payload,
      hash: computedHash ?? 0,
    };

    if (type === 'click' && typeof payload.x === 'number' && typeof payload.y === 'number') {
      const exp = this.guest?.exports;
      let semantic = null;
      if (exp?.app_semantic_hit) {
        const hit = exp.app_semantic_hit(payload.x, payload.y) >>> 0;
        const kind = hit >> 24;
        const id = hit & 0x00FFFFFF;
        if (kind !== 0) {
          if (kind === 1) {
            semantic = { kind: 'menu:home', id, label: id === 0 ? 'Home' : `Drawer Item ${id}` };
          } else if (kind === 2) {
            const files = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
            const file = files[id & 7];
            const rank = (id >> 3) + 1;
            semantic = { kind: 'chess:square', id, square: `${file}${rank}`, label: `Square ${file}${rank}` };
          } else if (kind === 3) {
            semantic = { kind: 'checkers:square', id, label: `Square ${id}` };
          } else if (kind === 4) {
            const r = Math.floor(id / 8);
            const c = id % 8;
            semantic = { kind: 'mines:cell', id, row: r, col: c, label: `Cell (${r}, ${c})` };
          } else if (kind === 5) {
            const btnNames = ['New Game', 'Mode', 'Difficulty', 'Flip', 'Rule', 'Multiplayer'];
            semantic = { kind: 'button', id, label: btnNames[id] || `Button ${id}` };
          }
        }
      }

      event.physical = {
        x: payload.x,
        y: payload.y,
        recordedRes: { ...this.resolution },
        hash: computedHash ?? 0,
        clientHash: clientHash ?? null,
      };

      if (semantic) {
        event.semantic = semantic;
      }
    }

    if (clientHash != null) {
      event.clientHash = clientHash;
    }
    if (gameHash != null) {
      event.gameHash = gameHash;
    }

    this.events.push(event);
    return event;
  }

  stop() {
    if (!this._recording) {
      return this.exportData();
    }
    this._recording = false;
    return this.exportData();
  }

  exportData() {
    const finalHash = this.guest?.exports?.app_fb_hash
      ? (this.guest.exports.app_fb_hash() >>> 0)
      : (this.guest?.fbHash || (this.events.length > 0 ? this.events[this.events.length - 1].hash : this.initialHash));

    const finalClientHash = this.guest?.exports?.app_client_fb_hash
      ? (this.guest.exports.app_client_fb_hash() >>> 0)
      : (this.events.length > 0 && this.events[this.events.length - 1].clientHash != null
          ? this.events[this.events.length - 1].clientHash
          : this.initialClientHash);

    return {
      version: 1,
      metadata: {
        guestId: this.guestId,
        buildId: this.buildId,
        resolution: { ...this.resolution },
        recordedResolution: { ...this.resolution },
        supportedModes: ['physical_exact', 'semantic_responsive'],
        startTime: this.startTime,
        endTime: this.lastEventTime,
        durationMs: Math.max(0, Math.round(this.lastEventTime - this.startTime)),
        actionCount: this.events.length,
        initialHash: this.initialHash,
        initialClientHash: this.initialClientHash,
        finalHash,
        finalClientHash,
        initialClock: this.initialClock ? { ...this.initialClock } : null,
      },
      events: [...this.events],
    };
  }
}

export class TrajectoryPlayer {
  constructor(deps = {}) {
    this.deps = deps;
    this.trajectoryData = null;
    this.state = 'idle'; // 'idle' | 'playing' | 'paused' | 'completed'
    this.currentIndex = 0;
    this.speed = 1.0; // 0: instant, 0.5: half, 1.0: normal, 2.0: 2x, etc.
    this.playbackMode = 'auto'; // 'auto' | 'physical' | 'semantic'
    this.timer = null;
    this.hashDivergenceCount = 0;
    this.stepOutcomes = [];
    this.cumulativeDt = 0;
    this.initialClock = null;

    // Callbacks
    this.onStateChange = null;
    this.onStep = null;
    this.onProgress = null;
    this.onHashDivergence = null;
    this.onComplete = null;
    this.onModeChange = null;
  }

  getPlaybackMode() {
    return this.playbackMode;
  }

  setPlaybackMode(mode) {
    if (['auto', 'physical', 'semantic'].includes(mode)) {
      this.playbackMode = mode;
      if (this.onModeChange) this.onModeChange(mode);
    }
  }

  setDependencies(deps) {
    this.deps = { ...this.deps, ...deps };
  }

  getGuest() {
    if (this.deps.getGuest) return this.deps.getGuest();
    return null;
  }

  load(trajectoryData) {
    if (this.state === 'playing') {
      this.stop();
    }

    if (!trajectoryData || !Array.isArray(trajectoryData.events)) {
      throw new Error('TrajectoryPlayer: Invalid trajectory data');
    }

    this.trajectoryData = trajectoryData;
    this.state = 'idle';
    this.currentIndex = 0;
    this.hashDivergenceCount = 0;
    this.stepOutcomes = [];
    this.cumulativeDt = 0;
    this.initialClock = trajectoryData.metadata?.initialClock
      ? { ...trajectoryData.metadata.initialClock }
      : null;

    this._notifyStateChange();
    this._notifyProgress();
  }

  _advanceVirtualClock(guest, dt) {
    if (!guest?.exports || !this.initialClock) return;
    this.cumulativeDt += dt;
    const baseDate = new Date(
      this.initialClock.year,
      this.initialClock.month - 1,
      this.initialClock.day,
      this.initialClock.hours,
      this.initialClock.minutes,
      this.initialClock.seconds,
      this.initialClock.ms,
    );
    const virtualDate = new Date(baseDate.getTime() + this.cumulativeDt);

    if (guest.exports.app_set_time) {
      guest.exports.app_set_time(
        virtualDate.getHours(),
        virtualDate.getMinutes(),
        virtualDate.getSeconds(),
        virtualDate.getMilliseconds(),
      );
    }
    if (guest.exports.app_set_date) {
      guest.exports.app_set_date(
        virtualDate.getFullYear(),
        virtualDate.getMonth() + 1,
        virtualDate.getDate(),
        virtualDate.getDay(),
      );
    }
    if (guest.exports.app_set_utc) {
      guest.exports.app_set_utc(
        virtualDate.getUTCFullYear(),
        virtualDate.getUTCMonth() + 1,
        virtualDate.getUTCDate(),
        virtualDate.getUTCHours(),
        virtualDate.getUTCMinutes(),
        virtualDate.getUTCSeconds(),
        -virtualDate.getTimezoneOffset(),
      );
    }
  }

  async play(speed = 1.0) {
    if (!this.trajectoryData) {
      throw new Error('TrajectoryPlayer: No trajectory loaded');
    }

    this.speed = speed;

    if (this.state === 'completed' || this.currentIndex >= this.trajectoryData.events.length) {
      this.currentIndex = 0;
      this.hashDivergenceCount = 0;
      this.stepOutcomes = [];
      this.cumulativeDt = 0;
    }

    this.state = 'playing';
    this._notifyStateChange();

    await this._runPlaybackLoop();
  }

  pause() {
    if (this.state === 'playing') {
      if (this.timer) {
        clearTimeout(this.timer);
        this.timer = null;
      }
      this.state = 'paused';
      this._notifyStateChange();
    }
  }

  stop() {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    this.state = 'idle';
    this.currentIndex = 0;
    this.cumulativeDt = 0;
    this._notifyStateChange();
    this._notifyProgress();
  }

  async stepForward() {
    if (!this.trajectoryData) {
      throw new Error('TrajectoryPlayer: No trajectory loaded');
    }

    if (this.state === 'playing') {
      this.pause();
    }

    if (this.currentIndex >= this.trajectoryData.events.length) {
      this.state = 'completed';
      this._notifyStateChange();
      return false;
    }

    const event = this.trajectoryData.events[this.currentIndex];
    const outcome = await this.executeEvent(event, this.currentIndex);
    this.currentIndex++;

    if (this.currentIndex >= this.trajectoryData.events.length) {
      this.state = 'completed';
      this._notifyStateChange();
      if (this.onComplete) {
        this.onComplete({
          divergences: this.hashDivergenceCount,
          total: this.trajectoryData.events.length,
          stepOutcomes: this.stepOutcomes,
        });
      }
    } else {
      this.state = 'paused';
      this._notifyStateChange();
    }

    this._notifyProgress();
    return outcome;
  }

  async executeEvent(event, index) {
    const guest = this.getGuest();
    if (!guest) {
      throw new Error('TrajectoryPlayer: Target guest VM not found');
    }

    // Advance virtual clock before executing action
    this._advanceVirtualClock(guest, event.dt || 0);

    const { type } = event;
    const MODS = { Shift: 1, Ctrl: 2, Alt: 4, Meta: 8 };

    switch (type) {
      case 'click': {
        let clickX = event.physical?.x ?? event.x;
        let clickY = event.physical?.y ?? event.y;

        const recRes = this.trajectoryData?.metadata?.recordedResolution || this.trajectoryData?.metadata?.resolution;
        const resMatch = recRes && guest.width === recRes.width && guest.height === recRes.height;

        let useSemantic = false;
        if (this.playbackMode === 'semantic') {
          useSemantic = true;
        } else if (this.playbackMode === 'auto') {
          useSemantic = !resMatch;
        }

        if (useSemantic && event.semantic && guest.exports?.app_semantic_coord) {
          const kindMap = {
            'menu:home': 1,
            'chess:square': 2,
            'checkers:square': 3,
            'mines:cell': 4,
            'button': 5,
          };
          const kind = kindMap[event.semantic.kind] || (typeof event.semantic.kind === 'number' ? event.semantic.kind : 0);
          const id = event.semantic.id ?? 0;
          if (kind > 0) {
            const packed = guest.exports.app_semantic_coord(kind, id) >>> 0;
            if (packed !== 0) {
              clickX = packed >> 16;
              clickY = packed & 0xFFFF;
            }
          }
        }

        if (guest.clickLogical) {
          guest.clickLogical(clickX, clickY);
        } else if (guest.exports?.app_click) {
          guest.exports.app_click(clickX, clickY);
        }
        if (guest.render) guest.render();
        break;
      }

      case 'key': {
        const code = event.code;
        const mods = event.mods || 0;
        if (guest.exports?.app_key_mods) {
          guest.exports.app_key_mods(code, mods);
        } else if (guest.exports?.app_key) {
          guest.exports.app_key(code);
        }
        if (guest.render) guest.render();
        break;
      }

      case 'openApp': {
        const slot = event.slot;
        const keyMap = {
          system: 11,   // Digit 1
          calc: 12,     // Digit 2
          files: 13,    // Digit 3
          mines: 14,    // Digit 4
          checkers: 15, // Digit 5
          chess: 16,    // Digit 6
        };
        const slotMap = {
          desktop: 0,
          system: 1,
          calc: 2,
          files: 3,
          mines: 4,
          checkers: 5,
          chess: 6,
        };

        if (typeof slot === 'string' && keyMap[slot] && guest.exports?.app_key_mods) {
          guest.exports.app_key_mods(keyMap[slot], MODS.Ctrl);
        } else if (typeof slot === 'number' && slot >= 1 && slot <= 6 && guest.exports?.app_key_mods) {
          guest.exports.app_key_mods(10 + slot, MODS.Ctrl);
        } else if (guest.clickLogical) {
          const targetSlot = typeof slot === 'number' ? slot : (slotMap[slot] || 0);
          if (guest.exports?.app_dock_visible && guest.exports.app_dock_visible() === 0) {
            guest.clickLogical(16, 16);
          }
          if (targetSlot > 0) {
            const dockY = 32 + 10 + (targetSlot - 1) * 76 + 38;
            guest.clickLogical(76, dockY);
          }
        }
        if (guest.render) guest.render();
        break;
      }

      case 'seedMines': {
        const seed = event.seed ?? 42;
        const maxStrikes = event.maxStrikes ?? 1;
        if (guest.exports?.app_mines_set_mp) {
          guest.exports.app_mines_set_mp(0, 1, 1, seed, maxStrikes);
        }
        if (guest.render) guest.render();
        break;
      }

      default:
        console.warn(`[TrajectoryPlayer] Unknown event type '${type}'`, event);
        break;
    }

    // Zero-drift hash verification
    // Prioritize app_client_fb_hash when available in guest & recorded event
    let currentHash = 0;
    let expectedHash = 0;
    let match = false;

    if (guest.exports?.app_client_fb_hash && event.clientHash != null) {
      currentHash = guest.exports.app_client_fb_hash() >>> 0;
      expectedHash = event.clientHash >>> 0;
      match = currentHash === expectedHash;
    } else {
      currentHash = guest.exports?.app_fb_hash ? (guest.exports.app_fb_hash() >>> 0) : (guest.fbHash || 0);
      expectedHash = event.hash >>> 0;
      match = currentHash === expectedHash;
    }

    const outcome = {
      index,
      event,
      currentHash,
      expectedHash,
      match,
    };

    if (!match) {
      this.hashDivergenceCount++;
      if (this.onHashDivergence) {
        this.onHashDivergence({
          index,
          event,
          expectedHash,
          currentHash,
          expectedHex: `0x${expectedHash.toString(16).padStart(8, '0')}`,
          actualHex: `0x${currentHash.toString(16).padStart(8, '0')}`,
        });
      }
    }

    this.stepOutcomes.push(outcome);

    if (this.onStep) {
      this.onStep(outcome);
    }

    return outcome;
  }

  async _runPlaybackLoop() {
    while (this.state === 'playing' && this.currentIndex < this.trajectoryData.events.length) {
      const event = this.trajectoryData.events[this.currentIndex];

      // Delay based on recorded dt and speed
      if (this.speed > 0 && event.dt > 0) {
        const waitMs = Math.round(event.dt / this.speed);
        if (waitMs > 0) {
          await new Promise((resolve) => {
            this.timer = setTimeout(resolve, waitMs);
          });
          this.timer = null;
        }
      }

      if (this.state !== 'playing') break;

      await this.executeEvent(event, this.currentIndex);
      this.currentIndex++;
      this._notifyProgress();

      // Yield frame when speed === 0 to avoid freezing thread
      if (this.speed === 0 && (this.currentIndex % 20 === 0)) {
        await new Promise((r) => setTimeout(r, 0));
      }
    }

    if (this.state === 'playing' && this.currentIndex >= this.trajectoryData.events.length) {
      this.state = 'completed';
      this._notifyStateChange();
      this._notifyProgress();
      if (this.onComplete) {
        this.onComplete({
          divergences: this.hashDivergenceCount,
          total: this.trajectoryData.events.length,
          stepOutcomes: this.stepOutcomes,
        });
      }
    }
  }

  _notifyStateChange() {
    if (this.onStateChange) {
      this.onStateChange(this.state);
    }
  }

  _notifyProgress() {
    if (this.onProgress && this.trajectoryData) {
      const total = this.trajectoryData.events.length;
      this.onProgress({
        index: this.currentIndex,
        total,
        percent: total > 0 ? (this.currentIndex / total) * 100 : 0,
        divergences: this.hashDivergenceCount,
      });
    }
  }
}

/**
 * Serialization Utilities
 */
export function exportTrajectoryJson(trajectoryData) {
  return JSON.stringify(trajectoryData, null, 2);
}

export function downloadTrajectoryFile(trajectoryData, filename) {
  const jsonStr = typeof trajectoryData === 'string' ? trajectoryData : exportTrajectoryJson(trajectoryData);
  const fname = filename || `session_${new Date().toISOString().replace(/[:.]/g, '-')}.trajectory.json`;

  if (typeof document === 'undefined') {
    return jsonStr;
  }

  const blob = new Blob([jsonStr], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = fname;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  return fname;
}

export function importTrajectoryJson(jsonString) {
  let data;
  try {
    data = typeof jsonString === 'string' ? JSON.parse(jsonString) : jsonString;
  } catch (err) {
    throw new Error(`Trajectory validation failed: Invalid JSON format (${err.message})`);
  }

  if (!data || typeof data !== 'object') {
    throw new Error('Trajectory validation failed: Root must be an object');
  }

  if (!Array.isArray(data.events)) {
    throw new Error('Trajectory validation failed: Missing or invalid "events" array');
  }

  for (let i = 0; i < data.events.length; i++) {
    const ev = data.events[i];
    if (!ev || typeof ev.type !== 'string') {
      throw new Error(`Trajectory validation failed: Event at index ${i} missing valid "type"`);
    }
  }

  return data;
}

/**
 * Converts a trajectory to an Auto Pilot Mission format
 */
export function trajectoryToAutoPilotMission(trajectoryData, opts = {}) {
  const data = typeof trajectoryData === 'string' ? importTrajectoryJson(trajectoryData) : trajectoryData;
  const id = opts.id || `mission-rec-${Date.now().toString(36)}`;
  const name = opts.name || `Recorded Session (${data.events.length} actions)`;
  const description = opts.description || `Auto Pilot mission generated from Trajectory session with ${data.events.length} recorded events.`;

  const steps = [];

  // Initial boot step
  const width = data.metadata?.resolution?.width || 1280;
  const height = data.metadata?.resolution?.height || 720;
  steps.push({
    action: 'boot',
    opts: { width, height },
    desc: `Boot guest VM at ${width}×${height}`,
  });

  for (let i = 0; i < data.events.length; i++) {
    const ev = data.events[i];
    const stepNum = i + 1;

    switch (ev.type) {
      case 'click':
        steps.push({
          action: 'click',
          x: ev.x,
          y: ev.y,
          desc: `[Action ${stepNum}] Click at (${ev.x}, ${ev.y})`,
        });
        break;

      case 'key':
        steps.push({
          action: 'key',
          code: ev.code,
          mods: ev.mods || 0,
          desc: `[Action ${stepNum}] Key code ${ev.code} (mods: ${ev.mods || 0})`,
        });
        break;

      case 'openApp':
        steps.push({
          action: 'openApp',
          slot: ev.slot,
          desc: `[Action ${stepNum}] Open app slot '${ev.slot}'`,
        });
        break;

      case 'seedMines':
        steps.push({
          action: 'seedMines',
          seed: ev.seed,
          maxStrikes: ev.maxStrikes,
          desc: `[Action ${stepNum}] Configure Minesweeper seed=${ev.seed}`,
        });
        break;

      default:
        break;
    }

    if (typeof ev.hash === 'number' && ev.hash !== 0) {
      steps.push({
        action: 'assertHash',
        expectedHash: ev.hash >>> 0,
        desc: `[Verify ${stepNum}] Assert FNV-1a hash matches 0x${(ev.hash >>> 0).toString(16).padStart(8, '0')}`,
      });
    }
  }

  return {
    id,
    name,
    description,
    steps,
  };
}
