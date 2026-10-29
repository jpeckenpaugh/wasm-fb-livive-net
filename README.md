# Wasm Framebuffer Media Browser

A C application compiled to WebAssembly that owns its entire UI. Every pixel
inside the panel is drawn by C into an RGBA8888 framebuffer in wasm linear
memory; the page only forwards input and copies bytes into a Canvas.

The point is the host/guest boundary, not a browser: no DOM in the guest, no
image decoder, no fonts, no networking, no OS emulation.

```
Browser host (HTML + JS)
  ├── instance manager            web/host.js
  ├── wasm instance A ── state A ── framebuffer A ── canvas A
  ├── wasm instance B ── state B ── framebuffer B ── canvas B
  └── input bridge: pointer / keys → _app_click / _app_key
```

## Run it

```bash
docker compose up --build serve   # http://localhost:8080
```

That builds the wasm module inside the pinned toolchain image (Emscripten
6.0.13 + Pillow 11.3.0) and serves `web/` with nginx. Nothing is written to the
host filesystem.

Append `?stats=1` for per-frame timings. The first panel starts automatically;
**Add instance** creates a fully independent second one.

## Develop

```bash
# compile + test in the dev stage, artifacts land in ./build
docker compose --profile dev run --rm dev make -j4 all test

# rebuild and serve the demo image
docker compose up --build serve
```

A native emsdk works too if you have one: `make && make test`.

## Test

```bash
docker compose --profile dev run --rm dev make test
```

`tests/test_pixels.c` runs under the host compiler. It asserts exact memory
byte order (`R,G,B,A`), clipping behaviour on all four edges, blit/scaled-blit
sampling, thumbnail hit testing incl. the gaps between thumbnails, and that the
drawn hero image is pixel-identical to the source asset. The wasm-only parts
(`src/main.c`) are deliberately excluded by keeping Emscripten headers out of
`framebuffer.c`/`media.c`/`ui.c`.

## Layout

```text
src/main.c         exported API + lifecycle, the only Emscripten consumer
src/framebuffer.c  clear / fill / border / blit / nearest-neighbour scale
src/media.c        asset table, selection, hit testing
src/ui.c           layout + drawing (incl. the M1 test pattern)
tools/             sample-asset generator (stdlib) + RGBA preprocessor (Pillow)
assets/            five source PNGs, committed
web/               index.html + host.js (transport and lifecycle only)
tests/             native pixel/logic tests
```

Geometry is chosen so no scaling ever happens for the hero image: assets are
pre-cropped to 480×360 (hero) and 96×72 (thumbnails), the hero sits at (80,12)
and the thumbnails at y=396 with 10px gaps. Scaling exists but is only used if
an asset does not match the layout.

## Guest API (`src/main.c`)

| Export | Purpose |
|---|---|
| `app_init(w, h)` | allocate the framebuffer, reset state, draw once |
| `app_render()` | redraw the current state |
| `app_click(x, y)` | logical coordinates; returns 1 if selection changed |
| `app_key(k)` | 1 = left, 2 = right, 3 = next; returns 1 if changed |
| `app_framebuffer()` | pointer to RGBA8888 bytes (JS reads via `HEAPU8`) |
| `app_width()` / `app_height()` | logical dimensions |
| `app_selected()` | current index (debug/stats) |
| `app_set_test_pattern(on)` | M1 colour/orientation pattern |
| `app_destroy()` | free the buffer |

## Framebuffer contract

- 640 × 480 logical pixels, RGBA8888, row-major, 1,228,800 bytes.
- Memory order is R,G,B,A — on little-endian wasm a `uint32_t` word is
  `0xAABBGGRR`, which is what `RGBA(r,g,b,a)` builds. The test pattern is the
  tripwire for this: red must be top-left, green top-right, blue bottom-left.
- `ALLOW_MEMORY_GROWTH=0` with `INITIAL_MEMORY=64MB`, so `HEAPU8` never goes
  stale. The host re-reads it anyway.
- Canvas backing store is fixed at 640×480; CSS scales it. Pointer mapping uses
  `getBoundingClientRect()` only — `devicePixelRatio` is intentionally not
  involved.

## Multiple instances

`-sMODULARIZE=1 -sEXPORT_ES6=1` makes the glue a factory: each
`createMediaBrowser()` call returns a distinct instance with its own linear
memory, static data and application state. The host fetches and compiles the
`.wasm` **once** into a `WebAssembly.Module` and hands it to every factory call
through the supported `instantiateWasm` hook, so download and compilation are
shared while memory stays separate.

(Note: `wasmBinary` is not in Emscripten 6's trimmed `INCOMING_MODULE_JS_API`,
hence `instantiateWasm` rather than passing the bytes directly.)

Teardown calls `app_destroy()`; Emscripten has no explicit instance
deallocation, so the module reference is dropped and the instance is reclaimed
by GC.

## Build flags that matter

| Flag | Why |
|---|---|
| `-sMODULARIZE=1 -sEXPORT_ES6=1` | factory function; this is what makes M4 possible |
| `-sALLOW_MEMORY_GROWTH=0` | removes the stale-view risk entirely |
| `-sENVIRONMENT=web -sFILESYSTEM=0` | no Node shims, no MEMFS; assets are compiled-in |
| `--no-entry` | no `main()`; JS owns the lifecycle |
| `-sEXPORTED_RUNTIME_METHODS=HEAPU8` | the pixel view |

## Measured on the reference run

| Metric | Value |
|---|---|
| `media_browser.wasm` | 3,604,248 bytes (3.44 MiB, mostly the 3.4 MB of raw RGBA assets) |
| `media_browser.mjs` glue | small; ES module, no FS or Node shims |
| instance init (`app_init` → first pixels) | 1–2.4 ms |
| full redraw + `putImageData` copy | 0.2–0.3 ms per 640×480 frame |
| per-frame copy | 1.17 MiB |
| reserved linear memory | 64 MiB (`ALLOW_MEMORY_GROWTH=0`) |
| cold container build (cached toolchain layer) | ~2 s of actual compilation |

Numbers come from the `?stats=1` panel, not a benchmark harness.

## Status against the concept milestones

| Milestone | State |
|---|---|
| M1 — C framebuffer baseline | done, incl. colour/orientation test pattern |
| M2 — bundled five-image gallery | done, zero network requests for image data |
| M3 — host-to-guest input | done, hit testing lives in C |
| M4 — independent instances | done, shared wasm binary / separate memories |
| M5 — cleanup and evaluation | `?stats=1` panel, `make test`, this README; module size printed at link |

### Known limitations

- Tests are native pixel/logic tests only; there is no headless-browser
  assertion of the Canvas output yet.
- No font rendering: the "UI" is frames, borders and an accent underline.
- Raw RGBA assets: 5 × (480·360 + 96·72) × 4 ≈ 3.6 MB inside the wasm binary.
  gzip does not help much on raw pixels.
- Emscripten 6.0.13 is pinned by tag. Pin the image digest for byte-for-byte
  rebuilds.
- `docker compose run` on Linux bind mounts writes root-owned files into
  `build/`; add `user: "${UID}:${GID}"` to the `dev` service there (the emsdk
  cache is world-writable, so non-root builds work).
