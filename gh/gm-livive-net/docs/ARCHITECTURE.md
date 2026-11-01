# Architecture

Gem Miner runs on the outer voxel shell of a 10×10×10 geode. The grid is pure
data; tiles are views. Historical names are retained: the scene file is
`volumetric_minesweeper.tscn` and the classes are `VolumetricGrid`,
`VolumetricTile3D`, and `VolumetricCubeController`.

## Scene tree

```
VolumetricMinesweeper (Node3D, volumetric_cube_controller.gd)
├── WorldEnvironment
├── FillLightRight (DirectionalLight3D)
├── FillLightLeft (DirectionalLight3D)
├── CubeRoot (Node3D)                       # rotated as a whole by 90° steps
│   └── ElasticPivot (Node3D)               # click spring impulse
│       ├── InnerSolidCore (MeshInstance3D) # dark box inside the shell
│       └── 488 × VolumetricTile3D (Area3D)
├── Camera3D (z = 18, fov 48)
│   └── CameraLight (DirectionalLight3D)
├── UI (Control, ui_panels.gd)
│   ├── Panel (PanelContainer)
│   │   └── VBoxContainer
│   │       ├── TitleRow (TitleLabel, TopToggle)
│   │       ├── FaceInfoLabel
│   │       ├── StatsLabel
│   │       └── ActionRow (DifficultyLabel, DifficultyOption, RestartButton)
│   ├── CubeNetPanel (PanelContainer)
│   │   └── VBox
│   │       ├── TitleRow (NetTitleLabel, NetToggle)
│   │       ├── NetCanvas
│   │       └── NetFooterLabel
│   └── ControlsHint
└── ExcavationFinale (Node3D, added at runtime on victory)
```

`NetCanvas` runs `claim_net.gd`. `ui_panels.gd` collapses either panel to a
small show button, hiding the panel content while keeping the toggle visible.
`VolumetricCubeController` owns `grid` (pure data) and `tile_nodes`
(`Vector3i -> VolumetricTile3D`). A tile is only a view plus collision/input;
the authoritative state is `VolumetricGrid.CellData`. The controller creates the
finale helper from code, so the scene has no finale node until a claim
completes.

## Coordinates and faces

- Grid coordinates are integer voxels `(x, y, z)` with each component in
  `[0, size-1]`. Cells that are not on the outer shell are never created.
- World position for `tile_spacing = 1`:
  `(coord.x - half, coord.y - half, coord.z - half)` where
  `half = (size - 1) * 0.5`.
- Shell cell count: `size³ - (size - 2)³`. For `size = 10` this is
  `1000 - 512 = 488`.
- Face enum and outward normals:

| Enum | Face | Normal |
| --- | --- | --- |
| 0 | FRONT | +Z |
| 1 | RIGHT | +X |
| 2 | BACK | -Z |
| 3 | LEFT | -X |
| 4 | TOP | +Y |
| 5 | BOTTOM | -Y |

`VolumetricGrid.face_uv_to_voxel(face, u, v, size)` maps a face-local
left-to-right (`u`) / top-to-bottom (`v`) coordinate to a voxel:

| Face | Voxel `(x, y, z)` |
| --- | --- |
| FRONT | `(u, last - v, last)` |
| RIGHT | `(last, last - v, last - u)` |
| BACK | `(last - u, last - v, 0)` |
| LEFT | `(0, last - v, u)` |
| TOP | `(u, last, v)` |
| BOTTOM | `(u, 0, last - v)` |

Edge and corner voxels belong to two or three faces. `face_cells` therefore
overlaps: each face lists 100 cells, but the union is 488 and the six face
memberships total 600.

## Grid construction

`init_grid(size)`:

1. Iterates the full cube and keeps only shell voxels. Each `CellData` records
   its `exterior_faces` (one to three `Face` values) and is added to
   `face_cells[f]` for every exterior face.
2. Pre-maps every `(u, v)` on each face to a voxel with `face_uv_to_voxel()`,
   then builds the **per-face** graph `face_neighbor_graph[f]`: each `(u, v)` is
   connected to the 8 surrounding coords inside that same face. This graph
   never wraps across an edge.
3. Builds the **global manifold** graph `neighbor_graph` by unioning the per-face
   connections. Shared edge/corner coords merge the faces into one connected 2D
   surface. Duplicate neighbors are removed.
