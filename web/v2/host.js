/* v2 Host Controller: Pure WebAssembly.instantiate() bridge + CDN/RGBA8888 Upstream.
 *
 * Responsibilities:
 * - Fetch and compile `build/appliance.wasm` once into a shared WebAssembly.Module.
 * - Instantiate isolated guests (`vm-01`, `vm-02`, ...), each with its own linear memory.
 * - Act as the `~/` Network Share & Codec Upstream: mount `/assets/*.png` into each guest,
 *   and on demand fetch + decode PNGs using the browser's native image decoder into raw
 *   `1280 x 688` RGBA8888 arrays written directly into Wasm linear memory.
 * - Forward pointer clicks and keyboard events into the guest Window Manager.
 * - Capture pixel-exact RGBA8888 snapshots + FNV-1a hashes.
 */

const DEFAULT_W = 1280;
const DEFAULT_H = 720;
const TITLE_H = 32;

/* Network mount catalog (`~/wallpapers/*` and `~/samples/*`) served by the JS Hypervisor */
const DEFAULT_MOUNTS = [
  { name: 'wallpapers/01_aurora.png',  url: '../assets/01_aurora.png',  bytes: 493568, kind: 'image' },
  { name: 'wallpapers/02_rings.png',   url: '../assets/02_rings.png',   bytes: 389120, kind: 'image' },
  { name: 'wallpapers/03_checker.png', url: '../assets/03_checker.png', bytes: 90112,  kind: 'image' },
  { name: 'wallpapers/04_dunes.png',   url: '../assets/04_dunes.png',   bytes: 11264,  kind: 'image' },
  { name: 'wallpapers/05_orbits.png',  url: '../assets/05_orbits.png',  bytes: 158720, kind: 'image' },
  { name: 'samples/app.js',            url: '../demo/app.js',           bytes: 1045,   kind: 'code' },
  { name: 'samples/data.json',         url: '../demo/data.json',        bytes: 527,    kind: 'code' },
  { name: 'samples/feed.xml',          url: '../demo/feed.xml',         bytes: 1051,   kind: 'code' },
  { name: 'samples/hello.c',           url: '../demo/hello.c',          bytes: 755,    kind: 'code' },
  { name: 'samples/page.html',         url: '../demo/page.html',        bytes: 851,    kind: 'code' },
  { name: 'samples/script.py',         url: '../demo/script.py',        bytes: 935,    kind: 'code' },
  { name: 'samples/style.css',         url: '../demo/style.css',        bytes: 655,    kind: 'code' },
  { name: 'samples/types.ts',          url: '../demo/types.ts',         bytes: 700,    kind: 'code' },
  { name: 'github/lib.rs',             url: '../gh/mines-sm/lib.rs',        bytes: 12638, kind: 'code' },
  { name: 'github/Cargo.toml',         url: '../gh/mines-sm/Cargo.toml',    bytes: 229,   kind: 'code' },
  { name: 'github/README.md',          url: '../gh/mines-sm/README.md',     bytes: 2599,  kind: 'code' },
];

/* Host-level decoded RGBA8888, UTF-8 text, and parsed .context.xml caches shared across guests */
const decodedRgbaCache = new Map();
const fetchedTextCache = new Map();
const contextXmlCache = new Map();
const textEncoder = new TextEncoder();

const KEY_CODES = {
  ArrowLeft: 1,
  ArrowRight: 2,
  ' ': 3,
  '0': 10, '1': 11, '2': 12, '3': 13, '4': 14,
  '5': 15, '6': 16, '7': 17, '8': 18, '9': 19,
  '+': 20, '-': 21, '*': 22, 'x': 22, 'X': 22, '/': 23,
  '=': 24, Enter: 24,
  Escape: 25, c: 25, C: 25,
  Backspace: 26,
  '.': 27, ',': 27,
  f: 30, F: 30,
  n: 31, N: 31,
  r: 32, R: 32,
  Tab: 33, i: 33, I: 33,
  PageUp: 34, '[': 34,
  PageDown: 35, ']': 35,
  '{': 36, '<': 36,
  '}': 37, '>': 37,
  k: 39, K: 39,
  j: 38, J: 38,
  ArrowUp: 50,
  ArrowDown: 51,
};

const KiB = 1024;
const MiB = 1024 * KiB;

