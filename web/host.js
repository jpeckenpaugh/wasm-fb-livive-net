/* v3 Host Controller: Immutable Build Registry + Side-by-Side Split View +
 * Zero-Setup In-Browser FastAPI / SQLite Turn-Based Match Broker.
 */

import { FastApiSqliteBroker, updateRoomLink, initNetworkCopyHandler } from './broker.js';
import { syncPlayerAvatarsToGuest } from './services/avatar.js';
import {
  configureFullscreenDeps,
  enterFullscreen,
  exitFullscreen,
  toggleFullscreen,
  updateFullscreenHud,
  showFullscreenHud,
  initFullscreenListeners,
  isFullscreen,
  getFullscreenGuest,
} from './fullscreen.js';
import {
  configureCheckersDeps,
  serviceCheckers,
  serviceCheckersAi,
  serviceCheckersMultiplayer,
  launchCheckersInGuest,
  startCheckersMatch,
} from './adapters/checkers.js';
import {
  configureChessDeps,
  serviceChessMultiplayer,
  serviceChessAi,
  launchChessInGuest,
  startChessMatch,
} from './adapters/chess.js';
import {
  configureMinesDeps,
  serviceMinesMultiplayer,
  launchMinesInGuest,
  startTwoPlayerMatch,
} from './adapters/mines.js';
import { AutoPilotRunner } from './autopilot.js';
import { MISSIONS, loadMissions } from './missions.js';
import {
  TrajectoryRecorder,
  TrajectoryPlayer,
  exportTrajectoryJson,
  downloadTrajectoryFile,
  importTrajectoryJson,
  trajectoryToAutoPilotMission,
} from './trajectory.js';
import { KEY_CODES, MODS } from './core/input-codes.js';
import { configureAutoPilotUiDeps, initAutoPilotUi } from './services/autopilot-ui.js';
import { configureTrajectoryUiDeps, initTrajectoryUi } from './services/trajectory-ui.js';
import { configureLaunchpadDeps, renderLaunchpad, hideLaunchpad } from './services/launchpad-ui.js';
import defaultMountsData from './data/default-mounts.json' with { type: 'json' };

const DEFAULT_W = 1280;
const DEFAULT_H = 720;
const TITLE_H = 32;
const HASH_EVERY_N_FRAMES = 10;

/* Network mount catalog (`~/wallpapers/*`, `~/samples/*`, and `gh://`) served from `/` */
const DEFAULT_MOUNTS = defaultMountsData;

/* Fallback build registry if `registry.json` hasn't loaded yet */
const DEFAULT_REGISTRY = {
  defaultBuild: 'build-03',
  repository: 'https://github.com/jpeckenpaugh/wasm-fb-livive-net',
  builds: [
    {
      id: 'build-03',
      label: 'Build #3 · v3.2 (Chess + Checkers + 2P Broker)',
      shortLabel: 'build-03 · v3.2',
      version: 'v3.2',
      wasmUrl: './build/appliance.wasm',
      gitCommit: 'HEAD',
      githubUrl: 'https://github.com/jpeckenpaugh/wasm-fb-livive-net/commits/main',
      minesBuild: 'Mines Build #2 (2P Turn-Based + Scoring)',
      checkersBuild: 'Checkers Build #1 (English Draughts + Minimax AI + 2P Broker)',
      chessBuild: 'Chess Build #1 (Classic Chess + Minimax AI + 2P Broker)',
      supportsMultiplayer: true,
      supportsCheckers: true,
      supportsChess: true,
      notes: 'Adds Classic Chess + English Draughts (Checkers) + Minimax AI + 2P Broker + chs-livive-net & chk-livive-net mounts',
    },
    {
      id: 'build-02',
      label: 'Build #2 · v3.0 (aea4b4b · 2P Mines + Registry)',
      shortLabel: 'build-02 · aea4b4b',
      version: 'v3.0',
      wasmUrl: './builds/appliance-v3.0-aea4b4b.wasm',
      gitCommit: 'aea4b4b',
      githubUrl: 'https://github.com/jpeckenpaugh/wasm-fb-livive-net/commit/aea4b4b',
      minesBuild: 'Mines Build #2 (2P Turn-Based + Scoring)',
      supportsMultiplayer: true,
      supportsCheckers: false,
      notes: 'Frozen v3.0 baseline with 2-player turn-based Minesweeper wrapper, Wasm MP mailbox ABI, and v3 registry',
    },
    {
      id: 'build-01',
      label: 'Build #1 · v2.0 (7ac8a67 · 1P Baseline)',
      shortLabel: 'build-01 · 7ac8a67',
      version: 'v2.0',
      wasmUrl: './builds/appliance-v2-7ac8a67.wasm',
      gitCommit: '7ac8a67',
      githubUrl: 'https://github.com/jpeckenpaugh/wasm-fb-livive-net/commit/7ac8a67',
      minesBuild: 'Mines Build #1 (1P Bitboard)',
      supportsMultiplayer: false,
      supportsCheckers: false,
      notes: 'Frozen v2 baseline with single-player Minesweeper, right-pane Files activation, and simplified menus',
    },
  ],
};

let buildRegistry = DEFAULT_REGISTRY;
const wasmModuleCache = new Map();

async function loadRegistry() {
  try {
    const res = await fetch(new URL(`./registry.json?v=${Date.now()}`, import.meta.url), { cache: 'no-store' });
    if (res.ok) {
      const data = await res.json();
      if (data && Array.isArray(data.builds) && data.builds.length) {
        buildRegistry = data;
      }
    }
  } catch (err) {
    console.warn('Using fallback build registry:', err);
  }
  populateBuildSelect();
  return buildRegistry;
}

function getBuildInfo(buildId) {
  return (
    buildRegistry.builds.find((b) => b.id === buildId) ||
    buildRegistry.builds[0] ||
    DEFAULT_REGISTRY.builds[0]
  );
}

const broker = new FastApiSqliteBroker({
  getGuests: () => guests,
  updateHostStats: () => updateHostStats(),
  updateGuestBanner: (g) => updateGuestBanner(g),
  fmtHex32: (n) => fmtHex32(n),
});
window.__broker = broker;

configureFullscreenDeps({
  getActiveGuest: () => guests.find((candidate) => candidate.id === activeId) || guests[0],
  setActiveId: (id) => { activeId = id; },
  setStatus: (msg, ok) => setStatus(msg, ok),
  selectionLabel: (guest) => selectionLabel(guest),
});

configureCheckersDeps({
  getBroker: () => broker,
  getGuests: () => guests,
  createGuest: (opts) => createGuest(opts),
  setSplitView: (on) => setSplitView(on),
  activate: (id) => activate(id),
  setStatus: (msg, ok) => setStatus(msg, ok),
  fmtHex32: (n) => fmtHex32(n),
});

configureChessDeps({
  getBroker: () => broker,
  getGuests: () => guests,
  createGuest: (opts) => createGuest(opts),
  setSplitView: (on) => setSplitView(on),
  activate: (id) => activate(id),
  setStatus: (msg, ok) => setStatus(msg, ok),
  fmtHex32: (n) => fmtHex32(n),
});

configureMinesDeps({
  getBroker: () => broker,
  getGuests: () => guests,
  createGuest: (opts) => createGuest(opts),
  setSplitView: (on) => setSplitView(on),
  activate: (id) => activate(id),
  setStatus: (msg, ok) => setStatus(msg, ok),
  getDefaultBuild: () => buildRegistry.defaultBuild || 'build-03',
});

const GH_REPOS = new Set(
  DEFAULT_MOUNTS
    .map((item) => item.name)
    .filter((name) => name.startsWith('gh/'))
    .map((name) => name.split('/').slice(1, 3).join('/')),
);

const decodedRgbaCache = new Map();
const fetchedTextCache = new Map();
const contextXmlCache = new Map();
const textEncoder = new TextEncoder();
const WEB_TEXT_CAP = 768 * 1024;

let webConfigPromise = null;
const webCaptureCache = new Map();
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function loadWebConfig() {
  if (!webConfigPromise) {
    webConfigPromise = fetch(new URL('./sites.json', import.meta.url), { cache: 'no-store' })
      .then((res) => (res.ok ? res.json() : null))
      .catch(() => null);
  }
  return webConfigPromise;
}

function oneLine(value) {
  return String(value == null ? '' : value).replace(/[\r\n]+/g, ' ').trim();
}

function webMetaText(state, detail, sites = [], opts = {}) {
  const ready = opts.ready ?? 0;
  const total = sites.length;
  const endpoint = opts.endpoint || 'http://127.0.0.1:8787/api/captures';
  const siteIds = sites.map((s) => s.id).filter(Boolean).join(',');
  const lines = [
    `STATE=${state}`,
    `SITES=${total}`,
    `READY=${ready}/${total}`,
    `ENDPOINT=${oneLine(endpoint)}`,
    `SITES_LIST=${siteIds}`,
    `DETAIL=${oneLine(detail)}`,
  ];
  return `${lines.join('\n')}\n`;
}

function writeWebMeta(guest, text) {
  if (!guest?.exports?.app_web_meta_ptr || !guest.exports.app_web_commit_meta) return;
  const bytes = textEncoder.encode(text).subarray(0, 512);
  const ptr = guest.exports.app_web_meta_ptr();
  new Uint8Array(guest.exports.memory.buffer).set(bytes, ptr);
  guest.exports.app_web_commit_meta(bytes.byteLength);
  if (guest.render) {
    guest.render();
  } else if (guest.blitToCanvas) {
    const t = performance.now();
    guest.blitToCanvas(t, t);
  }
}

async function decodeBlobToRgba(blob) {
  const bitmap = await createImageBitmap(blob);
  const canvas = document.createElement('canvas');
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const ctx = canvas.getContext('2d', { alpha: false });
  ctx.drawImage(bitmap, 0, 0);
  bitmap.close();
  return {
    width: canvas.width,
    height: canvas.height,
    rgba: ctx.getImageData(0, 0, canvas.width, canvas.height).data,
  };
}

async function captureSite(site, width, height, opts = {}) {
  const cfg = await loadWebConfig();
  const viewport = { width, height: Math.max(1, height - TITLE_H), dpr: 1 };
  const key = `${site.id}@${viewport.width}x${viewport.height}`;
  const maxAge = (site.maxAgeMinutes || 15) * 60_000;
  const hit = webCaptureCache.get(key);
  if (!opts.force && hit?.snapshot && Date.now() - hit.at < maxAge) return hit.snapshot;
  if (hit?.promise) return hit.promise;

  const promise = (async () => {
    // 1. Try pre-baked static snapshot first if not forcing a refresh
    if (!opts.force) {
      try {
        const bakedRes = await fetch(`./web/${site.id}/snapshot.json`, { cache: 'default' });
        if (bakedRes.ok) {
          const snapshot = await bakedRes.json();
          snapshot.at = Date.now();
          webCaptureCache.set(key, { snapshot, at: Date.now() });
          return snapshot;
        }
      } catch {
        /* Fall back to live capture endpoint if static snapshot fails */
      }
    }

    // 2. Fall back to live capture service endpoint (e.g. Docker container or local server)
    if (!cfg || !cfg.captureEndpoint) throw new Error('capture service is not configured');

    const res = await fetch(cfg.captureEndpoint, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        url: site.url,
        site: { id: site.id, label: site.label || site.id },
        viewport,
        options: {
          colorScheme: 'dark',
          reducedMotion: true,
          locale: navigator.language || 'en-US',
          timezone: Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC',
        },
      }),
      signal: AbortSignal.timeout ? AbortSignal.timeout(4000) : undefined,
    });
    if (!res.ok) throw new Error(`capture request failed: HTTP ${res.status}`);
    const job = await res.json();
    if (!job.statusUrl) throw new Error('capture service did not return a status URL');
    const statusUrl = new URL(job.statusUrl, cfg.captureEndpoint).href;
    const deadline = Date.now() + 30_000;
    while (Date.now() < deadline) {
      const statusRes = await fetch(statusUrl, { cache: 'no-store' });
      const status = await statusRes.json();
      if (status.state === 'error') throw new Error(status.error || 'capture failed');
      if (status.state === 'ready') {
        const snapshot = {
          manifest: status.manifest || {},
          context: status.context || '',
          artifacts: status.artifacts || {},
          at: Date.now(),
        };
        const textMeta = snapshot.artifacts['text.txt'];
        if (textMeta?.url) {
          try {
            const textRes = await fetch(textMeta.url);
            if (textRes.ok) snapshot.text = (await textRes.text()).slice(0, 12000);
          } catch {
            /* TTS is optional. */
          }
        }
        webCaptureCache.set(key, { snapshot, at: Date.now() });
        return snapshot;
      }
      await sleep(300);
    }
    throw new Error('capture timed out');
  })();

  webCaptureCache.set(key, { promise, at: 0 });
  try {
    return await promise;
  } catch (error) {
    webCaptureCache.delete(key);
    throw error;
  }
}

