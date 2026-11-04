/* Trajectory Recording & Playback UI wiring */
import { MISSIONS } from '../missions.js';
import {
  exportTrajectoryJson,
  downloadTrajectoryFile,
  importTrajectoryJson,
  trajectoryToAutoPilotMission,
} from '../trajectory.js';

let deps = null;

export function configureTrajectoryUiDeps(injectedDeps) {
  deps = injectedDeps;
}

export function initTrajectoryUi() {
  if (!deps) {
    throw new Error('TrajectoryUi dependencies not configured before init');
  }

  const {
    trajectoryRecorder,
    trajectoryPlayer,
    getActiveGuest,
    getActiveId,
    getGuests,
    setStatus,
    openInspectorSection,
  } = deps;

  const recordBtn = document.getElementById('record-btn');
  const trajDrawerBtn = document.getElementById('traj-drawer-btn');
  const trajectoryDrawer = document.getElementById('trajectory-drawer');
  const trajHideBtn = document.getElementById('traj-hide-btn');

  const trajRunnerState = document.getElementById('traj-runner-state');
  const trajStatusBadge = document.getElementById('traj-status-badge');
  const trajActionsStat = document.getElementById('traj-actions-stat');
  const trajDurationStat = document.getElementById('traj-duration-stat');
  const trajParityBadge = document.getElementById('traj-parity-badge');
  const trajTargetStat = document.getElementById('traj-target-stat');
  const trajRecordedResStat = document.getElementById('traj-recorded-res-stat');
  const trajModeBadge = document.getElementById('traj-mode-badge');
  const trajModeGroup = document.getElementById('traj-mode-group');

  const trajSaveBtn = document.getElementById('traj-save-btn');
  const trajLoadBtn = document.getElementById('traj-load-btn');
  const trajFileInput = document.getElementById('traj-file-input');
  const trajConvertBtn = document.getElementById('traj-convert-btn');

  const trajStepBtn = document.getElementById('traj-step-btn');
  const trajPlayBtn = document.getElementById('traj-play-btn');
  const trajPauseBtn = document.getElementById('traj-pause-btn');
  const trajStopBtn = document.getElementById('traj-stop-btn');
  const trajSpeedGroup = document.getElementById('traj-speed-group');

  const trajActionLabel = document.getElementById('traj-action-label');
  const trajProgressPct = document.getElementById('traj-progress-pct');
  const trajTimelineFill = document.getElementById('traj-timeline-fill');
  const trajEventsList = document.getElementById('traj-events-list');

  let lastTrajectoryData = null;
  let currentPlaybackSpeed = 1.0;
  let recPulseInterval = null;

  function formatMsDuration(ms) {
    const totalSecs = Math.floor(ms / 1000);
    const m = Math.floor(totalSecs / 60).toString().padStart(2, '0');
    const s = (totalSecs % 60).toString().padStart(2, '0');
    return `${m}:${s}`;
  }

  function updateRecordButtonUI(isRecording) {
    if (!recordBtn) return;
    if (isRecording) {
      recordBtn.classList.add('recording-active');
      if (recPulseInterval) clearInterval(recPulseInterval);
      recPulseInterval = setInterval(() => {
        if (trajectoryRecorder.isRecording()) {
          const dur = formatMsDuration(trajectoryRecorder.getDuration());
          const count = trajectoryRecorder.getActionCount();
          recordBtn.textContent = `⏺ REC ${dur} · ${count} act`;
        }
      }, 500);
      const dur = formatMsDuration(trajectoryRecorder.getDuration());
      const count = trajectoryRecorder.getActionCount();
      recordBtn.textContent = `⏺ REC ${dur} · ${count} act`;
    } else {
      recordBtn.classList.remove('recording-active');
      if (recPulseInterval) {
        clearInterval(recPulseInterval);
        recPulseInterval = null;
      }
      recordBtn.textContent = '⏺ record';
    }
  }

  function updateTrajectoryDrawerUI() {
    const data = lastTrajectoryData || trajectoryRecorder.exportData();
    const count = data?.events?.length || 0;
    const duration = data?.metadata?.durationMs || 0;
    const activeId = getActiveId ? getActiveId() : null;
    const targetId = data?.metadata?.guestId != null ? data.metadata.guestId : (activeId != null ? activeId : '—');

    if (trajActionsStat) trajActionsStat.textContent = count;
    if (trajDurationStat) trajDurationStat.textContent = `${duration}ms`;
    if (trajTargetStat) trajTargetStat.textContent = targetId;

    const recRes = data?.metadata?.recordedResolution || data?.metadata?.resolution;
    if (trajRecordedResStat) {
      trajRecordedResStat.textContent = recRes ? `${recRes.width}×${recRes.height}` : '—';
    }

    // Update mode pill
    const guests = getGuests ? getGuests() : [];
    const activeGuest = getActiveGuest ? getActiveGuest() : (guests[0] || null);
    const resMatch = recRes && activeGuest && activeGuest.width === recRes.width && activeGuest.height === recRes.height;
    const currentMode = trajectoryPlayer.getPlaybackMode();

    if (trajModeBadge) {
      if (currentMode === 'semantic' || (currentMode === 'auto' && !resMatch)) {
        trajModeBadge.className = 'traj-badge ok';
        trajModeBadge.textContent = '🧠 Semantic (Adaptive)';
      } else {
        trajModeBadge.className = 'traj-badge neutral';
        trajModeBadge.textContent = '🎯 Physical 1:1';
      }
    }

    // Update hash parity badge
    if (trajParityBadge) {
      if (trajectoryPlayer.hashDivergenceCount > 0) {
        trajParityBadge.className = 'traj-badge diverged';
        trajParityBadge.textContent = `${trajectoryPlayer.hashDivergenceCount} DIVERGED`;
      } else {
        trajParityBadge.className = 'traj-badge ok';
        trajParityBadge.textContent = '100% VERIFIED';
      }
    }

    // Populate events list
    if (trajEventsList && data && data.events) {
      trajEventsList.replaceChildren();
      if (data.events.length === 0) {
        const li = document.createElement('li');
        li.textContent = 'No events recorded yet. Click [⏺ record] to record interactions.';
        trajEventsList.appendChild(li);
      } else {
        data.events.forEach((ev, idx) => {
          const li = document.createElement('li');
          const hashHex = `0x${(ev.hash >>> 0).toString(16).padStart(8, '0')}`;
          const clientHex = ev.clientHash != null ? ` · client=0x${(ev.clientHash >>> 0).toString(16).padStart(8, '0')}` : '';
          let desc = '';
          if (ev.type === 'click') desc = `Click (${ev.x}, ${ev.y})`;
          else if (ev.type === 'key') desc = `Key code=${ev.code} mods=${ev.mods || 0}`;
          else if (ev.type === 'openApp') desc = `OpenApp slot=${ev.slot}`;
          else if (ev.type === 'seedMines') desc = `SeedMines seed=${ev.seed} strikes=${ev.maxStrikes}`;
          else desc = `${ev.type} ${JSON.stringify(ev)}`;

          li.textContent = `[#${idx + 1}] dt=${ev.dt}ms · ${desc} · hash=${hashHex}${clientHex}`;
          li.dataset.idx = idx;
          trajEventsList.appendChild(li);
        });
      }
    }
  }

  // Player event callbacks
  trajectoryPlayer.onStateChange = (state) => {
    if (trajRunnerState) trajRunnerState.textContent = state;
    if (trajStatusBadge) {
      trajStatusBadge.textContent = state.toUpperCase();
      if (state === 'playing') {
        trajStatusBadge.className = 'traj-badge ok';
      } else if (state === 'paused') {
        trajStatusBadge.className = 'traj-badge neutral';
      } else if (state === 'completed') {
        trajStatusBadge.className = 'traj-badge ok';
      } else {
        trajStatusBadge.className = 'traj-badge neutral';
      }
    }

    if (trajPlayBtn) trajPlayBtn.disabled = state === 'playing';
    if (trajPauseBtn) trajPauseBtn.disabled = state !== 'playing';
    if (trajStepBtn) trajStepBtn.disabled = state === 'playing';
    if (trajStopBtn) trajStopBtn.disabled = state === 'idle';
  };

  trajectoryPlayer.onProgress = ({ index, total, percent, divergences }) => {
    if (trajTimelineFill) {
      trajTimelineFill.style.width = `${percent.toFixed(1)}%`;
    }
    if (trajProgressPct) {
      trajProgressPct.textContent = `${Math.round(percent)}%`;
    }
    if (trajActionLabel) {
      trajActionLabel.textContent = `Action ${index}/${total} · ${divergences} divergence(s)`;
    }
    if (trajParityBadge) {
      if (divergences > 0) {
        trajParityBadge.className = 'traj-badge diverged';
        trajParityBadge.textContent = `${divergences} DIVERGED`;
      } else {
        trajParityBadge.className = 'traj-badge ok';
        trajParityBadge.textContent = '100% VERIFIED';
      }
    }

    // Highlight current event in list
    if (trajEventsList && trajEventsList.children) {
      const items = trajEventsList.children;
      for (let i = 0; i < items.length; i++) {
        if (i === index - 1) {
          items[i].classList.add('current');
          items[i].scrollIntoView({ block: 'nearest' });
        } else {
          items[i].classList.remove('current');
        }
      }
    }
  };

  trajectoryPlayer.onHashDivergence = ({ index, expectedHex, actualHex }) => {
    if (trajEventsList && trajEventsList.children[index]) {
      trajEventsList.children[index].classList.add('diverged');
    }
    setStatus(`Trajectory Hash Divergence at event #${index + 1}: expected ${expectedHex}, got ${actualHex}`, false);
  };

  trajectoryPlayer.onComplete = ({ divergences, total }) => {
    setStatus(`Trajectory playback finished: ${total} events replayed with ${divergences} divergences.`, divergences === 0);
  };

  // UI Element Event Handlers
  if (recordBtn) {
    recordBtn.addEventListener('click', () => {
      if (trajectoryRecorder.isRecording()) {
        const data = trajectoryRecorder.stop();
        updateRecordButtonUI(false);
        lastTrajectoryData = data;
        trajectoryPlayer.load(data);
        updateTrajectoryDrawerUI();
        if (trajectoryDrawer && openInspectorSection) {
          openInspectorSection(trajectoryDrawer);
        }
        setStatus(`Trajectory recording saved: ${data.events.length} actions captured.`, true);
      } else {
        const g = getActiveGuest ? getActiveGuest() : null;
        if (!g) {
          setStatus('Cannot start recording: No active guest VM', false);
          return;
        }
        trajectoryRecorder.start(g);
        updateRecordButtonUI(true);
        setStatus(`⏺ Recording started on guest ${g.name} (${g.width}×${g.height})`, true);
      }
    });
  }

  if (trajDrawerBtn && trajectoryDrawer) {
    trajDrawerBtn.addEventListener('click', () => {
      if (trajectoryDrawer.hidden) {
        updateTrajectoryDrawerUI();
        if (openInspectorSection) openInspectorSection(trajectoryDrawer);
      } else {
        trajectoryDrawer.hidden = true;
      }
    });
  }

  if (trajHideBtn && trajectoryDrawer) {
    trajHideBtn.addEventListener('click', () => {
      trajectoryDrawer.hidden = true;
    });
  }

  if (trajPlayBtn) {
    trajPlayBtn.addEventListener('click', () => {
      if (!lastTrajectoryData) {
        setStatus('No trajectory loaded to play', false);
        return;
      }
      trajectoryPlayer.play(currentPlaybackSpeed);
    });
  }

  if (trajPauseBtn) {
    trajPauseBtn.addEventListener('click', () => {
      trajectoryPlayer.pause();
    });
  }

  if (trajStopBtn) {
    trajStopBtn.addEventListener('click', () => {
      trajectoryPlayer.stop();
    });
  }

  if (trajStepBtn) {
    trajStepBtn.addEventListener('click', () => {
      if (!lastTrajectoryData) {
        setStatus('No trajectory loaded to step', false);
        return;
      }
      trajectoryPlayer.stepForward();
    });
  }

  if (trajSpeedGroup) {
    trajSpeedGroup.addEventListener('click', (ev) => {
      const btn = ev.target.closest('button[data-speed]');
      if (!btn) return;
      const speed = parseFloat(btn.dataset.speed);
      currentPlaybackSpeed = speed;
      Array.from(trajSpeedGroup.children).forEach((b) => b.classList.remove('active-speed'));
      btn.classList.add('active-speed');
      if (trajectoryPlayer.state === 'playing') {
        trajectoryPlayer.speed = speed;
      }
    });
  }

  if (trajModeGroup) {
    trajModeGroup.addEventListener('click', (ev) => {
      const btn = ev.target.closest('button[data-mode]');
      if (!btn) return;
      const mode = btn.dataset.mode;
      trajectoryPlayer.setPlaybackMode(mode);
      Array.from(trajModeGroup.children).forEach((b) => b.classList.remove('active-speed'));
      btn.classList.add('active-speed');
      updateTrajectoryDrawerUI();
    });
  }

  trajectoryPlayer.onModeChange = () => {
    updateTrajectoryDrawerUI();
  };

  if (trajSaveBtn) {
    trajSaveBtn.addEventListener('click', () => {
      const data = lastTrajectoryData || trajectoryRecorder.exportData();
      if (!data || !data.events || data.events.length === 0) {
        setStatus('No trajectory events to save', false);
        return;
      }
      const fname = downloadTrajectoryFile(data);
      setStatus(`Trajectory saved to ${fname}`, true);
    });
  }

  if (trajLoadBtn && trajFileInput) {
    trajLoadBtn.addEventListener('click', () => {
      trajFileInput.click();
    });

    trajFileInput.addEventListener('change', (ev) => {
      const file = ev.target.files?.[0];
      if (!file) return;
      const reader = new FileReader();
      reader.onload = (e) => {
        try {
          const text = e.target.result;
          const data = importTrajectoryJson(text);
          lastTrajectoryData = data;
          trajectoryPlayer.load(data);
          updateTrajectoryDrawerUI();
          setStatus(`Loaded trajectory with ${data.events.length} events from ${file.name}`, true);
        } catch (err) {
          console.error('Failed to import trajectory:', err);
          setStatus(`Error loading trajectory: ${err.message}`, false);
        }
      };
      reader.readAsText(file);
      trajFileInput.value = '';
    });
  }

  if (trajConvertBtn) {
    trajConvertBtn.addEventListener('click', () => {
      const data = lastTrajectoryData || trajectoryRecorder.exportData();
      if (!data || !data.events || data.events.length === 0) {
        setStatus('No trajectory events to convert', false);
        return;
      }
      const mission = trajectoryToAutoPilotMission(data);
      MISSIONS.push(mission);

      const apMissionSelect = document.getElementById('ap-mission-select');
      if (apMissionSelect) {
        const opt = document.createElement('option');
        opt.value = mission.id;
        opt.textContent = `[Trajectory] ${mission.name}`;
        opt.selected = true;
        apMissionSelect.appendChild(opt);
      }

      const autoPilotPanel = document.getElementById('autopilot-panel');
      if (autoPilotPanel && openInspectorSection) {
        openInspectorSection(autoPilotPanel);
      }

      setStatus(`Converted trajectory to Auto Pilot mission "${mission.name}"!`, true);
    });
  }

  return {
    updateTrajectoryDrawerUI,
    updateRecordButtonUI,
  };
}
