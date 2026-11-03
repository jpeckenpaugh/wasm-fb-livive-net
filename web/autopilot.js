/* Auto Pilot Runner: Deterministic Mission Runner, Action Interpreter & State Assertions */

import { MISSIONS, getMissionById } from './missions.js';

const MODS = { Shift: 1, Ctrl: 2, Alt: 4, Meta: 8 };

export class AutoPilotRunner {
  constructor(deps = {}) {
    this.deps = deps;
    this.activeMission = null;
    this.currentStepIdx = 0;
    this.pacingMs = 400; // Default 400ms observable play, or 0ms instant
    this.state = 'idle'; // 'idle' | 'running' | 'paused' | 'completed' | 'cancelled' | 'error'
    this.timer = null;
    this.stepResolver = null;
    this.report = null;
    this.onStatusChange = null;
    this.onStepChange = null;
    this.onLog = null;
    this.onReportComplete = null;
  }

  setDependencies(deps) {
    this.deps = { ...this.deps, ...deps };
  }

  log(msg, level = 'info') {
    if (this.onLog) {
      this.onLog({ msg, level, timestamp: Date.now() });
    }
  }

  updateStatus(hudText, isOk = true) {
    if (this.onStatusChange) {
      this.onStatusChange(hudText, isOk);
    }
    if (this.deps.setStatus) {
      this.deps.setStatus(hudText, isOk);
    }
  }

  getActiveGuest() {
    if (this.deps.getGuest) {
      const g = this.deps.getGuest();
      if (g) return g;
    }
    if (this.deps.getGuests) {
      const list = this.deps.getGuests();
      if (list && list.length > 0) return list[0];
    }
    return null;
  }

  listMissions() {
    return MISSIONS.map((m) => ({
      id: m.id,
      name: m.name,
      description: m.description,
      stepCount: m.steps.length,
    }));
  }

  getReport() {
    return this.report;
  }

  inspectState() {
    const guest = this.getActiveGuest();
    return {
      runnerState: this.state,
      missionId: this.activeMission ? this.activeMission.id : null,
      stepIdx: this.currentStepIdx,
      totalSteps: this.activeMission ? this.activeMission.steps.length : 0,
      pacingMs: this.pacingMs,
      guest: guest
        ? {
            id: guest.id,
            name: guest.name,
            selectedApp: guest.exports?.app_selected ? guest.exports.app_selected() : 0,
            activePage: guest.exports?.app_active_page ? guest.exports.app_active_page() : 0,
            theme: guest.exports?.app_theme ? guest.exports.app_theme() : 0,
            fbHash: guest.fbHash || (guest.exports?.app_fb_hash ? guest.exports.app_fb_hash() >>> 0 : 0),
          }
        : null,
    };
  }