function mountWebArtifacts(guest, site, snapshot) {
  const { exports } = guest;
  if (!exports || !exports.app_mount_add) return;
  guest.webMounts = guest.webMounts || new Map();
  const order = [
    'index.html',
    'rendered.html',
    'page.png',
    'text.txt',
    'headers.txt',
    'console.txt',
    'network.json',
    'capture.json',
  ];
  for (const path of order) {
    const meta = snapshot.artifacts[path];
    if (!meta) continue;
    const mountKey = `${site.id}/${path}`;
    const existing = guest.webMounts.get(mountKey);
    if (existing != null) {
      exports.app_mount_update_size(existing, meta.bytes || 0);
      const item = guest.mounts[existing];
      if (item) {
        item.bytes = meta.bytes || 0;
        item.url = meta.url;
        item.context = snapshot.context;
        item.snapshot = snapshot;
      }
      continue;
    }
    const name = `web/${site.id}/${path}`;
    const encoded = textEncoder.encode(name).subarray(0, 64);
    const namePtr = exports.app_mount_name_buf();
    new Uint8Array(exports.memory.buffer).set(encoded, namePtr);
    const idx = exports.app_mount_add(encoded.length, meta.bytes || 0);
    if (idx < 0) continue;
    guest.webMounts.set(mountKey, idx);
    guest.mounts.push({
      name,
      source: 'web',
      siteId: site.id,
      artifact: path,
      bytes: meta.bytes || 0,
      kind: path.endsWith('.png') ? 'image' : 'code',
      url: meta.url,
      context: snapshot.context,
      snapshot,
    });
  }
}

async function stageWebArtifact(guest, idx, item) {
  const { exports } = guest;
  if (!exports) return;
  const truncated = item.kind === 'image' ? false : (item.bytes || 0) > WEB_TEXT_CAP;
  const contextText = `${item.context || ''}\nARTIFACT=${item.artifact} (${item.bytes || 0} B)\nBYTES=${item.bytes || 0}\nTRUNCATED=${truncated}\n`;
  const contextBytes = textEncoder.encode(contextText).subarray(0, 4096);
  const contextPtr = exports.app_mount_context_ptr();
  new Uint8Array(exports.memory.buffer).set(contextBytes, contextPtr);
  exports.app_mount_commit_context(idx, contextBytes.byteLength);

  const res = await fetch(item.url);
  if (!res.ok) throw new Error(`artifact ${item.artifact}: HTTP ${res.status}`);

  if (item.kind === 'image') {
    const decoded = await decodeBlobToRgba(await res.blob());
    const maxBytes = guest.width * guest.height * 4;
    if (decoded.rgba.byteLength > maxBytes) throw new Error('screenshot exceeds staging buffer');
    const stagingPtr = exports.app_mount_staging_ptr();
    new Uint8Array(exports.memory.buffer).set(decoded.rgba, stagingPtr);
    const t0 = performance.now();
    exports.app_mount_commit_rgba(idx, decoded.width, decoded.height);
    const t1 = performance.now();
    guest.blitToCanvas(t0, t1);
  } else {
    const bytes = new Uint8Array(await res.arrayBuffer()).subarray(0, WEB_TEXT_CAP);
    const textPtr = exports.app_mount_text_ptr();
    new Uint8Array(exports.memory.buffer).set(bytes, textPtr);
    const t0 = performance.now();
    exports.app_mount_commit_text(idx, bytes.byteLength);
    const t1 = performance.now();
    guest.blitToCanvas(t0, t1);
  }
}

async function initWebForGuest(guest) {
  try {
    const cfg = await loadWebConfig();
    if (!cfg || !cfg.captureEndpoint || !Array.isArray(cfg.sites) || !cfg.sites.length) {
      if (!guest.disposed && guest.exports) {
        writeWebMeta(guest, webMetaText('unconfigured', 'No capture endpoint or sites configured', [], { ready: 0, endpoint: 'unconfigured' }));
      }
      return;
    }
    if (guest.disposed || !guest.exports) return;
    guest.webConfig = cfg;
    const endpoint = cfg.captureEndpoint;
    const sites = (guest.webSiteId
      ? cfg.sites.filter((candidate) => candidate.id === guest.webSiteId)
      : cfg.sites
    ).slice(0, 12);
    if (!sites.length) return;
    guest.webSites = sites;
    guest.webSnapshots = new Map();
    writeWebMeta(guest, webMetaText('capturing', `Capturing ${sites.length} web sites...`, sites, { ready: 0, endpoint }));
    const results = await Promise.all(
      sites.map(async (site) => {
        try {
          return { site, snapshot: await captureSite(site, guest.width, guest.height) };
        } catch (error) {
          return { site, error };
        }
      }),
    );
    let ready = 0;
    let failed = 0;
    for (const { site, snapshot, error } of results) {
      if (guest.disposed || !guest.exports) return;
      if (snapshot) {
        guest.webSnapshots.set(site.id, snapshot);
        mountWebArtifacts(guest, site, snapshot);
        ready += 1;
      } else {
        failed += 1;
      }
      writeWebMeta(
        guest,
        webMetaText(
          'capturing',
          `${ready}/${sites.length} web sites ready${error ? ` · ${site.id} failed` : ''}`,
          sites,
          { ready, endpoint },
        ),
      );
    }
    if (guest.render) guest.render();
    const state = failed === 0 ? 'ready' : ready > 0 ? 'partial' : 'failed';
    const detail = state === 'failed'
      ? `0/${sites.length} captured · capture service unreachable at ${endpoint}`
      : `${ready}/${sites.length} web sites captured`;
    writeWebMeta(
      guest,
      webMetaText(state, detail, sites, { ready, endpoint }),
    );
    if (guest.render) guest.render();
    if (guest.exports.app_mount_pending_idx && guest.exports.app_mount_pending_idx() >= 0) {
      servicePendingMountFetch(guest).catch((error) => console.error(error));
    }
  } catch (error) {
    if (!guest.disposed && guest.exports) {
      const endpoint = guest.webConfig?.captureEndpoint || 'http://127.0.0.1:8787/api/captures';
      writeWebMeta(guest, webMetaText('failed', error.message || error, guest.webSites || [], { ready: 0, endpoint }));
    }
  }
}

async function refreshWebForGuest(guest) {
  const sites = guest.webSites?.length ? guest.webSites : guest.webConfig?.sites || [];
  if (!sites.length) {
    await initWebForGuest(guest);
    return;
  }
  const endpoint = guest.webConfig?.captureEndpoint || 'http://127.0.0.1:8787/api/captures';
  writeWebMeta(guest, webMetaText('capturing', `Refreshing ${sites.length} web sites...`, sites, { ready: 0, endpoint }));
  const results = await Promise.all(
    sites.map(async (site) => {
      try {
        return {
          site,
          snapshot: await captureSite(site, guest.width, guest.height, { force: true }),
        };
      } catch (error) {
        return { site, error };
      }
    }),
  );
  let ready = 0;
  let failed = 0;
  for (const { site, snapshot } of results) {
    if (guest.disposed || !guest.exports) return;
    if (snapshot) {
      guest.webSnapshots.set(site.id, snapshot);
      mountWebArtifacts(guest, site, snapshot);
      ready += 1;
    } else {
      failed += 1;
    }
    writeWebMeta(
      guest,
      webMetaText('capturing', `${ready}/${sites.length} web sites refreshed`, sites, { ready, endpoint }),
    );
  }
  const state = failed === 0 ? 'ready' : ready > 0 ? 'partial' : guest.webSnapshots.size ? 'stale' : 'failed';
  const detail = state === 'failed'
    ? `0/${sites.length} captured · capture service unreachable at ${endpoint}`
    : `${ready}/${sites.length} web sites refreshed`;
  writeWebMeta(guest, webMetaText(state, detail, sites, { ready, endpoint }));
  const selected = guest.exports.app_active_page ? guest.exports.app_active_page() : -1;
  if (selected >= 0 && guest.mounts[selected]?.source === 'web') {
    await stageWebArtifact(guest, selected, guest.mounts[selected]);
  } else if (guest.render) {
    guest.render();
  }
  if (guest.exports.app_mount_pending_idx && guest.exports.app_mount_pending_idx() >= 0) {
    servicePendingMountFetch(guest).catch((error) => console.error(error));
  }
}

function safeExternalUrl(value) {
  try {
    const url = new URL(value);
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return null;
    return url.href;
  } catch {
    return null;
  }
}

function resolveOpenTarget(guest, target) {
  if (!target) return null;
  if (target.startsWith('web/')) {
    const siteId = target.slice(4).split('/')[0];
    if (!siteId) return null;
    const site =
      guest.webSites?.find((candidate) => candidate.id === siteId) ||
      guest.webConfig?.sites?.find((candidate) => candidate.id === siteId);
    if (!site) return null;
    const snapshot = guest.webSnapshots?.get(siteId);
    return safeExternalUrl(site.url || snapshot?.manifest?.result?.finalUrl);
  }
  if (target.startsWith('gh/')) {
    const parts = target.slice(3).split('/').filter(Boolean);
    if (parts.length < 2) return null;
    const [owner, repo, ...path] = parts;
    if (!GH_REPOS.has(`${owner}/${repo}`)) return null;
    if (path.some((part) => part === '.' || part === '..')) return null;
    const base = `https://github.com/${owner}/${repo}`;
    if (!path.length) return base;
    return `${base}/blob/HEAD/${path.map(encodeURIComponent).join('/')}`;
  }
  return null;
}

function openExternal(guest) {
  if (!guest?.exports?.app_open_target_ptr || !guest.exports.app_open_target_len) return;
  const len = guest.exports.app_open_target_len();
  if (!len) {
    setStatus(`${guest.name} · nothing to open`);
    return;
  }
  const ptr = guest.exports.app_open_target_ptr();
  const target = new TextDecoder().decode(
    new Uint8Array(guest.exports.memory.buffer, ptr, len),
  );
  const url = resolveOpenTarget(guest, target);
  if (!url) {
    setStatus(`${guest.name} · cannot open ${target}`);
    return;
  }
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.target = '_blank';
  anchor.rel = 'noopener noreferrer';
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  setStatus(`${guest.name} · opened ${url} in a new tab`, true);
}

function serviceWebControl(guest) {
  if (!guest?.exports?.app_web_take_refresh) return;
  if (guest.exports.app_web_take_refresh() !== 0) {
    refreshWebForGuest(guest).catch((error) => console.error(error));
  }
}

const KiB = 1024;
const MiB = 1024 * KiB;

