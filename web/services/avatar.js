/**
 * DiceBear Avatar Generator & Decoder Service for Appliances.
 *
 * Fetches authentic DiceBear 10.x vector SVGs:
 * - Human avatars:  https://api.dicebear.com/10.x/voxel-art/svg?tags=animation&seed={seed}
 * - Bot / AI:       https://api.dicebear.com/10.x/voxel-bot/svg?tags=animation&seed={seed}
 *
 * Leverages the JS host environment as CDN + Offscreen Canvas Decoder:
 * - Synthesizes 3 animation frames (0: Rest, 1: Blink, 2: Shift/Bob)
 * - Renders a 384x128 RGBA8888 sprite strip (3 x 128x128)
 * - Directly copies into Guest WASM linear staging memory
 * - Includes an offline procedural fallback so appliances work fully offline.
 */

function fnv1a(str) {
  let h = 0x811c9dc5;
  for (let i = 0; i < str.length; i++) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

class Mulberry32 {
  constructor(seed) {
    this.state = seed >>> 0;
  }
  next() {
    let t = (this.state += 0x6d2b79f5);
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }
  range(min, max) {
    return Math.floor(this.next() * (max - min + 1)) + min;
  }
}

const VOXEL_PALETTES = [
  [0xf43f5eff, 0xe11d48ff, 0x9f1239ff, 0xfbbf24ff, 0xf59e0bff, 0x18181bff, 0x3f3f46ff],
  [0x38bdf8ff, 0x0284c7ff, 0x0369a1ff, 0x818cf8ff, 0x6366f1ff, 0x0f172aff, 0x1e293bff],
  [0x34d399ff, 0x059669ff, 0x047857ff, 0xa7f3d0ff, 0x6ee7b7ff, 0x064e3bff, 0x132a13ff],
  [0xc084fcff, 0x9333eaff, 0x7e22ceff, 0xe879f9ff, 0xd946eff, 0x3b0764ff, 0x2e1065ff],
  [0xfbbf24ff, 0xd97706ff, 0xb45309ff, 0xfef08aff, 0xfde047ff, 0x451a03ff, 0x78350fff],
];

const avatarCache = new Map();

/**
 * Generate a procedural offline 384x128 animated sprite strip (3 frames of 128x128).
 * @param {string} seed
 * @returns {Uint32Array} 49,152 words (384x128 RGBA8888)
 */
export function generateProceduralVoxelStrip(seed) {
  const cacheKey = `proc:${seed || 'tester-01'}`;
  if (avatarCache.has(cacheKey)) {
    return avatarCache.get(cacheKey);
  }

  const h = fnv1a(seed || 'tester-01');
  const rng = new Mulberry32(h);
  const palette = VOXEL_PALETTES[rng.range(0, VOXEL_PALETTES.length - 1)];

  // 16x16 logical base grid
  const baseGrid = new Uint32Array(16 * 16);
  const bgCol = 0x18181bff;
  baseGrid.fill(bgCol);

  const borderCol = palette[rng.range(0, 2)];
  for (let x = 0; x < 16; x++) {
    baseGrid[x] = borderCol;
    baseGrid[15 * 16 + x] = borderCol;
  }
  for (let y = 0; y < 16; y++) {
    baseGrid[y * 16] = borderCol;
    baseGrid[y * 16 + 15] = borderCol;
  }

  const fg1 = palette[rng.range(0, 2)];
  const fg2 = palette[rng.range(3, 4)];
  for (let y = 3; y <= 13; y++) {
    for (let x = 3; x <= 7; x++) {
      if (rng.next() > 0.35) {
        const col = rng.next() > 0.4 ? fg1 : fg2;
        baseGrid[y * 16 + x] = col;
        baseGrid[y * 16 + (15 - x)] = col;
      }
    }
  }

  // Eye and mouth baseline
  const eyeCol = 0xffffffff;
  const pupilCol = 0x000000ff;
  baseGrid[7 * 16 + 5] = eyeCol;
  baseGrid[7 * 16 + 6] = pupilCol;
  baseGrid[7 * 16 + 10] = eyeCol;
  baseGrid[7 * 16 + 9] = pupilCol;

  const mouthCol = palette[rng.range(1, 3)];
  baseGrid[10 * 16 + 6] = mouthCol;
  baseGrid[10 * 16 + 7] = mouthCol;
  baseGrid[10 * 16 + 8] = mouthCol;
  baseGrid[10 * 16 + 9] = mouthCol;

  // Render 3 frames of 128x128 into 384x128 buffer
  const strip = new Uint32Array(384 * 128);

  for (let f = 0; f < 3; f++) {
    const frameGrid = new Uint32Array(baseGrid);
    let dy = 0;
    if (f === 1) {
      // Blink: eyes closed (pupil matching skin/bg)
      frameGrid[7 * 16 + 5] = borderCol;
      frameGrid[7 * 16 + 6] = borderCol;
      frameGrid[7 * 16 + 10] = borderCol;
      frameGrid[7 * 16 + 9] = borderCol;
    } else if (f === 2) {
      // Shift / bob up by 1 grid cell
      dy = -1;
    }

    // Scale 16x16 up to 128x128 (8x block scale)
    for (let gy = 0; gy < 16; gy++) {
      const targetGy = Math.max(0, Math.min(15, gy + dy));
      for (let gx = 0; gx < 16; gx++) {
        const col = frameGrid[targetGy * 16 + gx];
        for (let py = 0; py < 8; py++) {
          const row = gy * 8 + py;
          for (let px = 0; px < 8; px++) {
            const c = f * 128 + gx * 8 + px;
            strip[row * 384 + c] = col;
          }
        }
      }
    }
  }

  avatarCache.set(cacheKey, strip);
  return strip;
}

/**
 * Apply declarative animation frame state to a DiceBear SVG.
 * @param {string} rawSvg
 * @param {number} frameIdx 0 = Rest, 1 = Blink, 2 = Shift/Bob
 * @returns {string}
 */
function makeFrameSvg(rawSvg, frameIdx) {
  let styleOverride = '';
  if (frameIdx === 0) {
    // Rest (Idle)
    styleOverride = '<style>.va-blink-medium, .vb-blink-medium { opacity: 0 !important; } .va-upper, .vb-upper, .vb-head { transform: translateY(0) !important; }</style>';
  } else if (frameIdx === 1) {
    // Blink (Eyes closed)
    styleOverride = '<style>.va-blink-medium { opacity: 1 !important; } [id^="eyes-"], use[href^="#eyes-"] { opacity: 0.15 !important; } .va-upper, .vb-upper, .vb-head { transform: translateY(0) !important; }</style>';
  } else if (frameIdx === 2) {
    // Shift / Bob
    styleOverride = '<style>.va-blink-medium, .vb-blink-medium { opacity: 0 !important; } .va-upper, .vb-upper, .vb-head { transform: translateY(-3px) !important; }</style>';
  }
  return rawSvg.replace('</svg>', `${styleOverride}</svg>`);
}

/**
 * Render SVG text onto a 2D canvas of size width x height.
 */
function renderSvgToImageData(svgText, width, height) {
  return new Promise((resolve, reject) => {
    if (typeof Image === 'undefined' || typeof document === 'undefined') {
      return reject(new Error('Canvas/DOM not available'));
    }
    const img = new Image();
    const blob = new Blob([svgText], { type: 'image/svg+xml;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    img.onload = () => {
      const cvs = document.createElement('canvas');
      cvs.width = width;
      cvs.height = height;
      const ctx = cvs.getContext('2d');
      ctx.drawImage(img, 0, 0, width, height);
      URL.revokeObjectURL(url);
      resolve(ctx.getImageData(0, 0, width, height));
    };
    img.onerror = (e) => {
      URL.revokeObjectURL(url);
      reject(e);
    };
    img.src = url;
  });
}

/**
 * Determine if a seed represents an AI / Bot player.
 */
function isBotSeed(seed) {
  const s = String(seed || '').toLowerCase();
  return s.includes('felix') || s.includes('bot') || s.includes('ai') || s.includes('minimax') || s.includes('engine');
}

/**
 * Fetch and decode an authentic DiceBear animated 384x128 sprite strip.
 * Returns a cached Uint32Array on success, or procedural fallback on network error.
 * @param {string} seed
 * @returns {Promise<Uint32Array>}
 */
export async function getAnimatedAvatarStrip(seed) {
  const cacheKey = `dicebear:${seed || 'tester-01'}`;
  if (avatarCache.has(cacheKey)) {
    return avatarCache.get(cacheKey);
  }

  const isBot = isBotSeed(seed);
  const endpoint = isBot ? 'voxel-bot' : 'voxel-art';
  const url = `https://api.dicebear.com/10.x/${endpoint}/svg?tags=animation&seed=${encodeURIComponent(seed)}`;

  try {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const svgText = await res.text();

    const cvs = document.createElement('canvas');
    cvs.width = 384;
    cvs.height = 128;
    const ctx = cvs.getContext('2d');

    for (let f = 0; f < 3; f++) {
      const frameSvg = makeFrameSvg(svgText, f);
      const imgData = await renderSvgToImageData(frameSvg, 128, 128);
      ctx.putImageData(imgData, f * 128, 0);
    }

    const fullStrip = ctx.getImageData(0, 0, 384, 128);
    const u32 = new Uint32Array(fullStrip.data.buffer);
    avatarCache.set(cacheKey, u32);
    return u32;
  } catch (err) {
    // Network or CORS error -> use clean offline procedural generator
    const proc = generateProceduralVoxelStrip(seed);
    avatarCache.set(cacheKey, proc);
    return proc;
  }
}

/**
 * Resolve player seeds dynamically based on game state (e.g. AI vs 2P).
 * If P2 is an AI opponent, defaults to "Felix" (voxel-bot).
 */
export function resolveGuestAvatarSeeds(guest, p1Default = 'tester-01', p2Default = 'tester-02') {
  let p1Seed = p1Default;
  let p2Seed = p2Default;

  // 1. Prefer VM session identity if provisioned
  if (guest && guest.identity) {
    if (guest.mpSeat === 2) {
      p2Seed = guest.identity.seed || guest.identity.name || p2Default;
    } else {
      p1Seed = guest.identity.seed || guest.identity.name || p1Default;
    }
  }

  // 2. If vs AI in local standalone mode, ensure P2 gets a robot avatar
  if (guest && guest.exports) {
    const isChessAi = guest.exports.app_chess_is_vs_ai ? guest.exports.app_chess_is_vs_ai() === 1 : false;
    const isCheckersAi = guest.exports.app_checkers_is_vs_ai ? guest.exports.app_checkers_is_vs_ai() === 1 : false;
    if ((isChessAi || isCheckersAi) && !isBotSeed(p2Seed)) {
      p2Seed = 'Felix'; // DiceBear voxel-bot
    }
  }

  return { p1Seed, p2Seed };
}

/**
 * Deliver animated avatars for P1 and P2 to guest Wasm memory.
 * Immediately applies procedural fallback, then seamlessly upgrades to authentic DiceBear
 * once fetched and decoded.
 *
 * @param {object} guest Wasm VM guest
 * @param {string} p1Requested Seed for player 1 (e.g. "tester-01")
 * @param {string} p2Requested Seed for player 2 (e.g. "tester-02" or "Felix")
 */
export function syncPlayerAvatarsToGuest(guest, p1Requested, p2Requested) {
  if (!guest || !guest.exports || !guest.exports.app_set_player_avatar) return;

  const { p1Seed, p2Seed } = resolveGuestAvatarSeeds(
    guest,
    p1Requested || 'tester-01',
    p2Requested || 'Felix'
  );

  const stagingPtr = guest.exports.app_avatar_staging_ptr
    ? guest.exports.app_avatar_staging_ptr()
    : guest.exports.app_mount_staging_ptr();

  if (!stagingPtr) return;

  // 1. Immediate sync using fast cached / procedural fallback
  const p1Immediate = avatarCache.get(`dicebear:${p1Seed}`) || generateProceduralVoxelStrip(p1Seed);
  const mem1 = new Uint32Array(guest.exports.memory.buffer, stagingPtr, 49152);
  mem1.set(p1Immediate);
  guest.exports.app_set_player_avatar(1, 384, 128);

  const p2Immediate = avatarCache.get(`dicebear:${p2Seed}`) || generateProceduralVoxelStrip(p2Seed);
  const mem2 = new Uint32Array(guest.exports.memory.buffer, stagingPtr, 49152);
  mem2.set(p2Immediate);
  guest.exports.app_set_player_avatar(2, 384, 128);

  if (guest.render) {
    guest.render();
  }

  // 2. Asynchronous background upgrade to authentic DiceBear SVGs
  Promise.all([getAnimatedAvatarStrip(p1Seed), getAnimatedAvatarStrip(p2Seed)])
    .then(([p1Strip, p2Strip]) => {
      if (!guest || !guest.exports || !guest.exports.app_set_player_avatar) return;
      const ptr = guest.exports.app_avatar_staging_ptr
        ? guest.exports.app_avatar_staging_ptr()
        : guest.exports.app_mount_staging_ptr();
      if (!ptr) return;

      const memA = new Uint32Array(guest.exports.memory.buffer, ptr, 49152);
      memA.set(p1Strip);
      guest.exports.app_set_player_avatar(1, 384, 128);

      const memB = new Uint32Array(guest.exports.memory.buffer, ptr, 49152);
      memB.set(p2Strip);
      guest.exports.app_set_player_avatar(2, 384, 128);

      if (guest.render) {
        guest.render();
      }
    })
    .catch(() => {});
}