const tabsEl = document.getElementById('tabs');
const panelsEl = document.getElementById('panels');
const resSelect = document.getElementById('vm-res');
const newVmButton = document.getElementById('new-vm');
const step1Button = document.getElementById('step1-btn');
const snapButton = document.getElementById('snap-btn');
const patternToggle = document.getElementById('pattern');
const statusLeft = document.getElementById('sb-left');
const statusRes = document.getElementById('sb-res');
const statusCopies = document.getElementById('sb-copies');
const hostModule = document.getElementById('h-module');
const hostGuests = document.getElementById('h-guests');
const hostMem = document.getElementById('h-mem');
const hostBlits = document.getElementById('h-frames');
const hostSnaps = document.getElementById('h-snaps');
const hostUptime = document.getElementById('h-uptime');
const snapDrawer = document.getElementById('snapshots-drawer');
const snapList = document.getElementById('snap-list');
const snapCountEl = document.getElementById('snap-count');
const clearSnapsBtn = document.getElementById('clear-snaps');

const guests = [];
const snapshots = [];
let activeId = null;
let activeTtsUtterance = null;

function stopTts(guest) {
  if (typeof window !== 'undefined' && 'speechSynthesis' in window) {
    window.speechSynthesis.cancel();
  }
  activeTtsUtterance = null;
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
    // Stopped speaking
    stopTts(guest);
    return;
  }

  // Get active file idx and read context data from Wasm memory or cache
  const activePage = guest.exports.app_active_page ? guest.exports.app_active_page() : -1;
  if (activePage < 0 || !guest.mounts || activePage >= guest.mounts.length) {
    stopTts(guest);
    return;
  }

  const item = guest.mounts[activePage];
  const ctxPtr = guest.exports.app_mount_context_ptr ? guest.exports.app_mount_context_ptr() : 0;
  let textToSpeak = '';

  if (ctxPtr) {
    const mem = new Uint8Array(guest.exports.memory.buffer);
    const slice = mem.subarray(ctxPtr, ctxPtr + 4096);
    const zeroIdx = slice.indexOf(0);
    const len = zeroIdx >= 0 ? zeroIdx : 4096;
    const raw = new TextDecoder().decode(slice.subarray(0, len));
    
    // Parse key-value lines
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
    activeTtsUtterance = null;
  };

  utterance.onerror = (e) => {
    console.warn('TTS error:', e);
    if (guest.exports && guest.exports.app_tts_set_active) {
      guest.exports.app_tts_set_active(0);
      guest.render();
    }
    activeTtsUtterance = null;
  };

  activeTtsUtterance = utterance;
  window.speechSynthesis.speak(utterance);
  setStatus(`${guest.name} · TTS reading context for ${item.name}…`, true);
}
let nextId = 1;
let totalBlits = 0;
let moduleBytes = 0;
const bootAt = performance.now();
let clockTimer = null;

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

const selectionLabel = (guest) => {
  if (!guest || !guest.exports) return 'desktop';
  const sel = guest.exports.app_selected();
  if (sel === 1) {
    const page = guest.exports.app_active_page();
    const sec = page === 1 ? 'locale' : page === 2 ? 'fonts' : 'about';
    return `system (${sec})`;
  }
  if (sel === 2) return 'calc';
  if (sel === 3) return 'files';
  if (sel === 4) return 'mines';
  return 'desktop';
};

let wasmModulePromise = null;
function loadWasmModule() {
  if (!wasmModulePromise) {
    const url = new URL(`./build/appliance.wasm?v=${Date.now()}`, import.meta.url);
    wasmModulePromise = fetch(url, { cache: 'no-cache' })
      .then((response) => {
        if (!response.ok) throw new Error(`failed to fetch ${url}: HTTP ${response.status}`);
        return response.arrayBuffer();
      })
      .then((bytes) => {
        moduleBytes = bytes.byteLength;
        updateHostStats();
        return WebAssembly.compile(bytes);
      });
  }
  return wasmModulePromise;
}

/* Upstream CDN Fetch + Native Browser Image Decode -> RGBA8888 Uint8ClampedArray */
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

  // Center-crop (cover) the source image into targetW x targetH
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
    const contextUrlStr = `${dirUrl}/.context.xml`;
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
      Array.from(doc.querySelectorAll('file')).find(el => el.getAttribute('name') === fileName);
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
    setStatus(`${guest.name} · failed to stage ${item.name}`);
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
  const guest = guests.find((candidate) => candidate.id === activeId);
  if (!guest || !guest.exports || !guest.render) return;
  syncWallClock(guest);
  if (guest.exports.app_tick()) guest.render();
  const wait = 1000 - (Date.now() % 1000);
  clockTimer = setTimeout(pumpClock, Math.max(50, wait));
}

