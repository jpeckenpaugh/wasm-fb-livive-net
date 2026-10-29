/* Host bridge: instantiate the wasm module, forward input, blit pixels.
 *
 * This file must never contain gallery logic. It knows only: logical size,
 * how to map a pointer into it, how to copy the framebuffer to a canvas, how
 * to create/destroy guests, and how to report the resulting telemetry.
 */
import createMediaBrowser from './build/media_browser.mjs';

const W = 640;
const H = 480;
const FB_BYTES = W * H * 4;

/* Keep in sync with KEY_* in src/main.c and CALC_KEY_* in src/calc.h. */
const KEY_CODES = {
  ArrowLeft: 1,
  ArrowRight: 2,
  ' ': 3,
  '0': 10, '1': 11, '2': 12, '3': 13, '4': 14,
  '5': 15, '6': 16, '7': 17, '8': 18, '9': 19,
  '+': 20, '-': 21, '*': 22, 'x': 22, 'X': 22, '/': 23,
  '=': 24, 'Enter': 24,
  'Escape': 25, 'c': 25, 'C': 25,
  'Backspace': 26,
  '.': 27, ',': 27,
};

/* Slot 1 is the calculator (see MEDIA_CALC_SLOT in src/media.h); the rest map
 * one-to-one to images. */
const CALC_SLOT = 1;

const selectionLabel = (guest) => (guest.selected === CALC_SLOT
  ? 'calculator'
  : `image ${guest.selected + 1} / ${guest.count}`);

const KiB = 1024;
const MiB = 1024 * KiB;

const tabsEl = document.getElementById('tabs');
const panelsEl = document.getElementById('panels');
const newVmButton = document.getElementById('new-vm');
const patternToggle = document.getElementById('pattern');
const statusLeft = document.getElementById('sb-left');
const statusCopies = document.getElementById('sb-copies');
const hostModule = document.getElementById('h-module');
const hostGuests = document.getElementById('h-guests');
const hostMem = document.getElementById('h-mem');
const hostBlits = document.getElementById('h-frames');
const hostUptime = document.getElementById('h-uptime');

const guests = [];
let activeId = null;
let nextId = 1;
let totalBlits = 0;
let moduleBytes = 0;
const bootAt = performance.now();

/* One timer for the active guest: the clock overlay only needs to repaint the
 * guest you can actually see, and activation catches up immediately. */
const CLOCK_MIN_WAIT_MS = 16;
let clockTimer = null;

const fmtBytes = (n) => {
  if (!n) return '0 B';
  if (n >= MiB) return `${(n / MiB).toFixed(2)} MiB`;
  if (n >= KiB) return `${(n / KiB).toFixed(1)} KiB`;
  return `${n} B`;
};
const fmtMs = (n) => `${n.toFixed(2)} ms`;
const fmtDuration = (ms) => {
  const total = Math.floor(ms / 1000);
  const hh = String(Math.floor(total / 3600)).padStart(2, '0');
  const mm = String(Math.floor((total % 3600) / 60)).padStart(2, '0');
  const ss = String(total % 60).padStart(2, '0');
  return `${hh}:${mm}:${ss}`;
};

/* Compile the module once and share it between guests; each factory call still
 * gets its own linear memory and application state.
 *
 * `wasmBinary` is not accepted by Emscripten 6's trimmed INCOMING_MODULE_JS_API,
 * so we use the supported `instantiateWasm` hook instead. */
