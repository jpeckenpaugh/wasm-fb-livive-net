//! Runtime FiveSeven -> smooth renderer (`smooth.rs`).
//!
//! A render *mode*, not a font: the mask is generated from the same 5x7 source
//! at the requested absolute scale, so `Native(s)` and `Smooth(s)` occupy the
//! same layout box for every `s`.
//!
//! | scale | mask   | encoding |
//! |-------|--------|----------|
//! | 1     | none   | native   |
//! | 2     | 12x20  | 2-bit    |
//! | 3     | 17x29  | 2-bit    |
//! | 4     | 22x38  | 2-bit    |
//! | 5     | 27x47  | 8-bit    |
//! | 6     | 32x56  | 8-bit    |
//! | 7     | 37x65  | 8-bit    |
//! | 8     | 42x74  | 8-bit    |
//!
//! Generation rules (per scale `s`):
//!
//! 1. every source pixel becomes a solid `s x s` block in a `5s x 7s` core with
//!    one pixel of ring padding around it;
//! 2. the source `dy` is multiplied by `s` and applied as the glyph offset;
//! 3. pass 1: empty + `>1` non-empty 4-neighbor -> 100%;
//! 4. pass 2: empty + `>1` non-empty 4-neighbor -> 66%;
//! 5. pass 3: empty + `>0` non-empty 4-neighbor -> 33%.
//!
//! The 1px ring lands in the inter-glyph gap and line leading, so a mask never
//! changes layout. At scale 1 there is no sub-pixel information to soften, so
//! the caller uses the native path.
//!
//! Costs: steps 0-2 keep the original 2-bit packed path (scale 2-4). Steps 3-6
//! use 8-bit masks with wider ramps and a 12-slot evictable cache (scale 5-8).

use crate::glyph;
use core::cell::UnsafeCell;

// --- averaging paths (2x and 3x) ------------------------------------------
//
// 2x: 12x20 frame, 1px ring, passes 1, 2/3, 1/3.
// 3x: 19x31 frame, 2px ring, passes 1, 3/4, 1/2, 1/4.
// Each pass fills an empty cell that has more than `threshold` non-zero
// 4-neighbours with average(neighbours) * num / den. Later passes see the
// values written by earlier passes.

pub const AVG_W: usize = 12;
pub const AVG_H: usize = 20;
pub const AVG_LEN: usize = AVG_W * AVG_H;
pub const AVG3_W: usize = 17;
pub const AVG3_H: usize = 29;
pub const AVG3_LEN: usize = AVG3_W * AVG3_H;
pub const AVG4_W: usize = 22;
pub const AVG4_H: usize = 38;
pub const AVG4_LEN: usize = AVG4_W * AVG4_H;
pub const AVG5_W: usize = 27;
pub const AVG5_H: usize = 47;
pub const AVG5_LEN: usize = AVG5_W * AVG5_H;
pub const AVG6_W: usize = 32;
pub const AVG6_H: usize = 56;
pub const AVG6_LEN: usize = AVG6_W * AVG6_H;
pub const AVG7_W: usize = 37;
pub const AVG7_H: usize = 65;
pub const AVG7_LEN: usize = AVG7_W * AVG7_H;
pub const AVG8_W: usize = 42;
pub const AVG8_H: usize = 74;
pub const AVG8_LEN: usize = AVG8_W * AVG8_H;

const AVG_MAX_W: usize = 42;
const AVG_MAX_H: usize = 74;

fn avg_pass(
    levels: &mut [[u8; AVG_MAX_W]; AVG_MAX_H],
    w: usize,
    h: usize,
    threshold: u8,
    num: u32,
    den: u32,
) {
    let mut out = *levels;
    for y in 0..h {
        for x in 0..w {
            if levels[y][x] != 0 {
                continue;
            }
            let mut sum = 0u32;
            let mut n = 0u32;
            if y > 0 && levels[y - 1][x] != 0 {
                sum += levels[y - 1][x] as u32;
                n += 1;
            }
            if y + 1 < h && levels[y + 1][x] != 0 {
                sum += levels[y + 1][x] as u32;
                n += 1;
            }
            if x > 0 && levels[y][x - 1] != 0 {
                sum += levels[y][x - 1] as u32;
                n += 1;
            }
            if x + 1 < w && levels[y][x + 1] != 0 {
                sum += levels[y][x + 1] as u32;
                n += 1;
            }
            if n > threshold as u32 {
                out[y][x] = ((sum * num + (n * den) / 2) / (n * den)) as u8;
            }
        }
    }
    *levels = out;
}

fn seed_avg(
    rows: &[u8; 7],
    dy: u8,
    mag: i32,
    ring: i32,
    levels: &mut [[u8; AVG_MAX_W]; AVG_MAX_H],
) {
    let oy = ring + dy as i32 * mag;
    for r in 0..7i32 {
        for c in 0..5i32 {
            if rows[r as usize] & (1 << (4 - c)) == 0 {
                continue;
            }
            let x0 = ring + c * mag;
            let y0 = oy + r * mag;
            for sy in 0..mag {
                for sx in 0..mag {
                    let y = (y0 + sy) as usize;
                    let x = (x0 + sx) as usize;
                    if y < AVG_MAX_H && x < AVG_MAX_W {
                        levels[y][x] = 255;
                    }
                }
            }
        }
    }
}

fn generate_avg2(rows: &[u8; 7], dy: u8) -> [u8; AVG_LEN] {
    let mut levels = [[0u8; AVG_MAX_W]; AVG_MAX_H];
    seed_avg(rows, dy, 2, 1, &mut levels);
    avg_pass(&mut levels, AVG_W, AVG_H, 1, 1, 1);
    avg_pass(&mut levels, AVG_W, AVG_H, 1, 2, 3);
    avg_pass(&mut levels, AVG_W, AVG_H, 0, 1, 3);
    let mut flat = [0u8; AVG_LEN];
    for y in 0..AVG_H {
        for x in 0..AVG_W {
            flat[y * AVG_W + x] = levels[y][x];
        }
    }
    flat
}

