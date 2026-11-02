/* FastApiSqliteBroker: Zero-Setup In-Browser FastAPI + SQLite Turn-Based Match Broker & SSE Network Relay */

export class FastApiSqliteBroker {
  constructor(deps = {}) {
    this.tables = {
      matches: [],
      match_events: [],
    };
    this.nextMatchId = 1;
    this.nextEventId = 1;
    this.activeMatchId = null;
    this.mode = 'local'; // 'local' or 'network'
    this.relayUrl = (typeof window !== 'undefined' && window.location) ? window.location.origin : '';
    this.clientId = 'client-' + Math.random().toString(36).slice(2, 9);
    this.sseSource = null;

    // Optional callbacks/getters for host decoupling
    this._getGuests = deps.getGuests || (() => (typeof window !== 'undefined' && window.__appliance_guests) || []);
    this._updateHostStats = deps.updateHostStats || (() => {});
    this._updateGuestBanner = deps.updateGuestBanner || (() => {});
    this._fmtHex32 = deps.fmtHex32 || ((n) => `0x${(n >>> 0).toString(16).padStart(8, '0')}`);
  }

  setDeps(deps = {}) {
    if (deps.getGuests) this._getGuests = deps.getGuests;
    if (deps.updateHostStats) this._updateHostStats = deps.updateHostStats;
    if (deps.updateGuestBanner) this._updateGuestBanner = deps.updateGuestBanner;
    if (deps.fmtHex32) this._fmtHex32 = deps.fmtHex32;
  }

  nowIso() {
    return new Date().toISOString().slice(11, 19);
  }

  setNetworkMode(enabled) {
    this.mode = enabled ? 'network' : 'local';
    const badge = document.getElementById('net-relay-status');
    if (badge) {
      badge.textContent = enabled ? 'Mode: Network Relay (Live SSE)' : 'Mode: Local Memory';
      badge.style.color = enabled ? '#34d399' : '#38bdf8';
      badge.style.borderColor = enabled ? '#34d399' : '#38bdf8';
    }
  }

  logEvent(matchId, seq, vmName, seat, action, cell, httpLine) {
    const row = {
      id: this.nextEventId++,
      match_id: matchId,
      seq,
      vm_name: vmName,
      seat,
      action,
      cell,
      http: httpLine,
      created_at: this.nowIso(),
    };
    this.tables.match_events.unshift(row);
    if (this.tables.match_events.length > 64) {
      this.tables.match_events.length = 64;
    }
    this.renderDrawer();
    return row;
  }

  connectSse(matchId) {
    if (this.sseSource) {
      this.sseSource.close();
      this.sseSource = null;
    }
    this.activeMatchId = matchId;
    this.setNetworkMode(true);

    try {
      const url = `${this.relayUrl}/api/matches/${matchId}/events`;
      const sse = new EventSource(url);
      this.sseSource = sse;

      sse.addEventListener('init', (e) => {
        try {
          const matchData = JSON.parse(e.data);
          const idx = this.tables.matches.findIndex((m) => m.id === matchData.id);
          if (idx >= 0) {
            this.tables.matches[idx] = matchData;
          } else {
            this.tables.matches.unshift(matchData);
          }
          this.renderDrawer();
          this._updateHostStats();
        } catch (err) {
          console.error('[SSE init error]', err);
        }
      });

      sse.addEventListener('move', (e) => {
        try {
          const ev = JSON.parse(e.data);
          if (ev.source_client_id === this.clientId) {
            return; // ignore echo of our own move
          }
          this.applyIncomingMove(ev);
        } catch (err) {
          console.error('[SSE move error]', err);
        }
      });

      sse.onerror = (err) => {
        console.warn('[SSE connection error or closed]', err);
      };
    } catch (err) {
      console.warn('Failed to initialize EventSource:', err);
    }
  }

