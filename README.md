# Test Appliance Orchestration Console

A WebAssembly hypervisor orchestration shell and deterministic framebuffer environment (historically evolved from the *Wasm Framebuffer Media Browser & Appliance* experiment). Every pixel is drawn into an RGBA8888 framebuffer in Wasm linear memory; the orchestration console forwards input, manages live multi-client matchmaking, facilitates side-by-side A/B build provenance comparisons, and blits raw bytes into HTML5 Canvas viewports.

The point is the host/guest boundary, not a browser: no DOM in the guest, no external UI frameworks, no fonts, no networking, no OS emulation.

```
Browser Host (HTML + JS)
  ├── instance manager            web/host.js & web/v2/host.js
  ├── wasm instance A ── state A ── framebuffer A ── canvas A
  ├── wasm instance B ── state B ── framebuffer B ── canvas B
  └── input / codec bridge: pointer / keys / PNG decoder / TTS
```

---

## Implementations

This repository serves three routes:

### 1. Appliance v3.1 (Root `/`) — Build Registry, Checkers, Chess & Mines 2P Broker, AutoPilot & Entangled Hypervisors
Served at `/` (`web/index.html`, `web/host.js`, `web/registry.json`):
- **Immutable Build Registry (`web/registry.json`)**: Select and instantiate historical and current `.wasm` appliance builds (`build-03` v3.1 Checkers + Chess + AI + 2P Broker, `build-02` v3.0 `aea4b4b` 2P Mines, `build-01` v2.0 `7ac8a67` 1P Baseline) per guest tab, with clickable GitHub commit provenance links (`↗ commit`).
- **Side-by-Side Split View (`⧉ split view` & `⇄ A/B builds`)**: Compare multiple guests or A/B test Build #3 (`v3.1`) against Build #2 (`aea4b4b`) side-by-side in one click.
- **AutoPilot & Deterministic Trajectory Engine (`✈️ Auto Pilot` & `⏺ Trajectory`)**: Autonomous flight plan runner (`web/autopilot.js`, `web/missions.js`) with 6 pre-configured mission scenarios (e.g. Checkers 2P Entangled Convergence, Minesweeper 2P Turn Relay, Chess Checkmate in 6 Moves), cross-VM semantic assertions (`assertHash`, `assertTurn`), dual-perspective composite split snapshots, and a live input recorder that converts interactive sessions into reusable JSON flight plans.
- **Actor Matrix HUD**: Real-time chassis badges indicating guest seat control: `[👤 Human]`, `[🤖 Minimax]`, and `[✈️ AutoPilot]`.
- **English Draughts (Checkers) + Alpha-Beta Minimax AI (`♟️ 2P checkers match`)**: Pure `#![no_std]` 32-square PDN draughts engine (`v2/src/checkers_sm.rs`, `v2/src/checkers.rs`) with mandatory jumps, multi-jump continuation, 2/4/6-ply alpha-beta Minimax + quiescence AI, Seat P2 perspective auto-flip, dynamic board scaling on high-DPI displays, and 2-guest match brokering.
- **Chess + Minimax AI (`♔ 2P chess match`)**: Pure `#![no_std]` 64-square chess engine (`v2/src/chess_sm.rs`, `v2/src/chess.rs`) with full legal move generation (castling, en passant, promotion), Minimax evaluation, UCI move relay, and perspective flipping.
- **2-Player Turn-Based Minesweeper (`🎮 2P mines match`)**: Wraps `mine_sm.rs` (`BoardState`) in a 2-player turn-based state machine with Wasm TX/RX mailboxes, sudden-death detonation, dynamic cell scaling up to 112px, and an in-browser FastAPI + SQLite match broker (`window.__broker` with live `matches` and `match_events` tables).
- **Peer-to-Peer Entangled Hypervisors & Live Trajectory Branching**: See [**docs/v3-entangled-hypervisors.md**](docs/v3-entangled-hypervisors.md) (and `~/samples/entangled.md` inside the `Files` app) for the architectural manifesto and LAN/WebRTC causal trajectory specification.

