/* Auto Pilot Mission Control UI wiring */
import { MISSIONS, loadMissions } from '../missions.js';

let deps = null;

export function configureAutoPilotUiDeps(injectedDeps) {
  deps = injectedDeps;
}

export function initAutoPilotUi() {
  if (!deps) {
    throw new Error('AutoPilotUi dependencies not configured before init');
  }

  const {
    autoPilotRunner,
    getGuests,
    updateGuestBanner,
    openInspectorSection,
    setStatus,
  } = deps;

  const autoPilotBtn = document.getElementById('autopilot-btn');
  const autoPilotPanel = document.getElementById('autopilot-panel');
  const apHideBtn = document.getElementById('ap-hide-btn');
  const apMissionSelect = document.getElementById('ap-mission-select');
  const apPacingSelect = document.getElementById('ap-pacing-select');
  const apResSelect = document.getElementById('ap-res-select');
  const apRunBtn = document.getElementById('ap-run-btn');
  const apPauseBtn = document.getElementById('ap-pause-btn');
  const apResumeBtn = document.getElementById('ap-resume-btn');
  const apCancelBtn = document.getElementById('ap-cancel-btn');
  const apRunnerState = document.getElementById('ap-runner-state');
  const apStatusBadge = document.getElementById('ap-status-badge');
  const apStepLabel = document.getElementById('ap-step-label');
  const apTimerLabel = document.getElementById('ap-timer-label');
  const apProgressBar = document.getElementById('ap-progress-bar');
  const apLogList = document.getElementById('ap-log-list');
  const apGalleryContainer = document.getElementById('ap-gallery-container');
  const apGalleryTray = document.getElementById('ap-gallery-tray');
  const apSnapCount = document.getElementById('ap-snap-count');

  let apTimerInterval = null;
  let apMissionStartTime = 0;

  function updateAutoPilotControls(state) {
    if (apRunnerState) apRunnerState.textContent = state;
    if (apStatusBadge) {
      apStatusBadge.style.display = 'inline-block';
      apStatusBadge.textContent = state.toUpperCase();
      if (state === 'running') {
        apStatusBadge.style.borderColor = 'var(--accent)';
        apStatusBadge.style.color = 'var(--accent)';
      } else if (state === 'error' || state === 'failed') {
        apStatusBadge.style.borderColor = '#f87171';
        apStatusBadge.style.color = '#f87171';
      } else if (state === 'completed') {
        apStatusBadge.style.borderColor = 'var(--ok)';
        apStatusBadge.style.color = 'var(--ok)';
      } else {
        apStatusBadge.style.borderColor = 'var(--dim)';
        apStatusBadge.style.color = 'var(--dim)';
      }
    }

    if (apRunBtn) apRunBtn.disabled = state === 'running';
    if (apPauseBtn) apPauseBtn.disabled = state !== 'running';
    if (apResumeBtn) apResumeBtn.disabled = state !== 'paused';
    if (apCancelBtn) apCancelBtn.disabled = state !== 'running' && state !== 'paused';
  }

  autoPilotRunner.onLog = ({ msg, level }) => {
    if (!apLogList) return;
    const li = document.createElement('li');
    if (level === 'error') li.className = 'error';
    li.textContent = msg;
    apLogList.appendChild(li);
    apLogList.scrollTop = apLogList.scrollHeight;
  };

  autoPilotRunner.onStepChange = ({ stepIdx, totalSteps, step }) => {
    const current = stepIdx + 1;
    const pct = Math.round((stepIdx / totalSteps) * 100);
    if (apProgressBar) apProgressBar.style.width = `${pct}%`;
    if (apStepLabel) {
      apStepLabel.textContent = `[Step ${current}/${totalSteps}] ${step.desc || step.action}`;
    }
    const guests = getGuests();
    for (const guest of guests) {
      updateGuestBanner(guest);
    }
  };

  autoPilotRunner.onReportComplete = (report) => {
    if (apTimerInterval) {
      clearInterval(apTimerInterval);
      apTimerInterval = null;
    }
    if (apProgressBar) {
      apProgressBar.style.width = report.status === 'passed' ? '100%' : apProgressBar.style.width;
    }
    if (apTimerLabel) {
      apTimerLabel.textContent = `${report.durationMs}ms`;
    }
    updateAutoPilotControls(report.status === 'passed' ? 'completed' : 'error');
    const guests = getGuests();
    for (const guest of guests) {
      updateGuestBanner(guest);
    }

    // Render snapshots in Auto Pilot gallery tray
    if (apGalleryContainer && apGalleryTray && report.snapshots && report.snapshots.length > 0) {
      apGalleryContainer.style.display = 'block';
      apGalleryTray.replaceChildren();
      if (apSnapCount) apSnapCount.textContent = report.snapshots.length;
      for (const snap of report.snapshots) {
        const card = document.createElement('div');
        card.className = 'ap-gallery-card';
        card.innerHTML = `
          <img src="${snap.dataUrl}" alt="${snap.label}">
          <div style="font-weight:600; color:var(--text);">${snap.label}</div>
          <div style="color:var(--dimmer);">${snap.hashHex} · seq #${snap.seq}</div>
        `;
        apGalleryTray.appendChild(card);
      }
    }
  };

  if (autoPilotBtn && autoPilotPanel) {
    autoPilotBtn.addEventListener('click', () => {
      if (autoPilotPanel.hidden) {
        openInspectorSection(autoPilotPanel);
      } else {
        autoPilotPanel.hidden = true;
      }
    });
  }

  if (apHideBtn && autoPilotPanel) {
    apHideBtn.addEventListener('click', () => {
      autoPilotPanel.hidden = true;
    });
  }

  function runSelectedMission(missionIdOverride, customOptions = {}) {
    const missionId = missionIdOverride || (apMissionSelect ? apMissionSelect.value : 'system-audit');
    const pacingMs = customOptions.pacingMs != null ? customOptions.pacingMs : (apPacingSelect ? parseInt(apPacingSelect.value, 10) : 400);
    let resolution = customOptions.resolution || null;
    if (!resolution && apResSelect && apResSelect.value !== 'default') {
      const parts = apResSelect.value.split('x').map((n) => parseInt(n, 10));
      if (parts.length === 2 && parts[0] > 0 && parts[1] > 0) {
        resolution = { width: parts[0], height: parts[1] };
      }
    }

    if (apLogList) {
      apLogList.replaceChildren();
    }
    if (apGalleryTray) {
      apGalleryTray.replaceChildren();
    }
    if (apGalleryContainer) {
      apGalleryContainer.style.display = 'none';
    }
    if (apProgressBar) {
      apProgressBar.style.width = '0%';
    }

    updateAutoPilotControls('running');
    const guests = getGuests();
    for (const guest of guests) {
      updateGuestBanner(guest);
    }
    apMissionStartTime = performance.now();
    if (apTimerInterval) clearInterval(apTimerInterval);
    apTimerInterval = setInterval(() => {
      if (apTimerLabel) {
        apTimerLabel.textContent = `${Math.round(performance.now() - apMissionStartTime)}ms`;
      }
    }, 100);

    return autoPilotRunner.runMission(missionId, { pacingMs, resolution }).catch((err) => {
      console.error('Auto Pilot mission failed:', err);
      updateAutoPilotControls('error');
      if (setStatus) setStatus(`Mission failed: ${err.message}`, false);
      throw err;
    });
  }

  if (apRunBtn) {
    apRunBtn.addEventListener('click', () => {
      runSelectedMission();
    });
  }

  // Warm the missions cache and sync dynamically
  loadMissions().then((missions) => {
    if (apMissionSelect && missions && missions.length > 0) {
      const currentVal = apMissionSelect.value;
      apMissionSelect.replaceChildren();
      for (const m of missions) {
        const opt = document.createElement('option');
        opt.value = m.id;
        opt.textContent = m.name;
        if (m.id === currentVal) opt.selected = true;
        apMissionSelect.appendChild(opt);
      }
    }
  }).catch((err) => console.warn('Could not warm missions cache:', err));

  if (apPauseBtn) {
    apPauseBtn.addEventListener('click', () => {
      autoPilotRunner.pause();
      updateAutoPilotControls('paused');
    });
  }

  if (apResumeBtn) {
    apResumeBtn.addEventListener('click', () => {
      autoPilotRunner.resume();
      updateAutoPilotControls('running');
      const guests = getGuests();
      for (const guest of guests) {
        updateGuestBanner(guest);
      }
    });
  }

  if (apCancelBtn) {
    apCancelBtn.addEventListener('click', () => {
      autoPilotRunner.cancel();
      if (apTimerInterval) {
        clearInterval(apTimerInterval);
        apTimerInterval = null;
      }
      updateAutoPilotControls('cancelled');
    });
  }

  return {
    runSelectedMission,
    updateAutoPilotControls,
  };
}
