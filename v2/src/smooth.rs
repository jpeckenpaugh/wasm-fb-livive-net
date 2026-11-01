//! Runtime FiveSeven -> smooth `TwelveSixteen` renderer (`smooth.rs`).
//!
//! The family is a ladder of generated faces, one per source magnification:
//!
//! | step | source | core  | cell   | advance |
//! |------|--------|-------|--------|---------|
//! | 0    | 2x     | 10x14 | 12x19  | 13      |
//! | 1    | 3x     | 15x21 | 17x28  | 18      |
//! | 2    | 4x     | 20x28 | 22x37  | 23      |
//!
//! Every step is produced at its own resolution with the same rules:
//!
//! 1. every source pixel becomes a solid `s x s` block in a `5s x 7s` core with
//!    one row/column of padding around it;
//! 2. the source `dy` is multiplied by `s` and applied as the glyph offset;
//! 3. pass 1: empty + `>1` non-empty 4-neighbor -> 100%;
//! 4. pass 2: empty + `>1` non-empty 4-neighbor -> 66%;
//! 5. pass 3: empty + `>0` non-empty 4-neighbor -> 33%.
//!
//! Packed at 2 bits per pixel and cached lazily in guest linear memory.

use crate::glyph;
use core::cell::UnsafeCell;

/// Number of ladder steps: 1x .. 8x labels, magnification 2 .. 9.
pub const STEPS: usize = 8;
/// Steps 0..2 keep the original 2-bit masks; 3..7 are 8-bit.
const LEGACY_STEPS: usize = 3;
const MAX_W: usize = 22;
const MAX_H: usize = 37;
const MAX_RB: usize = 6;
pub const MAX_BYTES: usize = MAX_H * MAX_RB; // 222

/// Extended (4x..8x) masks are bytes, up to 47x82.
const BIG_W: usize = 47;
const BIG_H: usize = 82;
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
    lsb: UnsafeCell<[[u8; STEPS]; COUNT]>,
    ink: UnsafeCell<[[u8; STEPS]; COUNT]>,
    valid: UnsafeCell<[[bool; STEPS]; COUNT]>,
}

// Single-threaded wasm guest / test process.
unsafe impl Sync for Cache {}

static CACHE: Cache = Cache {
    masks: UnsafeCell::new([[[0u8; MAX_BYTES]; STEPS]; COUNT]),
    lsb: UnsafeCell::new([[0u8; STEPS]; COUNT]),
    ink: UnsafeCell::new([[0u8; STEPS]; COUNT]),
    valid: UnsafeCell::new([[false; STEPS]; COUNT]),
};

static EMPTY: [u8; MAX_BYTES] = [0u8; MAX_BYTES];

/// Ladder step index for a draw scale (1 -> 2x, 2 -> 3x, 3+ -> 4x).
pub fn step_for_scale(scale: i32) -> usize {
    match scale {
        s if s <= 1 => 0,
        2 => 1,
        _ => 2,
    }
}

/// Source magnification for a step (2, 3, 4).
pub fn magnification(step: usize) -> i32 {
    step as i32 + 2
}

pub fn cell_w_ext(step: usize) -> i32 {
    cell_w(step)
}

pub fn cell_h_ext(step: usize) -> i32 {
    cell_h(step)
}

pub fn cell_w(step: usize) -> i32 {
    5 * magnification(step) + 2
}

pub fn cell_h(step: usize) -> i32 {
    1 + 9 * magnification(step)
}

pub fn row_bytes(step: usize) -> usize {
    ((cell_w(step) + 3) / 4) as usize
}

pub fn descent(step: usize) -> i32 {
    2 * magnification(step)
}

#[inline]
pub fn unpack(mask: &[u8], step: usize, x: i32, y: i32) -> u8 {
    if step < LEGACY_STEPS {
        let rb = row_bytes(step);
        let v = (mask[y as usize * rb + x as usize / 4] >> (6 - 2 * ((x as usize) % 4))) & 0x3;
        v * 85
    } else {
        mask[y as usize * cell_w(step) as usize + x as usize]
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
fn generate(rows: &[u8; 7], dy: u8, step: usize) -> ([u8; MAX_BYTES], u8, u8) {
    let mag = magnification(step);
    let w = cell_w(step);
    let h = cell_h(step);
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

    // 6. Proportional metrics.
    let mut lsb = w as u8;
    let mut right = 0u8;
    for x in 0..w as usize {
        let used = (0..h as usize).any(|y| levels[y][x] != 0);
        if used {
            if lsb == w as u8 {
                lsb = x as u8;
            }
            right = x as u8;
        }
    }
    let ink = if lsb == w as u8 { 0 } else { right - lsb + 1 };

    (packed, lsb, ink)
}

fn ensure(index: u16, rows: &[u8; 7], dy: u8, step: usize) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*CACHE.valid.get())[i][step] {
            let (packed, lsb, ink) = generate(rows, dy, step);
            (*CACHE.masks.get())[i][step] = packed;
            (*CACHE.lsb.get())[i][step] = lsb;
            (*CACHE.ink.get())[i][step] = ink;
            (*CACHE.valid.get())[i][step] = true;
        }
    }
}

/// Get (or lazily generate) the packed mask for a glyph at a ladder step.
pub fn glyph(index: u16, rows: &[u8; 7], dy: u8, step: usize) -> &'static [u8] {
    if index == MISSING_INDEX || index as usize >= COUNT || step >= STEPS {
        return &EMPTY;
    }
    if step < LEGACY_STEPS {
        ensure(index, rows, dy, step);
        unsafe { &(*CACHE.masks.get())[index as usize][step] }
    } else {
        big_glyph(index, rows, dy, step)
    }
}