fn generate_avg3(rows: &[u8; 7], dy: u8) -> [u8; AVG3_LEN] {
    let mut levels = [[0u8; AVG_MAX_W]; AVG_MAX_H];
    seed_avg(rows, dy, 3, 1, &mut levels);
    avg_pass(&mut levels, AVG3_W, AVG3_H, 1, 1, 1);
    avg_pass(&mut levels, AVG3_W, AVG3_H, 1, 3, 4);
    avg_pass(&mut levels, AVG3_W, AVG3_H, 1, 1, 2);
    avg_pass(&mut levels, AVG3_W, AVG3_H, 0, 1, 4);
    let mut flat = [0u8; AVG3_LEN];
    for y in 0..AVG3_H {
        for x in 0..AVG3_W {
            flat[y * AVG3_W + x] = levels[y][x];
        }
    }
    flat
}

fn generate_avg4(rows: &[u8; 7], dy: u8) -> [u8; AVG4_LEN] {
    let mut levels = [[0u8; AVG_MAX_W]; AVG_MAX_H];
    seed_avg(rows, dy, 4, 1, &mut levels);
    avg_pass(&mut levels, AVG4_W, AVG4_H, 1, 1, 1);
    avg_pass(&mut levels, AVG4_W, AVG4_H, 1, 4, 5);
    avg_pass(&mut levels, AVG4_W, AVG4_H, 1, 3, 5);
    avg_pass(&mut levels, AVG4_W, AVG4_H, 1, 2, 5);
    avg_pass(&mut levels, AVG4_W, AVG4_H, 0, 1, 5);
    let mut flat = [0u8; AVG4_LEN];
    for y in 0..AVG4_H {
        for x in 0..AVG4_W {
            flat[y * AVG4_W + x] = levels[y][x];
        }
    }
    flat
}

fn generate_avg5(rows: &[u8; 7], dy: u8) -> [u8; AVG5_LEN] {
    let mut levels = [[0u8; AVG_MAX_W]; AVG_MAX_H];
    seed_avg(rows, dy, 5, 1, &mut levels);
    avg_pass(&mut levels, AVG5_W, AVG5_H, 1, 1, 1);
    avg_pass(&mut levels, AVG5_W, AVG5_H, 1, 5, 6);
    avg_pass(&mut levels, AVG5_W, AVG5_H, 1, 2, 3);
    avg_pass(&mut levels, AVG5_W, AVG5_H, 1, 1, 2);
    avg_pass(&mut levels, AVG5_W, AVG5_H, 1, 1, 3);
    avg_pass(&mut levels, AVG5_W, AVG5_H, 0, 1, 6);
    let mut flat = [0u8; AVG5_LEN];
    for y in 0..AVG5_H {
        for x in 0..AVG5_W {
            flat[y * AVG5_W + x] = levels[y][x];
        }
    }
    flat
}

fn generate_avg6(rows: &[u8; 7], dy: u8) -> [u8; AVG6_LEN] {
    let mut levels = [[0u8; AVG_MAX_W]; AVG_MAX_H];
    seed_avg(rows, dy, 6, 1, &mut levels);
    avg_pass(&mut levels, AVG6_W, AVG6_H, 1, 1, 1);
    avg_pass(&mut levels, AVG6_W, AVG6_H, 1, 6, 7);
    avg_pass(&mut levels, AVG6_W, AVG6_H, 1, 5, 7);
    avg_pass(&mut levels, AVG6_W, AVG6_H, 1, 4, 7);
    avg_pass(&mut levels, AVG6_W, AVG6_H, 1, 3, 7);
    avg_pass(&mut levels, AVG6_W, AVG6_H, 1, 2, 7);
    avg_pass(&mut levels, AVG6_W, AVG6_H, 0, 1, 7);
    let mut flat = [0u8; AVG6_LEN];
    for y in 0..AVG6_H {
        for x in 0..AVG6_W {
            flat[y * AVG6_W + x] = levels[y][x];
        }
    }
    flat
}

fn generate_avg7(rows: &[u8; 7], dy: u8) -> [u8; AVG7_LEN] {
    let mut levels = [[0u8; AVG_MAX_W]; AVG_MAX_H];
    seed_avg(rows, dy, 7, 1, &mut levels);
    avg_pass(&mut levels, AVG7_W, AVG7_H, 1, 1, 1);
    avg_pass(&mut levels, AVG7_W, AVG7_H, 1, 7, 8);
    avg_pass(&mut levels, AVG7_W, AVG7_H, 1, 3, 4);
    avg_pass(&mut levels, AVG7_W, AVG7_H, 1, 5, 8);
    avg_pass(&mut levels, AVG7_W, AVG7_H, 1, 1, 2);
    avg_pass(&mut levels, AVG7_W, AVG7_H, 1, 3, 8);
    avg_pass(&mut levels, AVG7_W, AVG7_H, 1, 1, 4);
    avg_pass(&mut levels, AVG7_W, AVG7_H, 0, 1, 8);
    let mut flat = [0u8; AVG7_LEN];
    for y in 0..AVG7_H {
        for x in 0..AVG7_W {
            flat[y * AVG7_W + x] = levels[y][x];
        }
    }
    flat
}

fn generate_avg8(rows: &[u8; 7], dy: u8) -> [u8; AVG8_LEN] {
    let mut levels = [[0u8; AVG_MAX_W]; AVG_MAX_H];
    seed_avg(rows, dy, 8, 1, &mut levels);
    avg_pass(&mut levels, AVG8_W, AVG8_H, 1, 1, 1);
    avg_pass(&mut levels, AVG8_W, AVG8_H, 1, 8, 9);
    avg_pass(&mut levels, AVG8_W, AVG8_H, 1, 7, 9);
    avg_pass(&mut levels, AVG8_W, AVG8_H, 1, 2, 3);
    avg_pass(&mut levels, AVG8_W, AVG8_H, 1, 5, 9);
    avg_pass(&mut levels, AVG8_W, AVG8_H, 1, 4, 9);
    avg_pass(&mut levels, AVG8_W, AVG8_H, 1, 1, 3);
    avg_pass(&mut levels, AVG8_W, AVG8_H, 1, 2, 9);
    avg_pass(&mut levels, AVG8_W, AVG8_H, 0, 1, 9);
    let mut flat = [0u8; AVG8_LEN];
    for y in 0..AVG8_H {
        for x in 0..AVG8_W {
            flat[y * AVG8_W + x] = levels[y][x];
        }
    }
    flat
}