const tabsEl = document.getElementById('tabs');
const panelsEl = document.getElementById('panels');
const buildSelect = document.getElementById('vm-build');
const resSelect = document.getElementById('vm-res');
const newVmButton = document.getElementById('new-vm');
const chsMatchBtn = document.getElementById('chs-match-btn');
const chkMatchBtn = document.getElementById('chk-match-btn');
const mpMatchBtn = document.getElementById('mp-match-btn');
const abBtn = document.getElementById('ab-btn');
const splitBtn = document.getElementById('split-btn');
const step1Button = document.getElementById('step1-btn');
const snapButton = document.getElementById('snap-btn');
const patternToggle = document.getElementById('pattern');
const statusLeft = document.getElementById('sb-left');
const statusRes = document.getElementById('sb-res');
const statusCopies = document.getElementById('sb-copies');
const hostBuilds = document.getElementById('h-builds');
const hostModule = document.getElementById('h-module');
const hostGuests = document.getElementById('h-guests');
const hostMatches = document.getElementById('h-matches');
const hostMem = document.getElementById('h-mem');
const hostBlits = document.getElementById('h-frames');
const hostSnaps = document.getElementById('h-snaps');
const hostUptime = document.getElementById('h-uptime');
const hostStatsPill = document.getElementById('host-stats-pill');
const hostStatsDrawer = document.getElementById('host-stats-drawer');
const hostGuestsPill = document.getElementById('h-guests-pill');
const hostUptimePill = document.getElementById('h-uptime-pill');
const toggleSidebarBtn = document.getElementById('toggle-sidebar-btn');
const toggleInspectorBtn = document.getElementById('toggle-inspector-btn');
const closeInspectorBtn = document.getElementById('close-inspector-btn');
const inspectorPane = document.getElementById('inspector-pane');
const sidebarEl = document.getElementById('sidebar');
const peekTabSidebar = document.getElementById('peek-tab-sidebar');
const peekTabInspector = document.getElementById('peek-tab-inspector');
const pinSidebarBtn = document.getElementById('pin-sidebar-btn');
const pinInspectorBtn = document.getElementById('pin-inspector-btn');
const launchpadNavBtn = document.getElementById('launchpad-nav-btn');
const brokerDrawer = document.getElementById('broker-drawer');
const brokerResetBtn = document.getElementById('broker-reset-btn');
const brokerHideBtn = document.getElementById('broker-hide-btn');
const snapDrawer = document.getElementById('snapshots-drawer');
const snapList = document.getElementById('snap-list');
const snapCountEl = document.getElementById('snap-count');
const clearSnapsBtn = document.getElementById('clear-snaps');

const guests = [];
const snapshots = [];
let activeId = null;
let splitViewEnabled = false;
let nextId = 1;
let totalBlits = 0;
let moduleBytes = 0;
const bootAt = performance.now();
let clockTimer = null;

// Configure Fullscreen deps
configureFullscreenDeps({
  getActiveGuest: () => guests.find((candidate) => candidate.id === activeId) || guests[0],
  setActiveId: (id) => { activeId = id; },
  setStatus: (msg, ok) => setStatus(msg, ok),
  selectionLabel: (guest) => selectionLabel(guest),
});

function populateBuildSelect() {
  if (!buildSelect) return;
  const current = buildSelect.value || buildRegistry.defaultBuild;
  buildSelect.replaceChildren();
  for (const b of buildRegistry.builds) {
    const opt = document.createElement('option');
    opt.value = b.id;
    opt.textContent = b.label;
    if (b.id === current) opt.selected = true;
    buildSelect.append(opt);
  }
  if (hostBuilds) hostBuilds.textContent = String(buildRegistry.builds.length);
}

function setSplitView(enabled) {
  splitViewEnabled = Boolean(enabled);
  panelsEl.classList.toggle('split-view', splitViewEnabled);
  document.body.classList.toggle('wide-console', splitViewEnabled);
  if (splitBtn) splitBtn.classList.toggle('active-toggle', splitViewEnabled);
  for (const guest of guests) {
    const on = guest.id === activeId;
    guest.panel.hidden = splitViewEnabled ? false : !on;
    guest.panel.classList.toggle('active-panel', on);
  }
}

function stopTts(guest) {
  if (typeof window !== 'undefined' && 'speechSynthesis' in window) {
    window.speechSynthesis.cancel();
  }
  if (guest && guest.exports && guest.exports.app_tts_set_active) {
    guest.exports.app_tts_set_active(0);
    guest.render();
  }
}

function handleTtsToggle(guest) {
  if (typeof window === 'undefined' || !('speechSynthesis' in window)) {
    setStatus(`${guest.name} · Web Speech API not supported in this browser`);
    if (guest.exports && guest.exports.app_tts_set_active) {
      guest.exports.app_tts_set_active(0);
      guest.render();
    }
    return;
  }

  const isSpeaking = guest.exports && guest.exports.app_tts_active ? Boolean(guest.exports.app_tts_active()) : false;
  if (!isSpeaking) {
    stopTts(guest);
    return;
  }

  const activePage = guest.exports.app_active_page ? guest.exports.app_active_page() : -1;
  if (activePage < 0 || !guest.mounts || activePage >= guest.mounts.length) {
    stopTts(guest);
    return;
  }

  const item = guest.mounts[activePage];
  const ctxPtr = guest.exports.app_mount_context_ptr ? guest.exports.app_mount_context_ptr() : 0;
  let textToSpeak = '';

  if (item.source === 'web' && item.snapshot?.text) {
    textToSpeak = `Web snapshot of ${item.siteId}. ${item.snapshot.text.slice(0, 8000)}`;
  } else if (ctxPtr) {
    const mem = new Uint8Array(guest.exports.memory.buffer);
    const slice = mem.subarray(ctxPtr, ctxPtr + 4096);
    const zeroIdx = slice.indexOf(0);
    const len = zeroIdx >= 0 ? zeroIdx : 4096;
    const raw = new TextDecoder().decode(slice.subarray(0, len));

    const map = {};
    for (const line of raw.split('\n')) {
      const eq = line.indexOf('=');
      if (eq > 0) {
        map[line.slice(0, eq).trim()] = line.slice(eq + 1).trim();
      }
    }

    const title = item.name.split('/').pop();
    const role = map.ROLE ? `Role: ${map.ROLE}. ` : '';
    const summary = map.SUMMARY ? `Summary: ${map.SUMMARY}. ` : '';
    const arch = map.ARCHITECTURE ? `Architecture: ${map.ARCHITECTURE}` : '';
    textToSpeak = `Context for ${title}. ${role}${summary}${arch}`.trim();
  }

  if (!textToSpeak) {
    textToSpeak = `Viewing file ${item.name}. No technical summary available.`;
  }

  window.speechSynthesis.cancel();
  const utterance = new SpeechSynthesisUtterance(textToSpeak);
  utterance.rate = 1.0;
  utterance.pitch = 1.0;

  utterance.onend = () => {
    if (guest.exports && guest.exports.app_tts_set_active) {
      guest.exports.app_tts_set_active(0);
      guest.render();
    }
  };

  utterance.onerror = () => {
    if (guest.exports && guest.exports.app_tts_set_active) {
      guest.exports.app_tts_set_active(0);
      guest.render();
    }
  };

  window.speechSynthesis.speak(utterance);
  setStatus(`${guest.name} · TTS reading context for ${item.name}…`, true);
}

const fmtBytes = (n) => {
  if (!n) return '0 B';
  if (n >= MiB) return `${(n / MiB).toFixed(2)} MiB`;
  if (n >= KiB) return `${(n / KiB).toFixed(1)} KiB`;
  return `${n} B`;
};
const fmtMs = (n) => `${n.toFixed(2)} ms`;
const fmtHex32 = (n) => `0x${(n >>> 0).toString(16).padStart(8, '0')}`;
const fmtDuration = (ms) => {
  const total = Math.floor(ms / 1000);
  const hh = String(Math.floor(total / 3600)).padStart(2, '0');
  const mm = String(Math.floor((total % 3600) / 60)).padStart(2, '0');
  const ss = String(total % 60).padStart(2, '0');
  return `${hh}:${mm}:${ss}`;
};

const guestRenderMode = (guest) => {
  if (!guest || !guest.exports) return 0;
  return guest.exports.app_render_mode
    ? guest.exports.app_render_mode()
    : guest.exports.app_font_family();
};

const selectionLabel = (guest) => {
  if (!guest || !guest.exports) return 'desktop';
  const sel = guest.exports.app_selected();
  if (sel === 1) {
    const page = guest.exports.app_active_page();
    const sec =
      page === 1
        ? 'Locale'
        : page === 2
          ? 'Fonts'
          : page === 3
            ? 'Icons'
            : page === 4
              ? 'Theme'
              : 'About';
    return `System (${sec})`;
  }
  if (sel === 2) return 'Calc';
  if (sel === 3) return 'Files';
  if (sel === 4) return 'Mines';
  if (sel === 5) return 'Checkers';
  if (sel === 6) return 'Chess';
  return 'Desktop';
};

const MENU_NAMES = ['Home', 'App'];
const focusLabel = (code) => {
  if (code === 0) return 'client';
  if (code >= 200) {
    const idx = Math.floor((code - 200) / 10);
    const item = (code - 200) % 10;
    return `${MENU_NAMES[idx] || 'Menu'} ${item}`;
  }
  if (code >= 100) return MENU_NAMES[code - 100] || 'Menu bar';
  return `focus-${code}`;
};

function loadWasmModuleForBuild(buildInfo) {
  const key = buildInfo.id;
  if (!wasmModuleCache.has(key)) {
    const url = new URL(`${buildInfo.wasmUrl}?v=${Date.now()}`, import.meta.url);
    const promise = fetch(url, { cache: 'no-cache' })
      .then((response) => {
        if (!response.ok) throw new Error(`failed to fetch ${url}: HTTP ${response.status}`);
        return response.arrayBuffer();
      })
      .then(async (bytes) => {
        moduleBytes = bytes.byteLength;
        updateHostStats();
        const mod = await WebAssembly.compile(bytes);
        return { mod, byteLength: bytes.byteLength };
      });
    wasmModuleCache.set(key, promise);
  }
  return wasmModuleCache.get(key);
}