/// Cached ink width of the mask at a step.
pub fn ink_width(index: u16, rows: &[u8; 7], dy: u8, step: usize) -> u8 {
    if index == MISSING_INDEX || index as usize >= COUNT || step >= STEPS {
        return 0;
    }
    if step < LEGACY_STEPS {
        ensure(index, rows, dy, step);
    } else {
        let _ = big_glyph(index, rows, dy, step);
    }
    unsafe { (*CACHE.ink.get())[index as usize][step] }
}

/// Cached left bearing of the mask at a step.
pub fn left_bearing(index: u16, rows: &[u8; 7], dy: u8, step: usize) -> u8 {
    if index == MISSING_INDEX || index as usize >= COUNT || step >= STEPS {
        return 0;
    }
    if step < LEGACY_STEPS {
        ensure(index, rows, dy, step);
    } else {
        let _ = big_glyph(index, rows, dy, step);
    }
    unsafe { (*CACHE.lsb.get())[index as usize][step] }
}

fn ramp(step: usize) -> &'static [u8] {
    match step {
        3 => &[255, 191, 128, 64],
        4 => &[255, 204, 153, 102, 51],
        5 | 6 => &[255, 212, 170, 128, 85, 43],
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

/// 8-bit generator for steps 3..7 (4x..8x). Rings are gated to cells reachable
/// from the border so counters stay open.
#[allow(clippy::needless_range_loop)]
fn generate_big(rows: &[u8; 7], dy: u8, step: usize) -> ([u8; BIG_MAX], usize, u8, u8) {
    let mag = magnification(step);
    let w = cell_w(step);
    let h = cell_h(step);
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
    let mut push = |x: usize,
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

    let mut lsb = w as u8;
    let mut right = 0u8;
    for x in 0..w as usize {
        if (0..h as usize).any(|y| mask[y * w as usize + x] != 0) {
            if lsb == w as u8 {
                lsb = x as u8;
            }
            right = x as u8;
        }
    }
    let ink = if lsb == w as u8 { 0 } else { right - lsb + 1 };
    (mask, len, lsb, ink)
}

fn big_glyph(index: u16, rows: &[u8; 7], dy: u8, step: usize) -> &'static [u8] {
    let key = ((index as u32) << 4) | (step as u32 + 1);
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
                let (mask, len, lsb, ink) = generate_big(rows, dy, step);
                let slot = &mut slots[victim];
                slot.key = key;
                slot.len = len as u32;
                slot.age = tick;
                slot.data[..len].copy_from_slice(&mask[..len]);
                (*CACHE.lsb.get())[index as usize][step] = lsb;
                (*CACHE.ink.get())[index as usize][step] = ink;
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
    fn ladder_cells_are_expected() {
        assert_eq!((cell_w(0), cell_h(0)), (12, 19));
        assert_eq!((cell_w(1), cell_h(1)), (17, 28));
        assert_eq!((cell_w(2), cell_h(2)), (22, 37));
        assert_eq!(row_bytes(0), 3);
        assert_eq!(row_bytes(1), 5);
        assert_eq!(row_bytes(2), 6);
    }

    #[test]
    fn x_masks_keep_three_levels() {
        let g = glyph::lookup_fiveseven('X' as u32).unwrap();
        for step in 0..STEPS {
            let m = glyph(g.index, g.rows, g.dy, step);
            let w = cell_w(step);
            let h = cell_h(step);
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
        let a = glyph(g.index, g.rows, g.dy, 1).to_vec();
        let b = glyph(g.index, g.rows, g.dy, 1).to_vec();
        assert_eq!(a, b);
        assert!(ink_width(g.index, g.rows, g.dy, 1) > 0);
    }

    #[test]
    fn steps_match_magnification() {
        assert_eq!(step_for_scale(1), 0);
        assert_eq!(step_for_scale(2), 1);
        assert_eq!(step_for_scale(3), 2);
        assert_eq!(magnification(3), 5);
        assert_eq!(magnification(7), 9);
        assert_eq!((cell_w(3), cell_h(3)), (27, 46));
        assert_eq!((cell_w(7), cell_h(7)), (47, 82));
    }

    fn fnv(mask: &[u8]) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for &b in mask.iter().take(222) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    #[test]
    fn legacy_masks_are_pinned() {
        // Recorded from the approved 1x/2x/3x build. Any change here is a
        // regression in the masks the user has already tuned by eye.
        let pinned = [
            ('A', 0, 8114468120287716712u64),
            ('A', 1, 10191097592905013314),
            ('A', 2, 3734492700330503937),
            ('S', 0, 4610887887618203869),
            ('S', 1, 555551942518664327),
            ('S', 2, 12359185727515687505),
            ('e', 0, 640295919539083458),
            ('e', 1, 10347693170050180963),
            ('e', 2, 2990073217798374973),
            ('g', 0, 15416199664591670312),
            ('g', 1, 4908155948678889052),
            ('g', 2, 210272780627839880),
        ];
        for (ch, step, want) in pinned {
            let g = glyph::lookup_fiveseven(ch as u32).unwrap();
            let got = fnv(glyph(g.index, g.rows, g.dy, step));
            assert_eq!(got, want, "mask drift for {} step {}", ch, step);
        }
    }
}