  async runMission(missionOrId, opts = {}) {
    if (this.state === 'running') {
      this.cancel();
    }

    let mission = null;
    if (typeof missionOrId === 'string') {
      mission = getMissionById(missionOrId);
      if (!mission) {
        throw new Error(`Auto Pilot: Unknown mission ID '${missionOrId}'`);
      }
    } else {
      mission = missionOrId;
    }

    if (!mission || !Array.isArray(mission.steps)) {
      throw new Error('Auto Pilot: Invalid mission configuration');
    }

    this.activeMission = mission;
    this.currentStepIdx = 0;
    if (typeof opts.pacingMs === 'number') {
      this.pacingMs = opts.pacingMs;
    }

    this.state = 'running';
    const startTime = Date.now();
    this.report = {
      missionId: mission.id,
      missionName: mission.name,
      startTime: new Date(startTime).toISOString(),
      endTime: null,
      durationMs: 0,
      totalSteps: mission.steps.length,
      executedSteps: 0,
      status: 'running',
      stepOutcomes: [],
      stateHashes: [],
      snapshots: [],
      error: null,
    };

    this.log(`🚀 Starting Auto Pilot mission: "${mission.name}" (${mission.steps.length} steps, pacing: ${this.pacingMs}ms)`);
    this.updateStatus(`Auto Pilot: [Starting] ${mission.name}...`, true);

    try {
      while (this.currentStepIdx < mission.steps.length) {
        if (this.state === 'cancelled') {
          break;
        }

        if (this.state === 'paused') {
          await new Promise((resolve) => {
            this.stepResolver = resolve;
          });
          if (this.state === 'cancelled') break;
        }

        const step = mission.steps[this.currentStepIdx];
        const stepNum = this.currentStepIdx + 1;
        const total = mission.steps.length;

        const hudStatus = `Auto Pilot: [Step ${stepNum}/${total}] ${step.desc || step.action}...`;
        this.updateStatus(hudStatus, true);

        if (this.onStepChange) {
          this.onStepChange({
            stepIdx: this.currentStepIdx,
            totalSteps: total,
            step,
            mission,
          });
        }

        const stepStart = Date.now();
        let stepOutcome = {
          stepIndex: this.currentStepIdx,
          action: step.action,
          desc: step.desc || '',
          success: false,
          durationMs: 0,
          error: null,
        };

        try {
          await this.executeStepAction(step);
          stepOutcome.success = true;
          stepOutcome.durationMs = Date.now() - stepStart;
          this.report.stepOutcomes.push(stepOutcome);
          this.report.executedSteps++;

          const guest = this.getActiveGuest();
          if (guest?.exports?.app_fb_hash) {
            const h = guest.exports.app_fb_hash() >>> 0;
            this.report.stateHashes.push({
              stepIndex: this.currentStepIdx,
              hash: h,
              hex: `0x${h.toString(16).padStart(8, '0')}`,
            });
          }

          this.log(`  ✓ [Step ${stepNum}/${total}] ${step.desc || step.action} (${stepOutcome.durationMs}ms)`);
        } catch (err) {
          stepOutcome.success = false;
          stepOutcome.durationMs = Date.now() - stepStart;
          stepOutcome.error = err.message || String(err);
          this.report.stepOutcomes.push(stepOutcome);
          this.report.status = 'failed';
          this.report.error = stepOutcome.error;
          this.state = 'error';

          this.log(`  ✗ [Step ${stepNum}/${total}] FAILED: ${stepOutcome.error}`, 'error');
          this.updateStatus(`Auto Pilot: [FAIL Step ${stepNum}/${total}] ${stepOutcome.error}`, false);
          throw err;
        }

        this.currentStepIdx++;

        // Pacing delay between steps (if not instant)
        if (this.pacingMs > 0 && this.currentStepIdx < mission.steps.length) {
          await this.delay(this.pacingMs);
        }
      }

      if (this.state === 'running') {
        this.state = 'completed';
        this.report.status = 'passed';
        this.report.endTime = new Date().toISOString();
        this.report.durationMs = Date.now() - startTime;

        this.log(`🏁 Auto Pilot mission "${mission.name}" COMPLETED successfully in ${this.report.durationMs}ms!`);
        this.updateStatus(`Auto Pilot: [COMPLETED] ${mission.name} (${this.report.durationMs}ms) · PASS`, true);

        if (this.onReportComplete) {
          this.onReportComplete(this.report);
        }
      } else if (this.state === 'cancelled') {
        this.report.status = 'cancelled';
        this.report.endTime = new Date().toISOString();
        this.report.durationMs = Date.now() - startTime;
        this.log(`🛑 Auto Pilot mission "${mission.name}" was cancelled.`);
        this.updateStatus(`Auto Pilot: [CANCELLED] ${mission.name}`, false);
      }
    } catch (error) {
      this.report.endTime = new Date().toISOString();
      this.report.durationMs = Date.now() - startTime;
      if (this.onReportComplete) {
        this.onReportComplete(this.report);
      }
      throw error;
    }

    return this.report;
  }

