/* Mission Loader: Loads declarative pure-JSON missions and provides backward-compatible MISSIONS export */

const MISSION_FILES = [
  'system-audit',
  'mines-gold-path',
  'chess-checkers-opening',
  'chess-checkmate-4-moves',
  'chess-checkmate-6-moves',
  'chess-checkmate-long-6ply',
  'checkers-2p-entangled-convergence',
  'mines-2p-turn-relay',
  'checkers-2p-minimax-duel',
  'checkers-quad-minimax-duels',
  'multi-display-cockpit',
];

export let MISSIONS = [];
let loadPromise = null;

export async function loadMissions(basePath = './data/missions') {
  if (MISSIONS.length > 0) return MISSIONS;
  if (!loadPromise) {
    loadPromise = (async () => {
      const results = [];
      const isNode = typeof window === 'undefined' && typeof process !== 'undefined';

      for (const id of MISSION_FILES) {
        try {
          if (isNode) {
            const fs = await import('node:fs/promises');
            const path = await import('node:path');
            const p = path.resolve(basePath.startsWith('http') ? '.' : basePath, `${id}.json`);
            const raw = await fs.readFile(p, 'utf8');
            results.push(JSON.parse(raw));
          } else {
            const url = new URL(`${basePath}/${id}.json?v=${Date.now()}`, import.meta.url);
            const res = await fetch(url);
            if (res.ok) {
              const data = await res.json();
              results.push(data);
            }
          }
        } catch (err) {
          console.warn(`Could not load mission JSON for ${id}:`, err);
        }
      }
      MISSIONS = results;
      return results;
    })();
  }
  return loadPromise;
}

export function getMissionById(id) {
  return MISSIONS.find((m) => m.id === id) || null;
}