struct AvgCache {
    masks: UnsafeCell<[[u8; AVG_LEN]; COUNT]>,
    valid: UnsafeCell<[bool; COUNT]>,
}

unsafe impl Sync for AvgCache {}

static AVG: AvgCache = AvgCache {
    masks: UnsafeCell::new([[0u8; AVG_LEN]; COUNT]),
    valid: UnsafeCell::new([false; COUNT]),
};

struct Avg3Cache {
    masks: UnsafeCell<[[u8; AVG3_LEN]; COUNT]>,
    valid: UnsafeCell<[bool; COUNT]>,
}

unsafe impl Sync for Avg3Cache {}

static AVG3: Avg3Cache = Avg3Cache {
    masks: UnsafeCell::new([[0u8; AVG3_LEN]; COUNT]),
    valid: UnsafeCell::new([false; COUNT]),
};

struct Avg4Cache {
    masks: UnsafeCell<[[u8; AVG4_LEN]; COUNT]>,
    valid: UnsafeCell<[bool; COUNT]>,
}

unsafe impl Sync for Avg4Cache {}

static AVG4: Avg4Cache = Avg4Cache {
    masks: UnsafeCell::new([[0u8; AVG4_LEN]; COUNT]),
    valid: UnsafeCell::new([false; COUNT]),
};

struct Avg5Cache {
    masks: UnsafeCell<[[u8; AVG5_LEN]; COUNT]>,
    valid: UnsafeCell<[bool; COUNT]>,
}

unsafe impl Sync for Avg5Cache {}

static AVG5: Avg5Cache = Avg5Cache {
    masks: UnsafeCell::new([[0u8; AVG5_LEN]; COUNT]),
    valid: UnsafeCell::new([false; COUNT]),
};

struct Avg6Cache {
    masks: UnsafeCell<[[u8; AVG6_LEN]; COUNT]>,
    valid: UnsafeCell<[bool; COUNT]>,
}

unsafe impl Sync for Avg6Cache {}

static AVG6: Avg6Cache = Avg6Cache {
    masks: UnsafeCell::new([[0u8; AVG6_LEN]; COUNT]),
    valid: UnsafeCell::new([false; COUNT]),
};

struct Avg7Cache {
    masks: UnsafeCell<[[u8; AVG7_LEN]; COUNT]>,
    valid: UnsafeCell<[bool; COUNT]>,
}

unsafe impl Sync for Avg7Cache {}

static AVG7: Avg7Cache = Avg7Cache {
    masks: UnsafeCell::new([[0u8; AVG7_LEN]; COUNT]),
    valid: UnsafeCell::new([false; COUNT]),
};

struct Avg8Cache {
    masks: UnsafeCell<[[u8; AVG8_LEN]; COUNT]>,
    valid: UnsafeCell<[bool; COUNT]>,
}

unsafe impl Sync for Avg8Cache {}

static AVG8: Avg8Cache = Avg8Cache {
    masks: UnsafeCell::new([[0u8; AVG8_LEN]; COUNT]),
    valid: UnsafeCell::new([false; COUNT]),
};

static EMPTY_AVG: [u8; AVG_LEN] = [0u8; AVG_LEN];
static EMPTY_AVG3: [u8; AVG3_LEN] = [0u8; AVG3_LEN];
static EMPTY_AVG4: [u8; AVG4_LEN] = [0u8; AVG4_LEN];
static EMPTY_AVG5: [u8; AVG5_LEN] = [0u8; AVG5_LEN];
static EMPTY_AVG6: [u8; AVG6_LEN] = [0u8; AVG6_LEN];
static EMPTY_AVG7: [u8; AVG7_LEN] = [0u8; AVG7_LEN];
static EMPTY_AVG8: [u8; AVG8_LEN] = [0u8; AVG8_LEN];

fn ensure_avg2(index: u16, rows: &[u8; 7], dy: u8) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*AVG.valid.get())[i] {
            (*AVG.masks.get())[i] = generate_avg2(rows, dy);
            (*AVG.valid.get())[i] = true;
        }
    }
}

fn ensure_avg3(index: u16, rows: &[u8; 7], dy: u8) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*AVG3.valid.get())[i] {
            (*AVG3.masks.get())[i] = generate_avg3(rows, dy);
            (*AVG3.valid.get())[i] = true;
        }
    }
}

fn ensure_avg4(index: u16, rows: &[u8; 7], dy: u8) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*AVG4.valid.get())[i] {
            (*AVG4.masks.get())[i] = generate_avg4(rows, dy);
            (*AVG4.valid.get())[i] = true;
        }
    }
}

fn ensure_avg5(index: u16, rows: &[u8; 7], dy: u8) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*AVG5.valid.get())[i] {
            (*AVG5.masks.get())[i] = generate_avg5(rows, dy);
            (*AVG5.valid.get())[i] = true;
        }
    }
}

fn ensure_avg6(index: u16, rows: &[u8; 7], dy: u8) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*AVG6.valid.get())[i] {
            (*AVG6.masks.get())[i] = generate_avg6(rows, dy);
            (*AVG6.valid.get())[i] = true;
        }
    }
}

fn ensure_avg7(index: u16, rows: &[u8; 7], dy: u8) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*AVG7.valid.get())[i] {
            (*AVG7.masks.get())[i] = generate_avg7(rows, dy);
            (*AVG7.valid.get())[i] = true;
        }
    }
}

