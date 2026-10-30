/* v2 Host Controller: Pure WebAssembly.instantiate() bridge (zero Emscripten glue).
 *
 * Responsibilities:
 * - Fetch and compile `build/appliance.wasm` once into a shared WebAssembly.Module.
 * - Instantiate isolated guests (`vm-01`, `vm-02`, ...), each with its own linear memory.
 * - Forward pointer clicks and keyboard events into the guest Window Manager.
 * - Capture pixel-exact RGBA8888 snapshots + FNV-1a hashes (via `Testing -> Capture Snapshot`,
 *   the host `📷 snapshot` button, `▶ run step 1`, or `window.__appliance.snapshot(id)`).
 */

const W = 1280;
const H = 720;
const FB_BYTES = W * H * 4;

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
};

const KiB = 1024;
const MiB = 1024 * KiB;

const tabsEl = document.getElementById('tabs');
const panelsEl = document.getElementById('panels');
const newVmButton = document.getElementById('new-vm');
const step1Button = document.getElementById('step1-btn');
const snapButton = document.getElementById('snap-btn');
const patternToggle = document.getElementById('pattern');
const statusLeft = document.getElementById('sb-left');
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

const PANEL_TEMPLATE = `
  <div class="screen"><canvas tabindex="0" width="${W}" height="${H}"></canvas></div>
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
      <span class="dot running"></span><span>${vm.name}</span>
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
    width: W,
    height: H,
    hash,
    hashHex,
    timestamp: new Date().toISOString(),
    dataUrl,
  };
  recordSnapshotCard(snap);
  setStatus(`${guest.name} · snapshot #${seq} captured (${hashHex})`, true);
  return snap;
}

async function createGuest() {
  const id = nextId++;
  const vm = {
    id,
    name: `vm-${String(id).padStart(2, '0')}`,
    exports: null,
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
  vm.panel.innerHTML = PANEL_TEMPLATE;
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
    if (!exports.app_init(W, H)) throw new Error('app_init() returned 0');

    vm.boot = performance.now() - startedAt;
    const ctx = vm.canvas.getContext('2d', { alpha: false });
    const image = ctx.createImageData(W, H);

    const blitToCanvas = (t0, t1) => {
      const fbPtr = exports.app_framebuffer();
      const memBytes = new Uint8Array(exports.memory.buffer);
      vm.heapBytes = memBytes.byteLength;
      image.data.set(memBytes.subarray(fbPtr, fbPtr + FB_BYTES));
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
        } else {
          setStatus(`${vm.name} · ${selectionLabel(vm)} · ${fmtHex32(vm.fbHash)}`, true);
        }
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
      const x = Math.floor(((event.clientX - rect.left) * W) / rect.width);
      const y = Math.floor(((event.clientY - rect.top) * H) / rect.height);
      vm.canvas.focus({ preventScroll: true });
      clickLogical(x, y);
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
      }
    });

    vm.render = render;
    vm.clickLogical = clickLogical;
    vm.setPattern = setPattern;

    render();
    if (patternToggle.checked) setPattern(true);
    setStatus(`${vm.name} · booted in ${fmtMs(vm.boot)} · click top-left dock button`, true);
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

/* Automated Step 1 Pipeline:
 * 1. Click top-left Dock button (16, 16) to open the Left App Dock
 * 2. Click the 128x72 About App Icon (86, 100) to launch About (auto-hides dock)
 * 3. Capture environment snapshot
 */
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
  add: () => createGuest(),
};

setStatus('booting v2 rust appliance…', true);
createGuest().catch((error) => {
  console.error(error);
  setStatus('failed to start: see console');
});