  applyIncomingMove(ev) {
    const matchId = ev.matchId;
    let match = this.tables.matches.find((m) => m.id === matchId);
    if (!match) {
      match = {
        id: matchId,
        game: ev.game,
        status: ev.status || 'PLAYING',
        seq: ev.seq,
        turn_seat: ev.turn_seat,
        p1_score: ev.p1_score,
        p2_score: ev.p2_score,
        state_hash: ev.state_hash,
      };
      this.tables.matches.unshift(match);
    } else {
      match.seq = ev.seq;
      match.turn_seat = ev.turn_seat;
      match.p1_score = ev.p1_score;
      match.p2_score = ev.p2_score;
      match.status = ev.status;
      if (ev.state_hash) match.state_hash = ev.state_hash;
      if (ev.pdn) match.last_pdn = ev.pdn;
    }

    if (ev.log) {
      this.tables.match_events.unshift(ev.log);
      if (this.tables.match_events.length > 64) this.tables.match_events.length = 64;
    }

    const guests = this._getGuests();
    if (ev.game === 'chess') {
      const peers = guests.filter(
        (g) => g.mpMatchId === matchId && g.exports && g.exports.app_chess_apply_rx,
      );
      for (const peer of peers) {
        peer.exports.app_chess_apply_rx(ev.seq, ev.seat, ev.action, ev.from, ev.to, ev.promo);
        peer.render();
        this._updateGuestBanner(peer);
      }
    } else if (ev.game === 'checkers') {
      const peers = guests.filter(
        (g) => g.mpMatchId === matchId && g.exports && g.exports.app_checkers_apply_rx,
      );
      for (const peer of peers) {
        peer.exports.app_checkers_apply_rx(ev.seq, ev.seat, ev.action, ev.lo, ev.hi);
        peer.render();
        this._updateGuestBanner(peer);
      }
    } else {
      const peers = guests.filter(
        (g) => g.mpMatchId === matchId && g.exports && g.exports.app_mines_apply_rx,
      );
      for (const peer of peers) {
        peer.exports.app_mines_apply_rx(ev.seq, ev.seat, ev.action, ev.cell, ev.seed, ev.max_strikes);
        peer.render();
        this._updateGuestBanner(peer);
      }
    }

    this.renderDrawer();
    this._updateHostStats();
  }

