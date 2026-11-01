//! Runtime FiveSeven -> smooth 12x19-line renderer (`smooth.rs`).
//!
//! The smoothed face is deliberately **not** a baked font family. It is derived
//! from the 5x7 source at render time:
//!
//! 1. every source pixel becomes a solid 2x2 block in a 10x14 core with one
//!    row/column of padding around it;
//! 2. the source `dy` is doubled and applied as the glyph offset in a 19-row
//!    line, exactly like FiveSeven's 7-row glyph inside a 9-row line;
//! 3. pass 1: an empty cell with >1 non-empty 4-neighbor becomes 100%;
//! 4. pass 2: an empty cell with >1 non-empty 4-neighbor becomes 66%;
//! 5. pass 3: an empty cell with >0 non-empty 4-neighbor becomes 33%.
//!
//! Each pass sees pixels written by earlier passes.
//!
//! Generated masks are cached in guest linear memory (BSS), so the wasm binary
//! does not carry a second glyph table.

use crate::glyph;
use core::cell::UnsafeCell;

pub const CELL_W: i32 = 12;
pub const CELL_H: i32 = 19;
pub const ROW_BYTES: usize = 3;
pub const GLYPH_BYTES: usize = CELL_H as usize * ROW_BYTES; // 57

const COUNT: usize = glyph::FIVESEVEN_GLYPH_COUNT;
const MISSING_INDEX: u16 = u16::MAX;

struct Cache {
    masks: UnsafeCell<[[u8; GLYPH_BYTES]; COUNT]>,
    lsb: UnsafeCell<[u8; COUNT]>,
    ink: UnsafeCell<[u8; COUNT]>,
    valid: UnsafeCell<[bool; COUNT]>,
}

// Single-threaded wasm guest / test process.
unsafe impl Sync for Cache {}

static CACHE: Cache = Cache {
    masks: UnsafeCell::new([[0u8; GLYPH_BYTES]; COUNT]),
    lsb: UnsafeCell::new([0u8; COUNT]),
    ink: UnsafeCell::new([0u8; COUNT]),
    valid: UnsafeCell::new([false; COUNT]),
};

static EMPTY: [u8; GLYPH_BYTES] = [0u8; GLYPH_BYTES];

#[allow(clippy::needless_range_loop)]
fn fill_pass(
    levels: &[[u8; CELL_W as usize]; CELL_H as usize],
    threshold: u8,
    value: u8,
) -> [[u8; CELL_W as usize]; CELL_H as usize] {
    let mut out = *levels;
    for y in 0..CELL_H {
        for x in 0..CELL_W {
            if levels[y as usize][x as usize] != 0 {
                continue;
            }
            let n = non_empty(levels, x - 1, y) as u8
                + non_empty(levels, x + 1, y) as u8
                + non_empty(levels, x, y - 1) as u8
                + non_empty(levels, x, y + 1) as u8;
            if n > threshold {
                out[y as usize][x as usize] = value;
            }
        }
    }
    out
}

#[inline]
fn non_empty(levels: &[[u8; CELL_W as usize]; CELL_H as usize], x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= CELL_W || y >= CELL_H {
        return false;
    }
    levels[y as usize][x as usize] != 0
}