fn ensure_avg8(index: u16, rows: &[u8; 7], dy: u8) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*AVG8.valid.get())[i] {
            (*AVG8.masks.get())[i] = generate_avg8(rows, dy);
            (*AVG8.valid.get())[i] = true;
        }
    }
}

/// 8-bit 2x mask for the averaging renderer, cached per glyph.
pub fn glyph_avg2(_font: u8, index: u16, rows: &[u8; 7], dy: u8) -> &'static [u8; AVG_LEN] {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return &EMPTY_AVG;
    }
    ensure_avg2(index, rows, dy);
    unsafe { &(*AVG.masks.get())[index as usize] }
}

/// 8-bit 3x mask for the averaging renderer, cached per glyph.
pub fn glyph_avg3(_font: u8, index: u16, rows: &[u8; 7], dy: u8) -> &'static [u8; AVG3_LEN] {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return &EMPTY_AVG3;
    }
    ensure_avg3(index, rows, dy);
    unsafe { &(*AVG3.masks.get())[index as usize] }
}

/// 8-bit 4x mask for the averaging renderer, cached per glyph.
pub fn glyph_avg4(_font: u8, index: u16, rows: &[u8; 7], dy: u8) -> &'static [u8; AVG4_LEN] {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return &EMPTY_AVG4;
    }
    ensure_avg4(index, rows, dy);
    unsafe { &(*AVG4.masks.get())[index as usize] }
}

/// 8-bit 5x mask for the averaging renderer, cached per glyph.
pub fn glyph_avg5(_font: u8, index: u16, rows: &[u8; 7], dy: u8) -> &'static [u8; AVG5_LEN] {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return &EMPTY_AVG5;
    }
    ensure_avg5(index, rows, dy);
    unsafe { &(*AVG5.masks.get())[index as usize] }
}

/// 8-bit 6x mask for the averaging renderer, cached per glyph.
pub fn glyph_avg6(_font: u8, index: u16, rows: &[u8; 7], dy: u8) -> &'static [u8; AVG6_LEN] {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return &EMPTY_AVG6;
    }
    ensure_avg6(index, rows, dy);
    unsafe { &(*AVG6.masks.get())[index as usize] }
}

/// 8-bit 7x mask for the averaging renderer, cached per glyph.
pub fn glyph_avg7(_font: u8, index: u16, rows: &[u8; 7], dy: u8) -> &'static [u8; AVG7_LEN] {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return &EMPTY_AVG7;
    }
    ensure_avg7(index, rows, dy);
    unsafe { &(*AVG7.masks.get())[index as usize] }
}

/// 8-bit 8x mask for the averaging renderer, cached per glyph.
pub fn glyph_avg8(_font: u8, index: u16, rows: &[u8; 7], dy: u8) -> &'static [u8; AVG8_LEN] {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return &EMPTY_AVG8;
    }
    ensure_avg8(index, rows, dy);
    unsafe { &(*AVG8.masks.get())[index as usize] }
}

/// Number of smooth steps: absolute scales 2..=8.
pub const STEPS: usize = 7;
/// Steps 0..2 (scale 2..4) keep the original 2-bit masks; 3..6 are 8-bit.
const LEGACY_STEPS: usize = 3;
const MAX_W: usize = 22;
const MAX_H: usize = 38;
const MAX_RB: usize = 6;
pub const MAX_BYTES: usize = MAX_H * MAX_RB; // 228

/// Extended (scale 5..8) masks are bytes, up to 42x74.
const BIG_W: usize = 42;
const BIG_H: usize = 74;
const BIG_MAX: usize = BIG_W * BIG_H;
const BIG_SLOTS: usize = 12;

#[derive(Clone, Copy)]
struct BigSlot {
    data: [u8; BIG_MAX],
    key: u32,
    len: u32,
    age: u32,
}

impl BigSlot {
    const EMPTY: Self = BigSlot {
        data: [0u8; BIG_MAX],
        key: 0,
        len: 0,
        age: 0,
    };
}

struct BigCache(UnsafeCell<[BigSlot; BIG_SLOTS]>);
unsafe impl Sync for BigCache {}
struct BigTick(UnsafeCell<u32>);
unsafe impl Sync for BigTick {}

static BIG: BigCache = BigCache(UnsafeCell::new([BigSlot::EMPTY; BIG_SLOTS]));
static BIG_TICK: BigTick = BigTick(UnsafeCell::new(0));

const COUNT: usize = glyph::FIVESEVEN_GLYPH_COUNT;
const MISSING_INDEX: u16 = u16::MAX;

struct Cache {
    masks: UnsafeCell<[[[u8; MAX_BYTES]; STEPS]; COUNT]>,
    valid: UnsafeCell<[[bool; STEPS]; COUNT]>,
}

// Single-threaded wasm guest / test process.
unsafe impl Sync for Cache {}

static CACHE: Cache = Cache {
    masks: UnsafeCell::new([[[0u8; MAX_BYTES]; STEPS]; COUNT]),
    valid: UnsafeCell::new([[false; STEPS]; COUNT]),
};

static EMPTY: [u8; MAX_BYTES] = [0u8; MAX_BYTES];

/// Ladder step for an absolute scale (`2..=8`); `None` at the 5x7 base size.
pub fn step_for_scale(scale: i32) -> Option<usize> {
    if scale < 2 {
        None
    } else {
        Some(((scale - 2) as usize).min(STEPS - 1))
    }
}

/// Absolute scale of a ladder step (2..=8).
pub fn scale_for_step(step: usize) -> i32 {
    step as i32 + 2
}

/// Mask width in pixels: one ring column on each side of the `5 * scale` core.
pub fn mask_w(step: usize) -> i32 {
    5 * scale_for_step(step) + 2
}

/// Mask height in pixels: ring + up to `9 * scale` rows of ink + ring.
pub fn mask_h(step: usize) -> i32 {
    9 * scale_for_step(step) + 2
}

