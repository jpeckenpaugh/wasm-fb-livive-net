# Wasm Framebuffer Media Browser — Concept

## Purpose

Build a minimal graphical application in C, compile it to WebAssembly (Wasm), and run it inside an ordinary web browser. The application will own its UI, image-selection logic, and RGBA framebuffer. A small HTML/JavaScript host will instantiate the module, forward input, and display its framebuffer on a Canvas.

This is an intentionally narrow proof of concept for a possible future NetSurf-to-Wasm port. It tests the host/guest boundary without introducing HTML parsing, CSS, page JavaScript, networking, or an operating-system emulator.

## Goals

1. Compile a small native C application to Wasm with Emscripten.
2. Render the application's entire interface into C-owned linear memory.
3. Display that framebuffer using an HTML Canvas, without recreating the UI in HTML.
4. Forward pointer clicks and basic keyboard input from JavaScript to C.
5. Bundle five images and allow the user to select which image is displayed.
6. Demonstrate two independent instances of the same Wasm module, each with separate state and Canvas output.
7. Keep the build and runtime simple enough to inspect and modify easily.

## Non-goals

- Android emulation or an operating-system image.
- Running NetSurf in the first milestone.
- HTML/CSS parsing, DOM, or JavaScript execution inside the guest.
- HTTP fetching or loading arbitrary remote images.
- Video, audio, animations, persistent storage, or complex window management.
- A general-purpose graphics framework or plugin architecture.

## User experience

The host page initially displays one media-browser instance in a Canvas. Inside the Canvas, the C application draws a main image area and a strip of five selectable thumbnails. Clicking a thumbnail changes the main image and updates the selection indicator. Optional left/right keyboard controls cycle through images.

The host page may include a minimal control to create a second instance. Both instances must respond independently; selecting an image in one must not alter the other.

## Architecture

```text
Browser host (HTML + JavaScript)
  ├── Instance manager
  │    ├── Wasm instance A ── C app state A ── RGBA framebuffer A
  │    └── Wasm instance B ── C app state B ── RGBA framebuffer B
  ├── Input bridge: pointer / keyboard → exported C functions
  └── Display bridge: framebuffer bytes → ImageData → Canvas
```

**Guest responsibilities (C/Wasm):** own image assets, selection state, hit testing, layout, drawing primitives, and framebuffer pixels.

**Host responsibilities (HTML/JS):** load and instantiate Wasm, allocate/connect Canvas surfaces, translate pointer coordinates, forward input, request rendering, copy pixel data, and dispose of instances.

Each Wasm instance should have separate linear memory and application state. Start on the main thread for simplicity; Web Workers can be investigated later if rendering becomes expensive. The browser remains responsible for Wasm execution and memory safety at the sandbox boundary.

## Framebuffer contract

- Initial logical resolution: **640 × 480** pixels.
- Pixel format: **RGBA8888**, 8 bits per channel, contiguous row-major bytes.
- Buffer size: `width * height * 4` = **1,228,800 bytes** at 640 × 480.
- The C application allocates and owns the framebuffer.
- JavaScript reads the pointer and dimensions from exported functions, obtains a view of Wasm memory, and copies pixels into `ImageData` for Canvas rendering.
- The framebuffer pointer is only valid while the underlying Wasm memory allocation remains valid. If memory grows, JavaScript must refresh its typed-array view.
- Use a fixed logical resolution initially; CSS can scale the Canvas visually. Pointer coordinates must be mapped back into logical framebuffer coordinates.

### Illustrative guest API

```c
#include <stdint.h>

int app_init(int width, int height);
void app_click(int x, int y);
void app_key(int key);
void app_render(void);
uint8_t *app_framebuffer(void);
int app_width(void);
int app_height(void);
void app_destroy(void);
```

This API is illustrative, not a finalized ABI. For multiple instances, create separate Wasm instantiations rather than attempting to manage several application contexts inside one C module.

## Assets and rendering

Bundle exactly five sample images with the build. In the first iteration, preprocess source images into fixed-size raw RGBA arrays or files, with matching precomputed thumbnails. Embed or package these bytes with the Wasm application; the host must not draw or select the images itself.

This avoids needing a PNG/JPEG decoder in the guest. Use simple C drawing routines for rectangles, image blits, nearest-neighbor scaling if necessary, thumbnail borders, and a selected-image indicator. Avoid font rendering unless it proves necessary for basic usability.

A later experiment may add image decoding, dynamic assets, and a font rasterizer.

## Build and local execution

