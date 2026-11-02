# ♟️ Godot 3D Web Client Integration Plan

## 📌 Project Overview & Objectives
Build an alternate, 3D Web frontend for **English Draughts (Checkers)** using **Godot 4.7**, providing 3D rendering for the board, pieces, camera controls, menus, and AI persona banter.

### Key Requirements
1. **Zero-Install Web Delivery**: Runs directly in modern HTML5 browsers via WebAssembly/WebGL2 (no native Godot install or download needed by end users).
2. **Dual-Client Coexistence**:
   - The existing vanilla HTML5/JS 2D client remains completely functional and untouched at root (`/`).
   - The new Godot 3D client resides under `/godot/` (e.g. `/godot/index.html`).
3. **Engine Reuse via Bidirectional Bridge**:
   - Reuse the verified, 100% test-covered JavaScript game engine (`rules.js`, `controller.js`, `state.js`, `ai.js`, and the WebLLM WebGPU pipeline).
   - Godot serves as an alternate 3D view/presentation layer communicating with the JS engine via `JavaScriptBridge`.
4. **Project Management Workflow**:
   - Work is broken into distinct sections executed by specialized sub-agents.
   - Each sub-agent is bootstrapped with complete task context and tasked with identifying open questions and technical findings *before* implementing.
   - Findings and proposed recommendations are reviewed by the user before committing code.
   - Tested and verified increments are committed to `main` and pushed to `origin`.

---

## 🏗️ High-Level System Architecture

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        Browser Runtime (HTML5/Wasm)                    │
│                                                                        │
│   ┌───────────────────────────┐      JavaScriptBridge                  │
│   │     Godot 3D Frontend     │ <───────────────────────────────>      │
│   │   (Wasm / WebGL2 / Canvas)│                                        │
│   │  • 3D Board & Materials   │        Bidirectional Event Bus:        │
│   │  • 3D Pieces & Crowns     │        • onSquareClicked(index)        │
│   │  • Hop Arc & Slide Tweens │        • onBoardUpdate(state)          │
│   │  • Camera Orbit / Flip    │        • onAnimateMove(move)           │
│   │  • 3D Banter Speech Bubles│        • onBanterStream(token)         │
│   │  • Godot Menus & HUD      │        • onGameOver(stats)             │
│   └───────────────────────────┘                                        │
│                                                                        │
│   ┌────────────────────────────────────────────────────────────────┐   │
│   │                  Shared JavaScript Game Engine                 │   │
│   │  • src/rules.js       - Draughts rules, mandatory jumps        │   │
│   │  • src/controller.js  - Turn cycle, spectator scheduler        │   │
│   │  • src/state.js       - 32-square index topology               │   │
│   │  • src/ai.js          - Alpha-Beta Minimax heuristics          │   │
│   │  • src/llm/           - WebLLM on-device Llama 3.2 / SmolLM2   │   │
│   │  • src/godot_adapter.js - Implements CheckersRenderer contract │   │
│   └────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 📋 Breakdown of Work Sections

### **Section 1: Architecture, Scaffolding & Web Export Pipeline**
- **Goal:** Set up the Godot 4.7 project scaffold, export presets, headless compilation automation, and static web serving under `/godot/`.
- **Key Tasks:**
  1. Initialize `godot_project/` with `project.godot` configured for the `compatibility` (WebGL2) rendering method.
  2. Configure `export_presets.cfg` for HTML5 export using the `nothreads` template (enabling static hosting without requiring COOP/COEP headers).
  3. Create a headless build script (`scripts/build_godot.sh`) calling the local Godot 4.7 binary (`/Applications/Godot.app/Contents/MacOS/Godot --headless --export-release "Web" ...`).
  4. Ensure build artifacts output to `/godot/` (`/godot/index.html`, `/godot/checkers.wasm`, `/godot/checkers.pck`, `/godot/checkers.js`).
  5. Verify dual serving: `/` boots the 2D client; `/godot/` boots the 3D client.
- **Status:** `[COMPLETED]`

### **Section 2: Bidirectional JavaScript Bridge & Interface Contract**
- **Goal:** Establish a robust communication layer connecting the JS game controller to the Godot WebAssembly runtime.
- **Key Tasks:**
  1. Create `src/godot_adapter.js` that implements the standard `CheckersRenderer` interface expected by `CheckersController`.
  2. Implement `window.GodotCheckersBridge` in JS as the global messaging gateway.
  3. Implement `godot_project/scripts/bridge.gd` utilizing Godot 4's `JavaScriptBridge` (`JavaScriptBridge.get_interface`, `JavaScriptBridge.create_callback`).
  4. Map all lifecycle events:
     - JS ➔ Godot: Initial board state, turn changes, legal move masks, move animation requests, audio cues, and streaming banter tokens.
     - Godot ➔ JS: 3D square click selection, mode selection, spectator controls (play/pause/step/speed), and match restart.
- **Status:** `[COMPLETED]`

### **Section 3: 3D Board, Pieces & Movement Animation Engine**
- **Goal:** Implement the 3D visual world, materials, interactive raycasting, and animation tweens in Godot.
- **Key Tasks:**
  1. **3D Board**: 8x8 checkerboard model with alternating light/dark squares, coordinate number labels (1–32) for dark squares, and smooth board rotation (180° flip).
  2. **3D Pieces**: Cylindrical draughts with grooved detailing, Red and Black lacquer PBR materials, and crowned King states.
  3. **Raycast Input**: `Area3D` collision on the 32 playable dark squares with hover feedback and destination highlight rings.
  4. **Animation System**:
     - Regular slides: smooth linear/quadratic translation.
     - Jumps: parabolic 3D arc over captured pieces.
     - Captures: piece scale-down and dissipation.
- **Status:** `[COMPLETED]`

### **Section 4: Menus, HUD & Persona Banter Integration**
- **Goal:** Build the UI layer and LLM personality presentation inside the 3D frontend.
- **Key Tasks:**
  1. **Menus**: Title screen, mode selector (Pass & Play, Minimax Easy/Med/Hard, Player vs LLM, AI vs AI Arena), and pre-match staging screen.
  2. **Active HUD**: Player profile badges, turn indicators, piece/king counters, move timer, and mute/flip toggles.
  3. **Spectator Controls**: Arena playback bar (Play, Pause, Step Move, Speed slider) and active feed.
  4. **3D Banter Speech Bubbles**: Billboarded 3D callouts hovering above the board streaming real-time tokens from WebLLM personas.
  5. **Game Over Modal**: Victory/draw fanfare, match stats (turns, time, captures, kings), and rematch controls.
- **Status:** `[COMPLETED]`

### **Section 5: Verification, Polish, Documentation & Release**
- **Goal:** Comprehensive end-to-end testing across all modes, zero-regression verification, documentation, and git synchronization.
- **Key Tasks:**
  1. Verify zero regressions on the existing 2D frontend (`npm test` passes 100%).
  2. Test all 4 game modes in browser on `/godot/`.
  3. Add `npm run build:godot` scripts to `package.json`.
  4. Update `README.md` to document the 3D Godot client.
  5. Commit clean milestone increments to `main` and push to `origin`.
- **Status:** `[COMPLETED]`

---

## 🔄 Workflow Protocol
1. **Sub-Agent Dispatch**: Bootstrapped with task specifications, files, and constraints.
2. **Review & Open Questions**: Sub-agent returns findings, trade-offs, and questions.
3. **PM Synthesis & User Decision**: PM synthesizes options and presents recommendations to the user for approval.
4. **Execution & Automated Verification**: Sub-agent implements and verifies.
5. **Git Commit & Push**: Changes are committed to `main` and pushed to `origin`.