/// Ink width of the unscaled source core at this step.
pub fn ink_w(step: usize) -> i32 {
    5 * scale_for_step(step)
}

/// Ink height of the unscaled source core at this step.
pub fn ink_h(step: usize) -> i32 {
    7 * scale_for_step(step)
}

pub fn row_bytes(step: usize) -> usize {
    ((mask_w(step) + 3) / 4) as usize
}

/// Normalised coverage for a mask pixel: 0 (empty) ..= 255 (solid).
#[inline]
pub fn unpack(mask: &[u8], step: usize, x: i32, y: i32) -> u8 {
    if step < LEGACY_STEPS {
        let rb = row_bytes(step);
        let v = (mask[y as usize * rb + x as usize / 4] >> (6 - 2 * ((x as usize) % 4))) & 0x3;
        v * 85
    } else {
        mask[y as usize * mask_w(step) as usize + x as usize]
    }
}

fn fill_pass(levels: &mut [[u8; MAX_W]; MAX_H], w: i32, h: i32, threshold: u8, value: u8) {
    let mut out = *levels;
    for y in 0..h as usize {
        for x in 0..w as usize {
            if levels[y][x] != 0 {
                continue;
            }
            let mut n = 0u8;
            if x > 0 && levels[y][x - 1] != 0 {
                n += 1;
            }
            if x + 1 < w as usize && levels[y][x + 1] != 0 {
                n += 1;
            }
            if y > 0 && levels[y - 1][x] != 0 {
                n += 1;
            }
            if y + 1 < h as usize && levels[y + 1][x] != 0 {
                n += 1;
            }
            if n > threshold {
                out[y][x] = value;
            }
        }
    }
    *levels = out;
}

#[allow(clippy::needless_range_loop)]
fn generate(rows: &[u8; 7], dy: u8, step: usize) -> [u8; MAX_BYTES] {
    let mag = scale_for_step(step);
    let w = mask_w(step);
    let h = mask_h(step);
    let mut levels = [[0u8; MAX_W]; MAX_H];

    // 1. Exact source magnification. Leave one row/column of padding around the
    // core; the fill passes operate on that padding too.
    let oy = 1 + dy as i32 * mag;
    for r in 0..7i32 {
        for c in 0..5i32 {
            if rows[r as usize] & (1 << (4 - c)) == 0 {
                continue;
            }
            let x0 = 1 + c * mag;
            let y0 = oy + r * mag;
            for sy in 0..mag {
                for sx in 0..mag {
                    let y = y0 + sy;
                    let x = x0 + sx;
                    if y >= 0 && x >= 0 && y < h && x < w {
                        levels[y as usize][x as usize] = 3;
                    }
                }
            }
        }
    }

    // 2-4. Three sequential additive passes.
    fill_pass(&mut levels, w, h, 1, 3);
    fill_pass(&mut levels, w, h, 1, 2);
    fill_pass(&mut levels, w, h, 0, 1);

    // 5. Pack 2 bits per pixel.
    let rb = row_bytes(step);
    let mut packed = [0u8; MAX_BYTES];
    for y in 0..h as usize {
        for x in 0..w as usize {
            packed[y * rb + x / 4] |= levels[y][x] << (6 - 2 * (x % 4));
        }
    }
    packed
}

fn ensure(index: u16, rows: &[u8; 7], dy: u8, step: usize) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*CACHE.valid.get())[i][step] {
            (*CACHE.masks.get())[i][step] = generate(rows, dy, step);
            (*CACHE.valid.get())[i][step] = true;
        }
    }
}

/// Get (or lazily generate) the packed mask for a glyph at a ladder step.
///
/// `font` namespaces the big (8-bit) cache so a second font can never alias a
/// glyph index. The legacy 2-bit cache is indexed by glyph index alone and is
/// sized for the current font; a second font needs its own index space.
pub fn glyph(font: u8, index: u16, rows: &[u8; 7], dy: u8, step: usize) -> &'static [u8] {
    if index == MISSING_INDEX || index as usize >= COUNT || step >= STEPS {
        return &EMPTY;
    }
    if step < LEGACY_STEPS {
        ensure(index, rows, dy, step);
        unsafe { &(*CACHE.masks.get())[index as usize][step] }
    } else {
        big_glyph(font, index, rows, dy, step)
    }
}

fn ramp(step: usize) -> &'static [u8] {
    match step {
        3 => &[255, 191, 128, 64],
        4 => &[255, 204, 153, 102, 51],
        5 => &[255, 212, 170, 128, 85, 43],
        _ => &[255, 219, 182, 146, 109, 73, 36],
    }
}

#[allow(clippy::too_many_arguments)]
fn fill_big(
    levels: &mut [[u8; BIG_W]; BIG_H],
    outside: &[[bool; BIG_W]; BIG_H],
    w: i32,
    h: i32,
    threshold: u8,
    value: u8,
) {
    let mut out = *levels;
    for y in 0..h as usize {
        for x in 0..w as usize {
            if levels[y][x] != 0 || !outside[y][x] {
                continue;
            }
            let mut n = 0u8;
            if x > 0 && levels[y][x - 1] != 0 {
                n += 1;
            }
            if x + 1 < w as usize && levels[y][x + 1] != 0 {
                n += 1;
            }
            if y > 0 && levels[y - 1][x] != 0 {
                n += 1;
            }
            if y + 1 < h as usize && levels[y + 1][x] != 0 {
                n += 1;
            }
            if n > threshold {
                out[y][x] = value;
            }
        }
    }
    *levels = out;
}

