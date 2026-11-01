# Gem Miner

3D geode-claim minesweeper · gm.livive.net

Play at <https://gm.livive.net/>.

You are a surveyor staking a claim on the outer shell of a giant geode. The board
is the 488-voxel surface of a 10×10×10 cube: dark basalt rock hiding 60 gems.
Numbers are gem-scanner readings for the face you are viewing. An amber `X` is
an extraction stake. Swinging the pick into a gem cracks it: the crack costs
pick integrity, and the gem is staked automatically.

## Status

Playable end-to-end build:

- [x] 10×10×10 shell (488 cells) with shared edges and corners
- [x] 428 rock cells and 60 gems, placed lazily on the first pick
- [x] Face-local scanner numbers and cascades across the 2D shell
- [x] `X` stakes, cracked-gem crystals, lazy per-tile overlays
- [x] Chording on the viewed face
- [x] Easy / Medium / Hard pick-integrity limits
- [x] Neutral unfolded cube-net survey panel
- [x] Auto-stake, victory, and a restart-safe whole-cube finale
- [x] Headless rule/topology test suite

## Requirements

- Godot **4.7.x** (developed with 4.7.2)
- macOS commands below assume the default `/Applications/Godot.app`
- Everything is procedural: no imported meshes, textures, or other art assets

## Commands

```sh
make import   # once per fresh checkout; generates the global class cache
make run      # play
make editor   # open the editor
make check    # headless smoke test
make test     # headless rule/topology suite (fixed 60 fps)
make all      # check + test
```

`check` and `test` depend on `import`. `make test` runs at a fixed 60 fps and
only succeeds when the output contains `TEST RESULT: PASS`. Override the binary
with `make GODOT=/path/to/godot run`.

Equivalent raw commands:

```sh
/Applications/Godot.app/Contents/MacOS/Godot --path .
/Applications/Godot.app/Contents/MacOS/Godot -e --path .
/Applications/Godot.app/Contents/MacOS/Godot --headless --path . --quit-after 120 res://volumetric_minesweeper.tscn
/Applications/Godot.app/Contents/MacOS/Godot --headless --path . --fixed-fps 60 \
    --quit-after 9000 --script res://tests/run_tests.gd
```

## Web export

A single-threaded Web preset is checked in as `export_presets.cfg` and targets
`build/web/index.html`. The project uses a `web` renderer override
(`gl_compatibility`) so the desktop build can stay on Forward+.

1. Install the matching **Web export templates**:
   **Editor → Manage Export Templates…** → install **4.7.2** (the version must
   match the editor).
2. Open **Project → Export…**, select the **Web** preset, and confirm the
   export path is `build/web/index.html`.
3. Click **Export Project…**.
4. Serve the folder over HTTP — do not open `index.html` with `file://`:

   ```sh
   make serve-web          # python3 -m http.server on port 8000
   ```

   Then open `http://localhost:8000`.

From the command line, after the templates are installed:

```sh
make web          # release export
make web-debug    # debug export
```

Keep **Thread Support** off for simple hosting. If you enable it, the host must
send `Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp`, or enable the PWA cross-origin
isolation workaround.

### Publish to GitHub Pages

The workflow in `.github/workflows/web.yml` builds the Web export with
`barichello/godot-ci:4.7.2`, runs the headless suite, uploads `build/web` as a
Pages artifact, and deploys it with GitHub's official Pages actions.

This repository is the canonical Gem Miner source and publishes
`https://gm.livive.net/` from `main`.

One-time GitHub setup for a fork or recreated repository:

1. Open **Settings → Pages**.
2. Under **Build and deployment → Source**, choose **GitHub Actions**.
3. Under **Custom domain**, set `gm.livive.net` and enable **Enforce HTTPS**.
4. If the `github-pages` environment is protected, allow deployments from
   `main`.

After that, every push to `main` (or a manual **Actions → Build and Deploy Web →
Run workflow**) publishes the site. The export uses relative asset paths, so it
also works under a repository subpath if you publish without the custom domain.

## Controls

| Input | Action |
| --- | --- |
| Arrow keys / WASD | Rotate the geode 90° to inspect another face |
| Left click | Excavate rock, or crack a gem (costs pick integrity) |
| Right click on an unrevealed cell | Stake / unstake it |
| Left press ≥ 0.35 s | Long-press alternative for stake / unstake or chord |
| Right click on a satisfied number | Chord: excavate the remaining face-local neighbors |
| Difficulty dropdown | Easy / Medium / Hard; starts a new claim |
| New Claim | Rebuild the board and reset the HUD |
| Hide HUD / Hide Survey | Collapse the top panel or claim survey; the small show button remains |

The **current face** is whichever cube face points at the camera. Scanner
numbers are only shown for that face, and chording uses that face's number.

## Rules

- The board is the 488-voxel surface of a 10×10×10 cube. Each face lists 100
  cells; edge and corner cells belong to two or three faces.
- Gems are placed on the **first left click**, not at reset. The clicked cell
  and its global shell neighbors are guaranteed safe (up to 8; 6 at cube
  corners). Exactly 60 gems are placed among the remaining cells.
- A revealed rock cell shows the number of adjacent gems **on the face you are
  viewing**. Edge and corner cells store one number per exterior face. A blank
  reading is zero.