4. Resets `gems_populated` to `false`; gems are not placed at init.

Resulting topology:

- Face ownership: 8 cube corners on 3 faces, 96 edge cells on 2 faces, 384
  face-interior cells on 1 face.
- Global neighbors: 8 cube corners have 6; the other 480 cells have 8.
- Face-local neighbors: face corners have 3, face edges 5, face interiors 8.

`get_neighbors(coord)` returns the global graph; `get_face_neighbors(face,
coord)` returns the per-face graph. Missing keys return typed empty arrays.

## Gem placement and numbering

Gems are placed after the first left click, in
`grid.populate_gems(safe_coord, total_gems)`:

1. Clears `is_gem` and `face_neighbor_gems` on every cell. This makes the call
   total on re-entry: gems never accumulate. Play flags (`is_revealed`,
   `is_marked`, `is_detonated`) are preserved, so re-entry is only supported on
   a fresh grid.
2. Builds a safe set from the clicked cell plus its global neighbors (up to 8;
   6 at cube corners). Safety does not reduce the gem count.
3. Shuffles the remaining shell cells and marks `min(total_gems, available)`
   as gems. Exactly 60 are placed with the default configuration.
4. For every non-gem cell and every exterior face, counts adjacent gems on that
   face and stores the result in `face_neighbor_gems[face]`.
5. Sets `gems_populated = true`.

Existing pre-marks are neutral: they do not shrink the candidate pool, do not
change the layout, and may land on cells that become gems; the mark persists.
On the first click the controller calls `_sync_tiles_from_grid()`, copying
`is_gem` and `face_neighbor_gems` into every tile. Later clicks skip population.

## Cell state and completion

`CellData` carries four independent flags:

- `is_gem` — a gem occupies the cell.
- `is_revealed` — rock was excavated. Gems are never revealed.
- `is_marked` — an `X` stake is placed. A cracked gem is also marked.
- `is_detonated` — the gem was cracked.

`is_cell_resolved()` is the single source of truth:

```gdscript
if cell.is_gem:
    return cell.is_marked or cell.is_detonated
return cell.is_revealed
```

Consequences:

- A staked rock is **not** resolved; it blocks both face completion and board
  completion until the stake is pulled.
- A cracked gem counts once, as cracked. Stats check `is_detonated` before
  `is_marked` so a crack is never also counted as a clean stake.
- `is_board_complete()` requires `gems_populated` and every cell resolved.
- `get_face_stats(face).is_face_complete` is `face_resolved == face_total`
  (100 with the default board). Shared cells are counted on each face that owns
  them.
- `get_board_stats()` counts each cell once and exposes 18 keys, including
  `rock_excavated`, `gems_staked`, `gems_cracked`, `gems_remaining`,
  `misplaced_marks`, `all_rock_excavated`, `all_gems_resolved`, and
  `faces_complete`.

`misplaced_marks` is a separate diagnostic count. The live HUD never shows it as
a correctness signal.

## Runtime flow

### Reset / build

`reset_game()`:

- cancels a running finale;
- kills navigation, elastic, and shake tweens and restores `camera_home`;
- clears `game_over`, `game_won`, strikes, surveyed faces, and the auto-stake /
  claim latches;
- disables input and processing on old tiles, then `queue_free()`s them;
- clears `tile_nodes`, re-initializes the grid, and rebuilds the inner core;
- creates one `VolumetricTile3D` per shell cell and connects its signals;
- refreshes the HUD.

Camera distance is `size * 0.5 + 13`; for `size = 10` it sits at `z = 18`.

### Excavate

1. Left click on a staked, revealed, or cracked cell is ignored.
2. On the first left click, `populate_gems()` runs and tiles are synced.
3. A gem calls the crack path; rock runs `_flood_excavate()`.
4. `_flood_excavate()` is a BFS over the global manifold graph:
   - skips revealed, staked, and gem cells;
   - reveals the cell and calls `tile.excavate()`;
   - if the cell has a zero count on **any** exterior face, enqueues its
     unrevealed, unstaked, non-gem global neighbors;
   - cells with nonzero counts are revealed but do not expand.
5. Labels refresh and the post-mutation pipeline runs.

### Stake and chord

- Right click on an unrevealed cell toggles `is_marked` and calls
  `tile.set_marked()`. The rock cube stays visible; a lazy amber `X` overlay is
  built on first use and shown on every exterior face.