/// 8-bit generator for steps 3..6 (scale 5..8). Rings are gated to cells
/// reachable from the border so counters stay open.
#[allow(clippy::needless_range_loop)]
fn generate_big(rows: &[u8; 7], dy: u8, step: usize) -> ([u8; BIG_MAX], usize) {
    let mag = scale_for_step(step);
    let w = mask_w(step);
    let h = mask_h(step);
    let mut levels = [[0u8; BIG_W]; BIG_H];

    let oy = 1 + dy as i32 * mag;
    for r in 0..7i32 {
        for c in 0..5i32 {
            if rows[r as usize] & (1 << (4 - c)) == 0 {
                continue;
            }
            let x0 = 1 + c * mag;
            let y0 = oy + r * mag;
            for sy in 0..mag {
                for sx in 0..mag {
                    let y = y0 + sy;
                    let x = x0 + sx;
                    if y >= 0 && x >= 0 && y < h && x < w {
                        levels[y as usize][x as usize] = 255;
                    }
                }
            }
        }
    }

    // Outside mask (border-connected empty cells).
    let mut outside = [[false; BIG_W]; BIG_H];
    let mut stack = [(0usize, 0usize); BIG_MAX];
    let mut top = 0usize;
    let push = |x: usize,
                    y: usize,
                    stack: &mut [(usize, usize); BIG_MAX],
                    top: &mut usize,
                    outside: &mut [[bool; BIG_W]; BIG_H]| {
        if levels[y][x] == 0 && !outside[y][x] {
            outside[y][x] = true;
            stack[*top] = (x, y);
            *top += 1;
        }
    };
    for x in 0..w as usize {
        push(x, 0, &mut stack, &mut top, &mut outside);
        push(x, h as usize - 1, &mut stack, &mut top, &mut outside);
    }
    for y in 0..h as usize {
        push(0, y, &mut stack, &mut top, &mut outside);
        push(w as usize - 1, y, &mut stack, &mut top, &mut outside);
    }
    while top > 0 {
        top -= 1;
        let (x, y) = stack[top];
        let nbrs = [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ];
        for (nx, ny) in nbrs {
            if nx < w as usize && ny < h as usize && !outside[ny][nx] && levels[ny][nx] == 0 {
                outside[ny][nx] = true;
                stack[top] = (nx, ny);
                top += 1;
            }
        }
    }

    let values = ramp(step);
    fill_big(&mut levels, &outside, w, h, 1, values[0]);
    fill_big(&mut levels, &outside, w, h, 1, values[1]);
    for &v in &values[2..] {
        fill_big(&mut levels, &outside, w, h, 0, v);
    }

    let mut mask = [0u8; BIG_MAX];
    let len = (w * h) as usize;
    for y in 0..h as usize {
        for x in 0..w as usize {
            mask[y * w as usize + x] = levels[y][x];
        }
    }
    (mask, len)
}

