/* Chess adapter: serviceChessMultiplayer, serviceChessAi, launchChessInGuest, startChessMatch */
import { syncPlayerAvatarsToGuest } from '../services/avatar.js';

const MODS = { Shift: 1, Ctrl: 2, Alt: 4, Meta: 8 };

let _getBroker = () => (typeof window !== 'undefined' ? window.__broker : null);
let _getGuests = () => [];
let _createGuest = async () => null;
let _setSplitView = () => {};
let _activate = () => {};
let _setStatus = () => {};
let _fmtHex32 = (n) => `0x${(n >>> 0).toString(16).padStart(8, '0')}`;

export function configureChessDeps(deps = {}) {
  if (deps.getBroker) _getBroker = deps.getBroker;
  if (deps.getGuests) _getGuests = deps.getGuests;
  if (deps.createGuest) _createGuest = deps.createGuest;
  if (deps.setSplitView) _setSplitView = deps.setSplitView;
  if (deps.activate) _activate = deps.activate;
  if (deps.setStatus) _setStatus = deps.setStatus;
  if (deps.fmtHex32) _fmtHex32 = deps.fmtHex32;
}

export function decodeChessUci(from, to, promo, action) {
  if (action === 2) return 'reset';
  const files = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
  const f1 = files[from & 7] || 'a';
  const r1 = (from >> 3) + 1;
  const f2 = files[to & 7] || 'a';
  const r2 = (to >> 3) + 1;
  const pr = promo === 5 ? 'q' : promo === 4 ? 'r' : promo === 3 ? 'b' : promo === 2 ? 'n' : '';
  return `${f1}${r1}${f2}${r2}${pr}`;
}

/* Service Chess AI pacing (`app_chess_ai_pending`) */
export function serviceChessAi(guest) {
  if (!guest?.exports) return;
  if (guest.exports.app_chess_ai_pending && guest.exports.app_chess_ai_pending() !== 0) {
    if (!guest.chsAiTimer) {
      guest.chsAiTimer = setTimeout(() => {
        guest.chsAiTimer = null;
        if (guest.disposed || !guest.exports?.app_chess_step_ai) return;
        const t0 = performance.now();
        const rc = guest.exports.app_chess_step_ai();
        const t1 = performance.now();
        if (rc !== 0 && guest.blitToCanvas) {
          guest.blitToCanvas(t0, t1);
          _setStatus(
            `${guest.name} (${guest.buildInfo.version}) · Chess AI moved · ${_fmtHex32(guest.fbHash)}`,
            true,
          );
        }
      }, 260);
    }
  }
}

/* Service Chess 2P move mailbox (`app_chess_tx_pending` & `app_chess_take_tx`) */
export function serviceChessMultiplayer(guest) {
  if (!guest?.exports) return;
  if (!guest.exports.app_chess_tx_pending || guest.exports.app_chess_tx_pending() === 0) return;

  const broker = _getBroker();
  const word = guest.exports.app_chess_take_tx ? (guest.exports.app_chess_take_tx() >>> 0) : 0;
  const action = word & 0x0f;
  const promo = (word >>> 4) & 0x07;
  const seat = ((word >>> 8) & 0xff) || guest.mpSeat || 1;
  const from = (word >>> 16) & 0xff;
  const to = (word >>> 24) & 0xff;

  const uci = decodeChessUci(from, to, promo, action);
  const matchId = guest.mpMatchId || (broker && broker.activeMatchId) || 1;

  if (broker) {
    broker.fetch('POST', `/api/matches/${matchId}/moves`, {
      game: 'chess',
      source_vm_id: guest.id,
      vm_name: guest.name,
      seat,
      action,
      from,
      to,
      promo,
      uci,
    });
  }
}

/* Launch Chess inside a guest via Ctrl+6 shortcut (`app_key_mods(16, 2)`) */
export function launchChessInGuest(guest) {
  if (!guest || !guest.exports) return;
  if (guest.exports.app_key_mods) {
    guest.exports.app_key_mods(16, MODS.Ctrl);
  } else {
    if (guest.exports.app_dock_visible() === 0) guest.clickLogical(16, 16);
    guest.clickLogical(76, 482);
  }
  guest.render();
}

/* Start a 2-Player Classic Chess Match across two v3.2 guests */
export async function startChessMatch(_opts = {}) {
  const guests = _getGuests();
  let chsGuests = guests.filter((g) => g.exports && g.exports.app_chess_set_mp);
  while (chsGuests.length < 2) {
    const g = await _createGuest({ buildId: 'build-03' });
    if (g && g.exports && g.exports.app_chess_set_mp) {
      chsGuests.push(g);
    } else {
      break;
    }
  }
  if (chsGuests.length < 2) {
    _setStatus('Failed to start 2P Chess match: need two v3.2 guests with Chess ABI');
    return null;
  }

  const broker = _getBroker();
  const [p1, p2] = chsGuests;
  const initHash = p1.exports.app_chess_state_hash
    ? _fmtHex32(p1.exports.app_chess_state_hash() >>> 0)
    : '0x00000000';

  const { json: match } = await broker.fetch('POST', '/api/matches', {
    game: 'chess',
    state_hash: initHash,
    p1_vm: p1.name,
    p2_vm: p2.name,
    p1_id: p1.id,
    p2_id: p2.id,
  });

  p1.mpGame = 'chess';
  p1.mpMatchId = match.id;
  p1.mpSeat = 1;
  p2.mpGame = 'chess';
  p2.mpMatchId = match.id;
  p2.mpSeat = 2;

  p1.exports.app_chess_set_mp(1, match.id, 1, 0);
  p2.exports.app_chess_set_mp(1, match.id, 2, 0);

  syncPlayerAvatarsToGuest(p1, p1.name || 'tester-01', p2.name || 'tester-02');
  syncPlayerAvatarsToGuest(p2, p1.name || 'tester-01', p2.name || 'tester-02');

  p1.render();
  p2.render();

  if (p1.exports.app_chess_state_hash) {
    match.state_hash = _fmtHex32(p1.exports.app_chess_state_hash() >>> 0);
    broker.renderDrawer();
  }

  _setSplitView(true);
  const brokerDrawer = document.getElementById('broker-drawer');
  if (brokerDrawer) brokerDrawer.hidden = false;
  _activate(p1.id);
  _setStatus(
    `2P Chess Match #${match.id} started · ${p1.name} (Seat P1 White, Your Turn) vs ${p2.name} (Seat P2 Black)`,
    true,
  );
  return match;
}