- Right click on a revealed cell with a nonzero current-face number calls
  `_chord_excavate_neighbors()`. It counts marked face-local neighbors as
  accounted for; if that equals the number and undecided neighbors remain, it
  excavates them, cracking any real gems.
- Because any marked cell counts, a stake on rock can satisfy the chord. This
  is a known, characterized quirk rather than an oracle: the UI still does not
  say whether the stake was correct.

### Crack and strikes

`_on_gem_struck(coord)`:

1. ignores a cell that is not a gem or is already marked/cracked;
2. increments `strikes`, sets `is_detonated` and `is_marked`;
3. calls `tile.strike()` (charred rock, dim cracked crystal, labels hidden);
4. shakes the camera around `camera_home`;
5. ends the run when the limit is reached (Easy 9, Medium 3, Hard 0). Hard has
   no allowance and loses on the first crack.

A cracked gem is permanently staked; right-click and left-click are ignored
afterward.

### Post-mutation pipeline

Every top-level action ends in `_after_mutation()`:

1. **Loss precedence.** If `game_over`, update the HUD and stop. No surveys,
   auto-stake, or victory are processed.
2. **Auto-stake.** Once `all_rock_excavated` is true, stake every unresolved gem,
   update tile visuals, and latch `_auto_stake_done`. Staked rock means the rock
   is never fully excavated, so the auto-stake cannot fire until those stakes
   are pulled.
3. **Face survey.** For each face not yet in `surveyed_faces` whose current
   stats are complete, emit `face_surveyed(face)` exactly once and add a HUD
   milestone.
4. **HUD refresh.**
5. **Victory check.** `_check_victory()` runs when populated, not lost, not
   already won, and not already claimed.

### Victory and finale

`_trigger_claim_complete()` is the only entry point and is idempotent:

1. latches `_claim_triggered` and sets `game_won`;
2. stops navigation/elastic/shake tweens and restores the pivot and camera;
3. emits `claim_complete()` and updates the HUD;
4. ensures the `ExcavationFinale` helper exists and calls `begin()` with the
   tile map, inner core, camera, cube size, spacing, and secured/cracked counts.

The finale:

- schedules all 488 tiles through `VolumetricTile3D.play_extraction(delay)`;
- peels by camera depth (`z`), then radial distance, then angle, over roughly
  2.4 s of sweep with a small bias toward gems;
- dollies the camera out, then in;
- hides the inner rock core (rind), builds a procedural faceted icosphere mother
  lode with a heart and omni light, grows it, and spins it in `_process`;
- reports `CLAIM COMPLETE — all six faces surveyed · N gems secured`, where `N`
  includes cracked gems and a parenthetical cracked count is added when any were
  cracked;
- uses one finite master tween plus finite child tweens, so there are no
  looping animations or pending awaits;
- is restart-safe: `reset_game()` calls `cancel()`, which kills tweens, frees
  the core, restores the inner core visibility, and clears the helper's state.

The helper is reused across wins on the same controller instance.

## Face surveys and the cube net

There is no per-face dissolve during play. Completing a face only:

- emits `face_surveyed(face)` once per face per run, for the HUD milestone and
  net pulse;
- adds a gold check to the cube net.

Tiles never shrink or disappear while the board is playable. A face can become
incomplete again if a stake is pulled; the net check is recomputed from current
state, while the one-shot pulse is not repeated. On a loss, surveys and pulses
are suppressed.

`claim_net.gd` is a read-only `Control` at `UI/CubeNetPanel/VBox/NetCanvas`:

- polls every 0.1 s, snapshots 600 face cells, hashes the result, and redraws
  only on change;
- draws a 4×3 unfolded cross (Top above Front; Left/Front/Right/Back across;
  Bottom below Front), with 10×10 mini-grids;
- classifies each cell as rock, excavated, staked, or cracked. Unrevealed gems
  render as rock; staked gems and staked rock render identically;
- captions each face with its name, excavated rock count, stake count, and a
  gold check when currently complete;
- outlines the viewed face in cyan, pulses newly surveyed faces for 0.8 s,
  frames the whole net gold on a win and red on a loss;
- shows a footer with rock/stake/face progress, the pre-population expectation,
  or the stuck hint when no unstaked, unrevealed rock remains but the board is
  incomplete;
