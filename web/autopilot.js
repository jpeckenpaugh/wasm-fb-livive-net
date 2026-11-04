/* Auto Pilot Runner: Deterministic Mission Runner, Action Interpreter & State Assertions */

import { MISSIONS, getMissionById, loadMissions } from './missions.js';
import { syncPlayerAvatarsToGuest } from './services/avatar.js';

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

  resolveTargetGuest(targetVm, forBoot = false) {
    if (!targetVm || targetVm === 'active') {
      return this.getActiveGuest();
    }
    const guests = this.deps.getGuests ? this.deps.getGuests() : [];
    if (!guests || guests.length === 0) return null;

    if (typeof targetVm === 'number') {
      return guests.find((g) => g.id === targetVm) || guests[targetVm] || null;
    }

    const t = String(targetVm).toLowerCase();
    if (t === 'p1' || t === 'seat1' || t === 'seat-1' || t === 'vm-01' || t === 'vm1') {
      if (forBoot) {
        return guests[0] || null;
      }
      return (
        guests.find((g) => g.mpSeat === 1) ||
        guests[0] ||
        guests.find((g) => g.name === 'vm-01' || g.id === 1) ||
        null
      );
    }
    if (t === 'p2' || t === 'seat2' || t === 'seat-2' || t === 'vm-02' || t === 'vm2') {
      if (forBoot) {
        return guests[1] || null;
      }
      return (
        guests.find((g) => g.mpSeat === 2) ||
        guests[1] ||
        guests.find((g) => g.name === 'vm-02' || g.id === 2) ||
        null
      );
    }
    if (t === 'p3' || t === 'seat3' || t === 'seat-3' || t === 'vm-03' || t === 'vm3') {
      if (forBoot) {
        return guests[2] || null;
      }
      return (
        guests[2] ||
        guests.find((g) => g.name === 'vm-03' || g.id === 3) ||
        null
      );
    }
    if (t === 'p4' || t === 'seat4' || t === 'seat-4' || t === 'vm-04' || t === 'vm4') {
      if (forBoot) {
        return guests[3] || null;
      }
      return (
        guests[3] ||
        guests.find((g) => g.name === 'vm-04' || g.id === 4) ||
        null
      );
    }

    return guests.find((g) => g.name === targetVm || String(g.id) === t) || (forBoot ? null : this.getActiveGuest());
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

    if (!MISSIONS || MISSIONS.length === 0) {
      await loadMissions();
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
    this.customResolution = opts.resolution || null;

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
    let guest = step.targetVm ? this.resolveTargetGuest(step.targetVm, action === 'boot') : this.getActiveGuest();

    switch (action) {
      case 'boot': {
        const bootOpts = Object.assign({}, step.opts || {});
        if (this.customResolution) {
          bootOpts.width = this.customResolution.width;
          bootOpts.height = this.customResolution.height;
        }

        if (!guest && this.deps.createGuest) {
          guest = await this.deps.createGuest(bootOpts);
        } else if (guest && (bootOpts.width || bootOpts.height)) {
          // If already booted but resolution differs, re-instantiate guest to match resolution
          if ((bootOpts.width && guest.width !== bootOpts.width) ||
              (bootOpts.height && guest.height !== bootOpts.height)) {
            const preservedSeat = guest.mpSeat;
            const preservedMatchId = guest.mpMatchId;
            const preservedGame = guest.mpGame;
            const preservedActor = guest.actorType;
            if (this.deps.destroyGuest) {
              this.deps.destroyGuest(guest);
            } else if (guest.wrap && guest.panel) {
              guest.wrap.remove();
              guest.panel.remove();
            }
            if (this.deps.createGuest) {
              guest = await this.deps.createGuest(bootOpts);
              if (preservedSeat) guest.mpSeat = preservedSeat;
              if (preservedMatchId) guest.mpMatchId = preservedMatchId;
              if (preservedGame) guest.mpGame = preservedGame;
              if (preservedActor) guest.actorType = preservedActor;
            }
          }
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
        let clickX = step.x;
        let clickY = step.y;

        // Support semantic target in click action if present
        if (step.target) {
          const t = step.target;
          if (t.game === 'chess' || t.square) {
            let sqId = t.id;
            if (sqId == null && t.square) {
              const file = t.square.charCodeAt(0) - 97;
              const rank = parseInt(t.square[1], 10) - 1;
              sqId = rank * 8 + file;
            }
            if (guest.exports?.app_semantic_coord && sqId != null) {
              const packed = guest.exports.app_semantic_coord(2, sqId) >>> 0;
              if (packed !== 0) {
                clickX = packed >> 16;
                clickY = packed & 0xFFFF;
              }
            }
          } else if (t.game === 'mines' || (t.row != null && t.col != null)) {
            const cellId = t.id ?? (t.row * 8 + t.col);
            if (guest.exports?.app_semantic_coord && cellId != null) {
              const packed = guest.exports.app_semantic_coord(4, cellId) >>> 0;
              if (packed !== 0) {
                clickX = packed >> 16;
                clickY = packed & 0xFFFF;
              }
            }
          }
        }

        guest.clickLogical(clickX, clickY);
        if (guest.render) guest.render();
        break;
      }

      case 'clickSquare': {
        if (!guest || !guest.clickLogical) throw new Error('Action clickSquare: guest VM not available');
        const sq = step.square;
        let sqId = typeof step.id === 'number' ? step.id : null;
        if (sqId == null && typeof sq === 'string') {
          const file = sq.toLowerCase().charCodeAt(0) - 97;
          const rank = parseInt(sq[1], 10) - 1;
          if (file >= 0 && file < 8 && rank >= 0 && rank < 8) {
            sqId = rank * 8 + file;
          }
        }
        if (sqId == null) {
          throw new Error(`Action clickSquare: invalid square '${sq}'`);
        }

        let clickX = step.x;
        let clickY = step.y;
        if (guest.exports?.app_semantic_coord) {
          const kind = step.game === 'checkers' ? 3 : 2;
          const packed = guest.exports.app_semantic_coord(kind, sqId) >>> 0;
          if (packed !== 0) {
            clickX = packed >> 16;
            clickY = packed & 0xFFFF;
          }
        }
        if (clickX == null || clickY == null) {
          throw new Error(`Action clickSquare: unable to compute coordinates for square '${sq}'`);
        }

        guest.clickLogical(clickX, clickY);
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

        let clickX = null;
        let clickY = null;

        if (guest.exports?.app_semantic_coord) {
          const cellId = row * 8 + col;
          const packed = guest.exports.app_semantic_coord(4, cellId) >>> 0;
          if (packed !== 0) {
            clickX = packed >> 16;
            clickY = packed & 0xFFFF;
          }
        }

        if (clickX == null || clickY == null) {
          // Fallback coordinate calculation for Minesweeper 8x8 on 1280x720
          const cell = 72;
          const gap = 4;
          const boardX = 134;
          const boardY = 42;
          const titleH = 32;

          clickX = boardX + col * (cell + gap) + Math.floor(cell / 2);
          clickY = titleH + boardY + row * (cell + gap) + Math.floor(cell / 2);
        }

        guest.clickLogical(clickX, clickY);
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

      case 'splitSnapshot': {
        const guests = this.deps.getGuests ? this.deps.getGuests() : [];
        const g1 = this.resolveTargetGuest('p1') || guests[0];
        const g2 = this.resolveTargetGuest('p2') || guests[1];
        const g3 = this.resolveTargetGuest('p3') || guests[2];
        const g4 = this.resolveTargetGuest('p4') || guests[3];
        if (!g1 || !g2) {
          throw new Error('Action splitSnapshot requires at least two guests (P1 and P2)');
        }

        let snap = null;
        if (g3 && g4 && typeof document !== 'undefined') {
          // Quad-view composite snapshot (2x2 grid)
          const w = g1.width || 640;
          const h = g1.height || 480;
          const compW = w * 2;
          const compH = h * 2;

          const canvas = document.createElement('canvas');
          canvas.width = compW;
          canvas.height = compH;
          const ctx = canvas.getContext('2d');
          if (ctx) {
            ctx.fillStyle = '#0f172a';
            ctx.fillRect(0, 0, compW, compH);
            if (g1.canvas) ctx.drawImage(g1.canvas, 0, 0, w, h);
            if (g2.canvas) ctx.drawImage(g2.canvas, w, 0, w, h);
            if (g3.canvas) ctx.drawImage(g3.canvas, 0, h, w, h);
            if (g4.canvas) ctx.drawImage(g4.canvas, w, h, w, h);
          }
          const dataUrl = canvas.toDataURL('image/png');
          const h1 = g1.exports?.app_fb_hash ? `0x${(g1.exports.app_fb_hash() >>> 0).toString(16)}` : '0x0';
          const h2 = g2.exports?.app_fb_hash ? `0x${(g2.exports.app_fb_hash() >>> 0).toString(16)}` : '0x0';
          const h3 = g3.exports?.app_fb_hash ? `0x${(g3.exports.app_fb_hash() >>> 0).toString(16)}` : '0x0';
          const h4 = g4.exports?.app_fb_hash ? `0x${(g4.exports.app_fb_hash() >>> 0).toString(16)}` : '0x0';

          snap = {
            label: step.label || `Quad Snapshot (4-VM 2x2 Grid)`,
            dataUrl,
            hashHex: `${h1} | ${h2} | ${h3} | ${h4}`,
            seq: 1,
            view: 'Quad View 2x2 Grid',
          };
        } else if (this.deps.captureSplitSnapshot) {
          snap = this.deps.captureSplitSnapshot(g1, g2);
        } else if (typeof window !== 'undefined' && window.__appliance?.splitSnapshot) {
          snap = window.__appliance.splitSnapshot(g1.id, g2.id);
        } else if (typeof document !== 'undefined') {
          // In-browser canvas composite fallback
          const w1 = g1.width || 1280;
          const h1 = g1.height || 720;
          const w2 = g2.width || 1280;
          const h2 = g2.height || 720;
          const compW = w1 + w2;
          const compH = Math.max(h1, h2);

          const canvas = document.createElement('canvas');
          canvas.width = compW;
          canvas.height = compH;
          const ctx = canvas.getContext('2d');
          if (ctx) {
            ctx.fillStyle = '#0f172a';
            ctx.fillRect(0, 0, compW, compH);
            if (g1.canvas) ctx.drawImage(g1.canvas, 0, 0, w1, h1);
            if (g2.canvas) ctx.drawImage(g2.canvas, w1, 0, w2, h2);
          }
          const dataUrl = canvas.toDataURL('image/png');
          const h1Hex = g1.exports?.app_fb_hash ? `0x${(g1.exports.app_fb_hash() >>> 0).toString(16).padStart(8, '0')}` : '0x0';
          const h2Hex = g2.exports?.app_fb_hash ? `0x${(g2.exports.app_fb_hash() >>> 0).toString(16).padStart(8, '0')}` : '0x0';

          snap = {
            label: step.label || `Split Snapshot (P1 ${g1.name} | P2 ${g2.name})`,
            dataUrl,
            hashHex: `${h1Hex} | ${h2Hex}`,
            seq: 1,
            view: 'Split View Dual-Perspective',
          };
        }

        if (snap) {
          this.report.snapshots.push({
            stepIndex: this.currentStepIdx,
            label: step.label || snap.label || `Step ${this.currentStepIdx + 1} Split Snapshot`,
            hash: snap.hash || 0,
            hashHex: snap.hashHex || '',
            seq: snap.seq || 0,
            view: snap.view || 'Split View',
            dataUrl: snap.dataUrl,
          });
        }
        break;
      }

      case 'assertTurn': {
        if (!guest || !guest.exports) throw new Error('Action assertTurn: guest VM not available');
        const expectedSeat = step.seat;
        let actualTurn = null;
        if (guest.mpGame === 'checkers' && guest.exports.app_checkers_turn) {
          actualTurn = guest.exports.app_checkers_turn();
        } else if (guest.mpGame === 'chess' && guest.exports.app_chess_turn) {
          actualTurn = guest.exports.app_chess_turn();
        } else if (guest.mpGame === 'mines' && guest.exports.app_mines_turn) {
          actualTurn = guest.exports.app_mines_turn();
        } else if (guest.exports.app_mines_turn) {
          actualTurn = guest.exports.app_mines_turn();
        } else if (guest.exports.app_checkers_turn) {
          actualTurn = guest.exports.app_checkers_turn();
        } else if (guest.exports.app_chess_turn) {
          actualTurn = guest.exports.app_chess_turn();
        }
        if (expectedSeat != null && actualTurn !== expectedSeat) {
          throw new Error(`AssertTurn failed for ${guest.name}: expected turn ${expectedSeat}, got ${actualTurn}`);
        }
        break;
      }

      case 'assertState': {
        if (!guest || !guest.exports) throw new Error('Action assertState: guest VM not available');
        if (typeof step.check === 'function') {
          const pass = step.check(guest);
          if (!pass) {
            throw new Error(`AssertState failed for ${guest.name}: ${step.desc || 'check returned false'}`);
          }
        }
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

      case 'setSplitView': {
        if (typeof window !== 'undefined' && window.__appliance?.setSplitView) {
          window.__appliance.setSplitView(step.enabled !== false);
        }
        break;
      }

      case 'configureMatch': {
        if (!guest || !guest.exports) throw new Error('Action configureMatch: guest VM not available');
        const broker = typeof window !== 'undefined' ? window.__broker : null;
        const matchId = step.matchId || 101;
        const game = step.game || 'mines';
        const seat = step.seat || 1;
        const seed = step.seed ?? 42;
        const maxStrikes = step.maxStrikes ?? 1;

        if (broker) {
          let m = broker.tables.matches.find((x) => x.id === matchId);
          if (!m) {
            m = {
              id: matchId,
              game,
              seed,
              max_strikes: maxStrikes,
              status: 'PLAYING',
              turn_seat: 1,
              p1_vm: 'vm-01',
              p2_vm: 'vm-02',
              p1_id: 1,
              p2_id: 2,
              p1_score: game === 'chess' ? 16 : game === 'checkers' ? 12 : 0,
              p2_score: game === 'chess' ? 16 : game === 'checkers' ? 12 : 0,
              state_hash: game === 'mines' ? `seed:${seed}` : '0x00000000',
              last_pdn: '—',
              winner_seat: 0,
              seq: 0,
              created_at: broker.nowIso ? broker.nowIso() : new Date().toISOString().slice(11, 19),
            };
            broker.tables.matches.unshift(m);
          }
          broker.activeMatchId = matchId;
          broker.renderDrawer();
          if (typeof window !== 'undefined') {
            const brokerDrawer = document.getElementById('broker-drawer');
            if (brokerDrawer) brokerDrawer.hidden = false;
          }
        }

        if (game === 'checkers' && guest.exports.app_checkers_set_mp) {
          guest.exports.app_checkers_set_mp(1, matchId, seat, 1);
        } else if (game === 'chess' && guest.exports.app_chess_set_mp) {
          guest.exports.app_chess_set_mp(1, matchId, seat, 0);
        } else if (guest.exports.app_mines_set_mp) {
          guest.exports.app_mines_set_mp(1, matchId, seat, seed, maxStrikes);
        }

        guest.mpSeat = seat;
        guest.mpMatchId = matchId;
        guest.mpGame = game;
        guest.actorType = 'AutoPilot';

        // Synchronize match-level player identities across peer VMs
        const allGuests = this.deps.getGuests ? this.deps.getGuests() : [];
        const peers = allGuests.filter((g) => g.mpMatchId === matchId);
        const p1Vm = peers.find((g) => g.mpSeat === 1) || (seat === 1 ? guest : null);
        const p2Vm = peers.find((g) => g.mpSeat === 2) || (seat === 2 ? guest : null);

        const p1Seed = step.p1Seed || p1Vm?.identity?.seed || p1Vm?.identity?.name || (matchId === 802 ? 'Alex' : 'Jarad');
        const p2Seed = step.p2Seed || p2Vm?.identity?.seed || p2Vm?.identity?.name || (matchId === 802 ? 'Minimax-4ply' : 'Minimax-2ply');

        for (const peer of peers) {
          syncPlayerAvatarsToGuest(peer, p1Seed, p2Seed);
        }
        syncPlayerAvatarsToGuest(guest, p1Seed, p2Seed);

        if (guest.render) guest.render();
        break;
      }

      case 'assert': {
        if (!guest) throw new Error('Action assert: guest VM not available');
        if (typeof step.check === 'function') {
          const pass = step.check(guest);
          if (!pass) {
            throw new Error(`Assertion failed: ${step.desc || 'custom check returned false'}`);
          }
        } else if (step.assertType) {
          const exp = guest.exports;
          let pass = false;
          let actualVal = undefined;
          switch (step.assertType) {
            case 'app_selected':
              actualVal = exp?.app_selected ? exp.app_selected() : -1;
              pass = actualVal === step.expected;
              break;
            case 'app_active_page':
              actualVal = exp?.app_active_page ? exp.app_active_page() : -1;
              pass = actualVal === step.expected;
              break;
            case 'app_theme_not':
              actualVal = exp?.app_theme ? exp.app_theme() : 0;
              pass = actualVal !== step.expected;
              break;
            case 'mines_seed':
              actualVal = exp?.app_mines_seed ? exp.app_mines_seed() >>> 0 : -1;
              pass = actualVal === step.expected;
              break;
            case 'checkers_turn':
              actualVal = exp?.app_checkers_turn ? exp.app_checkers_turn() : -1;
              pass = actualVal === step.expected;
              break;
            case 'checkers_ai_pending':
              actualVal = exp?.app_checkers_ai_pending ? exp.app_checkers_ai_pending() : 0;
              pass = actualVal === step.expected;
              break;
            case 'chess_turn':
              actualVal = exp?.app_chess_turn ? exp.app_chess_turn() : -1;
              pass = actualVal === step.expected;
              break;
            case 'chess_ai_pending':
              actualVal = exp?.app_chess_ai_pending ? exp.app_chess_ai_pending() : 0;
              pass = actualVal === step.expected;
              break;
            case 'chess_status':
              actualVal = exp?.app_chess_status ? exp.app_chess_status() : -1;
              pass = actualVal === step.expected;
              break;
            case 'mines_p1_score_gt':
              actualVal = exp?.app_mines_p1_score ? exp.app_mines_p1_score() : 0;
              pass = actualVal > step.expected;
              break;
            case 'mines_p2_score_gt':
              actualVal = exp?.app_mines_p2_score ? exp.app_mines_p2_score() : 0;
              pass = actualVal > step.expected;
              break;
            case 'mines_winner':
              actualVal = exp?.app_mines_winner ? exp.app_mines_winner() : 0;
              pass = actualVal === step.expected;
              break;
            default:
              throw new Error(`Unknown assertType: ${step.assertType}`);
          }
          if (!pass) {
            throw new Error(`Assertion failed for ${guest.name}: [${step.assertType}] expected ${step.expected}, got ${actualVal}`);
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
        const hashFn = step.kind === 'checkers'
          ? (guest.exports.app_checkers_state_hash ? () => guest.exports.app_checkers_state_hash() >>> 0 : null)
          : step.kind === 'chess'
            ? (guest.exports.app_chess_state_hash ? () => guest.exports.app_chess_state_hash() >>> 0 : null)
            : (guest.exports.app_fb_hash ? () => guest.exports.app_fb_hash() >>> 0 : null);
        
        const currentHash = hashFn ? hashFn() : (guest.exports.app_fb_hash ? (guest.exports.app_fb_hash() >>> 0) : 0);

        if (step.compareToVm) {
          const peer = this.resolveTargetGuest(step.compareToVm);
          if (!peer || !peer.exports) throw new Error(`AssertHash compareToVm: peer VM '${step.compareToVm}' not available`);
          const peerHashFn = step.kind === 'checkers'
            ? (peer.exports.app_checkers_state_hash ? () => peer.exports.app_checkers_state_hash() >>> 0 : null)
            : step.kind === 'chess'
              ? (peer.exports.app_chess_state_hash ? () => peer.exports.app_chess_state_hash() >>> 0 : null)
              : (peer.exports.app_fb_hash ? () => peer.exports.app_fb_hash() >>> 0 : null);
          const peerHash = peerHashFn ? peerHashFn() : (peer.exports.app_fb_hash ? (peer.exports.app_fb_hash() >>> 0) : 0);
          if (currentHash !== peerHash) {
            throw new Error(`AssertHash divergence between ${guest.name} (0x${currentHash.toString(16)}) and ${peer.name} (0x${peerHash.toString(16)})`);
          }
        } else if (step.expectedHash && currentHash !== step.expectedHash) {
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