fn big_glyph(font: u8, index: u16, rows: &[u8; 7], dy: u8, step: usize) -> &'static [u8] {
    // The font id keeps a future second font from aliasing a glyph index.
    let key = ((font as u32) << 20) | ((index as u32) << 4) | (step as u32 + 1);
    unsafe {
        let tick = {
            let t = &mut *BIG_TICK.0.get();
            *t = t.wrapping_add(1);
            *t
        };
        let slots = &mut *BIG.0.get();
        let mut found: Option<usize> = None;
        let mut victim = 0usize;
        let mut oldest = u32::MAX;
        #[allow(clippy::needless_range_loop)]
        for i in 0..BIG_SLOTS {
            if slots[i].key == key {
                found = Some(i);
            }
            if slots[i].age < oldest {
                oldest = slots[i].age;
                victim = i;
            }
        }
        let idx = match found {
            Some(i) => {
                slots[i].age = tick;
                i
            }
            None => {
                let (mask, len) = generate_big(rows, dy, step);
                let slot = &mut slots[victim];
                slot.key = key;
                slot.len = len as u32;
                slot.age = tick;
                slot.data[..len].copy_from_slice(&mask[..len]);
                victim
            }
        };
        &slots[idx].data[..slots[idx].len as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladder_geometry_is_absolute() {
        // 1x is the 5x7 base and has no smooth mask.
        assert_eq!(step_for_scale(1), None);
        assert_eq!(step_for_scale(2), Some(0));
        assert_eq!(step_for_scale(8), Some(6));
        assert_eq!(step_for_scale(99), Some(STEPS - 1));

        // Mask = core + 1px ring; the core is the native footprint at that scale.
        assert_eq!((ink_w(0), ink_h(0)), (10, 14));
        assert_eq!((mask_w(0), mask_h(0)), (12, 20));
        assert_eq!((mask_w(6), mask_h(6)), (42, 74));
        assert_eq!(row_bytes(0), 3);
        assert_eq!(row_bytes(1), 5);
        assert_eq!(row_bytes(2), 6);
    }

    #[test]
    fn scale_one_is_not_a_step() {
        for scale in -4..=1 {
            assert!(step_for_scale(scale).is_none(), "scale {}", scale);
        }
    }

    #[test]
    fn x_masks_keep_three_levels() {
        let g = glyph::lookup_fiveseven('X' as u32).unwrap();
        for step in 0..STEPS {
            let m = glyph(0, g.index, &g.rows, g.dy, step);
            let w = mask_w(step);
            let h = mask_h(step);
            let (mut solid, mut mid) = (0, 0);
            for y in 0..h {
                for x in 0..w {
                    match unpack(m, step, x, y) {
                        255 => solid += 1,
                        0 => {}
                        _ => mid += 1,
                    }
                }
            }
            assert!(solid > 0, "step {} solid", step);
            assert!(mid > 0, "step {} mid", step);
        }
    }

    #[test]
    fn mask_cache_is_stable() {
        let g = glyph::lookup_fiveseven('g' as u32).unwrap();
        let a = glyph(0, g.index, &g.rows, g.dy, 1).to_vec();
        let b = glyph(0, g.index, &g.rows, g.dy, 1).to_vec();
        assert_eq!(a, b);
    }

    fn fnv(mask: &[u8]) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &b in mask.iter() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    fn fnv_used(mask: &[u8], step: usize) -> u64 {
        if step < LEGACY_STEPS {
            let used = row_bytes(step) * mask_h(step) as usize;
            fnv(&mask[..used])
        } else {
            fnv(&mask[..(mask_w(step) * mask_h(step)) as usize])
        }
    }

    #[test]
    fn legacy_masks_are_pinned() {
        // Recorded from the approved build, hashed over the used mask bytes.
        // Any change here is a regression in the masks tuned by eye.
        let pinned = [
            ('A', 0, 793793836167752488u64),
            ('A', 1, 9630369598213907606),
            ('A', 2, 17735013175661378873),
            ('S', 0, 9068962723563185685),
            ('S', 1, 5039739629943738349),
            ('S', 2, 4299594373721285129),
            ('e', 0, 8773792178305687538),
            ('e', 1, 1026546336409130977),
            ('e', 2, 8494955379271766421),
            ('g', 0, 11049120486642331054),
            ('g', 1, 15741348806582064322),
            ('g', 2, 1073923197653953169),
        ];
        for (ch, step, want) in pinned {
            let g = glyph::lookup_fiveseven(ch as u32).unwrap();
            let got = fnv_used(glyph(0, g.index, &g.rows, g.dy, step), step);
            assert_eq!(got, want, "mask drift for {} step {}", ch, step);
        }
    }

    #[test]
    fn avg3_reference_s() {
        let rows = [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E];
        let m = generate_avg3(&rows, 0);
        assert_eq!(m[0], 0);
        assert_eq!(m[AVG3_W * 1 + 3], 112);
        assert_eq!(m[AVG3_W * 2 + 3], 191);
        assert_eq!(fnv(&m), 11176960991489867612u64);
    }

    #[test]
    fn avg4_reference_s() {
        let rows = [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E];
        let m = generate_avg4(&rows, 0);
        assert_eq!(m[0], 0);
        assert_eq!(m[AVG4_W * 1 + 3], 13);
        assert_eq!(m[AVG4_W * 2 + 3], 52);
        assert_eq!(fnv(&m), 3211897364347968397u64);
    }

    #[test]
    fn avg5_reference_s() {
        let rows = [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E];
        let m = generate_avg5(&rows, 0);
        assert_eq!(m[0], 0);
        assert_eq!(m[AVG5_W + 5], 60);
        assert_eq!(m[AVG5_W * 2 + 5], 103);
        assert_eq!(fnv(&m), 15292096614725901540u64);
    }

    #[test]
    fn avg6_reference_s() {
        let rows = [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E];
        let m = generate_avg6(&rows, 0);
        assert_eq!(m[0], 0);
        assert_eq!(m[AVG6_W + 5], 5);
        assert_eq!(m[AVG6_W * 2 + 5], 18);
        assert_eq!(fnv(&m), 3899084773390586721u64);
    }

    #[test]
    fn avg7_reference_s() {
        let rows = [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E];
        let m = generate_avg7(&rows, 0);
        assert_eq!(m[0], 0);
        assert!(m.iter().any(|&v| v == 223));
        assert_eq!(fnv(&m), 12697047180094134924u64);
    }

    #[test]
    fn avg8_reference_s() {
        let rows = [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E];
        let m = generate_avg8(&rows, 0);
        assert_eq!(m[0], 0);
        assert!(m.iter().any(|&v| v == 227));
        assert_eq!(fnv(&m), 14602437259815236441u64);
    }

    #[test]
    fn avg2_reference_s() {
        let rows = [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E];
        let m = generate_avg2(&rows, 0);
        assert_eq!(m[2], 57);
        assert_eq!(m[14], 170);
        assert_eq!(m[15], 255);
        assert_eq!(m[40], 170);
        assert_eq!(m[52], 57);
        assert_eq!(fnv(&m), 16249398564372139549u64);
    }
}

#[cfg(test)]
mod fold_tests {
    use super::*;
    use crate::glyph;

    fn rev5(v: u8) -> u8 {
        ((v & 1) << 4) | ((v & 2) << 2) | (v & 4) | ((v & 8) >> 2) | ((v & 16) >> 4)
    }

    fn lr_rows(rows: &[u8; 7]) -> [u8; 7] {
        rows.map(rev5)
    }

    fn tb_rows(rows: &[u8; 7]) -> [u8; 7] {
        let mut r = *rows;
        r.reverse();
        r
    }

    fn both(rows: &[u8; 7]) -> [u8; 7] {
        tb_rows(&lr_rows(rows))
    }

    fn levels(rows: &[u8; 7], dy: u8, step: usize) -> Vec<u8> {
        let m = generate(rows, dy, step);
        let w = mask_w(step);
        let h = mask_h(step);
        let mut v = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                v.push(unpack(&m, step, x, y));
            }
        }
        v
    }

    fn fold_lr(v: &[u8], step: usize) -> Vec<u8> {
        let w = mask_w(step);
        let h = mask_h(step);
        let mut o = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                o[(y * w + x) as usize] = v[(y * w + (w - 1 - x)) as usize];
            }
        }
        o
    }

    /// Reverse the 7s-row glyph cell (plus its ring), leaving the line frame.
    fn fold_tb(v: &[u8], step: usize, dy: u8) -> Vec<u8> {
        let w = mask_w(step);
        let s = scale_for_step(step);
        let start = dy as i32 * s;
        let end = start + 7 * s + 2;
        let mut o = v.to_vec();
        for y in start..end {
            let src = end - 1 - (y - start);
            for x in 0..w {
                o[(y * w + x) as usize] = v[(src * w + x) as usize];
            }
        }
        o
    }

    fn glyphs() -> Vec<(u32, [u8; 7], u8)> {
        let mut v = Vec::new();
        for cp in 0x20..=0x26FFu32 {
            if let Some(g) = glyph::lookup_fiveseven(cp) {
                v.push((cp, g.rows, g.dy));
            }
        }
        v
    }

    #[test]
    fn geometric_folds_commute_with_upscale() {
        let gs = glyphs();
        let mut checks = 0;
        for (cp, rows, dy) in &gs {
            for step in 0..2 {
                let base = levels(rows, *dy, step);
                assert_eq!(
                    levels(&lr_rows(rows), *dy, step),
                    fold_lr(&base, step),
                    "LR U+{:04X} step {}",
                    cp,
                    step
                );
                assert_eq!(
                    levels(&tb_rows(rows), *dy, step),
                    fold_tb(&base, step, *dy),
                    "TB U+{:04X} step {}",
                    cp,
                    step
                );
                assert_eq!(
                    levels(&both(rows), *dy, step),
                    fold_tb(&fold_lr(&base, step), step, *dy),
                    "180 U+{:04X} step {}",
                    cp,
                    step
                );
                checks += 3;
            }
        }
        println!("fold equivariance: {} checks over {} glyphs", checks, gs.len());
    }

    /// Rotate the 5x5 core at rows 1..5 clockwise, keeping a 7-row canvas.
    fn rot5(rows: &[u8; 7]) -> [u8; 7] {
        let mut out = *rows;
        for r in 1..6 {
            out[r] = 0;
        }
        for r in 0..5 {
            for c in 0..5 {
                let bit = (rows[1 + (4 - c)] >> (4 - r)) & 1;
                if bit != 0 {
                    out[1 + r] |= 1 << (4 - c);
                }
            }
        }
        out
    }

    fn square(v: &[u8], step: usize) -> Vec<u8> {
        let w = mask_w(step);
        let s = scale_for_step(step);
        let mut o = Vec::new();
        for y in s..6 * s + 2 {
            for x in 0..5 * s + 2 {
                o.push(v[(y * w + x) as usize]);
            }
        }
        o
    }

    fn rot90_square(sq: &[u8], n: i32) -> Vec<u8> {
        let mut o = vec![0u8; (n * n) as usize];
        for y in 0..n {
            for x in 0..n {
                o[(y * n + x) as usize] = sq[((n - 1 - x) * n + y) as usize];
            }
        }
        o
    }

    fn clean(rows: &[u8; 7]) -> [u8; 7] {
        let mut r = *rows;
        r[0] = 0;
        r[6] = 0;
        r
    }

    #[test]
    fn quarter_turns_report() {
        let gs = glyphs();
        let mut clean_pass = 0;
        let mut clean_mismatch = 0;
        let mut d4 = 0;
        let mut d4_ok = 0;
        let mut line_d4 = 0;
        let mut line_d4_ok = 0;
        for (cp, rows, _) in &gs {
            for step in 0..2 {
                let s = scale_for_step(step);
                let n = 5 * s + 2;
                // Square cell in isolation: blank the rows outside the 5x5 core.
                let c = clean(rows);
                let base = square(&levels(&c, 0, step), step);
                let rot = square(&levels(&rot5(&c), 0, step), step);
                if rot == rot90_square(&base, n) {
                    clean_pass += 1;
                } else {
                    clean_mismatch += 1;
                }
                if rot5(&c) == c {
                    d4 += 1;
                    let ok = base == rot90_square(&base, n);
                    if ok {
                        d4_ok += 1;
                    }
                    if cp == &0x58 {
                        println!("X step {} clean square 4-fold: {}", step, ok);
                    }
                }
                // Same check in the full 7-row line frame.
                if rot5(&c) == c {
                    let lb = square(&levels(rows, 0, step), step);
                    line_d4 += 1;
                    if lb == rot90_square(&lb, n) {
                        line_d4_ok += 1;
                    }
                    if cp == &0x58 {
                        println!(
                            "X step {} line-frame 4-fold: {}",
                            step,
                            lb == rot90_square(&lb, n)
                        );
                    }
                }
            }
        }
        println!(
            "90deg clean square: {} pass, {} mismatch; D4 clean {}/{}, D4 line-frame {}/{}",
            clean_pass, clean_mismatch, d4_ok, d4, line_d4_ok, line_d4
        );
    }

    /// Naive isolated-quadrant upscale vs the full mask's top-left quadrant.
    #[test]
    fn isolated_quadrant_seam_report() {
        let gs = glyphs();
        let mut diffs = 0;
        let mut total = 0;
        for (cp, rows, _) in &gs {
            let core = rows;
            let q = [0u8, core[1], core[2], 0, 0]; // 3x3 at rows 1..3, cols 1..3 later
            let _ = q;
            for step in 0..2 {
                let s = scale_for_step(step);
                let n3 = 3 * s + 2;
                let n5 = 5 * s + 2;
                // Full mask's top-left quadrant (rows s..4s+2, cols 0..3s+2).
                let full = levels(rows, 0, step);
                let w = mask_w(step);
                let mut fq = Vec::new();
                for y in s..4 * s + 2 {
                    for x in 0..3 * s + 2 {
                        fq.push(full[(y * w + x) as usize]);
                    }
                }
                // Independently upscale the 3x3 quadrant embedded at rows 1..3,
                // cols 1..3 with blank margins.
                let mut emb = [0u8; 7];
                for r in 0..3 {
                    let src = rows[1 + r];
                    let mut bits = 0u8;
                    for c in 0..3 {
                        if src & (1 << (4 - c)) != 0 {
                            bits |= 1 << (3 - c);
                        }
                    }
                    emb[1 + r] = bits << 1; // cols 1..3
                }
                let qm = levels(&emb, 0, step);
                let wq = mask_w(step);
                let mut iq = Vec::new();
                for y in s..4 * s + 2 {
                    for x in s..4 * s + 2 {
                        iq.push(qm[(y * wq + x) as usize]);
                    }
                }
                let d = fq.iter().zip(iq.iter()).filter(|(a, b)| a != b).count();
                if d != 0 {
                    diffs += d;
                    if cp == &0x58 && step == 0 {
                        println!(
                            "X 2x quadrant: {} of {} cells differ (naive isolated upscale)",
                            d,
                            n3 * n3
                        );
                    }
                }
                total += (n3 * n3) as usize;
                let _ = n5;
            }
        }
        println!(
            "naive quadrant seam: {} of {} cells differ across {} glyphs",
            diffs,
            total,
            gs.len()
        );
    }
}