  async executeStepAction(step) {
    const { action } = step;
    let guest = this.getActiveGuest();

    switch (action) {
      case 'boot': {
        if (!guest && this.deps.createGuest) {
          guest = await this.deps.createGuest(step.opts || {});
        } else if (guest && step.opts) {
          // If already booted, guest exists
        }
        if (!guest) {
          throw new Error('Action boot: failed to acquire or create guest VM');
        }
        break;
      }

      case 'openApp': {
        if (!guest) throw new Error('Action openApp: guest VM not available');
        const slot = step.slot;
        const slotMap = {
          desktop: 0,
          system: 1,
          calc: 2,
          files: 3,
          mines: 4,
          checkers: 5,
          chess: 6,
        };

        const keyMap = {
          system: 11,   // Digit 1
          calc: 12,     // Digit 2
          files: 13,    // Digit 3
          mines: 14,    // Digit 4
          checkers: 15, // Digit 5
          chess: 16,    // Digit 6
        };

        if (guest.exports?.app_key_mods && keyMap[slot]) {
          guest.exports.app_key_mods(keyMap[slot], MODS.Ctrl);
        } else if (guest.clickLogical) {
          // Dock click fallback: dock icon centers at x=76, y = 32 + 10 + slotIndex * 76 + 38
          const targetSlot = typeof slot === 'number' ? slot : slotMap[slot];
          if (guest.exports?.app_dock_visible && guest.exports.app_dock_visible() === 0) {
            guest.clickLogical(16, 16); // Open dock
          }
          const dockY = 32 + 10 + (targetSlot - 1) * 76 + 38;
          guest.clickLogical(76, dockY);
        }
        if (guest.render) guest.render();
        break;
      }

      case 'key': {
        if (!guest || !guest.exports) throw new Error('Action key: guest VM not available');
        const code = step.code;
        const mods = step.mods || 0;
        if (guest.exports.app_key_mods) {
          guest.exports.app_key_mods(code, mods);
        } else if (guest.exports.app_key) {
          guest.exports.app_key(code);
        }
        if (guest.render) guest.render();
        break;
      }

      case 'click': {
        if (!guest || !guest.clickLogical) throw new Error('Action click: guest VM not available');
        guest.clickLogical(step.x, step.y);
        if (guest.render) guest.render();
        break;
      }

      case 'clickCell': {
        if (!guest || !guest.clickLogical) throw new Error('Action clickCell: guest VM not available');
        const row = step.row;
        const col = step.col;
        if (typeof row !== 'number' || typeof col !== 'number') {
          throw new Error(`Action clickCell: row and col must be numbers (got row=${row}, col=${col})`);
        }
        // Coordinate calculation for Minesweeper 8x8 on 1280x720 (board_x = 134, board_y = 42, cell = 72, gap = 4)
        const cell = 72;
        const gap = 4;
        const boardX = 134;
        const boardY = 42;
        const titleH = 32;

        const x = boardX + col * (cell + gap) + Math.floor(cell / 2);
        const y = titleH + boardY + row * (cell + gap) + Math.floor(cell / 2);

        guest.clickLogical(x, y);
        if (guest.render) guest.render();
        break;
      }

      case 'seedMines': {
        if (!guest || !guest.exports) throw new Error('Action seedMines: guest VM not available');
        const seed = step.seed ?? 42;
        const maxStrikes = step.maxStrikes ?? 1;
        if (guest.exports.app_mines_set_mp) {
          // Setting mp mode with enabled=0 (solo mode) configures seed & rules for single-player play
          const enabled = step.enabled ?? 0;
          guest.exports.app_mines_set_mp(enabled, 1, 1, seed, maxStrikes);
        }
        if (guest.render) guest.render();
        break;
      }

      case 'snapshot': {
        if (!guest) throw new Error('Action snapshot: guest VM not available');
        let snap = null;
        if (this.deps.captureSnapshot) {
          snap = this.deps.captureSnapshot(guest, false);
        } else if (typeof window !== 'undefined' && window.__appliance?.snapshot) {
          snap = window.__appliance.snapshot(guest.id);
        }
        if (snap) {
          this.report.snapshots.push({
            stepIndex: this.currentStepIdx,
            label: step.label || `Step ${this.currentStepIdx + 1}`,
            hash: snap.hash,
            hashHex: snap.hashHex,
            seq: snap.seq,
            view: snap.view,
            dataUrl: snap.dataUrl,
          });
        }
        break;
      }

      case 'assert': {
        if (!guest) throw new Error('Action assert: guest VM not available');
        if (typeof step.check === 'function') {
          const pass = step.check(guest);
          if (!pass) {
            throw new Error(`Assertion failed: ${step.desc || 'custom check returned false'}`);
          }
        }
        break;
      }

      case 'assertStatus': {
        if (!guest || !guest.exports) throw new Error('Action assertStatus: guest VM not available');
        const expected = step.status;
        let actual = 'UNKNOWN';

        const sel = guest.exports.app_selected ? guest.exports.app_selected() : 0;
        if (sel === 4) {
          // Minesweeper status inspection
          const fb = guest.exports.app_framebuffer();
          const mem = new Uint8Array(guest.exports.memory.buffer);
          // Check detonated cell 6 or revealed cell 0
          // If exploded, cell 6 (col 6, row 0) at (626, 110) has coral red danger background
          const cell6Off = fb + (110 * 1280 + 626) * 4;
          const r = mem[cell6Off];
          const g = mem[cell6Off + 1];
          const b = mem[cell6Off + 2];

          if (r > 200 && g > 150 && b > 150) {
            actual = 'EXPLODED';
          } else {
            actual = 'PLAYING';
          }
        }

        if (actual !== expected) {
          throw new Error(`AssertStatus failed: expected status '${expected}', got '${actual}'`);
        }
        break;
      }

      case 'assertHash': {
        if (!guest || !guest.exports) throw new Error('Action assertHash: guest VM not available');
        const currentHash = guest.exports.app_fb_hash ? (guest.exports.app_fb_hash() >>> 0) : 0;
        if (step.expectedHash && currentHash !== step.expectedHash) {
          throw new Error(`AssertHash failed: expected 0x${step.expectedHash.toString(16)}, got 0x${currentHash.toString(16)}`);
        }
        break;
      }

      case 'wait': {
        const ms = step.ms || 300;
        await this.delay(ms);
        break;
      }

      default:
        throw new Error(`Unknown step action: '${action}'`);
    }
  }

  pause() {
    if (this.state === 'running') {
      this.state = 'paused';
      this.log('⏸️ Auto Pilot paused');
      this.updateStatus('Auto Pilot: [PAUSED]', true);
    }
  }

  resume() {
    if (this.state === 'paused') {
      this.state = 'running';
      this.log('▶️ Auto Pilot resumed');
      if (this.stepResolver) {
        const res = this.stepResolver;
        this.stepResolver = null;
        res();
      }
    }
  }

  step() {
    if (this.state === 'paused') {
      this.log('⏭️ Auto Pilot step-by-step');
      if (this.stepResolver) {
        const res = this.stepResolver;
        this.stepResolver = null;
        res();
      }
    }
  }

  cancel() {
    if (this.state === 'running' || this.state === 'paused') {
      this.state = 'cancelled';
      this.log('🛑 Auto Pilot cancelled');
      if (this.stepResolver) {
        const res = this.stepResolver;
        this.stepResolver = null;
        res();
      }
    }
  }

  delay(ms) {
    return new Promise((resolve) => {
      this.timer = setTimeout(resolve, ms);
    });
  }
}
