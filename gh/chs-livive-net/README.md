# ♟️ Classic Chess & WebGPU AI Arena

> **The Royal Game Meets In-Browser Neural Intelligence** — Play instant alpha-beta Minimax, challenge client-side WebGPU LLM personas with live streaming banter, spectate AI vs. AI Arena battles, or play pass & play locally.

[![Play Now on Livive](https://img.shields.io/badge/▶_PLAY_NOW-chs.livive.net-ef4444?style=for-the-badge)](https://chs.livive.net/)

---

## 🎮 Play in Your Browser (`https://chs.livive.net/`)

- **4 Match Modes:** Player vs. Minimax (Depth 1/2/3, instant play, zero download), Player vs. WebLLM AI (tactical banter over WebGPU), AI vs. AI Spectator Arena, and Local Pass & Play.
- **Fast 2D Web Client:** Responsive, pixel-perfect 8×8 chessboard with full FIDE rules (castling, en passant, pawn promotion, 50-move draw, check & checkmate detection).
- **Standalone Package & Appliance Integration:** Operates standalone at https://chs.livive.net/ and embeds into the `wasm-fb` appliance window manager and broker orchestration network.

---

## 🛠️ For Developers

### Local Development Server
```bash
python3 -m http.server 4174
# or
npx serve .
```

### Project Layout
- `index.html`: Standalone 2D Web client & game shell
- `README.md`: Project overview & usage instructions
- `.context.xml`: Appliance Files virtual mount metadata