### 2. Appliance v2 (`/v2/`) — Frozen Rust `#![no_std]` Window Manager Baseline
Located in `v2/` and served at `/v2/`:
- **Size**: **~210 KiB `.wasm` binary** (zero baked-in assets; images and code are decoded and staged dynamically from the host).
- **Architecture**: Bare-metal `#![no_std]` Rust (`wasm32-unknown-unknown`), dynamic resolution scaling (360×800 up to 1080p+, with adaptive cell sizing up to 116px to prevent empty border waste), zero-allocation 2D software rasterizer, and a `FiveSeven` 5×7 bitmap font with Native and Smooth render modes at absolute scales (1× is the 5×7 base in both; Smooth is the default). Shared `theme`, `layout`, `ui`, `fmt`, and `input` modules keep colors, size classes, chrome, formatting, and key/result contracts in one place.
- **Theme page**: `System → Theme` selects one of ten built-in color themes; `System → Fonts` owns the global render mode and a selectable 1×–8× specimen ladder. See [docs/v2-themes.md](docs/v2-themes.md), [docs/v2-smooth.md](docs/v2-smooth.md), and [docs/glyphs.md](docs/glyphs.md).
- **Window Manager (`wm.rs`)**: 32px Title Bar with `Home` (`[≡]`) and active-app menus; the App Home drawer is the `Home` dropdown (containing the 4 app tiles plus `Test Pattern`). Mouse and keyboard share one focus model (`F9`/`Alt+D` Home drawer, `F10`/`Alt+M` menu bar, `Tab` client traversal, `Enter`/`Space` activation); see [docs/v2-input.md](docs/v2-input.md).
- **Applications**:
  - **System**: FNV-1a 64-bit build attestation (`build.rs`), performance telemetry, a Fonts page with a live 1×–8× specimen and style ladder, icon galleries, a theme picker, and test patterns.
  - **Calculator**: Interactive desk calculator with formatted separators.
  - **Minesweeper**: High-performance 8×8 bitboard engine with Classic/Shielded modes, 2-Player Turn-Based match wrapper, and RL observation tensors (`[f32; 64]`).
  - **Files & Preview**: Two-panel finder browsing `~/wallpapers`, `~/samples`, upstream GitHub repositories (`gh://jpeckenpaugh/mines-sm`, `gh://jpeckenpaugh/mines-livive-net`, `gh://jpeckenpaugh/gm-livive-net`), and **captured web snapshots (`web://` → `livive.net`, `mines.livive.net`, `mines-sm.livive.net`, `gm.livive.net`)** with raw source, post-load DOM, screenshot, context metadata, refresh, TTS, and **Open in Browser** live tabs for both `web://` and `gh://`. Includes **dynamic font scaling (`+`/`-`)**, **2D scrolling**, **enriched `.context.xml` knowledge graphs**, and **in-browser Text-to-Speech (TTS) narration**.
- See [docs/v2-appliance.md](docs/v2-appliance.md), [docs/context-and-tts.md](docs/context-and-tts.md), and [docs/web-capture.md](docs/web-capture.md) for full design specifications.

### 3. v1 Media Browser (`/v1/`, C99 + Emscripten)
Located in `src/` and served at `/v1/`:
- **Size**: ~17.8 MiB `.wasm` binary with compiled-in raw RGBA8888 wallpaper assets.
- **Navigation**: Horizontal bottom tile carousel (Calendar, Clock, Files, Preview, Calculator).
- **Design Notes**: See `docs/calc.md`, `docs/clock.md`, `docs/code.md`, `docs/fonts.md`, `docs/markdown.md`.

---

## Run it

### 1. Multiplayer Broker & Web Console (Recommended)

Run the combined static web host and live SSE / SQLite Match Broker:

```bash
make serve-relay
# open http://localhost:8080/
```

- Serves the unified web console at `http://localhost:8080/`.
- Hosts the FastAPI-style HTTP endpoints (`/api/matches`, `/api/matches/:id/moves`) and Server-Sent Events (`/api/matches/:id/events`) for cross-window / cross-device matchmaking.

### 2. Containerized Production Build

```bash
docker compose up --build serve   # http://localhost:8080
```

That builds the wasm module inside the pinned toolchain image (Emscripten
6.0.13 + Pillow 11.3.0) and serves `web/` with nginx. Nothing is written to the
host filesystem.

### 3. Optional Headless `web://` Capture Service

`web://` is a virtual folder over a local Playwright capture service; the appliance itself
still has no browser or network stack. Run the capture service alongside the web host:

```bash
# terminal 1: headless Chrome capture daemon
make web-capture   # http://127.0.0.1:8787/

# terminal 2: web console & match broker
make serve-relay   # http://localhost:8080/
```

Then open `Files → web://` and select `livive.net`, `mines.livive.net`,
`mines-sm.livive.net`, or `gm.livive.net`. Inside a site select `index.html`,
`rendered.html`, or `page.png`. `Enter` on `page.png` pins the full-screen
snapshot as the Desktop wallpaper. See [docs/web-capture.md](docs/web-capture.md).

The console boots the first guest automatically; **+ new guest** starts another.
Each guest is a tab with its own state, framebuffer and telemetry row; the top
bar shows host-level counters (module size, reserved memory, blits, uptime).

The tiles floating over the lower middle are the screen picker: five tiles.
The first is image 1 with the clock; the second is the calculator, drawn over
that slot's artwork; the rest are the remaining images. Arrow keys or a click
move between them.

## Mini apps

Tiles run left to right: **calendar, clock, files, preview, calculator**.

| Tile | What it shows |
|---|---|
| calendar | month grid, today highlighted; browse months/years; previews `OCT 29` |
| clock | photo + `HH:MM:SS` over `OCT 29, 2026 - THU`; `HH:MM` on its tile |
| files | `/` with `Wallpapers` (5 images), `demo` (8 source samples) and `ABOUT.md` |
| preview | full-bleed image, rendered `ABOUT.md`, or highlighted source code |
| calculator | keypad, integer and decimal entry with thousands separators |

