# GitHub Code Context, Sidecars (`.context.xml`) & Web Speech TTS Bridge

This document details the architectural design and implementation of on-demand code mounts, enriched `.context.xml` knowledge graphs, interactive viewports, and the Web Speech API Text-to-Speech (TTS) bridge in the `v2` appliance.

---

## 1. On-Demand GitHub Repository Mounts (`gh://`)

The `Files` application structures storage into two main domains:
1. **`HOME (~/)`**:
   - `~/wallpapers`: 5 wallpaper images (`assets/*.png`), staged as decoded RGBA8888 pixels.
   - `~/samples`: 8 general web/application source files (`demo/*`).
2. **`GITHUB (gh://)`**:
   - `gh://mines-sm`: Task-scoped upstream code files (`lib.rs`, `Cargo.toml`, `README.md`) located in `gh/mines-sm/`.

No repository files are baked into `appliance.wasm`. When a file is selected, the JS hypervisor fetches the UTF-8 text and stages it into `app_mount_text_ptr()` in Wasm linear memory.

---

## 2. Enriched Context Sidecars (`.context.xml`)

Each directory (`assets/`, `demo/`, `gh/mines-sm/`) maintains a hidden `.context.xml` sidecar file containing pre-computed grounding metadata:

### 2.1 Schema
```xml
<context-index scope="gh://jpeckenpaugh/mines-sm" ref="main" generator="deepseek-v4.1-flash">
  <file name="lib.rs" runtime="Rust (#![no_std])" role="Core 8x8 Bitboard State Machine">
    <graph>
      <imports>core::ops, core::cmp (zero external crates)</imports>
      <exports>BoardState, Rules, Action, ActionType, StepResult</exports>
      <dependents>v2/src/mines.rs (Appliance GUI), mines.livive.net (RL Policy Harness)</dependents>
      <invariants>SplitMix64 deterministic PRNG; Safe-First-Click relocation; 64-bit u64 masks</invariants>
    </graph>
    <summary>...</summary>
    <architecture>...</architecture>
  </file>
</context-index>
```

### 2.2 Zero-Allocation Wire Protocol
The JS hypervisor decodes the XML via the browser's native `DOMParser`, serializes the matched `<file>` entry into a newline-delimited `KEY=VALUE\n` buffer, and copies it into `app_mount_context_ptr()` (max 4096 bytes). 

The `#![no_std]` Rust guest parses this buffer with zero dynamic memory allocation using string slice scanning, rendering:
- **Knowledge Graph Table**: `SCOPE`, `RUNTIME`, `ROLE`, `IMPORTS`, `EXPORTS`, `DEPENDENTS`, `INVARIANTS`
- **Technical Summary & Architectural Fit**: Word-wrapped summary paragraphs.

---

## 3. Preview Controls: Font Scaling & 2D Scrolling

The Right 2/3 Preview control bar (`y: 0..40`) provides interactive viewport adjustments:

1. **`[ RAW VIEW ]` / `[ CONTEXT ]`**: Toggle between raw content (image or syntax-highlighted code) and structured context breakdown.
2. **Font Scaling (`[ A- ]` / `[ A+ ]`)**:
   - Dynamically scales font between scale 1, 2, and 3.
   - Adjusts line heights (`10 * scale + 2`), character advance (`6 * scale`), visible lines, and word-wrapping boundaries.
   - Keyboard: `+` / `=` to increase, `-` to decrease.
3. **2D Scrolling**:
   - **Horizontal Panning**: `{` / `<` to scroll left 8 columns, `}` / `>` to scroll right 8 columns.
   - **Vertical Line Scrolling**: `J` / `K` or `[` / `]` to scroll lines; `PageUp` / `PageDown` to page chunks.

---

## 4. Text-To-Speech (TTS) Hypervisor Bridge

Inside the `Files` control bar, users can click **`[ ▶ TTS ]`**:
1. The guest marks `tts_active = true`, renders the button as `[ ■ STOP ]` in emerald green, and returns `rc = 3` (`CLICK_TTS`) from `app_click`.
2. The JS hypervisor catches `rc = 3`, reads the active context metadata (`ROLE`, `SUMMARY`, `ARCHITECTURE`) from Wasm linear memory, and synthesizes speech using the browser's native **Web Speech API (`window.speechSynthesis`)**.
3. Clicking `[ ■ STOP ]` immediately calls `window.speechSynthesis.cancel()`.
4. When the utterance finishes, `utterance.onend` calls `guest.exports.app_tts_set_active(0)` to reset the button state.
5. Users can navigate code, page chunks, change font sizes, or switch between `RAW` and `CONTEXT` while narration plays in the background.