let wasmModulePromise = null;
function loadWasmModule() {
  if (!wasmModulePromise) {
    const url = new URL('./build/media_browser.wasm', import.meta.url);
    wasmModulePromise = fetch(url)
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
}

/* ------------------------------------------------------------- clock tick -- */

function stopClock() {
  if (clockTimer !== null) {
    clearTimeout(clockTimer);
    clockTimer = null;
  }
}

function scheduleClock() {
  stopClock();
  const guest = guests.find((candidate) => candidate.id === activeId);
  if (!guest || !guest.module) return;
  const wait = Math.max(CLOCK_MIN_WAIT_MS, guest.module._app_next_timeout_ms());
  clockTimer = setTimeout(pumpClock, Math.min(wait, 60000));
}

function pumpClock() {
  clockTimer = null;
  const guest = guests.find((candidate) => candidate.id === activeId);
  if (!guest || !guest.module || !guest.render) return;

  const now = new Date();
  guest.module._app_set_time(now.getHours(), now.getMinutes(),
                             now.getSeconds(), now.getMilliseconds());
  /* The guest draws into its framebuffer and tells us whether the visible
   * content changed; only then is a 1.17 MiB copy worth doing. */
  if (guest.module._app_tick()) guest.render();
  scheduleClock();
}

/* ---------------------------------------------------------------- panels -- */

const PANEL_TEMPLATE = `
  <div class="screen"><canvas tabindex="0" width="${W}" height="${H}"></canvas></div>
  <dl class="vm-stats">
    <div class="cell"><dt>state</dt><dd class="state" data-stat="state">running</dd></div>
    <div class="cell"><dt>boot</dt><dd data-stat="boot">—</dd></div>
    <div class="cell"><dt>render</dt><dd data-stat="render">—</dd></div>
    <div class="cell"><dt>blit</dt><dd data-stat="blit">—</dd></div>
    <div class="cell"><dt>present</dt><dd data-stat="present">—</dd></div>
    <div class="cell"><dt>blits</dt><dd data-stat="blits">0</dd></div>
    <div class="cell"><dt>selected</dt><dd data-stat="selected">—</dd></div>
    <div class="cell"><dt>guest mem</dt><dd data-stat="mem">—</dd></div>
    <div class="cell"><dt>framebuffer</dt><dd data-stat="fb">${fmtBytes(FB_BYTES)}</dd></div>
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
  tab.addEventListener('keydown', (event) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const index = guests.indexOf(vm);
    const step = event.key === 'ArrowRight' ? 1 : guests.length - 1;
    const next = guests[(index + step) % guests.length];
    activate(next.id);
    next.wrap.querySelector('.tab').focus();
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
  /* Catch up and restart the clock for whichever guest is visible now. */
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
  if (guest.id === activeId) stopClock(); /* never pump a freed module */
  guest.disposed = true;
  guests.splice(index, 1);

  if (guest.module) {
    try {
      guest.module._app_destroy();
    } catch (error) {
      console.error(error);
    }
    guest.module = null;
  }

  guest.wrap.remove();
  guest.panel.remove();

  if (activeId === guest.id) {
    const next = guests[Math.min(index, guests.length - 1)];
    activate(next ? next.id : null);
  }
  if (guests.length === 0) emptyState.hidden = false;
  updateHostStats();
  setStatus('ready · click a thumbnail to select', true);
}

/* ---------------------------------------------------------------- create -- */

async function createGuest() {
  const id = nextId++;
  const vm = {
    id,
    name: `vm-${String(id).padStart(2, '0')}`,
    module: null,
    frames: 0,
    boot: 0,
    heapBytes: 0,
    count: 0,
    selected: 0,
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

    /* instantiateWasm has no error channel: if linking fails, Emscripten's
     * factory promise never settles. Race it so the panel surfaces the error
     * instead of hanging blank forever. */
    let rejectInit;
    const initFailed = new Promise((_, reject) => { rejectInit = reject; });
    const startedAt = performance.now();
    const module = await Promise.race([
      createMediaBrowser({
        locateFile: (file) => new URL(`./build/${file}`, import.meta.url).href,
        instantiateWasm(imports, success) {
          WebAssembly.instantiate(wasmModule, imports).then(
            (instance) => success(instance),
            (error) => rejectInit(error),
          );
          return {}; /* exports are delivered asynchronously via success() */
        },
      }),
      initFailed,
    ]);

    if (vm.disposed) {
      module._app_destroy();
      return vm;
    }
    if (!module._app_init(W, H)) throw new Error('app_init() returned 0');

    vm.module = module;
    vm.boot = performance.now() - startedAt;
    vm.heapBytes = module.HEAPU8.length;
    vm.count = module._app_count();
    vm.selected = module._app_selected();

    const ctx = vm.canvas.getContext('2d', { alpha: false });
    const image = ctx.createImageData(W, H);
    const fbPtr = module._app_framebuffer();
    if (!fbPtr) throw new Error('app_framebuffer() returned null');

    const render = () => {
      if (vm.disposed || !vm.module) return;
      const t0 = performance.now();
      vm.module._app_render();
      const t1 = performance.now();
      image.data.set(vm.module.HEAPU8.subarray(fbPtr, fbPtr + FB_BYTES));
      const t2 = performance.now();
      ctx.putImageData(image, 0, 0);
      const t3 = performance.now();

      vm.last = { render: t1 - t0, blit: t2 - t1, present: t3 - t2 };
      vm.frames += 1;
      totalBlits += 1;
      vm.selected = vm.module._app_selected();
      updateGuestStats(vm);
      updateHostStats();
    };

    const clickLogical = (x, y) => {
      const changed = vm.module._app_click(x, y);
      if (changed) {
        render();
        setStatus(`${vm.name} · ${selectionLabel(vm)}`);
        /* Selecting image 1 switches the hero clock from minute to second
         * granularity, so the pending timer is no longer the right one. */
        scheduleClock();
      }
      return changed;
    };

    const setPattern = (enabled) => {
      vm.module._app_set_test_pattern(enabled ? 1 : 0);
      render();
    };

    vm.canvas.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      const rect = vm.canvas.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      /* CSS scaling only: the backing store is always W×H, so devicePixelRatio
       * must NOT be involved here. Measure before focusing: focus() may scroll
       * the page and move the rect underneath us. */
      const x = Math.floor(((event.clientX - rect.left) * W) / rect.width);
      const y = Math.floor(((event.clientY - rect.top) * H) / rect.height);
      vm.canvas.focus({ preventScroll: true });
      clickLogical(x, y);
    });

    vm.canvas.addEventListener('keydown', (event) => {
      const code = KEY_CODES[event.key];
      if (code == null) return;
      event.preventDefault();
      if (vm.module._app_key(code)) {
        render();
        setStatus(`${vm.name} · ${selectionLabel(vm)}`);
        scheduleClock();
      }
    });

    vm.render = render;
    vm.clickLogical = clickLogical;
    vm.setPattern = setPattern;

    render();
    if (patternToggle.checked) setPattern(true);
    setStatus(`${vm.name} · booted in ${fmtMs(vm.boot)}`, true);
    pumpClock(); /* first clock paint + schedule the next tick */
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

/* Background tabs throttle timers; catch up as soon as we are visible. */
document.addEventListener('visibilitychange', () => {
  if (!document.hidden) pumpClock();
});

/* Small debugging/verification surface; harmless in production. */
window.__mediaBrowser = {
  list: () => guests.map((guest) => ({
    id: guest.id,
    selected: guest.selected,
    renders: guest.frames,
  })),
  click: (id, x, y) => {
    const guest = guests.find((candidate) => candidate.id === id);
    if (!guest || !guest.clickLogical) return null;
    guest.clickLogical(x, y);
    return guest.selected;
  },
  add: () => createGuest(),
};

setStatus('booting wasm module…', true);
createGuest().catch((error) => {
  console.error(error);
  setStatus('failed to start: see console');
});