async function fetchAndDecodeRgba(urlStr, targetW, targetH) {
  const key = `${urlStr}@${targetW}x${targetH}`;
  if (decodedRgbaCache.has(key)) {
    return decodedRgbaCache.get(key);
  }
  const url = new URL(urlStr, import.meta.url);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP ${response.status} fetching ${url}`);
  const blob = await response.blob();
  const bitmap = await createImageBitmap(blob);

  const offscreen = document.createElement('canvas');
  offscreen.width = targetW;
  offscreen.height = targetH;
  const ctx = offscreen.getContext('2d', { alpha: false });

  const scale = Math.max(targetW / bitmap.width, targetH / bitmap.height);
  const dw = bitmap.width * scale;
  const dh = bitmap.height * scale;
  const dx = (targetW - dw) / 2;
  const dy = (targetH - dh) / 2;
  ctx.drawImage(bitmap, dx, dy, dw, dh);
  bitmap.close();

  const rgba = ctx.getImageData(0, 0, targetW, targetH).data;
  const result = { width: targetW, height: targetH, rgba };
  decodedRgbaCache.set(key, result);
  return result;
}

async function fetchTextBytes(urlStr) {
  if (fetchedTextCache.has(urlStr)) {
    return fetchedTextCache.get(urlStr);
  }
  const url = new URL(urlStr, import.meta.url);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`HTTP ${response.status} fetching ${url}`);
  const text = await response.text();
  const encoded = textEncoder.encode(text).subarray(0, 16384);
  fetchedTextCache.set(urlStr, encoded);
  return encoded;
}

function mountCatalogIntoGuest(guest, mounts = DEFAULT_MOUNTS) {
  const { exports } = guest;
  if (!exports || !exports.app_mount_clear) return;
  exports.app_mount_clear();
  guest.mounts = mounts.slice();
  guest.webMounts = new Map();
  guest.webSnapshots = new Map();
  guest.webSites = [];

  const namePtr = exports.app_mount_name_buf();
  for (const item of guest.mounts) {
    const encoded = textEncoder.encode(item.name).subarray(0, 64);
    const mem = new Uint8Array(exports.memory.buffer);
    mem.set(encoded, namePtr);
    exports.app_mount_add(encoded.length, item.bytes || 0);
  }
}

async function fetchContextForMount(item) {
  try {
    const lastSlash = item.url.lastIndexOf('/');
    const dirUrl = lastSlash !== -1 ? item.url.slice(0, lastSlash) : '.';
    const contextUrlStr = item.contextUrl || `${dirUrl}/.context.xml`;
    const fileName = item.name.slice(item.name.lastIndexOf('/') + 1);

    let doc = contextXmlCache.get(contextUrlStr);
    if (!doc) {
      const url = new URL(contextUrlStr, import.meta.url);
      const res = await fetch(url);
      if (!res.ok) return null;
      const xmlText = await res.text();
      const parser = new DOMParser();
      doc = parser.parseFromString(xmlText, 'application/xml');
      contextXmlCache.set(contextUrlStr, doc);
    }

    const fileNode = doc.querySelector(`file[name="${CSS.escape(fileName)}"]`) ||
      Array.from(doc.querySelectorAll('file')).find((el) => el.getAttribute('name') === fileName);
    if (!fileNode) return null;

    const root = doc.documentElement;
    const scope = root ? root.getAttribute('scope') || '' : '';
    const ref = root ? root.getAttribute('ref') || '' : '';
    const scopeVal = ref ? `${scope} (${ref})` : scope;
    const generatorVal = root ? root.getAttribute('generator') || '' : '';

    const runtimeVal = fileNode.getAttribute('runtime') || '';
    const roleVal = fileNode.getAttribute('role') || '';

    const graphNode = fileNode.querySelector('graph');
    const getCleanText = (parent, tag) => {
      if (!parent) return '';
      const el = parent.querySelector(tag);
      return el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : '';
    };

    const importsVal = getCleanText(graphNode, 'imports');
    const exportsVal = getCleanText(graphNode, 'exports');
    const dependentsVal = getCleanText(graphNode, 'dependents');
    const invariantsVal = getCleanText(graphNode, 'invariants');

    const summaryVal = getCleanText(fileNode, 'summary');
    const architectureVal = getCleanText(fileNode, 'architecture');

    const lines = [
      scopeVal ? `SCOPE=${scopeVal}` : null,
      generatorVal ? `GENERATOR=${generatorVal}` : null,
      runtimeVal ? `RUNTIME=${runtimeVal}` : null,
      roleVal ? `ROLE=${roleVal}` : null,
      importsVal ? `IMPORTS=${importsVal}` : null,
      exportsVal ? `EXPORTS=${exportsVal}` : null,
      dependentsVal ? `DEPENDENTS=${dependentsVal}` : null,
      invariantsVal ? `INVARIANTS=${invariantsVal}` : null,
      summaryVal ? `SUMMARY=${summaryVal}` : null,
      architectureVal ? `ARCHITECTURE=${architectureVal}` : null,
    ].filter(Boolean);

    const wireText = lines.join('\n') + '\n';
    return textEncoder.encode(wireText).subarray(0, 4096);
  } catch (err) {
    console.warn(`failed to fetch/parse context for ${item.name}:`, err);
    return null;
  }
}

async function servicePendingMountFetch(guest) {
  if (!guest || !guest.exports || !guest.exports.app_mount_pending_idx) return;
  const idx = guest.exports.app_mount_pending_idx();
  if (idx < 0 || !guest.mounts || idx >= guest.mounts.length) return;

  const item = guest.mounts[idx];
  const isImage = item.kind === 'image' || /\.(png|jpe?g)$/i.test(item.name);
  try {
    if (item.source === 'web') {
      setStatus(`${guest.name} · staging web://${item.siteId}/${item.artifact}…`, true);
      await stageWebArtifact(guest, idx, item);
      setStatus(`${guest.name} · staged web://${item.siteId}/${item.artifact}`, true);
      return;
    }
    if (isImage) {
      setStatus(`${guest.name} · decoding ~/${item.name} → RGBA8888…`, true);
      const clientH = Math.max(1, guest.height - TITLE_H);
      const [decoded, contextBytes] = await Promise.all([
        fetchAndDecodeRgba(item.url, guest.width, clientH),
        fetchContextForMount(item),
      ]);
      if (guest.disposed || !guest.exports) return;

      const stagingPtr = guest.exports.app_mount_staging_ptr();
      const mem = new Uint8Array(guest.exports.memory.buffer);
      mem.set(decoded.rgba, stagingPtr);

      if (contextBytes && guest.exports.app_mount_context_ptr && guest.exports.app_mount_commit_context) {
        const ctxPtr = guest.exports.app_mount_context_ptr();
        const ctxMem = new Uint8Array(guest.exports.memory.buffer);
        ctxMem.set(contextBytes, ctxPtr);
        guest.exports.app_mount_commit_context(idx, contextBytes.byteLength);
      }

      const t0 = performance.now();
      guest.exports.app_mount_commit_rgba(idx, decoded.width, decoded.height);
      const t1 = performance.now();
      guest.blitToCanvas(t0, t1);
      setStatus(
        `${guest.name} · staged ${item.name} (${decoded.width}×${decoded.height} RGBA8888)`,
        true,
      );
    } else {
      setStatus(`${guest.name} · fetching ~/${item.name}…`, true);
      const [bytes, contextBytes] = await Promise.all([
        fetchTextBytes(item.url),
        fetchContextForMount(item),
      ]);
      if (guest.disposed || !guest.exports) return;

      const textPtr = guest.exports.app_mount_text_ptr();
      const mem = new Uint8Array(guest.exports.memory.buffer);
      mem.set(bytes, textPtr);

      if (contextBytes && guest.exports.app_mount_context_ptr && guest.exports.app_mount_commit_context) {
        const ctxPtr = guest.exports.app_mount_context_ptr();
        const ctxMem = new Uint8Array(guest.exports.memory.buffer);
        ctxMem.set(contextBytes, ctxPtr);
        guest.exports.app_mount_commit_context(idx, contextBytes.byteLength);
      }

      const t0 = performance.now();
      guest.exports.app_mount_commit_text(idx, bytes.byteLength);
      const t1 = performance.now();
      guest.blitToCanvas(t0, t1);
      setStatus(`${guest.name} · staged ${item.name} (${bytes.byteLength} B UTF-8)`, true);
    }
  } catch (error) {
    console.error(error);
    if (guest.exports?.app_mount_commit_error) {
      guest.exports.app_mount_commit_error(idx, 1);
      const t = performance.now();
      guest.blitToCanvas(t, t);
    }
    setStatus(`${guest.name} · failed to stage ${item.name}: ${error.message || error}`);
  }
}

function setStatus(text, ok = false) {
  statusLeft.replaceChildren();
  if (ok) {
    const dot = document.createElement('span');
    dot.className = 'ok';
    dot.textContent = text;
    statusLeft.append(dot);
  } else {
    statusLeft.textContent = text;
  }
}

function updateHostStats() {
  hostModule.textContent = moduleBytes ? fmtBytes(moduleBytes) : '—';
  hostGuests.textContent = guests.length;
  if (hostGuestsPill) hostGuestsPill.textContent = String(guests.length);
  if (hostMatches) hostMatches.textContent = broker.tables.matches.length;
  const reserved = guests.reduce((sum, guest) => sum + (guest.heapBytes || 0), 0);
  hostMem.textContent = reserved ? fmtBytes(reserved) : '0 B';
  hostBlits.textContent = totalBlits;
  statusCopies.textContent = totalBlits;
  hostSnaps.textContent = snapshots.length;
  snapCountEl.textContent = snapshots.length;
}

function syncWallClock(guest) {
  if (!guest || !guest.exports) return;
  const now = new Date();
  guest.exports.app_set_time(
    now.getHours(),
    now.getMinutes(),
    now.getSeconds(),
    now.getMilliseconds(),
  );
  guest.exports.app_set_date(
    now.getFullYear(),
    now.getMonth() + 1,
    now.getDate(),
    now.getDay(),
  );
  if (guest.exports.app_set_utc) {
    guest.exports.app_set_utc(
      now.getUTCFullYear(),
      now.getUTCMonth() + 1,
      now.getUTCDate(),
      now.getUTCHours(),
      now.getUTCMinutes(),
      now.getUTCSeconds(),
      -now.getTimezoneOffset(),
    );
  }
}

function stopClock() {
  if (clockTimer !== null) {
    clearTimeout(clockTimer);
    clockTimer = null;
  }
}

function pumpClock() {
  stopClock();
  const visibleGuests = splitViewEnabled
    ? guests
    : guests.filter((candidate) => candidate.id === activeId);
  for (const guest of visibleGuests) {
    if (!guest || !guest.exports || !guest.render) continue;
    syncWallClock(guest);
    if (guest.exports.app_tick()) guest.render();
  }
  const wait = 1000 - (Date.now() % 1000);
  clockTimer = setTimeout(pumpClock, Math.max(50, wait));
}

const buildPanelHtml = (vm) => {
  const b = vm.buildInfo;
  const gameEngines = b.checkersBuild ? `${b.checkersBuild} · ${b.minesBuild}` : b.minesBuild;
  return `
  <header class="vm-chassis-header">
    <div class="vm-header-left">
      <span class="vm-status-tag" data-stat="status-tag"><span class="dot running"></span> RUNNING</span>
      <span class="outcome-badge" data-stat="outcome-badge" hidden></span>
      <span class="vm-id-badge">${vm.name}</span>
      <span class="actor-badge human" data-stat="actor-badge">👤 Human</span>
      <span class="vm-build-label">${gameEngines} (<a class="commit-link" href="${b.githubUrl}" target="_blank" rel="noopener noreferrer" title="${b.notes}">↗ ${b.gitCommit}</a>)</span>
    </div>
    <div class="vm-header-right">
      <span class="seat-pill" data-stat="seat-pill">Solo · ${vm.width}×${vm.height}</span>
      <button class="vm-action-btn icon-only" data-action="fullscreen" type="button" title="Full Screen (Zen overlay)">⤢</button>
      <button class="vm-action-btn icon-only" data-action="snap" type="button" title="Capture Framebuffer Snapshot">📷</button>
      <button class="vm-action-btn btn-danger icon-only" data-action="destroy" type="button" title="Terminate / Power Off VM">✕</button>
    </div>
  </header>
  <div class="screen">
    <div class="screen-outcome-overlay" data-stat="screen-outcome" hidden></div>
    <canvas tabindex="0" width="${vm.width}" height="${vm.height}"></canvas>
  </div>
  <footer class="vm-chassis-footer">
    <div class="vm-telemetry-summary">
      <span>Heap: <b data-summary="mem">0 B</b></span>
      <span>·</span>
      <span>Render: <b data-summary="render">0.00 ms</b></span>
      <span>·</span>
      <span>Blit: <b data-summary="blit">0.00 ms</b></span>
      <span>·</span>
      <span>Hash: <b data-summary="hash">0x00000000</b></span>
    </div>
    <button class="vm-metrics-toggle" data-action="toggle-metrics" type="button" title="Toggle detailed hypervisor metrics">▾ Inspect Metrics</button>
  </footer>
  <dl class="vm-stats" hidden>
    <div class="cell"><dt>state</dt><dd class="state" data-stat="state">running</dd></div>
    <div class="cell"><dt>build</dt><dd data-stat="build">${b.shortLabel} (<a class="commit-link" href="${b.githubUrl}" target="_blank" rel="noopener noreferrer">↗ ${b.gitCommit}</a>)</dd></div>
    <div class="cell"><dt>mines engine</dt><dd data-stat="mines-build">${b.minesBuild}</dd></div>
    <div class="cell"><dt>active view</dt><dd data-stat="selected">—</dd></div>
    <div class="cell"><dt>boot / render</dt><dd data-stat="boot">—</dd></div>
    <div class="cell"><dt>blit / present</dt><dd data-stat="blit">—</dd></div>
    <div class="cell"><dt>fb fnv1a-32</dt><dd data-stat="hash">—</dd></div>
    <div class="cell"><dt>guest mem</dt><dd data-stat="mem">—</dd></div>
  </dl>`;
};

function computeActorForGuest(guest) {
  if (guest.actorType) {
    if (guest.actorType === 'autopilot') return { cls: 'autopilot', label: '✈️ AutoPilot' };
    if (guest.actorType === 'minimax') {
      const name = guest.identity?.name || 'FlowBot777';
      return { cls: 'minimax', label: `🤖 ${name}` };
    }
    const name = guest.identity?.name || 'Human';
    return { cls: 'human', label: `👤 ${name}` };
  }
  if (guest.identity?.name) {
    const isRobot = guest.identity.type === 'robot';
    return {
      cls: isRobot ? 'minimax' : 'human',
      label: `${isRobot ? '🤖' : '👤'} ${guest.identity.name}`,
    };
  }
  if (typeof autoPilotRunner !== 'undefined' && autoPilotRunner.state === 'running') {
    return { cls: 'autopilot', label: '✈️ AutoPilot' };
  }
  if (guest.exports) {
    if (guest.exports.app_checkers_ai_pending && guest.exports.app_checkers_ai_pending() !== 0) {
      return { cls: 'minimax', label: '🤖 Minimax' };
    }
    if (guest.exports.app_chess_ai_pending && guest.exports.app_chess_ai_pending() !== 0) {
      return { cls: 'minimax', label: '🤖 Minimax' };
    }
  }
  return { cls: 'human', label: '👤 Human' };
}