All five live in the same guest and are reached with the existing
`app_click()`/`app_key()` path — no per-app exports. The clock repaints about
once a second while seconds are visible and once a minute otherwise; the
calculator repaints only on input.

See `docs/clock.md`, `docs/calc.md`, `docs/fonts.md`, `docs/markdown.md`, `docs/code.md` for the design notes.

## Develop

```bash
# compile + test in the dev stage, artifacts land in ./build
docker compose --profile dev run --rm dev make -j4 all test

# rebuild and serve the demo image
docker compose up --build serve
```

A native emsdk works too if you have one: `make && make test`.

## Publish

`https://wasm-fb.livive.net/` is served by GitHub Pages from
`.github/workflows/pages.yml` on every push to `main`. That workflow builds the
wasm module in the same pinned toolchain image, assembles `web/` plus
`build/media_browser.{mjs,wasm}` into an artifact, and deploys it. No build
artifacts are committed.

One-time repository setting: **Settings → Pages → Build and deployment → Source:
GitHub Actions**. (The custom domain and the `wasm-fb.livive.net →
jpeckenpaugh.github.io` DNS record are already in place; the workflow writes the
`CNAME` file into the artifact.)

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
pre-cropped to 1280×720 (hero) and 128×72 (tiles). The hero is the whole
canvas; the five tiles float over its lower middle rather than sitting in a
strip. Scaling exists but is only used if an asset does not match the layout.

## Guest API (`src/main.c`)

| Export | Purpose |
|---|---|
| `app_init(w, h)` | allocate the framebuffer, reset state, draw once |
| `app_render()` | redraw the current state |
| `app_click(x, y)` | logical coordinates; returns 1 if the screen changed |
| `app_key(k)` | 1 = left, 2 = right, 3 = next; calculator keys 10-27 (see `calc.h`); returns 1 if changed |
| `app_framebuffer()` | pointer to RGBA8888 bytes (JS reads via `HEAPU8`) |
| `app_width()` / `app_height()` | logical dimensions |
| `app_selected()` | current slot, `0..count` where `count` is the calculator |
| `app_count()` | number of bundled images |
| `app_set_test_pattern(on)` | M1 colour/orientation pattern |
| `app_destroy()` | free the buffer |

## Framebuffer contract

- 1280 × 720 logical pixels (720p), RGBA8888, row-major, 3,686,400 bytes.
- Memory order is R,G,B,A — on little-endian wasm a `uint32_t` word is
  `0xAABBGGRR`, which is what `RGBA(r,g,b,a)` builds. The test pattern is the
  tripwire for this: red must be top-left, green top-right, blue bottom-left.
- `ALLOW_MEMORY_GROWTH=0` with `INITIAL_MEMORY=64MB`, so `HEAPU8` never goes
  stale. The host re-reads it anyway.
- Canvas backing store is fixed at 1280×720; CSS scales it. Pointer mapping uses
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
| `media_browser.wasm` | 18,658,093 bytes (17.8 MiB, mostly the 17.8 MB of raw RGBA assets) |
| `media_browser.mjs` glue | small; ES module, no FS or Node shims |
| instance init (`app_init` → first pixels) | 1–2.4 ms |
| full redraw + `putImageData` copy | 0.6–1.0 ms per 1280×720 frame |
| per-frame copy | 3.52 MiB |
| reserved linear memory | 64 MiB (`ALLOW_MEMORY_GROWTH=0`) |
| cold container build (cached toolchain layer) | ~2 s of actual compilation |

Numbers come from the console telemetry (render / blit / present timings,
reserved linear memory), not a benchmark harness.

## Status against the concept milestones

| Milestone | State |
|---|---|
| M1 — C framebuffer baseline | done, incl. colour/orientation test pattern |
| M2 — bundled five-image gallery | done, zero network requests for image data |
| M3 — host-to-guest input | done, hit testing lives in C |
| M4 — independent instances | done, shared wasm binary / separate memories |
| M5 — cleanup and evaluation | console telemetry, `make test`, this README; module size printed at link |
| Extras | clock overlay (`docs/clock.md`), calculator screen (`docs/calc.md`) |

### Known limitations

- Tests are native pixel/logic tests only (`test_pixels`, `test_lcd`,
  `test_clock`, `test_calc`); there is no headless-browser assertion of the
  Canvas output yet.
- No font rendering: the "UI" is frames, borders and an accent underline.
- Raw RGBA assets: 5 × (1280·720 + 128·72) × 4 ≈ 17.8 MB inside the wasm binary.
  That is the price of a 720p framebuffer with decode-free assets; a decoder or
  compressed assets would cut it substantially.
  gzip does not help much on raw pixels.
- Emscripten 6.0.13 is pinned by tag. Pin the image digest for byte-for-byte
  rebuilds.
- `docker compose run` on Linux bind mounts writes root-owned files into
  `build/`; add `user: "${UID}:${GID}"` to the `dev` service there (the emsdk
  cache is world-writable, so non-root builds work).