  /* Dual-mode fetch: POST/GET to network relay server or fallback to local in-memory */
  async fetch(method, path, body = {}) {
    body.source_client_id = this.clientId;

    if (this.mode === 'network') {
      try {
        const res = await window.fetch(`${this.relayUrl}${path}`, {
          method,
          headers: { 'Content-Type': 'application/json' },
          body: method === 'GET' ? undefined : JSON.stringify(body),
        });
        if (res.ok) {
          const json = await res.json();
          if (path === '/api/matches' && method === 'POST') {
            const idx = this.tables.matches.findIndex((m) => m.id === json.id);
            if (idx >= 0) this.tables.matches[idx] = json;
            else this.tables.matches.unshift(json);
            this.activeMatchId = json.id;
            this.renderDrawer();
            this._updateHostStats();
          } else if (path.includes('/moves') && method === 'POST') {
            const idx = this.tables.matches.findIndex((m) => m.id === json.id);
            if (idx >= 0) this.tables.matches[idx] = json;
            this.renderDrawer();
          }
          return { status: res.status, json };
        }
      } catch (err) {
        console.warn('[Broker Network Relay error, falling back to local]', err);
      }
    }

    const guests = this._getGuests();

    // Local in-memory implementation
    if (method === 'POST' && path === '/api/matches') {
      const id = this.nextMatchId++;
      const game = body.game || 'mines';
      const seed = (body.seed ?? 42) >>> 0;
      const max_strikes = body.max_strikes || 1;
      const match = {
        id,
        game,
        seed,
        max_strikes,
        status: 'PLAYING',
        turn_seat: 1,
        p1_vm: body.p1_vm || 'vm-01',
        p2_vm: body.p2_vm || 'vm-02',
        p1_id: body.p1_id || 1,
        p2_id: body.p2_id || 2,
        p1_score: game === 'chess' ? 16 : game === 'checkers' ? 12 : 0,
        p2_score: game === 'chess' ? 16 : game === 'checkers' ? 12 : 0,
        state_hash: body.state_hash || (game === 'chess' || game === 'checkers' ? '0x00000000' : `seed:${seed}`),
        last_pdn: '—',
        winner_seat: 0,
        seq: 0,
        created_at: this.nowIso(),
      };
      this.tables.matches.unshift(match);
      this.activeMatchId = id;
      this.logEvent(
        id,
        0,
        `${match.p1_vm},${match.p2_vm}`,
        0,
        'CREATE',
        game,
        `POST /api/matches → 201 Created (match #${id}, game=${game}${game === 'mines' ? `, seed=${seed}` : ''})`,
      );
      this.renderDrawer();
      this._updateHostStats();
      return { status: 201, json: match };
    }

    const moveMatch = path.match(/^\/api\/matches\/(\d+)\/moves$/);
    if (method === 'POST' && moveMatch) {
      const matchId = parseInt(moveMatch[1], 10);
      const match = this.tables.matches.find((m) => m.id === matchId);
      if (!match) return { status: 404, json: { error: 'match not found' } };

      const game = body.game || match.game || 'mines';
      match.game = game;

      if (game === 'chess') {
        const actionCode = body.action ?? 1;
        const actionName = actionCode === 2 ? 'NEW_GAME' : 'MOVE';
        const seat = body.seat ?? 1;
        const from = body.from ?? 0;
        const to = body.to ?? 0;
        const promo = body.promo ?? 0;
        const uci = body.uci || body.pdn || (actionCode === 2 ? 'reset' : 'move');

        match.seq += 1;
        const seq = match.seq;
        match.last_pdn = uci;

        const peers = guests.filter(
          (g) => g.mpMatchId === matchId && g.exports && g.exports.app_chess_apply_rx,
        );
        for (const peer of peers) {
          if (peer.id !== body.source_vm_id) {
            peer.exports.app_chess_apply_rx(seq, seat, actionCode, from, to, promo);
            peer.render();
          }
        }

        const refGuest = peers[0];
        if (refGuest && refGuest.exports) {
          if (refGuest.exports.app_chess_turn) {
            match.turn_seat = refGuest.exports.app_chess_turn();
          }
          if (refGuest.exports.app_chess_white_count) {
            match.p1_score = refGuest.exports.app_chess_white_count();
            match.p2_score = refGuest.exports.app_chess_black_count();
          }
          if (refGuest.exports.app_chess_state_hash) {
            match.state_hash = this._fmtHex32(refGuest.exports.app_chess_state_hash() >>> 0);
          }
          if (refGuest.exports.app_chess_status) {
            const st = refGuest.exports.app_chess_status();
            // 0: InPlay, 1: Check, 2: Checkmate, 3: Stalemate, 4: Draw50Move
            if (st === 2) {
              match.winner_seat = match.turn_seat === 1 ? 2 : 1;
              match.status = match.winner_seat === 1 ? 'WHITE_WON' : 'BLACK_WON';
            } else if (st === 3 || st === 4) {
              match.winner_seat = 3;
              match.status = 'DRAW';
            } else {
              match.winner_seat = 0;
              match.status = 'PLAYING';
            }
          }
        }

        this.logEvent(
          matchId,
          seq,
          body.vm_name || `P${seat}`,
          seat,
          actionName,
          uci,
          `POST /api/matches/${matchId}/moves · seq=${seq} seat=P${seat} ${actionName} (${uci}) hash=${match.state_hash} → 200 OK`,
        );
        for (const peer of peers) {
          this._updateGuestBanner(peer);
        }
        return { status: 200, json: match };
      }

      if (game === 'checkers') {
        const actionCode = body.action ?? 1;
        const actionName = actionCode === 2 ? 'NEW_GAME' : 'MOVE';
        const seat = body.seat ?? 1;
        const lo = (body.lo ?? 0) >>> 0;
        const hi = (body.hi ?? 0) >>> 0;
        const pdn = body.pdn || (actionCode === 2 ? 'reset' : 'move');

        match.seq += 1;
        const seq = match.seq;
        match.last_pdn = pdn;

        const peers = guests.filter(
          (g) => g.mpMatchId === matchId && g.exports && g.exports.app_checkers_apply_rx,
        );
        for (const peer of peers) {
          if (peer.id !== body.source_vm_id) {
            peer.exports.app_checkers_apply_rx(seq, seat, actionCode, lo, hi);
            peer.render();
          }
        }

        const refGuest = peers[0];
        if (refGuest && refGuest.exports) {
          if (refGuest.exports.app_checkers_turn) {
            match.turn_seat = refGuest.exports.app_checkers_turn();
          }
          if (refGuest.exports.app_checkers_red_count) {
            match.p1_score = refGuest.exports.app_checkers_red_count();
            match.p2_score = refGuest.exports.app_checkers_black_count();
          }
          if (refGuest.exports.app_checkers_state_hash) {
            match.state_hash = this._fmtHex32(refGuest.exports.app_checkers_state_hash() >>> 0);
          }
          if (refGuest.exports.app_checkers_winner) {
            match.winner_seat = refGuest.exports.app_checkers_winner();
            match.status =
              match.winner_seat === 1
                ? 'RED_WON'
                : match.winner_seat === 2
                  ? 'BLACK_WON'
                  : match.winner_seat === 3
                    ? 'DRAW'
                    : 'PLAYING';
          }
        }

        this.logEvent(
          matchId,
          seq,
          body.vm_name || `P${seat}`,
          seat,
          actionName,
          pdn,
          `POST /api/matches/${matchId}/moves · seq=${seq} seat=P${seat} ${actionName} (${pdn}) hash=${match.state_hash} → 200 OK`,
        );
        for (const peer of peers) {
          this._updateGuestBanner(peer);
        }
        return { status: 200, json: match };
      }

      const actionCode = body.action ?? 0;
      const actionName =
        actionCode === 2 ? 'NEW_SEED' : actionCode === 1 ? 'FLAG' : 'REVEAL';
      const cell = body.cell ?? 0;
      const seat = body.seat ?? 1;
      const seed = (body.seed ?? match.seed) >>> 0;
      const maxStrikes = body.max_strikes ?? match.max_strikes;

      match.seq += 1;
      const seq = match.seq;
      match.seed = seed;
      match.max_strikes = maxStrikes;
      match.state_hash = `seed:${seed}`;

      // Deliver move to peer guest(s) in this match
      const peers = guests.filter(
        (g) => g.mpMatchId === matchId && g.exports && g.exports.app_mines_apply_rx,
      );
      for (const peer of peers) {
        if (peer.id !== body.source_vm_id) {
          peer.exports.app_mines_apply_rx(seq, seat, actionCode, cell, seed, maxStrikes);
          peer.render();
        }
      }

      // Sync authoritative scores/turn from the active v3 guest
      const refGuest = peers[0];
      if (refGuest && refGuest.exports) {
        if (refGuest.exports.app_mines_turn) {
          match.turn_seat = refGuest.exports.app_mines_turn();
        }
        if (refGuest.exports.app_mines_p1_score) {
          match.p1_score = refGuest.exports.app_mines_p1_score();
          match.p2_score = refGuest.exports.app_mines_p2_score();
        }
        if (refGuest.exports.app_mines_winner) {
          match.winner_seat = refGuest.exports.app_mines_winner();
          match.status =
            match.winner_seat === 1
              ? 'P1_WON'
              : match.winner_seat === 2
                ? 'P2_WON'
                : match.winner_seat === 3
                  ? 'DRAW'
                  : 'PLAYING';
        }
      }

      this.logEvent(
        matchId,
        seq,
        body.vm_name || `P${seat}`,
        seat,
        actionName,
        cell,
        `POST /api/matches/${matchId}/moves · seq=${seq} seat=P${seat} ${actionName} cell=${cell} → 200 OK`,
      );
      for (const peer of peers) {
        this._updateGuestBanner(peer);
      }
      return { status: 200, json: match };
    }

    if (method === 'GET' && path === '/api/matches') {
      return { status: 200, json: this.tables.matches };
    }

    return { status: 404, json: { error: 'route not found' } };
  }