function computeGuestOutcome(guest) {
  if (!guest || !guest.exports) return null;

  // 1. Checkers: winner 0=InPlay, 1=Red, 2=Black, 3=Draw
  if (guest.mpGame === 'checkers' || (!guest.mpGame && guest.exports.app_checkers_winner)) {
    if (guest.exports.app_checkers_winner) {
      const w = guest.exports.app_checkers_winner();
      if (w > 0) {
        const seat = guest.mpSeat || 1;
        if (w === 3) return { outcome: 'draw', label: 'DRAW', bannerText: '🤝 DRAW GAME' };
        if (w === seat) return { outcome: 'winner', label: 'VICTORY', bannerText: '👍 🏆 VICTORY!' };
        return { outcome: 'loser', label: 'DEFEAT', bannerText: '👎 ✕ DEFEAT' };
      }
    }
  }

  // 2. Chess: status 0=InPlay, 1=Check, 2=Checkmate, 3=Stalemate, 4=Draw50Move
  if (guest.mpGame === 'chess' || (!guest.mpGame && guest.exports.app_chess_status)) {
    if (guest.exports.app_chess_status) {
      const st = guest.exports.app_chess_status();
      if (st === 2) {
        // Checkmate: turn() is the mated player (loser)
        const loserSeat = guest.exports.app_chess_turn ? guest.exports.app_chess_turn() : 1;
        const winnerSeat = loserSeat === 1 ? 2 : 1;
        const seat = guest.mpSeat || 1;
        if (seat === winnerSeat) return { outcome: 'winner', label: 'CHECKMATE · VICTORY', bannerText: '👍 🏆 CHECKMATE! VICTORY' };
        return { outcome: 'loser', label: 'CHECKMATE · DEFEAT', bannerText: '👎 ✕ CHECKMATE! DEFEAT' };
      } else if (st === 3 || st === 4) {
        return { outcome: 'draw', label: 'DRAW', bannerText: st === 3 ? '🤝 STALEMATE (DRAW)' : '🤝 DRAW (50-MOVE)' };
      }
    }
  }

  // 3. Minesweeper: winner 0=InPlay, 1=P1, 2=P2, 3=Draw
  if (guest.exports.app_mines_winner) {
    const w = guest.exports.app_mines_winner();
    if (w > 0) {
      const seat = guest.mpSeat || 1;
      if (w === 3) return { outcome: 'draw', label: 'DRAW', bannerText: '🤝 DRAW' };
      if (w === seat) return { outcome: 'winner', label: 'VICTORY', bannerText: '👍 🏆 VICTORY!' };
      return { outcome: 'loser', label: 'DEFEAT', bannerText: '👎 ✕ DEFEAT' };
    }
  }

  return null;
}

function updateGuestBanner(guest) {
  const pill = guest?.panel?.querySelector('[data-stat="seat-pill"]');
  let infoText = '';
  if (guest.mpSeat && guest.mpMatchId) {
    if (guest.mpGame === 'chess' && guest.exports?.app_chess_turn) {
      const turn = guest.exports.app_chess_turn();
      const white = guest.exports.app_chess_white_count ? guest.exports.app_chess_white_count() : 16;
      const blk = guest.exports.app_chess_black_count ? guest.exports.app_chess_black_count() : 16;
      infoText = `Match #${guest.mpMatchId} · Chess Seat P${guest.mpSeat} · Turn P${turn} (${white}W:${blk}B)`;
    } else if (guest.mpGame === 'checkers' && guest.exports?.app_checkers_turn) {
      const turn = guest.exports.app_checkers_turn();
      const red = guest.exports.app_checkers_red_count ? guest.exports.app_checkers_red_count() : 12;
      const blk = guest.exports.app_checkers_black_count ? guest.exports.app_checkers_black_count() : 12;
      infoText = `Match #${guest.mpMatchId} · Checkers Seat P${guest.mpSeat} · Turn P${turn} (${red}R:${blk}B)`;
    } else {
      const turn = guest.exports?.app_mines_turn ? guest.exports.app_mines_turn() : 1;
      const p1 = guest.exports?.app_mines_p1_score ? guest.exports.app_mines_p1_score() : 0;
      const p2 = guest.exports?.app_mines_p2_score ? guest.exports.app_mines_p2_score() : 0;
      infoText = `Match #${guest.mpMatchId} · Seat P${guest.mpSeat} · Turn P${turn} (${p1}:${p2})`;
    }
  } else {
    infoText = `${guest.buildInfo.version} · ${guest.width}×${guest.height}`;
  }
  if (pill) pill.textContent = infoText;

  const actorBadge = guest?.panel?.querySelector('[data-stat="actor-badge"]');
  if (actorBadge) {
    const actor = computeActorForGuest(guest);
    actorBadge.className = `actor-badge ${actor.cls}`;
    actorBadge.textContent = actor.label;
  }

  // Update Game Outcome (Victory 👍 🏆 vs Defeat 👎 ✕)
  const outcomeInfo = computeGuestOutcome(guest);
  const statusTag = guest?.panel?.querySelector('[data-stat="status-tag"]');
  const outcomeBadge = guest?.panel?.querySelector('[data-stat="outcome-badge"]');
  const screenOutcome = guest?.panel?.querySelector('[data-stat="screen-outcome"]');
  const panel = guest?.panel;

  if (outcomeInfo) {
    if (statusTag) statusTag.hidden = true;
    if (outcomeBadge) {
      outcomeBadge.hidden = false;
      outcomeBadge.className = `outcome-badge outcome-${outcomeInfo.outcome}`;
      const badgeIcon = outcomeInfo.outcome === 'winner' ? '👍 🏆' : outcomeInfo.outcome === 'loser' ? '👎 ✕' : '🤝';
      outcomeBadge.textContent = `${badgeIcon} ${outcomeInfo.label}`;
    }
    if (screenOutcome) {
      if (guest._outcomeDismissedKey !== outcomeInfo.bannerText) {
        screenOutcome.hidden = false;
        screenOutcome.className = `screen-outcome-overlay outcome-${outcomeInfo.outcome}`;
        screenOutcome.innerHTML = `
          <span class="outcome-text">${outcomeInfo.bannerText}</span>
          <button class="outcome-close-btn" type="button" aria-label="Dismiss banner" title="Dismiss banner">✕</button>
        `;
        const closeBtn = screenOutcome.querySelector('.outcome-close-btn');
        if (closeBtn) {
          closeBtn.onclick = (e) => {
            e.stopPropagation();
            guest._outcomeDismissedKey = outcomeInfo.bannerText;
            screenOutcome.hidden = true;
          };
        }
      }
    }
    if (panel) {
      panel.classList.toggle('winner-panel', outcomeInfo.outcome === 'winner');
      panel.classList.toggle('loser-panel', outcomeInfo.outcome === 'loser');
    }
  } else {
    guest._outcomeDismissedKey = null;
    if (statusTag) statusTag.hidden = false;
    if (outcomeBadge) outcomeBadge.hidden = true;
    if (screenOutcome) screenOutcome.hidden = true;
    if (panel) {
      panel.classList.remove('winner-panel', 'loser-panel');
    }
  }

  if (getFullscreenGuest() === guest) {
    updateFullscreenHud(guest);
  }
}

function createTab(vm) {
  const wrap = document.createElement('div');
  wrap.className = 'tab-wrap';
  wrap.innerHTML = `
    <button class="tab" role="tab" type="button" id="tab-${vm.id}"
            aria-controls="panel-${vm.id}" aria-selected="false" tabindex="-1">
      <span class="dot running"></span>
      <span>${vm.name} · ${vm.width}×${vm.height}</span>
      <span class="tab-badge">${vm.buildInfo.version}</span>
    </button>
    <button class="tab-close" type="button" aria-label="Stop ${vm.name}"
            title="Stop ${vm.name}">×</button>`;

  const tab = wrap.querySelector('.tab');
  tab.addEventListener('click', () => {
    activate(vm.id);
    vm.canvas.focus({ preventScroll: true });
  });
  wrap.querySelector('.tab-close').addEventListener('click', (event) => {
    event.stopPropagation();
    destroyGuest(vm);
  });
  return wrap;
}

function activate(id) {
  activeId = id;
  for (const guest of guests) {
    const on = guest.id === id;
    guest.wrap.classList.toggle('active', on);
    const tab = guest.wrap.querySelector('.tab');
    tab.setAttribute('aria-selected', on ? 'true' : 'false');
    tab.tabIndex = on ? 0 : -1;
    guest.panel.hidden = splitViewEnabled ? false : !on;
    guest.panel.classList.toggle('active-panel', on);
    if (on && statusRes) {
      statusRes.textContent = `${guest.width}×${guest.height}`;
    }
  }
  pumpClock();
  updateHostStats();
}

function statNode(guest, key) {
  return guest.panel.querySelector(`[data-stat="${key}"]`);
}

function summaryNode(guest, key) {
  return guest.panel.querySelector(`[data-summary="${key}"]`);
}

function updateGuestStats(guest) {
  const bootNode = statNode(guest, 'boot');
  if (bootNode) bootNode.textContent = `${fmtMs(guest.boot)} / ${fmtMs(guest.last.render)}`;
  const blitNode = statNode(guest, 'blit');
  if (blitNode) blitNode.textContent = `${fmtMs(guest.last.blit)} (${guest.frames} blits)`;
  const selNode = statNode(guest, 'selected');
  if (selNode) selNode.textContent = selectionLabel(guest);
  const hashNode = statNode(guest, 'hash');
  if (hashNode) hashNode.textContent = fmtHex32(guest.fbHash);
  const memNode = statNode(guest, 'mem');
  if (memNode) memNode.textContent = fmtBytes(guest.heapBytes);

  // Update concise footer telemetry summary
  const sumMem = summaryNode(guest, 'mem');
  if (sumMem) sumMem.textContent = fmtBytes(guest.heapBytes);
  const sumRender = summaryNode(guest, 'render');
  if (sumRender) sumRender.textContent = fmtMs(guest.last.render);
  const sumBlit = summaryNode(guest, 'blit');
  if (sumBlit) sumBlit.textContent = fmtMs(guest.last.blit);
  const sumHash = summaryNode(guest, 'hash');
  if (sumHash) sumHash.textContent = fmtHex32(guest.fbHash);

  updateGuestBanner(guest);
}

function markFault(guest, message) {
  guest.faulted = true;
  const state = statNode(guest, 'state');
  if (state) {
    state.textContent = 'fault';
    state.style.color = '#ff9b9b';
  }
  guest.wrap.classList.remove('active');
  guest.wrap.querySelector('.dot').classList.remove('running');
  guest.panel.querySelector('.screen').innerHTML =
    `<div class="fault">instantiation failed<br><span>${message}</span></div>`;
  setStatus(`${guest.name} · fault`);
}

function destroyGuest(guest) {
  if (getFullscreenGuest() === guest) {
    exitFullscreen();
  }
  const index = guests.indexOf(guest);
  if (index < 0) return;
  if (guest.id === activeId) stopClock();
  if (guest.chkAiTimer) {
    clearTimeout(guest.chkAiTimer);
    guest.chkAiTimer = null;
  }
  guest.disposed = true;
  guests.splice(index, 1);

  if (guest.exports) {
    try {
      guest.exports.app_destroy();
    } catch (error) {
      console.error(error);
    }
    guest.exports = null;
  }

  guest.wrap.remove();
  guest.panel.remove();

  if (activeId === guest.id) {
    const next = guests[Math.min(index, guests.length - 1)];
    activate(next ? next.id : null);
  }
  if (guests.length === 0) {
    emptyState.hidden = true;
    showLaunchpad();
  }
  updateHostStats();
  setStatus('ready', true);
}

