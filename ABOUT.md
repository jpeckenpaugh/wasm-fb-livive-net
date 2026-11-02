# Test Appliance Orchestration Console (wasm-fb)

A WebAssembly hypervisor orchestrator and bare-metal framebuffer appliance.
The *guest* draws every pixel into linear memory; the orchestration console forwards input, manages multi-instance matchmaking, enables A/B build provenance verification, and blits raw frames.

## Heritage & Apps
- Originally started as the `wasm-fb` C framebuffer experiment (Clock, Calculator, Calendar, Files & Preview).
- Modern appliance features bare-metal `#![no_std]` Rust guests, English Draughts (Checkers) with minimax AI, 2-Player turn-based Minesweeper, in-browser SQLite matchmaking broker, and multi-guest split-view orchestration.

`github.com/jpeckenpaugh`