  renderDrawer() {
    const tbody = document.getElementById('broker-matches-tbody');
    const logEl = document.getElementById('broker-events-log');
    const countEl = document.getElementById('broker-events-count');
    const statusEl = document.getElementById('broker-status');
    if (!tbody || !logEl) return;

    tbody.replaceChildren();
    for (const m of this.tables.matches) {
      const tr = document.createElement('tr');
      const game = m.game || 'mines';
      const turnLabel = m.status === 'PLAYING' ? `P${m.turn_seat}` : '—';
      const scoreLabel =
        game === 'chess'
          ? `${m.p1_score}W : ${m.p2_score}B`
          : game === 'checkers'
            ? `${m.p1_score}R : ${m.p2_score}B`
            : `${m.p1_score} : ${m.p2_score}`;
      const hashOrSeed = (game === 'chess' || game === 'checkers') ? (m.state_hash || '—') : `seed ${m.seed}`;
      tr.innerHTML = `
        <td><b>#${m.id}</b></td>
        <td>${game}</td>
        <td>${m.p1_vm} vs ${m.p2_vm}</td>
        <td><b>${turnLabel}</b></td>
        <td>${scoreLabel}</td>
        <td>${hashOrSeed}</td>
        <td>${m.seq}</td>
        <td>${m.status}</td>`;
      tbody.append(tr);
    }

    logEl.replaceChildren();
    for (const ev of this.tables.match_events.slice(0, 20)) {
      const li = document.createElement('li');
      li.innerHTML = `<span>[${ev.created_at}]</span> <span class="method">SQL INSERT</span> <b>${ev.http}</b>`;
      logEl.append(li);
    }
    if (countEl) countEl.textContent = String(this.tables.match_events.length);
    if (statusEl) {
      const active = this.tables.matches[0];
      statusEl.textContent = active
        ? `match #${active.id} · ${active.game || 'mines'} · ${active.status}`
        : 'idle';
    }
  }
}

export function updateRoomLink(matchId, oppSeat = 2) {
  const linkBox = document.getElementById('net-room-info');
  const linkEl = document.getElementById('net-room-link');
  if (linkBox && linkEl) {
    const url = new URL(window.location.href);
    url.searchParams.set('match', matchId);
    url.searchParams.set('seat', oppSeat);
    linkEl.textContent = url.toString();
    linkBox.style.display = 'flex';
  }
}

export function initNetworkCopyHandler() {
  const netCopyLinkBtn = document.getElementById('net-copy-link-btn');
  const netRoomLinkEl = document.getElementById('net-room-link');

  if (netCopyLinkBtn && netRoomLinkEl) {
    netCopyLinkBtn.addEventListener('click', () => {
      const text = netRoomLinkEl.textContent;
      if (text && text !== '—') {
        navigator.clipboard.writeText(text).then(() => {
          netCopyLinkBtn.textContent = '✅ Copied!';
          setTimeout(() => { netCopyLinkBtn.textContent = '📋 Copy Link'; }, 2000);
        }).catch((err) => console.warn('Clipboard write failed:', err));
      }
    });
  }
}