const buildPanelHtml = (w, h) => `
  <div class="screen"><canvas tabindex="0" width="${w}" height="${h}"></canvas></div>
  <dl class="vm-stats">
    <div class="cell"><dt>state</dt><dd class="state" data-stat="state">running</dd></div>
    <div class="cell"><dt>boot</dt><dd data-stat="boot">—</dd></div>
    <div class="cell"><dt>render</dt><dd data-stat="render">—</dd></div>
    <div class="cell"><dt>blit</dt><dd data-stat="blit">—</dd></div>
    <div class="cell"><dt>present</dt><dd data-stat="present">—</dd></div>
    <div class="cell"><dt>blits</dt><dd data-stat="blits">0</dd></div>
    <div class="cell"><dt>active view</dt><dd data-stat="selected">—</dd></div>
    <div class="cell"><dt>fb fnv1a-32</dt><dd data-stat="hash">—</dd></div>
    <div class="cell"><dt>guest mem</dt><dd data-stat="mem">—</dd></div>
  </dl>`;

function createTab(vm) {
  const wrap = document.createElement('div');
  wrap.className = 'tab-wrap';
  wrap.innerHTML = `
    <button class="tab" role="tab" type="button" id="tab-${vm.id}"
            aria-controls="panel-${vm.id}" aria-selected="false" tabindex="-1">
      <span class="dot running"></span><span>${vm.name} · ${vm.width}×${vm.height}</span>
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
    guest.panel.hidden = !on;
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

function updateGuestStats(guest) {
  statNode(guest, 'boot').textContent = fmtMs(guest.boot);
  statNode(guest, 'render').textContent = fmtMs(guest.last.render);
  statNode(guest, 'blit').textContent = fmtMs(guest.last.blit);
  statNode(guest, 'present').textContent = fmtMs(guest.last.present);
  statNode(guest, 'blits').textContent = guest.frames;
  statNode(guest, 'selected').textContent = selectionLabel(guest);
  statNode(guest, 'hash').textContent = fmtHex32(guest.fbHash);
  statNode(guest, 'mem').textContent = fmtBytes(guest.heapBytes);
}

function markFault(guest, message) {
  guest.faulted = true;
  const state = statNode(guest, 'state');
  state.textContent = 'fault';
  state.style.color = '#ff9b9b';
  guest.wrap.classList.remove('active');
  guest.wrap.querySelector('.dot').classList.remove('running');
  guest.panel.querySelector('.screen').innerHTML =
    `<div class="fault">instantiation failed<br><span>${message}</span></div>`;
  setStatus(`${guest.name} · fault`);
}

function destroyGuest(guest) {
  const index = guests.indexOf(guest);
  if (index < 0) return;
  if (guest.id === activeId) stopClock();
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
  snapDrawer.hidden = false;
  updateHostStats();

  const card = document.createElement('div');
  card.className = 'snap-card';
  card.innerHTML = `
    <img src="${snap.dataUrl}" alt="Snapshot ${snap.seq} from ${snap.vm}">
    <div class="snap-meta">
      <span>${snap.vm} · #${snap.seq} (${snap.view})</span>
      <b>${snap.hashHex}</b>
    </div>
    <a class="snap-dl" href="${snap.dataUrl}" download="${snap.vm}-snap-${snap.seq}-${snap.hashHex}.png">
      ↓ download png receipt
    </a>`;
  snapList.prepend(card);
}

function captureSnapshot(guest, alreadyRecordedInGuest = false) {
  if (!guest || !guest.exports || !guest.canvas) return null;
  if (!alreadyRecordedInGuest) {
    syncWallClock(guest);
    guest.exports.app_record_snapshot();
    guest.render();
  }
  const hash = guest.exports.app_fb_hash() >>> 0;
  const hashHex = fmtHex32(hash);
  const seq = guest.exports.app_snapshot_count() >>> 0;
  const view = selectionLabel(guest);
  const dataUrl = guest.canvas.toDataURL('image/png');

  const snap = {
    vm: guest.name,
    guestId: guest.id,
    seq,
    view,
    width: guest.width,
    height: guest.height,
    hash,
    hashHex,
    timestamp: new Date().toISOString(),
    dataUrl,
  };
  recordSnapshotCard(snap);
  setStatus(`${guest.name} · snapshot #${seq} captured (${hashHex})`, true);
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
  const fbBytes = width * height * 4;
  const id = nextId++;
  const vm = {
    id,
    name: `vm-${String(id).padStart(2, '0')}`,
    width,
    height,
    exports: null,
    mounts: [],
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
  vm.panel.innerHTML = buildPanelHtml(width, height);
  panelsEl.append(vm.panel);

  vm.canvas = vm.panel.querySelector('canvas');
  guests.push(vm);
  emptyState.hidden = true;
  activate(id);
  updateHostStats();

  try {
    const wasmModule = await loadWasmModule();
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

    // Mount the 5 network wallpapers and 8 code samples into `~/` at runtime
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
      vm.fbHash = exports.app_fb_hash() >>> 0;
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
      if (rc !== 0) {
        blitToCanvas(t0, t1);
        if (rc === 2) {
          captureSnapshot(vm, true);
        } else if (rc === 3) {
          handleTtsToggle(vm);
        } else {
          setStatus(`${vm.name} · ${selectionLabel(vm)} · ${fmtHex32(vm.fbHash)}`, true);
        }
        servicePendingMountFetch(vm);
      }
      return rc;
    };

    const setPattern = (enabled) => {
      exports.app_set_test_pattern(enabled ? 1 : 0);
      render();
    };

    vm.canvas.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      const rect = vm.canvas.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      const x = Math.floor(((event.clientX - rect.left) * width) / rect.width);
      const y = Math.floor(((event.clientY - rect.top) * height) / rect.height);
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

    vm.canvas.addEventListener('keydown', (event) => {
      const code = KEY_CODES[event.key];
      if (code == null) return;
      event.preventDefault();
      const t0 = performance.now();
      const rc = exports.app_key(code);
      const t1 = performance.now();
      if (rc !== 0) {
        blitToCanvas(t0, t1);
        setStatus(`${vm.name} · ${selectionLabel(vm)} · ${fmtHex32(vm.fbHash)}`, true);
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
      `${vm.name} (${width}×${height}) · booted in ${fmtMs(vm.boot)} · ${vm.mounts.length} files mounted in ~/`,
      true,
    );
    pumpClock();
    return vm;
  } catch (error) {
    console.error(error);
    if (!vm.disposed) markFault(vm, String(error && error.message ? error.message : error));
    return vm;
  }
}

/* ------------------------------------------------------------------ boot -- */

const emptyState = document.createElement('div');
emptyState.className = 'empty';
emptyState.textContent = 'no guests attached — press “+ new guest”';
panelsEl.append(emptyState);

newVmButton.addEventListener('click', () => {
  createGuest().catch((error) => console.error(error));
});

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

patternToggle.addEventListener('change', () => {
  for (const guest of guests) {
    if (guest.setPattern) guest.setPattern(patternToggle.checked);
  }
});

setInterval(() => {
  hostUptime.textContent = fmtDuration(performance.now() - bootAt);
}, 1000);
hostUptime.textContent = '00:00:00';
updateHostStats();

document.addEventListener('visibilitychange', () => {
  if (!document.hidden) pumpClock();
});

/* Programmatic Agent / QA Test Harness Surface (`window.__appliance`) */
window.__appliance = {
  list: () => guests.map((guest) => ({
    id: guest.id,
    name: guest.name,
    width: guest.width,
    height: guest.height,
    view: selectionLabel(guest),
    dockVisible: guest.exports ? Boolean(guest.exports.app_dock_visible()) : false,
    renders: guest.frames,
    fbHash: guest.fbHash,
    fbHashHex: fmtHex32(guest.fbHash),
  })),
  click: (id, x, y) => {
    const guest = guests.find((candidate) => candidate.id === id) || guests[0];
    if (!guest || !guest.clickLogical) return null;
    const rc = guest.clickLogical(x, y);
    return {
      rc,
      view: selectionLabel(guest),
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
  snapshots: () => snapshots.slice(),
  add: (opts) => createGuest(opts),
};

setStatus('booting v2 rust appliance…', true);
createGuest().catch((error) => {
  console.error(error);
  setStatus('failed to start: see console');
});
