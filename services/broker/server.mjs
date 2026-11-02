// services/broker/server.mjs
// Lightweight, zero-dependency broker & static file relay server for wasm-fb appliance.
import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const REPO_ROOT = path.resolve(__dirname, '../..');
const WEB_ROOT = path.join(REPO_ROOT, 'web');

const PORT = parseInt(process.env.PORT || '8080', 10);
const HOST = process.env.HOST || '0.0.0.0';

// In-memory match state & SSE subscriptions
const matches = []; // list of match records
const matchEvents = []; // list of event log rows
const sseClients = new Map(); // matchId (number) -> Set of http.ServerResponse
let nextMatchId = 100;
let nextEventId = 1;

function nowIso() {
  return new Date().toISOString().slice(11, 19);
}

function fmtHex32(n) {
  return '0x' + (n >>> 0).toString(16).padStart(8, '0');
}

function logEvent(matchId, seq, vmName, seat, action, cell, httpLine) {
  const row = {
    id: nextEventId++,
    match_id: matchId,
    seq,
    vm_name: vmName,
    seat,
    action,
    cell,
    http: httpLine,
    created_at: nowIso(),
  };
  matchEvents.unshift(row);
  if (matchEvents.length > 256) {
    matchEvents.length = 256;
  }
  return row;
}

function broadcastEvent(matchId, eventType, data) {
  const clients = sseClients.get(matchId);
  if (!clients || clients.size === 0) return;
  const payload = `event: ${eventType}\ndata: ${JSON.stringify(data)}\n\n`;
  for (const res of clients) {
    try {
      res.write(payload);
    } catch {
      // client disconnected
    }
  }
}

// MIME types for static assets
const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.mjs': 'application/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.wasm': 'application/wasm',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.txt': 'text/plain; charset=utf-8',
  '.md': 'text/markdown; charset=utf-8',
  '.bmp': 'image/bmp',
};

function sendJson(res, status, obj) {
  const body = JSON.stringify(obj);
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
    'Access-Control-Allow-Headers': 'Content-Type',
    'Content-Length': Buffer.byteLength(body),
  });
  res.end(body);
}

function parseJsonBody(req) {
  return new Promise((resolve, reject) => {
    let raw = '';
    req.on('data', (chunk) => {
      raw += chunk;
      if (raw.length > 1e6) {
        req.destroy();
        reject(new Error('Payload too large'));
      }
    });
    req.on('end', () => {
      if (!raw) return resolve({});
      try {
        resolve(JSON.parse(raw));
      } catch (err) {
        reject(err);
      }
    });
    req.on('error', reject);
  });
}

function serveStatic(req, res, pathname) {
  let relative = pathname;
  if (relative === '/' || relative === '') {
    relative = '/index.html';
  }

  // Normalize and resolve path inside WEB_ROOT
  const safePath = path.normalize(relative).replace(/^(\.\.[/\\])+/, '');
  let filePath = path.join(WEB_ROOT, safePath);

  fs.stat(filePath, (err, stats) => {
    if (err || !stats.isFile()) {
      res.writeHead(404, { 'Content-Type': 'text/plain' });
      res.end('404 Not Found');
      return;
    }

    const ext = path.extname(filePath).toLowerCase();
    const contentType = MIME_TYPES[ext] || 'application/octet-stream';

    // Stream the file
    res.writeHead(200, {
      'Content-Type': contentType,
      'Content-Length': stats.size,
      'Access-Control-Allow-Origin': '*',
      'Cache-Control': 'no-cache',
    });
    const stream = fs.createReadStream(filePath);
    stream.pipe(res);
  });
}

