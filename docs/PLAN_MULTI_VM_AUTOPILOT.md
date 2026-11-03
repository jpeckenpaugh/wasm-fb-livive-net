# PLAN: Multi-VM Auto Pilot Orchestration & Actor Matrix Architecture

> **Specification & Implementation Roadmap**  
> *Target: Multi-Appliance Entangled Flight Plans, Unified Actor Abstractions, and Automated Causal Convergence Verification.*

---

## 1. Executive Summary & Problem Framing

Until now, the Auto Pilot engine (`web/autopilot.js`) and Trajectory Runner operated under a **single-guest assumption**:
- Steps targeted whichever VM was currently active or `guests[0]`.
- Matches against "AI" assumed the guest's hardcoded internal `#![no_std]` Alpha-Beta Minimax C heuristic.

As autonomous LLM agents and multi-client testing evolve, a "2-Player Game Test" takes on three distinct shapes:
1. **Shape 1 (Solo vs Kernel Engine)**: 1 VM with local human/script vs. internal Minimax heuristic.
2. **Shape 2 (Concurrent Duels)**: 2 independent VMs running parallel workloads for isolation and A/B benchmarking.
3. **Shape 3 (Entangled Distributed Match)**: 2 distinct VMs connected via the broker relay where an automated scenario drives both Seat P1 and Seat P2, verifying causal convergence across linear memory boundaries.

This plan details the architecture and engineering assignments to unlock **Multi-VM Target Orchestration (Shape 3)** and re-frame **"Mode vs AI"** as a unified, actor-agnostic matrix.

---

## 2. Core Architecture Upgrades

### 2.1 Multi-VM Step Addressing in Auto Pilot (`web/autopilot.js`)
Extend the step interpreter so actions can target specific VM roles:
- `targetVm: 'p1' | 'p2' | 'active' | number`:
  - `'p1'`: Automatically resolves to the guest bound to Seat 1 of the active match.
  - `'p2'`: Automatically resolves to the guest bound to Seat 2 of the active match.
  - `number`: Direct VM ID addressing (e.g. `vmId: 2`).
- Actions supported across targets:
  - `click`, `clickSquare`, `clickCell`, `key`, `assertTurn`, `assertHash`, `assertState`, `snapshot`.

### 2.2 Dual-Perspective Split Capture
- Introduce a composite snapshot action:
  - `action: 'splitSnapshot'`: Captures both `vm-01` (Seat P1, Red/White at bottom) and `vm-02` (Seat P2, Black at bottom, auto-flipped) side-by-side into a single multi-lens verification proof.

### 2.3 The Unified Actor Matrix (Re-framing "Mode vs AI")
Decouple **Seat** from **Actor Type**:
- **Actor Kinds**:
  - `Human`: Interactive pointer/keyboard inputs.
  - `KernelEngine`: Built-in `#![no_std]` Minimax heuristic (Depths 1..6).
  - `AutoPilot`: Scripted deterministic trajectory scenario.
  - `ExternalAgent`: External LLM / RL policy agent via `window.__appliance`.
  - `RemotePeer`: LAN or WebRTC remote participant over broker relay.
- Update VM chassis headers to display the active Actor Badge (e.g. `Seat P1 [✈️ AutoPilot] vs Seat P2 [✈️ AutoPilot]`).

---

## 3. Pre-Canned Multi-VM Missions (`web/missions.js`)

### Mission 5: `checkers-2p-entangled-convergence`
- **Objective**: Mathematically prove zero-drift state convergence across two separate Wasm guest linear memories in real-time.
- **Flight Plan**:
  1. Boot `vm-01` and `vm-02` in Split View at 1280×720.
  2. Initiate Brokered 2P Checkers Match `#201`.
  3. `P1 (vm-01)` plays move `11-15` $\rightarrow$ Assert `vm-01` state hash.
  4. Assert `vm-02` receives RX transition $\rightarrow$ Assert `vm-02` state hash matches `vm-01`.
  5. `P2 (vm-02)` plays move `24-19` $\rightarrow$ Assert `vm-02` state hash.
  6. Assert `vm-01` receives RX transition $\rightarrow$ Assert `vm-01` state hash matches `vm-02`.
  7. Repeat for 6 opening moves.
  8. Capture dual-screen verification split snapshot.

### Mission 6: `mines-2p-turn-relay`
- **Objective**: Verify multiplayer turn-passing, bitboard scoring, and sudden-death elimination across two VMs.
- **Flight Plan**:
  1. Boot `vm-01` and `vm-02` in Split View with Seed 42, Max Strikes 1.
  2. Initiate 2P Mines Match.
  3. `P1 (vm-01)` reveals safe cell $(0, 0)$ $\rightarrow$ Assert points awarded to P1.
  4. Assert turn passes to `P2 (vm-02)`.
  5. `P2 (vm-02)` reveals safe cell $(0, 7)$ $\rightarrow$ Assert points awarded to P2.
  6. Capture dual-screen snapshot.
  7. `P1 (vm-01)` clicks known bomb $(0, 6)$ $\rightarrow$ Assert sudden-death detonation on both screens.

---

## 4. Work Breakdown & Status
*(Status: **COMPLETED & DEPLOYED**)*

| Role | Component / Files | Responsibilities / Deliverables | Status |
|---|---|---|---|
| **1. Hypervisor Engine Specialist** | `web/autopilot.js`<br>`web/missions.js`<br>`web/data/missions/*.json` | • Implement multi-target resolution (`targetVm: 'p1' \| 'p2' \| id`) in `AutoPilotRunner`.<br>• Add `splitSnapshot` action to capture side-by-side framebuffers.<br>• Author and register Mission 5 (`checkers-2p-entangled-convergence`) and Mission 6 (`mines-2p-turn-relay`) as 100% declarative JSON. | **Done** |
| **2. Broker & Mailbox Specialist** | `web/broker.js`<br>`web/adapters/*.js` | • Update match broker to record actor types (`Human`, `KernelEngine`, `AutoPilot`, `ExternalAgent`) in the `matches` table.<br>• Ensure synchronous TX/RX event delivery notifications wake the Auto Pilot step resolvers immediately. | **Done** |
| **3. UI & VM Chassis Architect** | `web/index.html`<br>`web/host.js`<br>`web/core/vm-manager.js` | • Update VM chassis header to display actor identity badges (`[✈️ AutoPilot]`, `[🤖 Minimax]`, `[👤 Human]`).<br>• Add multi-VM mission controls, custom resolution dropdown, and compact icon-only buttons (`⤢`, `📷`, `✕`).<br>• Render dual-screen snapshot cards in the snapshot gallery tray. | **Done** |

---

## 5. Verification & Quality Gates

1. **Syntax & Unit Test Gate**:
   - `node -c web/*.js web/adapters/*.js` (0 errors).
   - `python3 -c HTMLParser` on `web/index.html` (valid DOM structure).
   - `cargo test --manifest-path v2/Cargo.toml` (80/80 Rust tests passing).
2. **Headless Execution Gate**:
   - Execute Mission 5 and Mission 6 in `Instant (0ms)` mode via Node script to verify that both guests converge to 100% identical state hashes without timing out.
3. **Browser Acceptance Gate**:
   - Run Mission 5 in `Live Paced (400ms)` mode in the browser.
   - Verify that Split View launches automatically, moves alternate seamlessly between the two boards, and the dual-screen snapshot card downloads cleanly.