function recordSnapshotCard(snap) {
  snapshots.unshift(snap);
  openInspectorSection(snapDrawer);
  updateHostStats();

  const card = document.createElement('div');
  card.className = 'snap-card';
  card.innerHTML = `
    <img src="${snap.dataUrl}" alt="Snapshot ${snap.seq} from ${snap.vm}">
    <div class="snap-meta">
      <span>${snap.vm} (${snap.buildId}) · #${snap.seq} (${snap.view})</span>
      <b>${snap.hashHex}</b>
    </div>
    <a class="snap-dl" href="${snap.dataUrl}" download="${snap.filename || `${snap.vm}-${snap.buildId}-snap-${snap.seq}-${snap.hashHex}.png`}">
      ↓ download png receipt
    </a>`;
  snapList.prepend(card);
}

function captureSnapshot(guest, alreadyRecordedInGuest = false) {
  if (!guest || !guest.exports || !guest.canvas) return null;
  let hash;
  if (!alreadyRecordedInGuest) {
    syncWallClock(guest);
    hash = guest.exports.app_record_snapshot() >>> 0;
    guest.render();
  } else {
    hash = guest.exports.app_fb_hash() >>> 0;
  }
  guest.fbHash = hash;
  const hashHex = fmtHex32(hash);
  const seq = guest.exports.app_snapshot_count() >>> 0;
  const view = selectionLabel(guest);
  const dataUrl = guest.canvas.toDataURL('image/png');

  const snap = {
    vm: guest.name,
    guestId: guest.id,
    buildId: guest.buildInfo.id,
    gitCommit: guest.buildInfo.gitCommit,
    seq,
    view,
    width: guest.width,
    height: guest.height,
    hash,
    hashHex,
    timestamp: new Date().toISOString(),
    dataUrl,
    filename: `${guest.name}-${guest.buildInfo.id}-snap-${seq}-${hashHex}.png`,
  };
  recordSnapshotCard(snap);
  setStatus(`${guest.name} · snapshot #${seq} captured (${hashHex})`, true);
  return snap;
}

function captureSplitSnapshot(g1, g2, label) {
  if (!g1 || !g2 || !g1.canvas || !g2.canvas) return null;
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
    ctx.drawImage(g1.canvas, 0, 0, w1, h1);
    ctx.drawImage(g2.canvas, w1, 0, w2, h2);
  }

  const h1Hex = g1.exports?.app_fb_hash ? fmtHex32(g1.exports.app_fb_hash() >>> 0) : '0x00000000';
  const h2Hex = g2.exports?.app_fb_hash ? fmtHex32(g2.exports.app_fb_hash() >>> 0) : '0x00000000';
  const dataUrl = canvas.toDataURL('image/png');
  const snapSeq = snapshots.length + 1;

  const snap = {
    vm: `${g1.name} | ${g2.name}`,
    guestId: g1.id,
    buildId: `${g1.buildInfo.id}+${g2.buildInfo.id}`,
    gitCommit: g1.buildInfo.gitCommit,
    seq: snapSeq,
    view: label || 'Split Dual-Perspective',
    width: compW,
    height: compH,
    hash: 0,
    hashHex: `${h1Hex} | ${h2Hex}`,
    timestamp: new Date().toISOString(),
    dataUrl,
    filename: `split-${g1.name}-${g2.name}-snap-${snapSeq}.png`,
  };

  recordSnapshotCard(snap);
  setStatus(`Split View Dual snapshot captured (${h1Hex} | ${h2Hex})`, true);
  return snap;
}

function parseSelectedResolution() {
  if (!resSelect || !resSelect.value) return { width: DEFAULT_W, height: DEFAULT_H };
  const parts = resSelect.value.split('x').map((n) => parseInt(n, 10));
  if (parts.length === 2 && parts[0] > 0 && parts[1] > 0) {
    return { width: parts[0], height: parts[1] };
  }
  return { width: DEFAULT_W, height: DEFAULT_H };
}

async function createGuest(opts = {}) {
  const chosen = parseSelectedResolution();
  const width = opts.width || chosen.width;
  const height = opts.height || chosen.height;
  const buildId = opts.buildId || (buildSelect && buildSelect.value) || buildRegistry.defaultBuild;
  const buildInfo = getBuildInfo(buildId);
  const fbBytes = width * height * 4;
  const id = nextId++;
  const uri = opts.uri || opts.name || `vm-${String(id).padStart(2, '0')}`;
  const identity = opts.identity || {
    name: opts.userName || opts.name || `User-${String(id).padStart(2, '0')}`,
    seed: opts.seed || (id === 1 ? 'Jarad' : id === 3 ? 'Alex' : opts.name || `User-${String(id).padStart(2, '0')}`),
    type: opts.actorType === 'minimax' ? 'robot' : 'human',
  };
  const vm = {
    id,
    name: uri,
    uri,
    identity,
    actorType: opts.actorType || null,
    buildId: buildInfo.id,
    buildInfo,
    width,
    height,
    exports: null,
    mounts: [],
    mpGame: null,
    mpMatchId: null,
    mpSeat: 0,
    chkAiTimer: null,
    frames: 0,
    boot: 0,
    heapBytes: 0,
    fbHash: 0,
    disposed: false,
    faulted: false,
    last: { render: 0, blit: 0, present: 0 },
  };

  vm.wrap = createTab(vm);
  tabsEl.append(vm.wrap);

  vm.panel = document.createElement('section');
  vm.panel.className = 'vm-panel';
  vm.panel.id = `panel-${id}`;
  vm.panel.setAttribute('role', 'tabpanel');
  vm.panel.setAttribute('aria-labelledby', `tab-${id}`);
  vm.panel.hidden = true;
  vm.panel.innerHTML = buildPanelHtml(vm);
  panelsEl.append(vm.panel);

  vm.canvas = vm.panel.querySelector('canvas');

  // Wire chassis action buttons
  const bannerFsBtn = vm.panel.querySelector('[data-action="fullscreen"]');
  if (bannerFsBtn) {
    bannerFsBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      toggleFullscreen(vm);
    });
  }

  const chassisSnapBtn = vm.panel.querySelector('[data-action="snap"]');
  if (chassisSnapBtn) {
    chassisSnapBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      captureSnapshot(vm, false);
    });
  }

  const chassisDestroyBtn = vm.panel.querySelector('[data-action="destroy"]');
  if (chassisDestroyBtn) {
    chassisDestroyBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      destroyGuest(vm);
    });
  }

  const metricsToggleBtn = vm.panel.querySelector('[data-action="toggle-metrics"]');
  if (metricsToggleBtn) {
    metricsToggleBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      const statsGrid = vm.panel.querySelector('.vm-stats');
      if (statsGrid) {
        statsGrid.hidden = !statsGrid.hidden;
        metricsToggleBtn.textContent = statsGrid.hidden ? '▾ Inspect Metrics' : '▴ Collapse Metrics';
      }
    });
  }
  guests.push(vm);
  emptyState.hidden = true;
  activate(id);
  updateHostStats();

  try {
    const { mod: wasmModule, byteLength } = await loadWasmModuleForBuild(buildInfo);
    vm.moduleBytes = byteLength;
    const startedAt = performance.now();
    const instance = await WebAssembly.instantiate(wasmModule, {});
    const exports = instance.exports;

    if (vm.disposed) {
      exports.app_destroy();
      return vm;
    }

    vm.exports = exports;
    exports.app_set_session(id);
    syncWallClock(vm);
    if (!exports.app_init(width, height)) throw new Error('app_init() returned 0');

    mountCatalogIntoGuest(vm, DEFAULT_MOUNTS);

    vm.boot = performance.now() - startedAt;
    const ctx = vm.canvas.getContext('2d', { alpha: false });
    const image = ctx.createImageData(width, height);

    const blitToCanvas = (t0, t1) => {
      const fbPtr = exports.app_framebuffer();
      const memBytes = new Uint8Array(exports.memory.buffer);
      vm.heapBytes = memBytes.byteLength;
      image.data.set(memBytes.subarray(fbPtr, fbPtr + fbBytes));
      const t2 = performance.now();
      ctx.putImageData(image, 0, 0);
      const t3 = performance.now();

      vm.last = { render: t1 - t0, blit: t2 - t1, present: t3 - t2 };
      vm.frames += 1;
      totalBlits += 1;
      if (vm.frames === 1 || vm.frames % HASH_EVERY_N_FRAMES === 0) {
        vm.fbHash = exports.app_fb_hash() >>> 0;
      }
      updateGuestStats(vm);
      updateHostStats();
    };

    const render = () => {
      if (vm.disposed || !vm.exports) return;
      const t0 = performance.now();
      exports.app_render();
      const t1 = performance.now();
      blitToCanvas(t0, t1);
    };

    const clickLogical = (x, y) => {
      syncWallClock(vm);
      const t0 = performance.now();
      const rc = exports.app_click(x, y);
      const t1 = performance.now();
      if (trajectoryRecorder.isRecording() && trajectoryRecorder.guest?.id === vm.id) {
        trajectoryRecorder.recordEvent('click', { x, y });
      }
      if (rc !== 0) {
        blitToCanvas(t0, t1);
        serviceMinesMultiplayer(vm);
        serviceCheckers(vm);
        serviceChessMultiplayer(vm);
        serviceChessAi(vm);
        if (rc === 5) {
          openExternal(vm);
        } else if (rc === 2) {
          captureSnapshot(vm, true);
        } else if (rc === 3) {
          handleTtsToggle(vm);
        } else {
          setStatus(`${vm.name} (${vm.buildInfo.version}) · ${selectionLabel(vm)} · ${fmtHex32(vm.fbHash)}`, true);
        }
        serviceWebControl(vm);
        servicePendingMountFetch(vm);
      }
      return rc;
    };

    const setPattern = (enabled) => {
      exports.app_set_test_pattern(enabled ? 1 : 0);
      render();
    };

    const logicalPoint = (event) => {
      const rect = vm.canvas.getBoundingClientRect();
      if (!rect.width || !rect.height) return null;
      return {
        x: Math.floor(((event.clientX - rect.left) * width) / rect.width),
        y: Math.floor(((event.clientY - rect.top) * height) / rect.height),
      };
    };

    vm.canvas.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      if (activeId !== vm.id) activate(vm.id);
      const point = logicalPoint(event);
      if (!point) return;
      const { x, y } = point;
      vm.canvas.focus({ preventScroll: true });
      const rightClick = event.button === 2 || event.ctrlKey;
      if (rightClick && exports.app_selected() === 4) {
        exports.app_key(30); // toggle Flag mode for this click
        clickLogical(x, y);
        exports.app_key(30); // restore previous mode
        render();
      } else {
        clickLogical(x, y);
      }
    });

    vm.canvas.addEventListener('contextmenu', (event) => {
      event.preventDefault();
    });

    let movePending = false;
    let movePoint = null;
    const flushMove = () => {
      movePending = false;
      if (!movePoint || vm.disposed || !vm.exports) return;
      const point = movePoint;
      movePoint = null;
      const t0 = performance.now();
      const rc = exports.app_pointer_move(point.x, point.y);
      const t1 = performance.now();
      if (rc !== 0) blitToCanvas(t0, t1);
    };
    vm.canvas.addEventListener('pointermove', (event) => {
      const point = logicalPoint(event);
      if (!point) return;
      movePoint = point;
      if (!movePending) {
        movePending = true;
        requestAnimationFrame(flushMove);
      }
    });
    vm.canvas.addEventListener('pointerleave', () => {
      if (!vm.exports) return;
      movePoint = null;
      const t0 = performance.now();
      const rc = exports.app_pointer_leave();
      const t1 = performance.now();
      if (rc !== 0) blitToCanvas(t0, t1);
    });

    vm.canvas.addEventListener('keydown', (event) => {
      let code = KEY_CODES[event.key];
      if (code == null && event.altKey) {
        if (event.code === 'KeyM') code = 61;
        if (event.code === 'KeyD') code = 60;
      }
      if (code == null) return;
      if (event.repeat && (code === 60 || code === 61)) return;

      const mods =
        (event.shiftKey ? MODS.Shift : 0) |
        (event.ctrlKey ? MODS.Ctrl : 0) |
        (event.altKey ? MODS.Alt : 0) |
        (event.metaKey ? MODS.Meta : 0);
      const t0 = performance.now();
      const rc = exports.app_key_mods
        ? exports.app_key_mods(code, mods)
        : exports.app_key(code);
      const t1 = performance.now();

      if (trajectoryRecorder.isRecording() && trajectoryRecorder.guest?.id === vm.id) {
        trajectoryRecorder.recordEvent('key', { code, mods });
      }

      if (rc === 0) return;
      event.preventDefault();
      if (rc === 1 || rc === 2 || rc === 3 || rc === 5) {
        blitToCanvas(t0, t1);
        serviceMinesMultiplayer(vm);
        serviceCheckers(vm);
        serviceChessMultiplayer(vm);
        serviceChessAi(vm);
        if (rc === 5) {
          openExternal(vm);
        } else if (rc === 2) {
          captureSnapshot(vm, true);
        } else if (rc === 3) {
          handleTtsToggle(vm);
        } else {
          setStatus(
            `${vm.name} (${vm.buildInfo.version}) · ${selectionLabel(vm)} · focus ${focusLabel(exports.app_focus())} · ${fmtHex32(vm.fbHash)}`,
            true,
          );
        }
        serviceWebControl(vm);
        servicePendingMountFetch(vm);
      }
    });

    vm.blitToCanvas = blitToCanvas;
    vm.render = render;
    vm.clickLogical = clickLogical;
    vm.setPattern = setPattern;

    render();
    if (patternToggle.checked) setPattern(true);
    setStatus(
      `${vm.name} (${vm.buildInfo.shortLabel}) · booted in ${fmtMs(vm.boot)} · ${vm.mounts.length} files mounted`,
      true,
    );
    pumpClock();
    const p1Seed = vm.identity?.seed || 'Jarad';
    const p2Seed = 'Minimax-2ply';
    syncPlayerAvatarsToGuest(vm, p1Seed, p2Seed);
    initWebForGuest(vm).catch((error) => console.error(error));
    return vm;
  } catch (error) {
    console.error(error);
    if (!vm.disposed) markFault(vm, String(error && error.message ? error.message : error));
    return vm;
  }
}