const server = http.createServer(async (req, res) => {
  // CORS preflight
  if (req.method === 'OPTIONS') {
    res.writeHead(204, {
      'Access-Control-Allow-Origin': '*',
      'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type',
    });
    res.end();
    return;
  }

  const url = new URL(req.url, `http://${req.headers.host || 'localhost'}`);
  const pathname = url.pathname;

  // SSE stream endpoint: GET /api/matches/:id/events
  const sseMatch = pathname.match(/^\/api\/matches\/(\d+)\/events$/);
  if (req.method === 'GET' && sseMatch) {
    const matchId = parseInt(sseMatch[1], 10);
    const match = matches.find((m) => m.id === matchId);
    if (!match) {
      return sendJson(res, 404, { error: 'Match not found' });
    }

    res.writeHead(200, {
      'Content-Type': 'text/event-stream; charset=utf-8',
      'Cache-Control': 'no-cache, no-transform',
      'Connection': 'keep-alive',
      'Access-Control-Allow-Origin': '*',
      'X-Accel-Buffering': 'no',
    });

    if (!sseClients.has(matchId)) {
      sseClients.set(matchId, new Set());
    }
    const set = sseClients.get(matchId);
    set.add(res);

    // Initial connected event with current match state
    res.write(`event: init\ndata: ${JSON.stringify(match)}\n\n`);

    const keepAliveTimer = setInterval(() => {
      try {
        res.write(': keep-alive\n\n');
      } catch {
        clearInterval(keepAliveTimer);
      }
    }, 15000);

    req.on('close', () => {
      clearInterval(keepAliveTimer);
      set.delete(res);
      if (set.size === 0) sseClients.delete(matchId);
    });
    return;
  }

  // GET /api/matches/:id
  const getSingleMatch = pathname.match(/^\/api\/matches\/(\d+)$/);
  if (req.method === 'GET' && getSingleMatch) {
    const matchId = parseInt(getSingleMatch[1], 10);
    const match = matches.find((m) => m.id === matchId);
    if (!match) return sendJson(res, 404, { error: 'Match not found' });
    return sendJson(res, 200, match);
  }

  // GET /api/matches
  if (req.method === 'GET' && pathname === '/api/matches') {
    return sendJson(res, 200, matches);
  }

  // GET /api/events
  if (req.method === 'GET' && pathname === '/api/events') {
    return sendJson(res, 200, matchEvents);
  }

  // POST /api/matches
  if (req.method === 'POST' && pathname === '/api/matches') {
    try {
      const body = await parseJsonBody(req);
      const id = nextMatchId++;
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
        p1_vm: body.p1_vm || 'Player 1',
        p2_vm: body.p2_vm || 'Player 2',
        p1_id: body.p1_id || 1,
        p2_id: body.p2_id || 2,
        p1_score: game === 'checkers' ? 12 : 0,
        p2_score: game === 'checkers' ? 12 : 0,
        state_hash: body.state_hash || (game === 'checkers' ? '0x00000000' : `seed:${seed}`),
        last_pdn: '—',
        winner_seat: 0,
        seq: 0,
        created_at: nowIso(),
      };
      matches.unshift(match);
      logEvent(
        id,
        0,
        `${match.p1_vm},${match.p2_vm}`,
        0,
        'CREATE',
        game,
        `POST /api/matches → 201 Created (match #${id}, game=${game}${game === 'mines' ? `, seed=${seed}` : ''})`,
      );
      return sendJson(res, 201, match);
    } catch (err) {
      return sendJson(res, 400, { error: err.message });
    }
  }

  // POST /api/matches/:id/moves
  const moveMatch = pathname.match(/^\/api\/matches\/(\d+)\/moves$/);
  if (req.method === 'POST' && moveMatch) {
    const matchId = parseInt(moveMatch[1], 10);
    const match = matches.find((m) => m.id === matchId);
    if (!match) return sendJson(res, 404, { error: 'Match not found' });

    try {
      const body = await parseJsonBody(req);
      const game = body.game || match.game || 'mines';
      match.game = game;

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
        if (body.state_hash) {
          match.state_hash = body.state_hash;
        }
        if (body.turn_seat !== undefined) {
          match.turn_seat = body.turn_seat;
        }
        if (body.p1_score !== undefined) {
          match.p1_score = body.p1_score;
          match.p2_score = body.p2_score;
        }
        if (body.winner_seat !== undefined) {
          match.winner_seat = body.winner_seat;
          match.status =
            match.winner_seat === 1
              ? 'RED_WON'
              : match.winner_seat === 2
                ? 'BLACK_WON'
                : match.winner_seat === 3
                  ? 'DRAW'
                  : 'PLAYING';
        }

        const ev = logEvent(
          matchId,
          seq,
          body.vm_name || `P${seat}`,
          seat,
          actionName,
          pdn,
          `POST /api/matches/${matchId}/moves · seq=${seq} seat=P${seat} ${actionName} (${pdn}) hash=${match.state_hash} → 200 OK`,
        );

        // Broadcast to all clients connected to SSE
        const eventData = {
          type: 'move',
          game: 'checkers',
          matchId,
          seq,
          seat,
          action: actionCode,
          lo,
          hi,
          pdn,
          state_hash: match.state_hash,
          turn_seat: match.turn_seat,
          p1_score: match.p1_score,
          p2_score: match.p2_score,
          winner_seat: match.winner_seat,
          status: match.status,
          source_vm_id: body.source_vm_id,
          source_client_id: body.source_client_id,
          log: ev,
        };
        broadcastEvent(matchId, 'move', eventData);

        return sendJson(res, 200, match);
      }

      // Minesweeper
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

      if (body.turn_seat !== undefined) {
        match.turn_seat = body.turn_seat;
      }
      if (body.p1_score !== undefined) {
        match.p1_score = body.p1_score;
        match.p2_score = body.p2_score;
      }
      if (body.winner_seat !== undefined) {
        match.winner_seat = body.winner_seat;
        match.status =
          match.winner_seat === 1
            ? 'P1_WON'
            : match.winner_seat === 2
              ? 'P2_WON'
              : match.winner_seat === 3
                ? 'DRAW'
                : 'PLAYING';
      }

      const ev = logEvent(
        matchId,
        seq,
        body.vm_name || `P${seat}`,
        seat,
        actionName,
        cell,
        `POST /api/matches/${matchId}/moves · seq=${seq} seat=P${seat} ${actionName} cell=${cell} → 200 OK`,
      );

      const eventData = {
        type: 'move',
        game: 'mines',
        matchId,
        seq,
        seat,
        action: actionCode,
        cell,
        seed,
        max_strikes: maxStrikes,
        turn_seat: match.turn_seat,
        p1_score: match.p1_score,
        p2_score: match.p2_score,
        winner_seat: match.winner_seat,
        status: match.status,
        source_vm_id: body.source_vm_id,
        source_client_id: body.source_client_id,
        log: ev,
      };
      broadcastEvent(matchId, 'move', eventData);

      return sendJson(res, 200, match);
    } catch (err) {
      return sendJson(res, 400, { error: err.message });
    }
  }

  // Fallback to static files
  serveStatic(req, res, pathname);
});

server.listen(PORT, HOST, () => {
  console.log(`[Broker Relay Server] listening on http://${HOST}:${PORT}`);
  console.log(`[Broker Relay Server] Serving static web directory: ${WEB_ROOT}`);
  console.log(`[Broker Relay Server] Endpoints:`);
  console.log(`  - GET  /api/matches`);
  console.log(`  - POST /api/matches`);
  console.log(`  - GET  /api/matches/:id`);
  console.log(`  - GET  /api/matches/:id/events (SSE stream)`);
  console.log(`  - POST /api/matches/:id/moves`);
});