#[allow(clippy::needless_range_loop)]
fn generate(rows: &[u8; 7], dy: u8) -> ([u8; GLYPH_BYTES], u8, u8) {
    let mut base = [[0u8; CELL_W as usize]; CELL_H as usize];

    // 1. Exact 2x core. The source dy is doubled, matching FiveSeven's
    // placement of a 7-row glyph inside a 9-row line. Leave one row/column
    // of padding around the core; the fill passes operate on that padding too.
    for r in 0..7 {
        for c in 0..5 {
            if rows[r] & (1 << (4 - c)) == 0 {
                continue;
            }
            let x0 = 1 + 2 * c;
            let y0 = 1 + 2 * dy as i32 + 2 * r as i32;
            for sy in 0..2 {
                for sx in 0..2 {
                    let y = y0 + sy;
                    let x = x0 + sx;
                    if (0..CELL_H).contains(&y) && (0..CELL_W).contains(&x) {
                        base[y as usize][x as usize] = 3;
                    }
                }
            }
        }
    }

    // Three sequential additive passes. Each pass counts any non-empty
    // (100/66/33) 4-neighbor, and writes its result simultaneously.
    let mut levels = base;
    levels = fill_pass(&levels, 1, 3);
    levels = fill_pass(&levels, 1, 2);
    levels = fill_pass(&levels, 0, 1);

    // Pack 12 pixels x 2 bits into three bytes per row.
    let mut packed = [0u8; GLYPH_BYTES];
    for y in 0..CELL_H as usize {
        for x in 0..CELL_W as usize {
            packed[y * ROW_BYTES + x / 4] |= levels[y][x] << (6 - 2 * (x % 4));
        }
    }

    let mut lsb = 12u8;
    let mut right = 0u8;
    for x in 0..CELL_W as usize {
        let used = (0..CELL_H as usize).any(|y| levels[y][x] != 0);
        if used {
            if lsb == 12 {
                lsb = x as u8;
            }
            right = x as u8;
        }
    }
    let ink = if lsb == 12 { 0 } else { right - lsb + 1 };
    (packed, lsb, ink)
}

fn ensure(index: u16, rows: &[u8; 7], dy: u8) {
    let i = index as usize;
    if index == MISSING_INDEX || i >= COUNT {
        return;
    }
    unsafe {
        if !(*CACHE.valid.get())[i] {
            let (packed, lsb, ink) = generate(rows, dy);
            (*CACHE.masks.get())[i] = packed;
            (*CACHE.lsb.get())[i] = lsb;
            (*CACHE.ink.get())[i] = ink;
            (*CACHE.valid.get())[i] = true;
        }
    }
}

/// Get (or lazily generate) the packed 12x19-line mask for a source glyph.
pub fn glyph(index: u16, rows: &[u8; 7], dy: u8) -> &'static [u8; GLYPH_BYTES] {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return &EMPTY;
    }
    ensure(index, rows, dy);
    unsafe { &(*CACHE.masks.get())[index as usize] }
}

/// Cached ink width of the smoothed mask, used for proportional spacing.
pub fn ink_width(index: u16, rows: &[u8; 7], dy: u8) -> u8 {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return 0;
    }
    ensure(index, rows, dy);
    unsafe { (*CACHE.ink.get())[index as usize] }
}

/// Cached left bearing of the smoothed mask.
pub fn left_bearing(index: u16, rows: &[u8; 7], dy: u8) -> u8 {
    if index == MISSING_INDEX || index as usize >= COUNT {
        return 0;
    }
    ensure(index, rows, dy);
    unsafe { (*CACHE.lsb.get())[index as usize] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x_is_filled_and_still_has_solid_core() {
        let g = glyph::lookup_fiveseven('X' as u32).unwrap();
        let m = glyph(g.index, g.rows, g.dy);
        let mut solid = 0;
        let mut level2 = 0;
        let mut level1 = 0;
        for &b in m.iter() {
            for shift in [6, 4, 2, 0] {
                match (b >> shift) & 0x3 {
                    3 => solid += 1,
                    2 => level2 += 1,
                    1 => level1 += 1,
                    _ => {}
                }
            }
        }
        assert!(solid > 0, "X must retain full-coverage pixels");
        assert!(level2 > 0, "X must receive 66% fill pixels");
        assert!(level1 > 0, "X must receive 33% fill pixels");
    }

    #[test]
    fn mask_cache_is_stable() {
        let g = glyph::lookup_fiveseven('g' as u32).unwrap();
        let a = *glyph(g.index, g.rows, g.dy);
        let b = *glyph(g.index, g.rows, g.dy);
        assert_eq!(a, b);
        assert!(ink_width(g.index, g.rows, g.dy) > 0);
    }
}
