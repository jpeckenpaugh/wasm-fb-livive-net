/* Host bridge: instantiate the wasm module, forward input, blit pixels.
 *
 * This file must never contain gallery logic. It knows only: logical size,
 * how to map a pointer into it, how to copy the framebuffer to a canvas, and
 * how to create/destroy instances.
 */
import createMediaBrowser from './build/media_browser.mjs';

const W = 640;
const H = 480;
const FB_BYTES = W * H * 4;

/* Keep in sync with KEY_* in src/main.c. */
const KEY_CODES = { ArrowLeft: 1, ArrowRight: 2, ' ': 3, Enter: 3 };

const showStats = new URLSearchParams(location.search).has('stats');

const instancesEl = document.getElementById('instances');
const statusEl = document.getElementById('status');
const addButton = document.getElementById('add');
const patternToggle = document.getElementById('pattern');

/* Compile the module once and share it between instances; each factory call
 * still gets its own linear memory and application state.
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
      .then((bytes) => WebAssembly.compile(bytes));
  }
  return wasmModulePromise;
}

const instances = new Set();
let nextId = 1;

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text != null) node.textContent = text;
  return node;
}

async function createInstance() {
  const id = nextId++;

  const section = el('section', 'instance');
  const header = el('header');
  header.append(
    el('span', 'title', `Instance ${id}`),
    el('span', 'meta', `${W} × ${H} · RGBA8888`),
  );
  const closeButton = el('button', 'close', 'Close');
  closeButton.type = 'button';
  header.append(closeButton);

  const canvas = el('canvas');
  canvas.width = W;
  canvas.height = H;
  canvas.tabIndex = 0;

  const stats = el('div', 'stats');
  if (!showStats) stats.hidden = true;

  section.append(header, canvas, stats);
  instancesEl.append(section);

  const ctx = canvas.getContext('2d', { alpha: false });
  const image = ctx.createImageData(W, H);

  let module = null;
  try {
    const wasmModule = await loadWasmModule();
    const startedAt = performance.now();
    /* instantiateWasm has no error channel: if linking fails, Emscripten's
     * factory promise never settles. Race it so the panel surfaces the error
     * instead of hanging blank forever. */
    let rejectInit;
    const initFailed = new Promise((_, reject) => { rejectInit = reject; });
    module = await Promise.race([
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
    if (!module._app_init(W, H)) throw new Error('app_init() returned 0');
    const initMs = performance.now() - startedAt;

    const fbPtr = module._app_framebuffer();
    const heap = module.HEAPU8;
    if (!heap) throw new Error('HEAPU8 is not exported; check EXPORTED_RUNTIME_METHODS');

    let lastRenderMs = 0;
    let renders = 0;

    const render = () => {
      const started = performance.now();
      module._app_render();
      // No ALLOW_MEMORY_GROWTH in this build, so the view is stable; we still
      // re-read it every frame because that is the documented contract.
      image.data.set(module.HEAPU8.subarray(fbPtr, fbPtr + FB_BYTES));
      ctx.putImageData(image, 0, 0);
      lastRenderMs = performance.now() - started;
      renders++;
      if (showStats) {
        stats.textContent =
          `${lastRenderMs.toFixed(2)} ms/render · ` +
          `${(FB_BYTES / 1048576).toFixed(2)} MiB copy · ` +
          `heap ${(module.HEAPU8.length / 1048576).toFixed(1)} MiB · ` +
          `selected ${module._app_selected()} · init ${initMs.toFixed(1)} ms · ` +
          `${renders} render(s)`;
      }
    };

    const clickLogical = (x, y) => {
      const changed = module._app_click(x, y);
      if (changed) {
        render();
        updateStatus();
      }
      return changed;
    };

    const setTestPattern = (enabled) => {
      module._app_set_test_pattern(enabled ? 1 : 0);
      render();
    };

    canvas.addEventListener('pointerdown', (event) => {
      event.preventDefault();
      const rect = canvas.getBoundingClientRect();
      if (!rect.width || !rect.height) return;
      // CSS scaling only: the backing store is always W×H, so devicePixelRatio
      // must NOT be involved here. Measure before focusing: focus() may scroll
      // the page and move the rect underneath us.
      const x = Math.floor(((event.clientX - rect.left) * W) / rect.width);
      const y = Math.floor(((event.clientY - rect.top) * H) / rect.height);
      canvas.focus({ preventScroll: true });
      clickLogical(x, y);
    });

    canvas.addEventListener('keydown', (event) => {
      const code = KEY_CODES[event.key];
      if (code == null) return;
      event.preventDefault();
      if (module._app_key(code)) {
        render();
        updateStatus();
      }
    });

    render();

    const handle = {
      id,
      section,
      render,
      clickLogical,
      setTestPattern,
      selected: () => module._app_selected(),
      renders: () => renders,
      destroy() {
        instances.delete(handle);
        module._app_destroy();
        module = null;
        section.remove();
        updateStatus();
      },
    };

    closeButton.addEventListener('click', () => handle.destroy());
    instances.add(handle);
    if (patternToggle.checked) handle.setTestPattern(true);
    updateStatus();
    return handle;
  } catch (error) {
    console.error(error);
    section.querySelector('.title').textContent = `Instance ${id} — failed`;
    section.append(el('div', 'error', String(error && error.message ? error.message : error)));
    canvas.remove();
    stats.remove();
    return null;
  }
}

function updateStatus() {
  const selected = [...instances].map((i) => i.selected() + 1).join(', ');
  statusEl.textContent = instances.size === 0
    ? 'no instances'
    : `${instances.size} instance(s) · selected image: ${selected}`;
}

addButton.addEventListener('click', () => {
  createInstance().catch((error) => console.error(error));
});

patternToggle.addEventListener('change', () => {
  for (const instance of instances) instance.setTestPattern(patternToggle.checked);
});

/* Small debugging/verification surface; harmless in production. */
window.__mediaBrowser = {
  list: () => [...instances].map((i) => ({
    id: i.id,
    selected: i.selected(),
    renders: i.renders(),
  })),
  click: (id, x, y) => {
    const instance = [...instances].find((i) => i.id === id);
    if (!instance) return null;
    instance.clickLogical(x, y);
    return instance.selected();
  },
  add: () => createInstance(),
};

createInstance().catch((error) => {
  console.error(error);
  statusEl.textContent = 'failed to start: see console';
});
