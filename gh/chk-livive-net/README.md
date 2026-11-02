# ♟️ Classic Checkers & WebGPU AI Arena

> **English Draughts Meets In-Browser Neural Intelligence** — Play instant alpha-beta Minimax, challenge client-side WebGPU LLM personas with live streaming banter, spectate AI vs. AI Arena battles, or switch to the Godot 4 3D edition.

[![Play Now on Livive](https://img.shields.io/badge/▶_PLAY_NOW-chk.livive.net-ef4444?style=for-the-badge)](https://chk.livive.net/)

---

## 🎮 Play in Your Browser (`https://chk.livive.net/`)

- **4 Match Modes:** Player vs. Minimax (Depth 2/4/6, instant play, zero download), Player vs. WebLLM AI (6 in-character personas streaming live tactical banter over WebGPU), AI vs. AI Spectator Arena, and Local Pass & Play.
- **2D Web Client + 3D Godot 4 Client:** Switch seamlessly between the fast 2D board and the 3D Godot 4 WebGL edition (`https://chk.livive.net/godot/`).
- **Built-in LLM & Prompt Inspector:** Inspect raw prompts, token/sec throughput, and WebGPU hardware status right from the header toolbar.

---

## 🛠️ For Developers (The Other 10%)

### Local Development Server
```bash
npx serve .
# or
python3 -m http.server 4173
```

### Test Suite & Showcase Runner
```bash
npm test
./test-runner/run.sh --showcase
```

### Project Layout
- `index.html`: Main 2D Web client & UI
- `src/`: Core checkers rules, AI engines, WebGPU / WebLLM controllers
- `godot_project/`: Godot 4 3D WebGL edition sources
- `test/`: Unit tests and Playwright test scripts
- `test-runner/`: Containerized test runner and recording suite
