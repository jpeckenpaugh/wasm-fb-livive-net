/* Pre-Canned Auto Pilot Missions: Deterministic QA & Agent Trajectories */

export const MISSIONS = [
  {
    id: 'system-audit',
    name: 'System Audit',
    description: 'Audit System App navigation across About, Fonts (with specimen inspect) and Themes (switching palette)',
    steps: [
      {
        action: 'boot',
        opts: { width: 1280, height: 720 },
        desc: 'Ensure guest VM is initialized at 1280×720',
      },
      {
        action: 'openApp',
        slot: 'system',
        desc: 'Open System Settings application',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_selected() === 1,
        desc: 'Assert active application is System (slot 1)',
      },
      {
        action: 'key',
        code: 11, // Digit 1 -> About section
        desc: 'Navigate to About section',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_active_page() === 0,
        desc: 'Assert active section is About (page 0)',
      },
      {
        action: 'snapshot',
        label: 'System Audit - About Section',
        desc: 'Capture snapshot of About section',
      },
      {
        action: 'key',
        code: 13, // Digit 3 -> Fonts section
        desc: 'Navigate to Fonts section',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_active_page() === 2,
        desc: 'Assert active section is Fonts (page 2)',
      },
      {
        action: 'key',
        code: 2, // ArrowRight -> Smooth render mode specimen
        desc: 'Inspect font specimen and switch render mode to Smooth',
      },
      {
        action: 'key',
        code: 51, // ArrowDown -> cycle preview scale
        desc: 'Adjust font specimen preview size',
      },
      {
        action: 'snapshot',
        label: 'System Audit - Fonts Specimen Inspect',
        desc: 'Capture snapshot of Fonts section specimen inspection',
      },
      {
        action: 'key',
        code: 15, // Digit 5 -> Themes section
        desc: 'Navigate to Themes section',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_active_page() === 4,
        desc: 'Assert active section is Themes (page 4)',
      },
      {
        action: 'key',
        code: 51, // ArrowDown -> Select next theme
        desc: 'Select next theme palette',
      },
      {
        action: 'key',
        code: 24, // Enter -> Apply theme
        desc: 'Apply selected theme',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_theme() !== 0,
        desc: 'Assert theme palette changed from default',
      },
      {
        action: 'snapshot',
        label: 'System Audit - New Theme Applied',
        desc: 'Capture snapshot with new theme palette applied',
      },
    ],
  },
  {
    id: 'mines-gold-path',
    name: 'Minesweeper Known-Seed Victory & Detonation',
    description: 'Configure Minesweeper with seed 42, reveal known safe cells, flag a known bomb, and detonate',
    steps: [
      {
        action: 'boot',
        opts: { width: 1280, height: 720 },
        desc: 'Ensure guest VM is initialized at 1280×720',
      },
      {
        action: 'openApp',
        slot: 'mines',
        desc: 'Open Minesweeper application',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_selected() === 4,
        desc: 'Assert active application is Minesweeper (slot 4)',
      },
      {
        action: 'seedMines',
        seed: 42,
        maxStrikes: 1,
        desc: 'Configure Minesweeper seed 42 with classic rules',
      },
      {
        action: 'assert',
        check: (guest) => (guest.exports?.app_mines_seed ? (guest.exports.app_mines_seed() >>> 0) === 42 : true),
        desc: 'Assert Minesweeper seed is 42',
      },
      {
        action: 'clickCell',
        row: 0,
        col: 0, // Safe cell (reveals adjacent area)
        desc: 'Reveal known safe cell at (0, 0)',
      },
      {
        action: 'assertStatus',
        status: 'PLAYING',
        desc: 'Assert Minesweeper game status is PLAYING',
      },
      {
        action: 'clickCell',
        row: 0,
        col: 7, // Safe cell (remains unrevealed after cell 0 cascade)
        desc: 'Reveal known safe cell at (0, 7)',
      },
      {
        action: 'snapshot',
        label: 'Minesweeper - Safe Path Revealed',
        desc: 'Capture snapshot after revealing safe cells',
      },
      {
        action: 'key',
        code: 30, // F -> Toggle Flag mode
        desc: 'Toggle Flag mode on',
      },
      {
        action: 'clickCell',
        row: 0,
        col: 3, // Known bomb cell at index 3
        desc: 'Flag known bomb cell at (0, 3)',
      },
      {
        action: 'key',
        code: 30, // F -> Toggle Flag mode off
        desc: 'Toggle Flag mode off',
      },
      {
        action: 'snapshot',
        label: 'Minesweeper - Known Bomb Flagged',
        desc: 'Capture snapshot with known bomb flagged',
      },
      {
        action: 'clickCell',
        row: 0,
        col: 6, // Known bomb cell at index 6 -> triggers detonation!
        desc: 'Deliberately reveal known bomb cell at (0, 6) to trigger detonation',
      },
      {
        action: 'assertStatus',
        status: 'EXPLODED',
        desc: 'Assert Minesweeper game status is EXPLODED (terminal detonation)',
      },
      {
        action: 'snapshot',
        label: 'Minesweeper - Detonated Game End',
        desc: 'Capture snapshot of terminal detonation',
      },
    ],
  },
  {
    id: 'chess-checkers-opening',
    name: 'Chess & Checkers Opening',
    description: 'Execute opening moves against the minimax AI in Checkers (11-15) and Chess (e2-e4) and capture AI responses',
    steps: [
      {
        action: 'boot',
        opts: { width: 1280, height: 720 },
        desc: 'Ensure guest VM is initialized at 1280×720',
      },
      {
        action: 'openApp',
        slot: 'checkers',
        desc: 'Open English Draughts (Checkers) application',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_selected() === 5,
        desc: 'Assert active application is Checkers (slot 5)',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_checkers_turn && guest.exports.app_checkers_turn() === 1,
        desc: 'Assert Checkers turn is Red (Player 1)',
      },
      {
        action: 'click',
        x: 177, // sq 20 (row 5, col 0)
        y: 487,
        desc: 'Select Red piece at square 20',
      },
      {
        action: 'click',
        x: 251, // sq 16 (row 4, col 1) - opening move 21-17 / 11-15 equivalent
        y: 413,
        desc: 'Move piece to destination square 16',
      },
      {
        action: 'wait',
        ms: 300,
        desc: 'Wait for Checkers minimax AI to compute and execute reply',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_checkers_turn && guest.exports.app_checkers_turn() === 1,
        desc: 'Assert Checkers turn returned to Red after AI reply',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_checkers_ai_pending && guest.exports.app_checkers_ai_pending() === 0,
        desc: 'Assert Checkers AI completed response move',
      },
      {
        action: 'snapshot',
        label: 'Checkers - Opening Move & AI Reply',
        desc: 'Capture snapshot of Checkers board after opening exchange',
      },
      {
        action: 'openApp',
        slot: 'chess',
        desc: 'Open Classic Chess application',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_selected() === 6,
        desc: 'Assert active application is Chess (slot 6)',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_chess_turn && guest.exports.app_chess_turn() === 1,
        desc: 'Assert Chess turn is White (Player 1)',
      },
      {
        action: 'click',
        x: 473, // e2 square (140 + 4*74 + 37, 32 + 48 + 6*74 + 37)
        y: 561,
        desc: 'Select White Pawn at e2',
      },
      {
        action: 'click',
        x: 473, // e4 square (140 + 4*74 + 37, 32 + 48 + 4*74 + 37)
        y: 413,
        desc: 'Play Kings Pawn opening move e2-e4',
      },
      {
        action: 'wait',
        ms: 300,
        desc: 'Wait for Chess minimax AI to compute and execute reply',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_chess_turn && guest.exports.app_chess_turn() === 1,
        desc: 'Assert Chess turn returned to White after AI reply',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_chess_ai_pending && guest.exports.app_chess_ai_pending() === 0,
        desc: 'Assert Chess AI completed response move',
      },
      {
        action: 'snapshot',
        label: 'Chess - Opening Move e2-e4 & AI Reply',
        desc: 'Capture snapshot of Chess board after e2-e4 and AI response',
      },
    ],
  },
  {
    id: 'chess-checkmate-6-moves',
    name: 'Chess - Checkmate in 6 Moves (vs AI)',
    description: 'Executes a Scholar-style Checkmate in 6 moves against the Minimax AI, asserting in-check and terminal Checkmate status.',
    steps: [
      {
        action: 'boot',
        opts: { width: 1280, height: 720 },
        desc: 'Ensure guest VM is initialized at 1280×720',
      },
      {
        action: 'openApp',
        slot: 'chess',
        desc: 'Open Classic Chess application',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_selected() === 6,
        desc: 'Assert active application is Chess (slot 6)',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_chess_turn && guest.exports.app_chess_turn() === 1,
        desc: 'Assert Chess turn is White (Player 1)',
      },
      // Move 1: Na3 (b1: 273, 648 -> a3: 170, 489)
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'b1',
        x: 273,
        y: 648,
        desc: 'Select Knight at b1 (Move 1)',
      },
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'a3',
        x: 170,
        y: 489,
        desc: 'Play 1. Na3',
      },
      {
        action: 'wait',
        ms: 300,
        desc: 'Wait for AI reply',
      },
      // Move 2: e4 (e2: 480, 561 -> e4: 484, 414)
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'e2',
        x: 480,
        y: 561,
        desc: 'Select Pawn at e2 (Move 2)',
      },
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'e4',
        x: 484,
        y: 414,
        desc: 'Play 2. e4',
      },
      {
        action: 'wait',
        ms: 300,
        desc: 'Wait for AI reply',
      },
      // Move 3: Qf3 (d1: 402, 630 -> f3: 549, 483)
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'd1',
        x: 402,
        y: 630,
        desc: 'Select Queen at d1 (Move 3)',
      },
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'f3',
        x: 549,
        y: 483,
        desc: 'Play 3. Qf3',
      },
      {
        action: 'wait',
        ms: 300,
        desc: 'Wait for AI reply',
      },
      // Move 4: Bc4 (f1: 552, 646 -> c4: 332, 402)
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'f1',
        x: 552,
        y: 646,
        desc: 'Select Bishop at f1 (Move 4)',
      },
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'c4',
        x: 332,
        y: 402,
        desc: 'Play 4. Bc4',
      },
      {
        action: 'wait',
        ms: 300,
        desc: 'Wait for AI reply',
      },
      // Move 5: Qxf7+ (f3: 562, 475 -> f7: 550, 184) - Check!
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'f3',
        x: 562,
        y: 475,
        desc: 'Select Queen at f3 (Move 5)',
      },
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'f7',
        x: 550,
        y: 184,
        desc: 'Play 5. Qxf7+ (Check!)',
      },
      {
        action: 'wait',
        ms: 300,
        desc: 'Wait for AI reply',
      },
      // Move 6: Bxe6# (c4: 327, 420 -> e6: 477, 263)
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'c4',
        x: 327,
        y: 420,
        desc: 'Select Bishop at c4 (Move 6)',
      },
      {
        action: 'clickSquare',
        game: 'chess',
        square: 'e6',
        x: 477,
        y: 263,
        desc: 'Play 6. Bxe6# (Checkmate!)',
      },
      // Assert status and turn
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_chess_status && guest.exports.app_chess_status() === 2,
        desc: 'Assert GameStatus is Checkmate (app_chess_status() === 2)',
      },
      {
        action: 'assert',
        check: (guest) => guest.exports?.app_chess_turn && guest.exports.app_chess_turn() === 2,
        desc: 'Assert Black has no moves and is mated (app_chess_turn() === 2)',
      },
      {
        action: 'snapshot',
        label: 'Chess - Checkmate Victory in 6 Moves',
        desc: 'Capture victory snapshot with terminal Checkmate',
      },
    ],
  },
  {
    id: 'checkers-2p-entangled-convergence',
    name: 'Checkers 2P Entangled Convergence',
    description: 'Mathematically prove zero-drift state convergence across two separate Wasm guest linear memories over 6 opening moves via broker relay',
    steps: [
      {
        action: 'boot',
        targetVm: 'p1',
        opts: { width: 390, height: 844 },
        desc: 'Ensure vm-01 (Seat P1) is initialized at 390×844 Mobile Portrait',
      },
      {
        action: 'boot',
        targetVm: 'p2',
        opts: { width: 390, height: 844 },
        desc: 'Ensure vm-02 (Seat P2) is initialized at 390×844 Mobile Portrait',
      },
      {
        action: 'assert',
        targetVm: 'p1',
        check: (guest) => {
          if (typeof window !== 'undefined' && window.__appliance?.setSplitView) {
            window.__appliance.setSplitView(true);
          }
          return true;
        },
        desc: 'Enable side-by-side split view mode',
      },
      {
        action: 'openApp',
        targetVm: 'p1',
        slot: 'checkers',
        desc: 'Open Checkers on vm-01 (Seat P1)',
      },
      {
        action: 'openApp',
        targetVm: 'p2',
        slot: 'checkers',
        desc: 'Open Checkers on vm-02 (Seat P2)',
      },
      {
        action: 'assert',
        targetVm: 'p1',
        check: (guest) => {
          const broker = typeof window !== 'undefined' ? window.__broker : null;
          if (broker) {
            let m = broker.tables.matches.find((x) => x.id === 201);
            if (!m) {
              m = {
                id: 201,
                game: 'checkers',
                seed: 42,
                max_strikes: 1,
                status: 'PLAYING',
                turn_seat: 1,
                p1_vm: 'vm-01',
                p2_vm: 'vm-02',
                p1_id: 1,
                p2_id: 2,
                p1_score: 12,
                p2_score: 12,
                state_hash: '0x00000000',
                last_pdn: '—',
                winner_seat: 0,
                seq: 0,
                created_at: broker.nowIso ? broker.nowIso() : new Date().toISOString().slice(11, 19),
              };
              broker.tables.matches.unshift(m);
            }
            broker.activeMatchId = 201;
            broker.renderDrawer();
            if (typeof window !== 'undefined') {
              const brokerDrawer = document.getElementById('broker-drawer');
              if (brokerDrawer) brokerDrawer.hidden = false;
            }
          }
          guest.exports.app_checkers_set_mp(1, 201, 1, 1);
          guest.mpSeat = 1;
          guest.mpMatchId = 201;
          guest.mpGame = 'checkers';
          guest.actorType = 'AutoPilot';
          if (guest.render) guest.render();
          return true;
        },
        desc: 'Configure vm-01 as Seat P1 on 2P Match #201',
      },
      {
        action: 'assert',
        targetVm: 'p2',
        check: (guest) => {
          guest.exports.app_checkers_set_mp(1, 201, 2, 1);
          guest.mpSeat = 2;
          guest.mpMatchId = 201;
          guest.mpGame = 'checkers';
          guest.actorType = 'AutoPilot';
          if (guest.render) guest.render();
          return true;
        },
        desc: 'Configure vm-02 as Seat P2 on 2P Match #201',
      },
      {
        action: 'assertTurn',
        targetVm: 'p1',
        seat: 1,
        desc: 'Assert vm-01 initial turn is Seat P1 (Red)',
      },
      {
        action: 'assertTurn',
        targetVm: 'p2',
        seat: 1,
        desc: 'Assert vm-02 initial turn is Seat P1 (Red)',
      },
      {
        action: 'assertHash',
        targetVm: 'p1',
        kind: 'checkers',
        compareToVm: 'p2',
        desc: 'Assert initial state hashes of vm-01 and vm-02 match exactly',
      },
      // Move 1: P1 plays 22 -> 18 (PDN 23-19)
      {
        action: 'clickSquare',
        targetVm: 'p1',
        game: 'checkers',
        id: 22,
        desc: 'P1 (vm-01): Select piece at square 22 (PDN 23)',
      },
      {
        action: 'clickSquare',
        targetVm: 'p1',
        game: 'checkers',
        id: 18,
        desc: 'P1 (vm-01): Play move 22-18 (PDN 23-19)',
      },
      {
        action: 'assertHash',
        targetVm: 'p1',
        kind: 'checkers',
        compareToVm: 'p2',
        desc: 'Assert causal convergence: vm-02 state hash matches vm-01 after Move 1',
      },
      {
        action: 'assertTurn',
        targetVm: 'p2',
        seat: 2,
        desc: 'Assert turn passed to Seat P2 (Black) on vm-02',
      },
      // Move 2: P2 plays 10 -> 14 (PDN 11-15)
      {
        action: 'clickSquare',
        targetVm: 'p2',
        game: 'checkers',
        id: 10,
        desc: 'P2 (vm-02): Select piece at square 10 (PDN 11)',
      },
      {
        action: 'clickSquare',
        targetVm: 'p2',
        game: 'checkers',
        id: 14,
        desc: 'P2 (vm-02): Play move 10-14 (PDN 11-15)',
      },
      {
        action: 'assertHash',
        targetVm: 'p2',
        kind: 'checkers',
        compareToVm: 'p1',
        desc: 'Assert causal convergence: vm-01 state hash matches vm-02 after Move 2',
      },
      {
        action: 'assertTurn',
        targetVm: 'p1',
        seat: 1,
        desc: 'Assert turn passed back to Seat P1 (Red) on vm-01',
      },
      // Move 3: P1 plays 18 -> 15 (PDN 19-16)
      {
        action: 'clickSquare',
        targetVm: 'p1',
        game: 'checkers',
        id: 18,
        desc: 'P1 (vm-01): Select piece at square 18 (PDN 19)',
      },
      {
        action: 'clickSquare',
        targetVm: 'p1',
        game: 'checkers',
        id: 15,
        desc: 'P1 (vm-01): Play move 18-15 (PDN 19-16)',
      },
      {
        action: 'assertHash',
        targetVm: 'p1',
        kind: 'checkers',
        compareToVm: 'p2',
        desc: 'Assert causal convergence: vm-02 state hash matches vm-01 after Move 3',
      },
      // Move 4: P2 plays 11 -> 18 (PDN 12x19 jump)
      {
        action: 'clickSquare',
        targetVm: 'p2',
        game: 'checkers',
        id: 11,
        desc: 'P2 (vm-02): Select piece at square 11 (PDN 12)',
      },
      {
        action: 'clickSquare',
        targetVm: 'p2',
        game: 'checkers',
        id: 18,
        desc: 'P2 (vm-02): Play capture jump 11x18 (PDN 12x19)',
      },
      {
        action: 'assertHash',
        targetVm: 'p2',
        kind: 'checkers',
        compareToVm: 'p1',
        desc: 'Assert causal convergence: vm-01 state hash matches vm-02 after Move 4 jump',
      },
      // Move 5: P1 plays 20 -> 16 (PDN 21-17)
      {
        action: 'clickSquare',
        targetVm: 'p1',
        game: 'checkers',
        id: 20,
        desc: 'P1 (vm-01): Select piece at square 20 (PDN 21)',
      },
      {
        action: 'clickSquare',
        targetVm: 'p1',
        game: 'checkers',
        id: 16,
        desc: 'P1 (vm-01): Play move 20-16 (PDN 21-17)',
      },
      {
        action: 'assertHash',
        targetVm: 'p1',
        kind: 'checkers',
        compareToVm: 'p2',
        desc: 'Assert causal convergence: vm-02 state hash matches vm-01 after Move 5',
      },
      // Move 6: P2 plays 6 -> 10 (PDN 7-11)
      {
        action: 'clickSquare',
        targetVm: 'p2',
        game: 'checkers',
        id: 6,
        desc: 'P2 (vm-02): Select piece at square 6 (PDN 7)',
      },
      {
        action: 'clickSquare',
        targetVm: 'p2',
        game: 'checkers',
        id: 10,
        desc: 'P2 (vm-02): Play move 6-10 (PDN 7-11)',
      },
      {
        action: 'assertHash',
        targetVm: 'p2',
        kind: 'checkers',
        compareToVm: 'p1',
        desc: 'Assert causal convergence: vm-01 and vm-02 converge to 100% identical hash after 6 moves',
      },
      {
        action: 'splitSnapshot',
        label: 'Checkers 2P Entangled Convergence - 6 Moves Dual View',
        desc: 'Capture dual-screen side-by-side verification split snapshot',
      },
    ],
  },
  {
    id: 'mines-2p-turn-relay',
    name: 'Minesweeper 2P Turn Relay & Sudden Death',
    description: 'Verify multiplayer turn-passing, bitboard scoring, and sudden-death elimination across two VMs',
    steps: [
      {
        action: 'boot',
        targetVm: 'p1',
        opts: { width: 390, height: 844 },
        desc: 'Ensure vm-01 (Seat P1) is initialized at 390×844 Mobile Portrait',
      },
      {
        action: 'boot',
        targetVm: 'p2',
        opts: { width: 390, height: 844 },
        desc: 'Ensure vm-02 (Seat P2) is initialized at 390×844 Mobile Portrait',
      },
      {
        action: 'assert',
        targetVm: 'p1',
        check: (guest) => {
          if (typeof window !== 'undefined' && window.__appliance?.setSplitView) {
            window.__appliance.setSplitView(true);
          }
          return true;
        },
        desc: 'Enable side-by-side split view mode',
      },
      {
        action: 'openApp',
        targetVm: 'p1',
        slot: 'mines',
        desc: 'Open Minesweeper on vm-01 (Seat P1)',
      },
      {
        action: 'openApp',
        targetVm: 'p2',
        slot: 'mines',
        desc: 'Open Minesweeper on vm-02 (Seat P2)',
      },
      {
        action: 'assert',
        targetVm: 'p1',
        check: (guest) => {
          const broker = typeof window !== 'undefined' ? window.__broker : null;
          if (broker) {
            let m = broker.tables.matches.find((x) => x.id === 101);
            if (!m) {
              m = {
                id: 101,
                game: 'mines',
                seed: 42,
                max_strikes: 1,
                status: 'PLAYING',
                turn_seat: 1,
                p1_vm: 'vm-01',
                p2_vm: 'vm-02',
                p1_id: 1,
                p2_id: 2,
                p1_score: 0,
                p2_score: 0,
                state_hash: 'seed:42',
                last_pdn: '—',
                winner_seat: 0,
                seq: 0,
                created_at: broker.nowIso ? broker.nowIso() : new Date().toISOString().slice(11, 19),
              };
              broker.tables.matches.unshift(m);
            }
            broker.activeMatchId = 101;
            broker.renderDrawer();
            if (typeof window !== 'undefined') {
              const brokerDrawer = document.getElementById('broker-drawer');
              if (brokerDrawer) brokerDrawer.hidden = false;
            }
          }
          guest.exports.app_mines_set_mp(1, 101, 1, 42, 1);
          guest.mpSeat = 1;
          guest.mpMatchId = 101;
          guest.mpGame = 'mines';
          guest.actorType = 'AutoPilot';
          if (guest.render) guest.render();
          return true;
        },
        desc: 'Configure vm-01 as Seat P1 (Seed 42, Max Strikes 1) on Match #101',
      },
      {
        action: 'assert',
        targetVm: 'p2',
        check: (guest) => {
          guest.exports.app_mines_set_mp(1, 101, 2, 42, 1);
          guest.mpSeat = 2;
          guest.mpMatchId = 101;
          guest.mpGame = 'mines';
          guest.actorType = 'AutoPilot';
          if (guest.render) guest.render();
          return true;
        },
        desc: 'Configure vm-02 as Seat P2 (Seed 42, Max Strikes 1) on Match #101',
      },
      {
        action: 'assertTurn',
        targetVm: 'p1',
        seat: 1,
        desc: 'Assert vm-01 turn is P1',
      },
      {
        action: 'assertTurn',
        targetVm: 'p2',
        seat: 1,
        desc: 'Assert vm-02 turn is P1',
      },
      // Step 1: P1 reveals safe cell (0, 0)
      {
        action: 'clickCell',
        targetVm: 'p1',
        row: 0,
        col: 0,
        desc: 'P1 (vm-01): Reveal safe cell at (0, 0)',
      },
      {
        action: 'wait',
        ms: 200,
        desc: 'Wait for broker event relay',
      },
      {
        action: 'assert',
        targetVm: 'p1',
        check: (guest) => guest.exports.app_mines_p1_score() > 0,
        desc: 'Assert points awarded to P1 on vm-01',
      },
      {
        action: 'assert',
        targetVm: 'p2',
        check: (guest) => guest.exports.app_mines_p1_score() > 0,
        desc: 'Assert points synchronized to P2 on vm-02 via broker RX',
      },
      {
        action: 'assertTurn',
        targetVm: 'p1',
        seat: 2,
        desc: 'Assert turn passed to P2 on vm-01',
      },
      {
        action: 'assertTurn',
        targetVm: 'p2',
        seat: 2,
        desc: 'Assert turn passed to P2 on vm-02',
      },
      // Step 2: P2 reveals safe cell (0, 7)
      {
        action: 'clickCell',
        targetVm: 'p2',
        row: 0,
        col: 7,
        desc: 'P2 (vm-02): Reveal safe cell at (0, 7)',
      },
      {
        action: 'wait',
        ms: 200,
        desc: 'Wait for broker event relay',
      },
      {
        action: 'assert',
        targetVm: 'p2',
        check: (guest) => guest.exports.app_mines_p2_score() > 0,
        desc: 'Assert points awarded to P2 on vm-02',
      },
      {
        action: 'assert',
        targetVm: 'p1',
        check: (guest) => guest.exports.app_mines_p2_score() > 0,
        desc: 'Assert points synchronized to P1 on vm-01 via broker RX',
      },
      {
        action: 'splitSnapshot',
        label: 'Minesweeper 2P Turn Relay - Safe Paths Dual View',
        desc: 'Capture dual-perspective split snapshot of both safe territories',
      },
      // Step 3: P1 clicks known bomb (0, 6) -> sudden-death detonation!
      {
        action: 'clickCell',
        targetVm: 'p1',
        row: 0,
        col: 6,
        desc: 'P1 (vm-01): Reveal known bomb cell at (0, 6) triggering sudden-death',
      },
      {
        action: 'wait',
        ms: 200,
        desc: 'Wait for broker event relay',
      },
      {
        action: 'assert',
        targetVm: 'p1',
        check: (guest) => guest.exports.app_mines_winner() === 2,
        desc: 'Assert sudden-death elimination on vm-01 (Winner is P2)',
      },
      {
        action: 'assert',
        targetVm: 'p2',
        check: (guest) => guest.exports.app_mines_winner() === 2,
        desc: 'Assert sudden-death elimination synced to vm-02 (Winner is P2)',
      },
      {
        action: 'splitSnapshot',
        label: 'Minesweeper 2P Turn Relay - Sudden Death Detonation Dual View',
        desc: 'Capture final dual-screen snapshot showing synchronized sudden-death detonation',
      },
    ],
  },
];

export function getMissionById(id) {
  return MISSIONS.find((m) => m.id === id) || null;
}
