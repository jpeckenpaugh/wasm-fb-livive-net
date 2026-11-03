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
];

export function getMissionById(id) {
  return MISSIONS.find((m) => m.id === id) || null;
}
