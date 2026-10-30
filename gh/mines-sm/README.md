# mines-sm

Minimalist, allocation-free (`#![no_std]`) 8×8 Minesweeper bitboard state machine with RL observation tensors.

Vendored from [jpeckenpaugh/mines-sm](https://github.com/jpeckenpaugh/mines-sm).

## Architecture & Bitboard Representation

`mines-sm` models an 8×8 board using flat 64-bit unsigned integers (`u64`), where bit index `i = row * 8 + col` corresponds directly to cell `(row, col)`. All game state operations are pure bitwise masks with zero dynamic heap allocation:

- **`mine_mask: u64`**: Ground-truth bomb placement (hidden from agent).
- **`revealed_mask: u64`**: Cells exposed by player actions or zero-cascade flood fill.
- **`flagged_mask: u64`**: Cells marked by the player with caution flags.
- **`detonated_mask: u64`**: Exploded bombs that triggered strike penalties or terminal losses.

## SplitMix64 Deterministic PRNG

Board generation uses an embedded `SplitMix64` pseudo-random number generator. Given any 64-bit integer `seed`, the PRNG generates byte-for-byte identical mine placements across native x86_64, ARM64, and WebAssembly (`wasm32-unknown-unknown`) runtimes:

```rust
let mut state = seed;
// state = state.wrapping_add(0x9E3779B97F4A7C15);
// z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
// ...
```

A safe first-click invariant relocates any bomb under the initial reveal cell to the first available non-bomb slot, guaranteeing opening moves never detonate.

## BoardState & Rules

- **`Rules`**:
  - Classic sudden death (`max_strikes: 1`): First mine hit ends the game in defeat.
  - Shielded / multi-strike (`max_strikes: N`): Allows up to `N` mine strikes before termination, facilitating reinforcement learning exploration without immediate trajectory truncation.
- **`BoardState`**:
  - `new(seed: u64, num_bombs: u8, rules: Rules) -> Self`: Constructs clean board state.
  - `step(action: Action) -> StepResult`: Applies `Reveal` or `Flag` on cell `0..63`, updating BFS cascades and victory status (`safe_revealed >= 64 - num_bombs`).

## Reinforcement Learning Interface

`mines-sm` is designed for seamless integration with neural network policy evaluation and ONNX runtimes without serialization overhead:

- **`get_observation(&self, out: &mut [f32; 64])`**:
  Encodes board state into a normalized `[f32; 64]` tensor:
  - `-1.0`: Hidden / unrevealed cell
  - `-2.0`: Detonated bomb
  - `-0.5`: Flagged cell
  - `0.0 .. 0.8`: Revealed safe cell (`adjacent_mines / 10.0`)
- **`get_action_mask(&self, mask_out: &mut [f32; 64])`**:
  Returns binary valid-action mask (`1.0` legal, `0.0` illegal). Terminal boards emit all zeros.
