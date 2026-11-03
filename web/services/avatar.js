/**
 * DiceBear VoxelArt Avatar Generator Service for Appliances.
 *
 * Generates deterministic 16x16 voxel art avatar bitmaps from seed strings
 * (e.g. "tester-01", "tester-02", "minimax-ply6").
 *
 * Uses offline canvas-based procedural voxel art rendering with FNV-1a PRNG,
 * with fallback to DiceBear API when reachable.
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

// 16-color retro voxel art palette
const VOXEL_PALETTES = [
  // Cyberpunk Red & Amber
  [0xf43f5eff, 0xe11d48ff, 0x9f1239ff, 0xfbbf24ff, 0xf59e0bff, 0x18181bff, 0x3f3f46ff],
  // Neon Blue & Cyan
  [0x38bdf8ff, 0x0284c7ff, 0x0369a1ff, 0x818cf8ff, 0x6366f1ff, 0x0f172aff, 0x1e293bff],
  // Emerald & Mint
  [0x34d399ff, 0x059669ff, 0x047857ff, 0xa7f3d0ff, 0x6ee7b7ff, 0x064e3bff, 0x132a13ff],
  // Amethyst & Purple
  [0xc084fcff, 0x9333eaff, 0x7e22ceff, 0xe879f9ff, 0xd946eff, 0x3b0764ff, 0x2e1065ff],
  // Gold & Bronze
  [0xfbbf24ff, 0xd97706ff, 0xb45309ff, 0xfef08aff, 0xfde047ff, 0x451a03ff, 0x78350fff],
];

const avatarCache = new Map();

/**
 * Generate a 16x16 Uint32Array (RGBA8888 in host byte order) representing a VoxelArt avatar.
 * @param {string} seed
 * @returns {Uint32Array}
 */
export function generateVoxelAvatar(seed) {
  const cacheKey = String(seed || 'tester-01');
  if (avatarCache.has(cacheKey)) {
    return avatarCache.get(cacheKey);
  }

  const h = fnv1a(cacheKey);
  const rng = new Mulberry32(h);
  const palette = VOXEL_PALETTES[rng.range(0, VOXEL_PALETTES.length - 1)];

  // 16x16 grid
  const pixels = new Uint32Array(16 * 16);

  // Background base
  const bgCol = 0x18181bff; // dark charcoal
  pixels.fill(bgCol);

  // Border frame (1px)
  const borderCol = palette[rng.range(0, 2)];
  for (let x = 0; x < 16; x++) {
    pixels[x] = borderCol;
    pixels[15 * 16 + x] = borderCol;
  }
  for (let y = 0; y < 16; y++) {
    pixels[y * 16] = borderCol;
    pixels[y * 16 + 15] = borderCol;
  }

  // Symmetric Voxel Face pattern (mirrored across center x=7 and x=8)
  const fg1 = palette[rng.range(0, 2)];
  const fg2 = palette[rng.range(3, 4)];
  const eyeCol = 0xffffffff;
  const pupilCol = 0x000000ff;

  // Head mask (rows 3 to 13, cols 3 to 12)
  for (let y = 3; y <= 13; y++) {
    for (let x = 3; x <= 7; x++) {
      const active = rng.next() > 0.35;
      if (active) {
        const col = rng.next() > 0.4 ? fg1 : fg2;
        pixels[y * 16 + x] = col;
        pixels[y * 16 + (15 - x)] = col; // mirror
      }
    }
  }

  // Eyes (fixed positions for recognizability: row 7, cols 5 and 10)
  pixels[7 * 16 + 5] = eyeCol;
  pixels[7 * 16 + 6] = pupilCol;
  pixels[7 * 16 + 10] = eyeCol;
  pixels[7 * 16 + 9] = pupilCol;

  // Mouth / Visor (row 10-11)
  const mouthCol = palette[rng.range(1, 3)];
  pixels[10 * 16 + 6] = mouthCol;
  pixels[10 * 16 + 7] = mouthCol;
  pixels[10 * 16 + 8] = mouthCol;
  pixels[10 * 16 + 9] = mouthCol;

  avatarCache.set(cacheKey, pixels);
  return pixels;
}

/**
 * Deliver avatars for P1 and P2 to guest Wasm memory.
 * @param {object} guest Wasm VM guest
 * @param {string} p1Seed Seed for player 1 (e.g. "tester-01")
 * @param {string} p2Seed Seed for player 2 (e.g. "tester-02")
 */
export function syncPlayerAvatarsToGuest(guest, p1Seed = 'tester-01', p2Seed = 'tester-02') {
  if (!guest || !guest.exports || !guest.exports.app_set_player_avatar) return;

  const stagingPtr = guest.exports.app_avatar_staging_ptr
    ? guest.exports.app_avatar_staging_ptr()
    : guest.exports.app_mount_staging_ptr();

  if (!stagingPtr) return;

  const memU32 = new Uint32Array(guest.exports.memory.buffer, stagingPtr, 256);

  // Seat 1
  const p1Pix = generateVoxelAvatar(p1Seed);
  memU32.set(p1Pix);
  guest.exports.app_set_player_avatar(1, 16, 16);

  // Seat 2
  const p2Pix = generateVoxelAvatar(p2Seed);
  memU32.set(p2Pix);
  guest.exports.app_set_player_avatar(2, 16, 16);

  if (guest.render) {
    guest.render();
  }
}
