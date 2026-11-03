/* Mines adapter: serviceMinesMultiplayer, launchMinesInGuest, startTwoPlayerMatch */
import { syncPlayerAvatarsToGuest } from '../services/avatar.js';

const MODS = { Shift: 1, Ctrl: 2, Alt: 4, Meta: 8 };

let _getBroker = () => (typeof window !== 'undefined' ? window.__broker : null);
let _getGuests = () => [];
let _createGuest = async () => null;
let _setSplitView = () => {};
let _activate = () => {};
let _setStatus = () => {};
let _getDefaultBuild = () => 'build-03';

export function configureMinesDeps(deps = {}) {
  if (deps.getBroker) _getBroker = deps.getBroker;
  if (deps.getGuests) _getGuests = deps.getGuests;
  if (deps.createGuest) _createGuest = deps.createGuest;
  if (deps.setSplitView) _setSplitView = deps.setSplitView;
  if (deps.activate) _activate = deps.activate;
  if (deps.setStatus) _setStatus = deps.setStatus;
  if (deps.getDefaultBuild) _getDefaultBuild = deps.getDefaultBuild;
}

/* Service outbound 2P Minesweeper move mailbox (`app_mines_tx_pending`) */
export function serviceMinesMultiplayer(guest) {
  if (!guest?.exports?.app_mines_tx_pending || !guest.exports.app_mines_take_tx) return;
  if (guest.exports.app_mines_tx_pending() === 0) return;

  const broker = _getBroker();
  const packed = guest.exports.app_mines_take_tx() >>> 0;
  const action = packed & 0xff;
  const cell = (packed >>> 8) & 0xff;
  const seat = ((packed >>> 16) & 0xff) || guest.mpSeat || 1;
  const maxStrikes = ((packed >>> 24) & 0xff) || 1;
  const seed = guest.exports.app_mines_seed ? guest.exports.app_mines_seed() >>> 0 : 42;
  const matchId = guest.mpMatchId || (broker && broker.activeMatchId) || 1;

  if (broker) {
    broker.fetch('POST', `/api/matches/${matchId}/moves`, {
      game: 'mines',
      source_vm_id: guest.id,
      vm_name: guest.name,
      seat,
      action,
      cell,
      seed,
      max_strikes: maxStrikes,
    });
  }
}

/* Launch Minesweeper inside a guest via Ctrl+4 shortcut (`app_key_mods(14, 2)`) */
export function launchMinesInGuest(guest) {
  if (!guest || !guest.exports) return;
  if (guest.exports.app_key_mods) {
    guest.exports.app_key_mods(14, MODS.Ctrl);
  } else {
    if (guest.exports.app_dock_visible() === 0) guest.clickLogical(16, 16);
    guest.clickLogical(76, 330);
  }
  guest.render();
}

/* Start a 2-Player Turn-Based Minesweeper Match across two v3 guests */
export async function startTwoPlayerMatch(opts = {}) {
  const guests = _getGuests();
  let v3Guests = guests.filter((g) => g.exports && g.exports.app_mines_set_mp);
  while (v3Guests.length < 2) {
    const defaultBuild = _getDefaultBuild() || 'build-03';
    const g = await _createGuest({ buildId: defaultBuild });
    if (g && g.exports && g.exports.app_mines_set_mp) {
      v3Guests.push(g);
    } else {
      break;
    }
  }
  if (v3Guests.length < 2) {
    _setStatus('Failed to start 2P match: need two v3 guests with multiplayer ABI');
    return null;
  }

  const broker = _getBroker();
  const [p1, p2] = v3Guests;
  const seed = opts.seed ?? (broker.tables.matches.length === 0 ? 42 : 42 + broker.tables.matches.length);
  const maxStrikes = opts.maxStrikes ?? 1;

  const { json: match } = await broker.fetch('POST', '/api/matches', {
    game: 'mines',
    seed,
    max_strikes: maxStrikes,
    p1_vm: p1.name,
    p2_vm: p2.name,
    p1_id: p1.id,
    p2_id: p2.id,
  });

  p1.mpGame = 'mines';
  p1.mpMatchId = match.id;
  p1.mpSeat = 1;
  p2.mpGame = 'mines';
  p2.mpMatchId = match.id;
  p2.mpSeat = 2;

  p1.exports.app_mines_set_mp(1, match.id, 1, seed, maxStrikes);
  p2.exports.app_mines_set_mp(1, match.id, 2, seed, maxStrikes);

  syncPlayerAvatarsToGuest(p1, p1.name || 'tester-01', p2.name || 'tester-02');
  syncPlayerAvatarsToGuest(p2, p1.name || 'tester-01', p2.name || 'tester-02');

  p1.render();
  p2.render();

  _setSplitView(true);
  const brokerDrawer = document.getElementById('broker-drawer');
  if (brokerDrawer) brokerDrawer.hidden = false;
  _activate(p1.id);
  _setStatus(
    `2P Match #${match.id} started (Seed ${seed}) · ${p1.name} (Seat P1, Your Turn) vs ${p2.name} (Seat P2)`,
    true,
  );
  return match;
}
