/* Checkers adapter: serviceCheckers, serviceCheckersAi, serviceCheckersMultiplayer, launchCheckersInGuest, startCheckersMatch */

const MODS = { Shift: 1, Ctrl: 2, Alt: 4, Meta: 8 };

let _getBroker = () => (typeof window !== 'undefined' ? window.__broker : null);
let _getGuests = () => [];
let _createGuest = async () => null;
let _setSplitView = () => {};
let _activate = () => {};
let _setStatus = () => {};
let _fmtHex32 = (n) => `0x${(n >>> 0).toString(16).padStart(8, '0')}`;

export function configureCheckersDeps(deps = {}) {
  if (deps.getBroker) _getBroker = deps.getBroker;
  if (deps.getGuests) _getGuests = deps.getGuests;
  if (deps.createGuest) _createGuest = deps.createGuest;
  if (deps.setSplitView) _setSplitView = deps.setSplitView;
  if (deps.activate) _activate = deps.activate;
  if (deps.setStatus) _setStatus = deps.setStatus;
  if (deps.fmtHex32) _fmtHex32 = deps.fmtHex32;
}

export function decodeCheckersPdn(lo, hi, action) {
  if (action === 2) return 'reset';
  const hopCount = (lo >>> 8) & 0xf;
  const isJump = ((lo >>> 12) & 1) !== 0;
  if (hopCount < 2) return 'move';
  const sep = isJump ? 'x' : '-';
  const hops = [];
  for (let i = 0; i < Math.min(hopCount, 7); i++) {
    const idx =
      i < 3
        ? (lo >>> (16 + i * 5)) & 0x1f
        : (hi >>> ((i - 3) * 6)) & 0x1f;
    hops.push(idx + 1);
  }
  return hops.join(sep);
}

/* Service Checkers AI pacing (`app_checkers_ai_pending`) */
export function serviceCheckersAi(guest) {
  if (!guest?.exports) return;
  if (guest.exports.app_checkers_ai_pending && guest.exports.app_checkers_ai_pending() !== 0) {
    if (!guest.chkAiTimer) {
      guest.chkAiTimer = setTimeout(() => {
        guest.chkAiTimer = null;
        if (guest.disposed || !guest.exports?.app_checkers_step_ai) return;
        const t0 = performance.now();
        const rc = guest.exports.app_checkers_step_ai();
        const t1 = performance.now();
        if (rc !== 0 && guest.blitToCanvas) {
          guest.blitToCanvas(t0, t1);
          _setStatus(
            `${guest.name} (${guest.buildInfo.version}) · Checkers AI moved · ${_fmtHex32(guest.fbHash)}`,
            true,
          );
        }
      }, 260);
    }
  }
}

/* Service Checkers 2P move mailbox (`app_checkers_tx_pending`) */
export function serviceCheckersMultiplayer(guest) {
  if (!guest?.exports) return;
  if (!guest.exports.app_checkers_tx_pending || guest.exports.app_checkers_tx_pending() === 0) return;

  const broker = _getBroker();
  const lo = guest.exports.app_checkers_take_tx_lo ? guest.exports.app_checkers_take_tx_lo() >>> 0 : 0;
  const hi = guest.exports.app_checkers_take_tx_hi ? guest.exports.app_checkers_take_tx_hi() >>> 0 : 0;
  const action = lo & 0xf;
  const seat = ((lo >>> 4) & 0xf) || guest.mpSeat || 1;
  const pdn = decodeCheckersPdn(lo, hi, action);
  const matchId = guest.mpMatchId || (broker && broker.activeMatchId) || 1;

  if (broker) {
    broker.fetch('POST', `/api/matches/${matchId}/moves`, {
      game: 'checkers',
      source_vm_id: guest.id,
      vm_name: guest.name,
      seat,
      action,
      lo,
      hi,
      pdn,
    });
  }
}

/* Service Checkers combines both AI and Multiplayer servicing */
export function serviceCheckers(guest) {
  serviceCheckersAi(guest);
  serviceCheckersMultiplayer(guest);
}

/* Launch Checkers inside a guest via Ctrl+5 shortcut (`app_key_mods(15, 2)`) */
export function launchCheckersInGuest(guest) {
  if (!guest || !guest.exports) return;
  if (guest.exports.app_key_mods) {
    guest.exports.app_key_mods(15, MODS.Ctrl);
  } else {
    if (guest.exports.app_dock_visible() === 0) guest.clickLogical(16, 16);
    guest.clickLogical(76, 406);
  }
  guest.render();
}

/* Start a 2-Player English Draughts (Checkers) Match across two v3.1 guests */
export async function startCheckersMatch(_opts = {}) {
  const guests = _getGuests();
  let chkGuests = guests.filter((g) => g.exports && g.exports.app_checkers_set_mp);
  while (chkGuests.length < 2) {
    const g = await _createGuest({ buildId: 'build-03' });
    if (g && g.exports && g.exports.app_checkers_set_mp) {
      chkGuests.push(g);
    } else {
      break;
    }
  }
  if (chkGuests.length < 2) {
    _setStatus('Failed to start 2P Checkers match: need two v3.1 guests with Checkers ABI');
    return null;
  }

  const broker = _getBroker();
  const [p1, p2] = chkGuests;
  const initHash = p1.exports.app_checkers_state_hash
    ? _fmtHex32(p1.exports.app_checkers_state_hash() >>> 0)
    : '0x00000000';

  const { json: match } = await broker.fetch('POST', '/api/matches', {
    game: 'checkers',
    state_hash: initHash,
    p1_vm: p1.name,
    p2_vm: p2.name,
    p1_id: p1.id,
    p2_id: p2.id,
  });

  p1.mpGame = 'checkers';
  p1.mpMatchId = match.id;
  p1.mpSeat = 1;
  p2.mpGame = 'checkers';
  p2.mpMatchId = match.id;
  p2.mpSeat = 2;

  p1.exports.app_checkers_set_mp(1, match.id, 1);
  p1.render();
  p2.exports.app_checkers_set_mp(1, match.id, 2);
  p2.render();

  if (p1.exports.app_checkers_state_hash) {
    match.state_hash = _fmtHex32(p1.exports.app_checkers_state_hash() >>> 0);
    broker.renderDrawer();
  }

  _setSplitView(true);
  const brokerDrawer = document.getElementById('broker-drawer');
  if (brokerDrawer) brokerDrawer.hidden = false;
  _activate(p1.id);
  _setStatus(
    `2P Checkers Match #${match.id} started · ${p1.name} (Seat P1 Red, Your Turn) vs ${p2.name} (Seat P2 Black)`,
    true,
  );
  return match;
}