- Excavating a blank cell (zero on any exterior face) cascades through connected
  blank cells across the whole shell, including shared edges and corners.
- A staked cell cannot be excavated. Stakes are legal on rock as well as on
  gems, so a wrong stake can block progress. The live UI never confirms whether
  a stake is correct: it reports only stakes placed, and staked gems look like
  staked rock. Un-stake a cell to reopen it.
- Left-clicking a gem cracks it. A crack always costs one integrity point,
  auto-stakes the gem, and counts it as resolved. A cracked gem cannot be
  un-staked and its crack visual is permanent.
- A run is lost when strikes reach the limit: Easy 9, Medium 3, Hard 0. Hard
  loses on the first crack.
- Right-clicking a satisfied number chords only its **face-local** neighbors.
  Marked cells count toward the number, so a wrong stake can satisfy a chord
  and crack the real gem.
- Completing a face is a survey milestone only. Tiles are never removed during
  play; the cube net pulses and checks the face.
- Once every rock cell is revealed, all remaining unrevealed gems are
  auto-staked, then victory is checked.
- **Victory** requires every rock cell revealed **and** every gem staked or
  cracked. Loss takes precedence: a cracked gem on the last strike ends the
  run even if the board is otherwise resolved.
- Victory triggers once. The whole cube then runs the extraction finale:
  rock sinks away in a sweep, staked and cracked gems rise, and a faceted
  mother-lode core is revealed. The finale is finite and can be interrupted
  with New Claim.

## Configuration

Board shape and gem count are exported on the root node in
`volumetric_minesweeper.tscn`:

```ini
initial_cube_size = 10
total_gems = 60
tile_spacing = 1.0
```

Before the first pick the board is provisional: 488 rock, no gems. After
population it is 428 rock + 60 gems. Difficulty changes only the strike limit
(`DIFFICULTY_STRIKES` in `volumetric_cube_controller.gd`); the gem count stays
60. Scaling gems with difficulty or board size is future work.

The cube-net panel is built for a 10×10 board. Other sizes still play, but the
net disables itself with a warning.

## Project layout

| Path | Role |
| --- | --- |
| `volumetric_minesweeper.tscn` | Main scene: HUD, cube-net panel, camera, lights, board container |
| `volumetric_cube_controller.gd` | Game state, reset/build, input, excavation, staking, chording, cracks, HUD, victory |
| `volumetric_grid.gd` | Shell topology, face mapping, neighbor graphs, gem placement, stats |
| `volumetric_tile_3d.gd` | One shell voxel: rock mesh, labels, lazy stakes/crystals, input, extraction hook |
| `claim_net.gd` | Read-only unfolded cube-net survey panel |
| `excavation_finale.gd` | Whole-cube extraction sweep and mother-lode reveal |
| `ui_panels.gd` | Collapsible top HUD and claim survey |
| `tests/` | Headless suites, assertion helper, and runner |
| `Makefile` | `import` / `run` / `editor` / `check` / `test` targets |
| `docs/ARCHITECTURE.md` | Data model, scene tree, and runtime flow |
| `docs/GEM_MINER_PLAN.md` | Approved decisions and phase history |
| `AGENTS.md` | Bootstrap guide for coding agents |

### Legacy

Kept for reference and **not loaded by the main scene**:

| Path | Role |
| --- | --- |
| `main.tscn`, `player.*`, `dodecahedron_planet.gd`, `face_camera_controller.gd` | Dodecahedron planet prototype |
| `minesweeper_3d.*`, `minesweeper_tile_3d.gd` | Earlier 2D face-peel prototype |

The 4×4×4 gem-excavation experiment is retired; its history remains in the
repository. The main scene, scripts, and `Volumetric*` class names keep their
historical names even though the game is now Gem Miner.

## Known gaps

- Cascades start from a cell that is zero on **any** exterior face, then expand
  over the global shell. A cascade can reveal a cell on a face where its number
  is nonzero. Intentionally retained and characterized by tests.
- Chording treats every marked cell as accounted for, so a wrong stake can
  satisfy the number and crack the real gem. Intentionally retained and
  characterized by tests.
- The gem count is fixed at 60. Difficulty does not scale it.
- The cube net requires a 10×10 board; other sizes run without the net.
- No free-orbit camera, audio, or mobile-specific UI. Long-press exists, but
  touch behavior has not been verified on a device.
- The headless suites cover rules and topology; there is no rendering or visual
  regression suite.

## Live test checklist

1. Rotate all six faces; numbers stay upright and only the viewed face shows
   scanner readings.
2. First left-click never cracks a gem; blanks cascade across edges and corners.
3. Left-click excavates rock. Right-click stakes and un-stakes; a staked cell
   cannot be excavated, and un-staking reopens it.
4. A satisfied number chords only face-local neighbors.
5. Easy and Medium can continue after early cracks; Hard ends on the first
   crack.
6. Survey a face and confirm the cube net pulses/checks without removing tiles.
7. Reveal all rock and confirm remaining gems auto-stake, then the claim
   completes and the finale runs once.
8. New Claim during and after the finale rebuilds a clean 488-tile board,
   resets the HUD and net, and leaves no errors or orphaned nodes.
9. Counts: 488 cells, 428 rock, 60 gems; each face lists 100 cells.