- Language: C (C11 is sufficient).
- Compiler/toolchain: Emscripten (`emcc`).
- Host: plain HTML, Canvas 2D, and JavaScript.
- No SDL, GTK, X11, or browser UI framework in the first iteration.
- Build artifacts: Wasm module and minimal Emscripten JavaScript glue, plus host files.
- Serve through a local static HTTP server; do not depend on opening `index.html` directly from `file://`.

An initial repository layout:

```text
wasm-media-browser/
├── src/
│   ├── main.c             # Lifecycle and exported interface
│   ├── framebuffer.c      # Pixel drawing / blitting
│   ├── framebuffer.h
│   ├── media.c            # Assets and selection logic
│   └── media.h
├── assets/
│   └── ...                # Five source images
├── tools/
│   └── ...                # Optional image preprocessing
├── web/
│   ├── index.html
│   └── host.js
├── Makefile
└── README.md
```

## Implementation milestones

### M1 — C framebuffer baseline

Create a C app that allocates a 640 × 480 RGBA framebuffer and draws solid rectangles and simple UI regions. Compile to Wasm, instantiate it from JavaScript, and show the framebuffer on Canvas.

**Pass:** The visible Canvas pixels originate from C-owned Wasm memory, not JavaScript drawing commands.

### M2 — Bundled image gallery

Preprocess and bundle five images. Draw one large image and five thumbnails, with a visible selection indicator.

**Pass:** The initial image and thumbnails render correctly with no network requests for image content.

### M3 — Host-to-guest input

Forward pointer clicks, implement thumbnail hit testing in C, and rerender on state changes. Add optional keyboard navigation.

**Pass:** Selecting any thumbnail updates the displayed image; no gallery selection logic lives in JavaScript.

### M4 — Independent instances

Create two separate Wasm instances and Canvas elements from the same build. Give each instance its own memory and application state.

**Pass:** Both galleries work simultaneously and independently; destroying one does not break the other.

### M5 — Cleanup and evaluation

Measure module size, initial load time, framebuffer-copy cost, memory footprint, and interaction responsiveness. Document toolchain requirements and known limitations.

**Pass:** A fresh checkout can be built and served locally with documented commands, and both instances operate without console errors.

## Key design decisions

**Raw framebuffer over SDL:** This deliberately exercises the host/guest rendering boundary and minimizes external dependencies. SDL remains a future alternative, not an initial requirement.

**Event-driven rendering:** Render on initialization and user interaction instead of maintaining a continuously running native event loop.

**Predecoded assets:** Avoid adding image codecs until the basic Wasm framebuffer is working.

**Instance-per-sandbox:** Multiple independent Wasm instantiations are simpler than refactoring the C program for multiple in-module contexts.

**Minimal host:** JavaScript is a transport and lifecycle layer, not the media-browser UI implementation.

## Risks and considerations

- **Wasm memory growth:** Typed-array views can become stale after memory growth; refresh them before framebuffer reads.
- **Pixel format mismatch:** Explicitly test RGBA channel ordering and alpha behavior with known color patterns.
- **Pointer mapping:** Canvas CSS scaling and device pixel ratio can cause incorrect click coordinates.
- **Asset size:** Raw RGBA images are larger than compressed files; use modest source dimensions and document total module/package size.
- **Export and initialization behavior:** Emscripten-generated glue and exported symbols must be configured consistently for independent instantiation.
- **Isolation limits:** Separate Wasm instances isolate their linear memories but still share the host page's JavaScript authority; host-side capabilities must be deliberately controlled.

## Relationship to a future NetSurf experiment

This project validates the execution and presentation boundary needed by a browser-engine port:

| This experiment | Possible NetSurf equivalent |
|---|---|
| C-owned RGBA framebuffer | NetSurf framebuffer plotting surface |
| Host pointer/key forwarding | Browser frontend input events |
| `app_render()` | Browser redraw integration |
| Bundled images | Browser resources and decoded images |
| Separate Wasm instances | Separate browser sessions |
| Host lifecycle management | Browser-instance creation and teardown |

It does **not** establish that NetSurf's source, dependencies, network stack, or event loop compile to Wasm. Those remain separate feasibility questions.

## Definition of done

A user opens a locally served web page, sees a media browser rendered entirely by C/Wasm, clicks among five bundled images, creates a second independent media browser, and can interact with both without page reloads, a native server process, or any Android/OS emulation.

The resulting codebase should remain intentionally small, understandable, and reusable as a foundation for subsequent graphical Wasm experiments.
