/* ==========================================================================
   Scenario & Mission Launchpad Component
   Renders rich scenario choices with latest missions first (Mission 9, 8...)
   plus single appliance deployment options.
   ========================================================================== */

import { loadMissions } from '../missions.js';

let deps = null;

export function configureLaunchpadDeps(injectedDeps) {
  deps = injectedDeps;
}

export async function renderLaunchpad(container) {
  if (!deps) throw new Error('Launchpad deps not configured');
  const { onLaunchMission, onLaunchAppliance } = deps;

  container.innerHTML = '';
  const wrap = document.createElement('div');
  wrap.className = 'launchpad-container';
  wrap.id = 'launchpad-root';

  // Hero Section
  const hero = document.createElement('div');
  hero.className = 'launchpad-hero';
  hero.innerHTML = `
    <div class="launchpad-hero-title">
      <span>⚡ Test Appliance & Scenario Launchpad</span>
      <span class="badge">Hypervisor Ready</span>
    </div>
    <div class="launchpad-hero-desc">
      Choose an automated multi-VM scenario, multiplayer exhibition, or standalone appliance session.
      All workloads execute directly in-browser on the WebAssembly hypervisor with zero backend required.
    </div>
  `;
  wrap.appendChild(hero);

  // Section 1: Automated Scenarios & Missions (Latest first)
  const missionsSec = document.createElement('div');
  missionsSec.innerHTML = `<div class="launchpad-section-title"><span>🚀 Automated Scenarios & Missions (Latest First)</span></div>`;
  const missionsGrid = document.createElement('div');
  missionsGrid.className = 'launchpad-grid';

  const missions = await loadMissions();
  // Reverse missions so latest (Mission 9, 8, 7...) appear first
  const reversedMissions = [...missions].reverse();

  reversedMissions.forEach((m, idx) => {
    const card = document.createElement('div');
    card.className = `launchpad-card ${idx === 0 ? 'featured' : ''}`;
    
    // Tag derivation
    const isMultiVm = m.steps?.some(s => s.action === 'boot' && s.targetVm && s.targetVm.includes('display'));
    const isDuel = m.id.includes('duel') || m.id.includes('match') || m.id.includes('relay') || m.id.includes('cockpit');
    
    card.innerHTML = `
      <div class="launchpad-card-header">
        <h3 class="launchpad-card-title">${m.name}</h3>
      </div>
      <p class="launchpad-card-desc">${m.description || 'Automated orchestrated flight plan.'}</p>
      <div class="launchpad-card-footer">
        <div class="launchpad-pills">
          <span class="launchpad-pill highlight">${m.steps?.length || 0} steps</span>
          ${isMultiVm ? '<span class="launchpad-pill">Multi-VM</span>' : '<span class="launchpad-pill">Single VM</span>'}
          ${isDuel ? '<span class="launchpad-pill">2P/4P Broker</span>' : ''}
        </div>
        <button class="launchpad-card-btn" type="button">Launch Mission ▶</button>
      </div>
    `;

    card.addEventListener('click', () => {
      onLaunchMission(m.id);
    });

    missionsGrid.appendChild(card);
  });
  missionsSec.appendChild(missionsGrid);
  wrap.appendChild(missionsSec);

  // Section 2: Standalone Appliances
  const applianceSec = document.createElement('div');
  applianceSec.innerHTML = `<div class="launchpad-section-title"><span>🎮 Standalone Guest Appliances</span></div>`;
  const applianceGrid = document.createElement('div');
  applianceGrid.className = 'launchpad-grid';

  const appliances = [
    {
      id: 'app-checkers',
      title: '♟️ English Draughts (Checkers) + AI',
      desc: 'Interactive 8×8 Checkers with Minimax AI opponent, piece capturing, and king promotion.',
      pills: ['Build #3', 'Minimax AI', 'Single Player'],
      action: () => onLaunchAppliance('checkers'),
    },
    {
      id: 'app-chess',
      title: '♟️ Classic Chess + AI',
      desc: 'Interactive 8×8 Chess with piece move validation and Minimax AI engine.',
      pills: ['Build #3', 'Minimax AI', 'Single Player'],
      action: () => onLaunchAppliance('chess'),
    },
    {
      id: 'app-mines',
      title: '💣 Minesweeper 2-Player Battle',
      desc: 'Turn-based Minesweeper board with scoring, strike mitigation, and broker relay.',
      pills: ['Build #3', 'Turn Relay', '2-Player'],
      action: () => onLaunchAppliance('mines'),
    },
    {
      id: 'app-desktop-720p',
      title: '🖥️ Clean Workstation Guest (720p HD)',
      desc: 'Standard 1280×720 desktop framebuffer with filesystem mounts, wallpapers, and samples.',
      pills: ['1280×720', 'Build #3', 'Workspace'],
      action: () => onLaunchAppliance('desktop', { width: 1280, height: 720 }),
    },
    {
      id: 'app-desktop-mobile',
      title: '📱 Mobile Viewport Guest (390×844)',
      desc: 'Mobile portrait resolution guest suitable for side-by-side split view comparison.',
      pills: ['390×844', 'Portrait', 'Build #3'],
      action: () => onLaunchAppliance('desktop', { width: 390, height: 844 }),
    },
  ];

  appliances.forEach((app) => {
    const card = document.createElement('div');
    card.className = 'launchpad-card';
    card.innerHTML = `
      <div class="launchpad-card-header">
        <h3 class="launchpad-card-title">${app.title}</h3>
      </div>
      <p class="launchpad-card-desc">${app.desc}</p>
      <div class="launchpad-card-footer">
        <div class="launchpad-pills">
          ${app.pills.map(p => `<span class="launchpad-pill">${p}</span>`).join('')}
        </div>
        <button class="launchpad-card-btn" type="button">Deploy VM +</button>
      </div>
    `;

    card.addEventListener('click', () => {
      app.action();
    });

    applianceGrid.appendChild(card);
  });

  applianceSec.appendChild(applianceGrid);
  wrap.appendChild(applianceSec);

  container.appendChild(wrap);
}

export function hideLaunchpad(container) {
  const root = container.querySelector('#launchpad-root');
  if (root) {
    root.remove();
  }
}
