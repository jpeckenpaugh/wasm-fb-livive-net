# Appliance v2: Minimalist `no_std` Rust Wasm Window Manager & Appliance

`v2/` represents the next-generation evolution of the `wasm-fb` project. While v1 established the host/guest framebuffer contract in C with compiled-in assets, v2 re-architects the guest as a **75 KiB `#![no_std]` Rust appliance and window manager** with zero baked-in assets, on-demand host staging, and built-in system attestation.

```
Browser Host (web/v2/host.js)
  ├── PNG / XML / Text Upstream Decoder (DOMParser, ImageDecoder, Web Speech API)
  ├── Staging Bridge (RGBA8888, UTF-8 text, KEY=VALUE\n context wire buffer)
  └── Single-threaded Rust Wasm Guest (v2/src/lib.rs)
        ├── Dynamic linear memory paging (__heap_base -> FB + Staging)
        ├── 32px Title Bar + Left Overlay App Dock (wm.rs)
        ├── Zero-allocation 2D Software Rasterizer & Font Engine (fb.rs, font.rs)
        └── Bundled Applications:
              ├── System (system.rs): FNV-1a build attestation, telemetry, test patterns
              ├── Calculator (calc.rs): Desk calculator with formatted numeric display
              ├── Minesweeper (mines.rs, mine_sm.rs): 8x8 bitboard state machine
              └── Files & Preview (files.rs, code.rs): Two-panel finder, code syntax highlighting,
                  enriched context sidecars, and TTS readback
```

---

## 1. Architectural Highlights

### 1.1 Pure `#![no_std]` on `wasm32-unknown-unknown`
- Zero external runtime dependencies, zero standard library (`no_std`), and panic = abort.
- Compiles down to **~75 KiB** of Wasm bytecode.
- Linear memory is dynamically grown via `memory_grow` above `__heap_base` in `app_init(width, height)` to support dynamic resolutions ranging from mobile (`360x800`) to 1080p (`1920x1080`).

### 1.2 Upstream Host Decoding vs. Zero Static Bloat
- In v1, the Wasm binary was ~17.8 MiB because raw RGBA pixels were compiled in.
- In v2, no images, source samples, or documentation strings are baked into the Wasm binary.
- The JS hypervisor (`web/v2/host.js`) decodes PNGs into raw RGBA8888, parses XML, and fetches UTF-8 source code, staging them into designated guest linear memory buffers only when selected.

### 1.3 Appliance Window Manager (`wm.rs`)
- **32px Title Bar**: Context menus (`SYSTEM`/`FILES`/`VIEW`/`TESTING`), status, session clock.
- **Left Overlay App Dock**: Vertical launcher with 1/10th scale preview tiles (`128x72`) that slide over the active surface without displacing client windows, auto-hiding on click.
- **Flat Maximized Surface**: Zero nested frame borders or redundant window chrome.

### 1.4 Deterministic Attestation (`build.rs`)
- At compile time, `v2/build.rs` computes an FNV-1a 64-bit digest across the entire `v2` source tree.
- Surfaced via the **System** app alongside target architecture, compiler version, and profile.

---

## 2. Bundled Applications

| Application | Modules | Description |
|---|---|---|
| **System** | `system.rs` | System overview, hardware/build attestation, performance telemetry, and test patterns. |
| **Calculator** | `calc.rs` | Interactive desk calculator with integer/decimal entry, thousands grouping, and keyboard entry. |
| **Minesweeper** | `mines.rs`, `mine_sm.rs` | High-performance 8×8 bitboard engine (`u64` bitmasks for mines, flags, reveals) with SplitMix64 PRNG, RL observation tensor exports (`[f32; 64]`), and Classic/Shielded strike rules. |
| **Files & Preview** | `files.rs`, `code.rs` | Two-panel explorer (Left 1/3 folder navigation, Right 2/3 live preview) supporting wallpapers, general code samples, task-scoped GitHub repo mounts (`gh://`), syntax highlighting, and TTS. |