- disables itself with a warning for boards that are not 10×10;
- wires itself to New Claim and the difficulty selector after the scene is
  ready, without mutating gameplay state.

The net is intentionally anti-oracle: it never shows a confirmed-gem count, a
misplaced-stake count, or whether an individual stake is correct.

## Rotation and labels

- Arrow keys / WASD call `rotate_cube_view(axis, ±90°)`, which slerps
  `CubeRoot.basis` over 0.45 s.
- `_identify_current_front_face()` picks the face whose rotated normal has the
  largest dot product with `+Z` (toward the camera).
- `_refresh_all_labels()` asks every tile to show only the label for the current
  face, oriented upright using the camera's up vector. The label basis is
  converted into the tile's local space, with a determinant guard against
  degenerate scale.
- Rotation and clicks are ignored while navigating, lost, or won.

## Tile view

`VolumetricTile3D` uses shared static resources for all 488 tiles:

- one `BoxMesh` and one `BoxShape3D` (0.94³) for the rock body and picking;
- shared state materials for rock, hover, excavated, charred, stake, intact
  crystal, cracked crystal, and cracks;
- one `Label3D` per exterior face, created eagerly, offset `0.49` along the
  outward normal.

Overlays are lazy:

- the amber `X` stake (`StakeOverlay`) is built on the first stake and reused;
- the crystal cluster (`CrystalOverlay`) is built on a crack or during
  extraction. Cracked gems get a dim crystal plus a fracture torus and fissures.
- Per-tile crystal material is duplicated only when a finite extraction glow
  pulse runs, so shared resources stay immutable.

Processing and input:

- `_process()` is disabled at setup and in `_ready()` and runs only while a left
  press is held, driving the 0.35 s long-press timer;
- right press immediately emits `tile_pressed` (elastic knock) and
  `tile_clicked(..., MOUSE_BUTTON_RIGHT)`;
- a completed long press emits a synthetic right click and suppresses the
  release left click;
- extraction disables picking, so finale tiles cannot be clicked.

`play_extraction(delay)` waits, hides labels, sinks/shrinks the rock inward, and
for gems reveals the crystal (intact unless cracked), pulses it, and lifts it
outward. The finale orchestrates timing; the tile owns the local animation.

## Tests

`tests/run_tests.gd` is a `SceneTree` entry point that awaits each suite in
order and prints `TEST RESULT: PASS` only when every check passes. `make test`
also rejects script errors, a non-zero exit, or a missing PASS marker. It runs
with `--fixed-fps 60` and a 60 s watchdog, so awaited finale frames are
deterministic.

Suites:

- `grid_topology` — shell counts, face mapping/orientation, both neighbor
  graphs, non-shell guards.
- `gem_placement` — lazy placement, 60 gems / 428 rock, first-click safety,
  pre-mark neutrality, total re-entry, determinism, multi-seed invariants.
- `face_numbering` — face-local counts, edge/corner multi-face entries,
  face-local vs global differences.
- `resolution_accounting` — resolution truth table, population gate, frozen
  stats keys, edge-gem double-face resolution, crack accounting, completion.
- `controller_flow` — boot, first click, mark/unmark, cascade, chord and the
  wrong-stake quirk, strikes, Hard loss precedence, auto-stake, surveys, input
  lock, reset.
- `prepopulation` — provisional stats, neutral pre-population HUD, no progress
  before population, cube-net boot and secured-gem copy.
- `finale_lifecycle` — lazy helper, one-shot sweep, input lock, mid-sweep reset,
  finish, reset after finish, orphan checks.

The suites instantiate the real scene headless; there is no rendering or mouse
picking coverage.

## Extension points

- **Cascade rule:** decide whether expansion should require zero on all exterior
  faces instead of any face. This is the main retained rules quirk.
- **Chord accounting:** stop treating arbitrary marked cells as confirmed gems,
  if the wrong-stake quirk is ever scheduled.
- **Gem difficulty:** scale `total_gems` with difficulty or board size.
- **Cube net size:** generalize `claim_net.gd` from its 10×10 constants.
- **Input:** add touch-specific affordances and verify long-press on devices.
- **Presentation:** free-orbit camera, audio, or imported art would be new
  dependencies; the current build is procedural.
- **Verification:** add a headless input/visual harness or screenshot regression
  suite; the current tests cover data and controller flow only.