/* One-click A/B Comparison between Build #3 (`v3.1`, Checkers + 2P Mines) and Build #2 (`aea4b4b`, v3.0) */
async function startAbComparison() {
  let b3 = guests.find((g) => g.buildId === 'build-03');
  let b2 = guests.find((g) => g.buildId === 'build-02') || guests.find((g) => g.buildId === 'build-01');
  if (!b3) b3 = await createGuest({ buildId: 'build-03' });
  if (!b2) b2 = await createGuest({ buildId: 'build-02' });

  if (b3.exports && b3.exports.app_checkers_set_mp) {
    launchCheckersInGuest(b3);
  } else {
    launchMinesInGuest(b3);
  }
  launchMinesInGuest(b2);
  setSplitView(true);
  activate(b3.id);
  setStatus(
    `A/B Build Comparison active · ${b3.name} (${b3.buildInfo.shortLabel}) vs ${b2.name} (${b2.buildInfo.shortLabel})`,
    true,
  );
  return { build03: b3.name, build02: b2.name, build01: b2.name };
}

/* Join an existing Network Match as Seat P1 or Seat P2 */
async function joinNetworkMatch(matchId, seat = 2) {
  let targetMatch = broker.tables.matches.find((m) => m.id === matchId);
  if (!targetMatch) {
    try {
      const res = await window.fetch(`${broker.relayUrl}/api/matches/${matchId}`);
      if (res.ok) {
        targetMatch = await res.json();
        const idx = broker.tables.matches.findIndex((m) => m.id === targetMatch.id);
        if (idx >= 0) broker.tables.matches[idx] = targetMatch;
        else broker.tables.matches.unshift(targetMatch);
      }
    } catch (err) {
      console.warn('Could not fetch match from relay:', err);
    }
  }

  if (!targetMatch) {
    setStatus(`Failed to join network match #${matchId}: match not found on relay`);
    return null;
  }

  const game = targetMatch.game || 'mines';
  let g = guests[0];
  if (!g) {
    g = await createGuest({ buildId: 'build-03' });
  }

  g.mpGame = game;
  g.mpMatchId = matchId;
  g.mpSeat = seat;

  if (game === 'chess' && g.exports?.app_chess_set_mp) {
    g.exports.app_chess_set_mp(1, matchId, seat, 0);
    launchChessInGuest(g);
  } else if (game === 'checkers' && g.exports?.app_checkers_set_mp) {
    g.exports.app_checkers_set_mp(1, matchId, seat);
    launchCheckersInGuest(g);
  } else if (g.exports?.app_mines_set_mp) {
    g.exports.app_mines_set_mp(1, matchId, seat, targetMatch.seed ?? 42, targetMatch.max_strikes ?? 1);
    launchMinesInGuest(g);
  }

  broker.connectSse(matchId);
  updateGuestBanner(g);
  openInspectorSection(brokerDrawer);
  broker.renderDrawer();
  updateHostStats();

  // Show room link
  updateRoomLink(matchId, seat === 1 ? 2 : 1);

  setStatus(
    `Joined Network Match #${matchId} (${game}) as Seat P${seat} · Turn P${targetMatch.turn_seat || 1}`,
    true,
  );
  return targetMatch;
}

/* Create a new Network Match as Seat P1 and subscribe to relay SSE */
async function createNetworkMatch(game = 'checkers') {
  let g = guests[0];
  if (!g) {
    g = await createGuest({ buildId: 'build-03' });
  }

  broker.setNetworkMode(true);
  const initHash =
    game === 'chess' && g.exports?.app_chess_state_hash
      ? fmtHex32(g.exports.app_chess_state_hash() >>> 0)
      : game === 'checkers' && g.exports?.app_checkers_state_hash
        ? fmtHex32(g.exports.app_checkers_state_hash() >>> 0)
        : '0x00000000';
  const seed = Math.floor(100 + Math.random() * 900);

  const { json: match } = await broker.fetch('POST', '/api/matches', {
    game,
    seed,
    state_hash: initHash,
    p1_vm: 'Browser 1 (P1)',
    p2_vm: 'Remote Player (P2)',
    p1_id: 1,
    p2_id: 2,
  });

  g.mpGame = game;
  g.mpMatchId = match.id;
  g.mpSeat = 1;

  if (game === 'chess' && g.exports?.app_chess_set_mp) {
    g.exports.app_chess_set_mp(1, match.id, 1, 0);
    launchChessInGuest(g);
  } else if (game === 'checkers' && g.exports?.app_checkers_set_mp) {
    g.exports.app_checkers_set_mp(1, match.id, 1);
    launchCheckersInGuest(g);
  } else if (g.exports?.app_mines_set_mp) {
    g.exports.app_mines_set_mp(1, match.id, 1, seed, 1);
    launchMinesInGuest(g);
  }

  broker.connectSse(match.id);
  updateGuestBanner(g);
  openInspectorSection(brokerDrawer);
  broker.renderDrawer();
  updateHostStats();

  updateRoomLink(match.id, 2);

  setStatus(
    `Network Match #${match.id} (${game}) created! Joined as Seat P1. Share link for Seat P2.`,
    true,
  );
  return match;
}

/* ------------------------------------------------------------------ boot -- */

const emptyState = document.createElement('div');
emptyState.className = 'empty';
emptyState.textContent = 'no guests attached — press “+ new guest”';
panelsEl.append(emptyState);

newVmButton.addEventListener('click', () => {
  createGuest().catch((error) => console.error(error));
});

if (chsMatchBtn) {
  chsMatchBtn.addEventListener('click', () => {
    startChessMatch().catch((error) => console.error(error));
  });
}

if (chkMatchBtn) {
  chkMatchBtn.addEventListener('click', () => {
    startCheckersMatch().catch((error) => console.error(error));
  });
}

mpMatchBtn.addEventListener('click', () => {
  startTwoPlayerMatch().catch((error) => console.error(error));
});

abBtn.addEventListener('click', () => {
  startAbComparison().catch((error) => console.error(error));
});

splitBtn.addEventListener('click', () => {
  setSplitView(!splitViewEnabled);
});

if (brokerResetBtn) {
  brokerResetBtn.addEventListener('click', () => {
    const activeMatch = broker.tables.matches[0];
    if (activeMatch && activeMatch.game === 'chess') {
      startChessMatch().catch((error) => console.error(error));
    } else if (activeMatch && activeMatch.game === 'checkers') {
      startCheckersMatch().catch((error) => console.error(error));
    } else {
      const nextSeed = Math.floor(100 + Math.random() * 900);
      startTwoPlayerMatch({ seed: nextSeed }).catch((error) => console.error(error));
    }
  });
}

if (brokerHideBtn) {
  brokerHideBtn.addEventListener('click', () => {
    brokerDrawer.hidden = true;
  });
}

snapButton.addEventListener('click', () => {
  const guest = guests.find((candidate) => candidate.id === activeId);
  if (guest) captureSnapshot(guest, false);
});

step1Button.addEventListener('click', () => {
  const guest = guests.find((candidate) => candidate.id === activeId);
  if (!guest || !guest.clickLogical) return;
  if (guest.exports.app_dock_visible() === 0) {
    guest.clickLogical(16, 16);
  }
  guest.clickLogical(76, 80);
  captureSnapshot(guest, false);
});

clearSnapsBtn.addEventListener('click', () => {
  snapshots.length = 0;
  snapList.replaceChildren();
  snapDrawer.hidden = true;
  updateHostStats();
});

// Network Match UI handlers
const netMatchBtn = document.getElementById('net-match-btn');
const netCreateChsBtn = document.getElementById('net-create-chs');
const netCreateChkBtn = document.getElementById('net-create-chk');
const netCreateMinesBtn = document.getElementById('net-create-mines');
const netJoinIdInput = document.getElementById('net-join-id');
const netJoinP1Btn = document.getElementById('net-join-p1-btn');
const netJoinP2Btn = document.getElementById('net-join-p2-btn');

if (netMatchBtn) {
  netMatchBtn.addEventListener('click', () => {
    openInspectorSection(brokerDrawer);
    const panel = document.getElementById('net-broker-panel');
    if (panel) {
      panel.scrollIntoView({ behavior: 'smooth' });
    }
  });
}

if (netCreateChsBtn) {
  netCreateChsBtn.addEventListener('click', () => {
    createNetworkMatch('chess').catch((err) => console.error(err));
  });
}

if (netCreateChkBtn) {
  netCreateChkBtn.addEventListener('click', () => {
    createNetworkMatch('checkers').catch((err) => console.error(err));
  });
}

if (netCreateMinesBtn) {
  netCreateMinesBtn.addEventListener('click', () => {
    createNetworkMatch('mines').catch((err) => console.error(err));
  });
}

if (netJoinP1Btn && netJoinIdInput) {
  netJoinP1Btn.addEventListener('click', () => {
    const matchId = parseInt(netJoinIdInput.value.trim(), 10);
    if (matchId) joinNetworkMatch(matchId, 1).catch((err) => console.error(err));
  });
}

if (netJoinP2Btn && netJoinIdInput) {
  netJoinP2Btn.addEventListener('click', () => {
    const matchId = parseInt(netJoinIdInput.value.trim(), 10);
    if (matchId) joinNetworkMatch(matchId, 2).catch((err) => console.error(err));
  });
}

initNetworkCopyHandler();

initFullscreenListeners();

patternToggle.addEventListener('change', () => {
  for (const guest of guests) {
    if (guest.setPattern) guest.setPattern(patternToggle.checked);
  }
});

if (hostStatsPill && hostStatsDrawer) {
  hostStatsPill.addEventListener('click', () => {
    if (hostStatsDrawer.hidden) {
      openInspectorSection(hostStatsDrawer);
    } else {
      hostStatsDrawer.hidden = true;
    }
  });
}

setInterval(() => {
  const uptimeStr = fmtDuration(performance.now() - bootAt);
  if (hostUptime) hostUptime.textContent = uptimeStr;
  if (hostUptimePill) hostUptimePill.textContent = uptimeStr;
}, 1000);
if (hostUptime) hostUptime.textContent = '00:00:00';
if (hostUptimePill) hostUptimePill.textContent = '00:00:00';
updateHostStats();

document.addEventListener('visibilitychange', () => {
  if (!document.hidden) pumpClock();
});

