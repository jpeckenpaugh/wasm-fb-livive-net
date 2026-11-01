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

/// Number of ladder steps (2x, 3x, 4x magnification).
pub const STEPS: usize = 3;
const MAX_W: usize = 22;
const MAX_H: usize = 37;
const MAX_RB: usize = 6;
pub const MAX_BYTES: usize = MAX_H * MAX_RB; // 222

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
    let rb = row_bytes(step);
    (mask[y as usize * rb + x as usize / 4] >> (6 - 2 * ((x as usize) % 4))) & 0x3
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
pub fn glyph(index: u16, rows: &[u8; 7], dy: u8, step: usize) -> &'static [u8; MAX_BYTES] {
    if index == MISSING_INDEX || index as usize >= COUNT || step >= STEPS {
        return &EMPTY;
    }
    ensure(index, rows, dy, step);
    unsafe { &(*CACHE.masks.get())[index as usize][step] }
}

/// Cached ink width of the mask at a step.
pub fn ink_width(index: u16, rows: &[u8; 7], dy: u8, step: usize) -> u8 {
    if index == MISSING_INDEX || index as usize >= COUNT || step >= STEPS {
        return 0;
    }
    ensure(index, rows, dy, step);
    unsafe { (*CACHE.ink.get())[index as usize][step] }
}

/// Cached left bearing of the mask at a step.
pub fn left_bearing(index: u16, rows: &[u8; 7], dy: u8, step: usize) -> u8 {
    if index == MISSING_INDEX || index as usize >= COUNT || step >= STEPS {
        return 0;
    }
    ensure(index, rows, dy, step);
    unsafe { (*CACHE.lsb.get())[index as usize][step] }
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
            let (mut solid, mut mid, mut low) = (0, 0, 0);
            for y in 0..h {
                for x in 0..w {
                    match unpack(m, step, x, y) {
                        3 => solid += 1,
                        2 => mid += 1,
                        1 => low += 1,
                        _ => {}
                    }
                }
            }
            assert!(solid > 0, "step {} solid", step);
            assert!(mid > 0, "step {} mid", step);
            assert!(low > 0, "step {} low", step);
        }
    }

    #[test]
    fn mask_cache_is_stable() {
        let g = glyph::lookup_fiveseven('g' as u32).unwrap();
        let a = *glyph(g.index, g.rows, g.dy, 1);
        let b = *glyph(g.index, g.rows, g.dy, 1);
        assert_eq!(a, b);
        assert!(ink_width(g.index, g.rows, g.dy, 1) > 0);
    }

    #[test]
    fn steps_match_magnification() {
        assert_eq!(step_for_scale(1), 0);
        assert_eq!(step_for_scale(2), 1);
        assert_eq!(step_for_scale(3), 2);
    }
}
