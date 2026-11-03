/* v3 Host Controller: Immutable Build Registry + Side-by-Side Split View +
 * Zero-Setup In-Browser FastAPI / SQLite Turn-Based Match Broker.
 */

import { FastApiSqliteBroker, updateRoomLink, initNetworkCopyHandler } from './broker.js';
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
import { MISSIONS } from './missions.js';
import {
  TrajectoryRecorder,
  TrajectoryPlayer,
  exportTrajectoryJson,
  downloadTrajectoryFile,
  importTrajectoryJson,
  trajectoryToAutoPilotMission,
} from './trajectory.js';

const DEFAULT_W = 1280;
const DEFAULT_H = 720;
const TITLE_H = 32;
const HASH_EVERY_N_FRAMES = 10;

/* Network mount catalog (`~/wallpapers/*`, `~/samples/*`, and `gh://`) served from `/` */
const DEFAULT_MOUNTS = [
  { name: 'wallpapers/01_aurora.png',  url: './assets/01_aurora.png',  bytes: 493568, kind: 'image' },
  { name: 'wallpapers/02_rings.png',   url: './assets/02_rings.png',   bytes: 389120, kind: 'image' },
  { name: 'wallpapers/03_checker.png', url: './assets/03_checker.png', bytes: 90112,  kind: 'image' },
  { name: 'wallpapers/04_dunes.png',   url: './assets/04_dunes.png',   bytes: 11264,  kind: 'image' },
  { name: 'wallpapers/05_orbits.png',  url: './assets/05_orbits.png',  bytes: 158720, kind: 'image' },
  { name: 'samples/app.js',            url: './demo/app.js',           bytes: 1045,   kind: 'code' },
  { name: 'samples/data.json',         url: './demo/data.json',        bytes: 527,    kind: 'code' },
  { name: 'samples/entangled.md',      url: './demo/entangled.md',     bytes: 9860,   kind: 'code', contextUrl: './demo/.context.xml' },
  { name: 'samples/feed.xml',          url: './demo/feed.xml',         bytes: 1051,   kind: 'code' },
  { name: 'samples/hello.c',           url: './demo/hello.c',          bytes: 755,    kind: 'code' },
  { name: 'samples/page.html',         url: './demo/page.html',        bytes: 851,    kind: 'code' },
  { name: 'samples/script.py',         url: './demo/script.py',        bytes: 935,    kind: 'code' },
  { name: 'samples/style.css',         url: './demo/style.css',        bytes: 655,    kind: 'code' },
  { name: 'samples/types.ts',          url: './demo/types.ts',         bytes: 700,    kind: 'code' },
  { name: 'gh/jpeckenpaugh/mines-sm/lib.rs', url: './gh/mines-sm/lib.rs', bytes: 12638, kind: 'code', contextUrl: './gh/mines-sm/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-sm/Cargo.toml', url: './gh/mines-sm/Cargo.toml', bytes: 229, kind: 'code', contextUrl: './gh/mines-sm/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-sm/README.md', url: './gh/mines-sm/README.md', bytes: 2599, kind: 'code', contextUrl: './gh/mines-sm/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/README.md', url: './gh/mines-livive-net/README.md', bytes: 2394, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/index.html', url: './gh/mines-livive-net/index.html', bytes: 12877, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/src/app.js', url: './gh/mines-livive-net/src/app.js', bytes: 35616, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/src/contract.js', url: './gh/mines-livive-net/src/contract.js', bytes: 2346, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/src/engine.js', url: './gh/mines-livive-net/src/engine.js', bytes: 7728, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/src/featurize.js', url: './gh/mines-livive-net/src/featurize.js', bytes: 2622, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/src/policy.js', url: './gh/mines-livive-net/src/policy.js', bytes: 5657, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/tools/train.py', url: './gh/mines-livive-net/tools/train.py', bytes: 14560, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/tools/eval.py', url: './gh/mines-livive-net/tools/eval.py', bytes: 11962, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/models/manifest.json', url: './gh/mines-livive-net/models/manifest.json', bytes: 9300, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/mines-livive-net/docs/checkpoints.md', url: './gh/mines-livive-net/docs/checkpoints.md', bytes: 10602, kind: 'code', contextUrl: './gh/mines-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/README.md', url: './gh/gm-livive-net/README.md', bytes: 10784, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/docs/ARCHITECTURE.md', url: './gh/gm-livive-net/docs/ARCHITECTURE.md', bytes: 17121, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/claim_net.gd', url: './gh/gm-livive-net/claim_net.gd', bytes: 15018, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/dodecahedron_planet.gd', url: './gh/gm-livive-net/dodecahedron_planet.gd', bytes: 9764, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/face_camera_controller.gd', url: './gh/gm-livive-net/face_camera_controller.gd', bytes: 2360, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/minesweeper_3d.gd', url: './gh/gm-livive-net/minesweeper_3d.gd', bytes: 17425, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/minesweeper_tile_3d.gd', url: './gh/gm-livive-net/minesweeper_tile_3d.gd', bytes: 5950, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/player.gd', url: './gh/gm-livive-net/player.gd', bytes: 5666, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/project.godot', url: './gh/gm-livive-net/project.godot', bytes: 3617, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/gm-livive-net/tests/run_tests.gd', url: './gh/gm-livive-net/tests/run_tests.gd', bytes: 2367, kind: 'code', contextUrl: './gh/gm-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chk-livive-net/README.md', url: './gh/chk-livive-net/README.md', bytes: 1520, kind: 'code', contextUrl: './gh/chk-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chk-livive-net/index.html', url: './gh/chk-livive-net/index.html', bytes: 39194, kind: 'code', contextUrl: './gh/chk-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chk-livive-net/src/state.js', url: './gh/chk-livive-net/src/state.js', bytes: 4942, kind: 'code', contextUrl: './gh/chk-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chk-livive-net/src/rules.js', url: './gh/chk-livive-net/src/rules.js', bytes: 11366, kind: 'code', contextUrl: './gh/chk-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chk-livive-net/src/ai.js', url: './gh/chk-livive-net/src/ai.js', bytes: 13235, kind: 'code', contextUrl: './gh/chk-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chk-livive-net/src/controller.js', url: './gh/chk-livive-net/src/controller.js', bytes: 53707, kind: 'code', contextUrl: './gh/chk-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chk-livive-net/src/renderer.js', url: './gh/chk-livive-net/src/renderer.js', bytes: 25476, kind: 'code', contextUrl: './gh/chk-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chk-livive-net/src/godot_adapter.js', url: './gh/chk-livive-net/src/godot_adapter.js', bytes: 20395, kind: 'code', contextUrl: './gh/chk-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chk-livive-net/docs/godot_plan.md', url: './gh/chk-livive-net/docs/godot_plan.md', bytes: 8762, kind: 'code', contextUrl: './gh/chk-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chs-livive-net/README.md', url: './gh/chs-livive-net/README.md', bytes: 1316, kind: 'code', contextUrl: './gh/chs-livive-net/.context.xml' },
  { name: 'gh/jpeckenpaugh/chs-livive-net/index.html', url: './gh/chs-livive-net/index.html', bytes: 26202, kind: 'code', contextUrl: './gh/chs-livive-net/.context.xml' },
];

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
  if (!cfg || !cfg.captureEndpoint) throw new Error('capture service is not configured');
  const viewport = { width, height: Math.max(1, height - TITLE_H), dpr: 1 };
  const key = `${site.id}@${viewport.width}x${viewport.height}`;
  const maxAge = (site.maxAgeMinutes || 15) * 60_000;
  const hit = webCaptureCache.get(key);
  if (!opts.force && hit?.snapshot && Date.now() - hit.at < maxAge) return hit.snapshot;
  if (hit?.promise) return hit.promise;

  const promise = (async () => {
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

/* Flat key namespace mirrored by `v2/src/input.rs::Key`. */
const KEY_CODES = {
  ArrowLeft: 1,
  ArrowRight: 2,
  ' ': 3,
  '0': 10, '1': 11, '2': 12, '3': 13, '4': 14,
  '5': 15, '6': 16, '7': 17, '8': 18, '9': 19,
  '+': 20, '-': 21, '*': 22, 'x': 22, 'X': 22, '/': 23,
  '=': 24, Enter: 24,
  Escape: 25,
  Backspace: 26,
  '.': 27, ',': 27,
  f: 30, F: 30,
  n: 31, N: 31,
  r: 32, R: 32,
  Tab: 33,
  PageUp: 34, '[': 34,
  PageDown: 35, ']': 35,
  '{': 36, '<': 36,
  '}': 37, '>': 37,
  k: 39, K: 39,
  j: 38, J: 38,
  i: 40, I: 40,
  o: 63, O: 63,
  d: 64, D: 64,
  Home: 41,
  End: 42,
  c: 43, C: 43,
  m: 44, M: 44,
  ArrowUp: 50,
  ArrowDown: 51,
  F9: 60,
  F10: 61,
  F5: 62,
};

const MODS = { Shift: 1, Ctrl: 2, Alt: 4, Meta: 8 };

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
      <span class="vm-status-tag"><span class="dot running"></span> RUNNING</span>
      <span class="vm-id-badge">${vm.name}</span>
      <span class="actor-badge human" data-stat="actor-badge">👤 Human</span>
      <span class="vm-build-label">${gameEngines} (<a class="commit-link" href="${b.githubUrl}" target="_blank" rel="noopener noreferrer" title="${b.notes}">↗ ${b.gitCommit}</a>)</span>
    </div>
    <div class="vm-header-right">
      <span class="seat-pill" data-stat="seat-pill">Solo · ${vm.width}×${vm.height}</span>
      <button class="vm-action-btn" data-action="fullscreen" type="button" title="Full Screen (Zen overlay)">⤢ Fullscreen</button>
      <button class="vm-action-btn" data-action="snap" type="button" title="Capture Framebuffer Snapshot">📷 Snapshot</button>
      <button class="vm-action-btn btn-danger" data-action="destroy" type="button" title="Terminate / Power Off VM">✕ Terminate</button>
    </div>
  </header>
  <div class="screen"><canvas tabindex="0" width="${vm.width}" height="${vm.height}"></canvas></div>
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
    if (guest.actorType === 'minimax') return { cls: 'minimax', label: '🤖 Minimax' };
    return { cls: 'human', label: '👤 Human' };
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
  if (guests.length === 0) emptyState.hidden = false;
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
  const vm = {
    id,
    name: `vm-${String(id).padStart(2, '0')}`,
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

if (apRunBtn) {
  apRunBtn.addEventListener('click', () => {
    const missionId = apMissionSelect ? apMissionSelect.value : 'system-audit';
    const pacingMs = apPacingSelect ? parseInt(apPacingSelect.value, 10) : 400;
    let resolution = null;
    if (apResSelect && apResSelect.value !== 'default') {
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

    autoPilotRunner.runMission(missionId, { pacingMs, resolution }).catch((err) => {
      console.error('Auto Pilot mission failed:', err);
      updateAutoPilotControls('error');
    });
  });
}

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

/* ---------------- Trajectory Recording & Playback UI Wiring ---------------- */
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
  const targetId = data?.metadata?.guestId != null ? data.metadata.guestId : (activeId != null ? activeId : '—');

  if (trajActionsStat) trajActionsStat.textContent = count;
  if (trajDurationStat) trajDurationStat.textContent = `${duration}ms`;
  if (trajTargetStat) trajTargetStat.textContent = targetId;

  const recRes = data?.metadata?.recordedResolution || data?.metadata?.resolution;
  if (trajRecordedResStat) {
    trajRecordedResStat.textContent = recRes ? `${recRes.width}×${recRes.height}` : '—';
  }

  // Update mode pill
  const activeGuest = guests.find((cand) => cand.id === activeId) || guests[0];
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
      // Stop recording
      const data = trajectoryRecorder.stop();
      updateRecordButtonUI(false);
      lastTrajectoryData = data;
      trajectoryPlayer.load(data);
      updateTrajectoryDrawerUI();
      if (trajectoryDrawer) {
        openInspectorSection(trajectoryDrawer);
      }
      setStatus(`Trajectory recording saved: ${data.events.length} actions captured.`, true);
    } else {
      // Start recording active guest
      const g = guests.find((cand) => cand.id === activeId) || guests[0];
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
      openInspectorSection(trajectoryDrawer);
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

    // Add option to apMissionSelect
    const apMissionSelect = document.getElementById('ap-mission-select');
    if (apMissionSelect) {
      const opt = document.createElement('option');
      opt.value = mission.id;
      opt.textContent = `[Trajectory] ${mission.name}`;
      opt.selected = true;
      apMissionSelect.appendChild(opt);
    }

    // Open Auto Pilot Panel
    const autoPilotPanel = document.getElementById('autopilot-panel');
    if (autoPilotPanel) {
      openInspectorSection(autoPilotPanel);
    }

    setStatus(`Converted trajectory to Auto Pilot mission "${mission.name}"!`, true);
  });
}

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

function initLayoutControls() {
  if (toggleSidebarBtn && sidebarEl) {
    toggleSidebarBtn.addEventListener('click', () => {
      const isCollapsed = sidebarEl.classList.toggle('collapsed');
      toggleSidebarBtn.classList.toggle('active-toggle', !isCollapsed);
    });
  }

  if (toggleInspectorBtn && inspectorPane) {
    toggleInspectorBtn.addEventListener('click', () => {
      const isCollapsed = inspectorPane.classList.toggle('collapsed');
      toggleInspectorBtn.classList.toggle('active-toggle', !isCollapsed);
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
    // Check URL parameters for match join: ?match=101&seat=2 and ?fullscreen=1
    const params = new URLSearchParams(window.location.search);
    const matchParam = params.get('match');
    const seatParam = parseInt(params.get('seat') || '2', 10);
    const fullscreenParam = params.get('fullscreen') === '1' || params.get('fs') === '1';
    const isMobile = window.innerWidth < 800;

    let initPromise;
    if (matchParam) {
      const matchId = parseInt(matchParam, 10);
      initPromise = joinNetworkMatch(matchId, seatParam);
    } else {
      initPromise = createGuest();
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