/* Auto Pilot Trajectory Runner & UI Wiring */
const autoPilotRunner = new AutoPilotRunner({
  getGuest: () => guests.find((candidate) => candidate.id === activeId) || guests[0],
  getGuests: () => guests,
  createGuest: (opts) => createGuest(opts),
  destroyGuest: (g) => destroyGuest(g),
  captureSnapshot: (g, alreadyRecorded) => captureSnapshot(g, alreadyRecorded),
  captureSplitSnapshot: (g1, g2, label) => captureSplitSnapshot(g1, g2, label),
  setStatus: (msg, ok) => setStatus(msg, ok),
});

/* Trajectory Recording & Playback Engine */
const trajectoryRecorder = new TrajectoryRecorder();
const trajectoryPlayer = new TrajectoryPlayer({
  getGuest: () => guests.find((candidate) => candidate.id === activeId) || guests[0],
  getGuests: () => guests,
});

/* Wire Modular Auto Pilot UI */
configureAutoPilotUiDeps({
  autoPilotRunner,
  getGuests: () => guests,
  updateGuestBanner: (guest) => updateGuestBanner(guest),
  openInspectorSection: (section) => openInspectorSection(section),
  setStatus: (msg, ok) => setStatus(msg, ok),
});
const autoPilotUi = initAutoPilotUi();

/* Wire Modular Trajectory UI */
configureTrajectoryUiDeps({
  trajectoryRecorder,
  trajectoryPlayer,
  getActiveGuest: () => guests.find((cand) => cand.id === activeId) || guests[0],
  getActiveId: () => activeId,
  getGuests: () => guests,
  setStatus: (msg, ok) => setStatus(msg, ok),
  openInspectorSection: (sec) => openInspectorSection(sec),
});
const trajectoryUi = initTrajectoryUi();

/* Programmatic Agent / QA Test Harness Surface (`window.__appliance`) */
window.__appliance = {
  registry: () => buildRegistry,
  broker: () => broker,
  list: () => guests.map((guest) => ({
    id: guest.id,
    name: guest.name,
    buildId: guest.buildId,
    gitCommit: guest.buildInfo.gitCommit,
    githubUrl: guest.buildInfo.githubUrl,
    minesBuild: guest.buildInfo.minesBuild,
    mpMatchId: guest.mpMatchId,
    mpSeat: guest.mpSeat,
    width: guest.width,
    height: guest.height,
    view: selectionLabel(guest),
    dockVisible: guest.exports ? Boolean(guest.exports.app_dock_visible()) : false,
    renders: guest.frames,
    fbHash: guest.fbHash,
    fbHashHex: fmtHex32(guest.fbHash),
    focus: guest.exports ? guest.exports.app_focus() : 0,
    theme: guest.exports ? guest.exports.app_theme() : 0,
    renderMode: guestRenderMode(guest),
    fontFamily: guestRenderMode(guest),
  })),
  click: (id, x, y) => {
    const guest = guests.find((candidate) => candidate.id === id) || guests[0];
    if (!guest || !guest.clickLogical) return null;
    const rc = guest.clickLogical(x, y);
    if (guest.exports) guest.fbHash = guest.exports.app_fb_hash() >>> 0;
    return {
      rc,
      view: selectionLabel(guest),
      focus: guest.exports.app_focus(),
      theme: guest.exports.app_theme(),
      renderMode: guestRenderMode(guest),
      fontFamily: guestRenderMode(guest),
      dockVisible: Boolean(guest.exports.app_dock_visible()),
      fbHash: guest.fbHash,
      fbHashHex: fmtHex32(guest.fbHash),
    };
  },
  mount: (id, mounts) => {
    const guest = guests.find((candidate) => candidate.id === id) || guests[0];
    if (!guest) return false;
    mountCatalogIntoGuest(guest, mounts);
    guest.render();
    return true;
  },
  snapshot: (id) => {
    const guest = guests.find((candidate) => candidate.id === id) || guests[0];
    return captureSnapshot(guest, false);
  },
  runStep1: (id) => {
    const guest = guests.find((candidate) => candidate.id === id) || guests[0];
    if (!guest || !guest.clickLogical) return null;
    if (guest.exports.app_dock_visible() === 0) {
      guest.clickLogical(16, 16);
    }
    guest.clickLogical(76, 80);
    return captureSnapshot(guest, false);
  },
  startMatch: (opts) => startTwoPlayerMatch(opts),
  startCheckersMatch: (opts) => startCheckersMatch(opts),
  startChessMatch: (opts) => startChessMatch(opts),
  createNetworkMatch: (game) => createNetworkMatch(game),
  joinNetworkMatch: (matchId, seat) => joinNetworkMatch(matchId, seat),
  toggleFullscreen: (guest) => toggleFullscreen(guest),
  enterFullscreen: (guest) => enterFullscreen(guest),
  exitFullscreen: () => exitFullscreen(),
  isFullscreen: () => isFullscreen(),
  compareBuilds: () => startAbComparison(),
  setSplitView: (on) => setSplitView(on),
  snapshots: () => snapshots.slice(),
  splitSnapshot: (id1, id2, label) => {
    const g1 = (id1 != null ? guests.find((cand) => cand.id === id1) : null) || guests[0];
    const g2 = (id2 != null ? guests.find((cand) => cand.id === id2) : null) || guests[1];
    return captureSplitSnapshot(g1, g2, label);
  },
  add: (opts) => createGuest(opts),
  autoPilot: {
    runMission: (id, opts) => autoPilotRunner.runMission(id, opts),
    pause: () => autoPilotRunner.pause(),
    resume: () => autoPilotRunner.resume(),
    step: () => autoPilotRunner.step(),
    cancel: () => autoPilotRunner.cancel(),
    listMissions: () => autoPilotRunner.listMissions(),
    getReport: () => autoPilotRunner.getReport(),
    inspectState: () => autoPilotRunner.inspectState(),
  },
  trajectory: {
    startRecord: (guestId) => {
      const g = (guestId != null ? guests.find((cand) => cand.id === guestId) : null) || guests.find((cand) => cand.id === activeId) || guests[0];
      if (!g) throw new Error('startRecord: guest not found');
      trajectoryRecorder.start(g);
      updateRecordButtonUI(true);
      return { recording: true, guestId: g.id };
    },
    stopRecord: () => {
      const data = trajectoryRecorder.stop();
      updateRecordButtonUI(false);
      lastTrajectoryData = data;
      trajectoryPlayer.load(data);
      updateTrajectoryDrawerUI();
      return data;
    },
    download: (filename) => {
      const data = lastTrajectoryData || trajectoryRecorder.exportData();
      return downloadTrajectoryFile(data, filename);
    },
    play: (speed = 1.0) => trajectoryPlayer.play(speed),
    pause: () => trajectoryPlayer.pause(),
    step: () => trajectoryPlayer.stepForward(),
    stop: () => trajectoryPlayer.stop(),
    load: (jsonOrData) => {
      const data = typeof jsonOrData === 'string' ? importTrajectoryJson(jsonOrData) : jsonOrData;
      lastTrajectoryData = data;
      trajectoryPlayer.load(data);
      updateTrajectoryDrawerUI();
      return data;
    },
    exportJson: () => exportTrajectoryJson(lastTrajectoryData || trajectoryRecorder.exportData()),
    getData: () => lastTrajectoryData || trajectoryRecorder.exportData(),
    convertToMission: (opts) => {
      const data = lastTrajectoryData || trajectoryRecorder.exportData();
      return trajectoryToAutoPilotMission(data, opts);
    },
  },
};

function openInspectorSection(sectionEl) {
  if (inspectorPane) {
    inspectorPane.classList.remove('collapsed');
  }
  if (toggleInspectorBtn) {
    toggleInspectorBtn.classList.add('active-toggle');
  }
  if (sectionEl) {
    sectionEl.hidden = false;
    sectionEl.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
  }
}

/* ------------------------------------------------------------------ launchpad & layout controls -- */

let sidebarPinned = true;
let inspectorPinned = true;

function showLaunchpad() {
  emptyState.hidden = true;
  renderLaunchpad(panelsEl);
}

configureLaunchpadDeps({
  onLaunchMission: (missionId) => {
    hideLaunchpad(panelsEl);
    autoPilotUi.runSelectedMission(missionId).catch((err) => console.error(err));
  },
  onLaunchAppliance: (kind, opts = {}) => {
    hideLaunchpad(panelsEl);
    if (kind === 'checkers') {
      launchCheckersInGuest().catch((err) => console.error(err));
    } else if (kind === 'chess') {
      launchChessInGuest().catch((err) => console.error(err));
    } else if (kind === 'mines') {
      startTwoPlayerMatch().catch((err) => console.error(err));
    } else {
      createGuest(opts).catch((err) => console.error(err));
    }
  },
});

if (launchpadNavBtn) {
  launchpadNavBtn.addEventListener('click', () => {
    showLaunchpad();
  });
}

function initLayoutControls() {
  // Sidebar toggle
  if (toggleSidebarBtn && sidebarEl) {
    toggleSidebarBtn.addEventListener('click', () => {
      const isCollapsed = sidebarEl.classList.toggle('collapsed');
      toggleSidebarBtn.classList.toggle('active-toggle', !isCollapsed);
    });
  }

  // Sidebar Peek Tab
  if (peekTabSidebar && sidebarEl) {
    peekTabSidebar.addEventListener('click', () => {
      sidebarEl.classList.remove('collapsed');
      if (toggleSidebarBtn) toggleSidebarBtn.classList.add('active-toggle');
    });
  }

  // Sidebar Pin
  if (pinSidebarBtn && sidebarEl) {
    pinSidebarBtn.addEventListener('click', () => {
      sidebarPinned = !sidebarPinned;
      pinSidebarBtn.classList.toggle('pinned', sidebarPinned);
      pinSidebarBtn.textContent = sidebarPinned ? '📌 Pin' : '📍 Unpinned';
    });
  }

  // Inspector toggle
  if (toggleInspectorBtn && inspectorPane) {
    toggleInspectorBtn.addEventListener('click', () => {
      const isCollapsed = inspectorPane.classList.toggle('collapsed');
      toggleInspectorBtn.classList.toggle('active-toggle', !isCollapsed);
    });
  }

  // Inspector Peek Tab
  if (peekTabInspector && inspectorPane) {
    peekTabInspector.addEventListener('click', () => {
      inspectorPane.classList.remove('collapsed');
      if (toggleInspectorBtn) toggleInspectorBtn.classList.add('active-toggle');
    });
  }

  // Inspector Pin
  if (pinInspectorBtn && inspectorPane) {
    pinInspectorBtn.addEventListener('click', () => {
      inspectorPinned = !inspectorPinned;
      pinInspectorBtn.classList.toggle('pinned', inspectorPinned);
      pinInspectorBtn.textContent = inspectorPinned ? '📌 Pin' : '📍 Unpinned';
    });
  }

  if (closeInspectorBtn && inspectorPane) {
    closeInspectorBtn.addEventListener('click', () => {
      inspectorPane.classList.add('collapsed');
      if (toggleInspectorBtn) {
        toggleInspectorBtn.classList.remove('active-toggle');
      }
    });
  }
}
initLayoutControls();

setStatus('booting Test Appliance Orchestration Console…', true);
loadRegistry()
  .then(() => {
    // Check URL parameters for match join: ?match=101&seat=2, ?mission=xxx, and ?fullscreen=1
    const params = new URLSearchParams(window.location.search);
    const matchParam = params.get('match');
    const missionParam = params.get('mission');
    const seatParam = parseInt(params.get('seat') || '2', 10);
    const fullscreenParam = params.get('fullscreen') === '1' || params.get('fs') === '1';
    const isMobile = window.innerWidth < 800;

    let initPromise;
    if (matchParam) {
      const matchId = parseInt(matchParam, 10);
      initPromise = joinNetworkMatch(matchId, seatParam);
    } else if (missionParam) {
      initPromise = autoPilotUi.runSelectedMission(missionParam);
    } else {
      // Default landing experience: Launchpad
      showLaunchpad();
      initPromise = Promise.resolve();
    }

    return initPromise.then(() => {
      if (fullscreenParam || (matchParam && isMobile)) {
        const guest = guests[0];
        if (guest) {
          enterFullscreen(guest);
        }
      }
    });
  })
  .catch((error) => {
    console.error(error);
    setStatus('failed to start: see console');
  });
