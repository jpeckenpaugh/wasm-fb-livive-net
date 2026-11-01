//! Fixed-width orbit/fold decoder (`folded.rs`).
//!
//! Reads `glyphs/fiveseven.fold`, produced by `tools/encode_font.py`:
//!
//! ```text
//! column word (8 bits)
//!   bits 0..5   orbit id (36 shapes under invert + reverse)
//!   bit  6      polarity  (invert)
//!   bit  7      direction (reverse)
//!
//! glyph record: 2-bit mode, payload, 2-bit dy
//!   0 raw        5 column words
//!   1 self       1-bit fold (lr/180) + 3 column words
//!   2 reference  10-bit glyph index + 2-bit fold
//!   3 template   9-bit 3x3 quadrant + 2-bit row extrusion
//! ```
//!
//! No entropy coding: every field is fixed width. Enabled with
//! `--features folded-glyphs`.

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::glyph::FIVESEVEN_GLYPH_COUNT;

const BLOB: &[u8] = include_bytes!("../glyphs/fiveseven.fold");
const N: usize = FIVESEVEN_GLYPH_COUNT;
const MAX_ORBITS: usize = 64;

pub const BLOB_LEN: usize = BLOB.len();

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn get(&mut self, n: u8) -> u32 {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = self.data[self.pos >> 3];
            let bit = (byte >> (7 - (self.pos & 7))) & 1;
            v = (v << 1) | bit as u32;
            self.pos += 1;
        }
        v
    }
}

fn rev7(v: u8) -> u8 {
    let mut r = 0u8;
    for i in 0..7 {
        if v & (1 << i) != 0 {
            r |= 1 << (6 - i);
        }
    }
    r
}

fn col_from_word(w: u8, protos: &[u8; MAX_ORBITS]) -> u8 {
    let mut p = protos[(w & 0x3F) as usize];
    if w & 0x80 != 0 {
        p = rev7(p);
    }
    if w & 0x40 != 0 {
        p ^= 0x7F;
    }
    p
}

/// kind: 0 same, 1 left-right, 2 top-bottom, 3 180 degrees.
fn fold(c: &[u8; 5], kind: u8) -> [u8; 5] {
    match kind {
        0 => *c,
        1 => [c[4], c[3], c[2], c[1], c[0]],
        2 => [rev7(c[0]), rev7(c[1]), rev7(c[2]), rev7(c[3]), rev7(c[4])],
        _ => [
            rev7(c[4]),
            rev7(c[3]),
            rev7(c[2]),
            rev7(c[1]),
            rev7(c[0]),
        ],
    }
}

fn to_rows(c: &[u8; 5]) -> [u8; 7] {
    let mut rows = [0u8; 7];
    for (col, &v) in c.iter().enumerate() {
        for (r, row) in rows.iter_mut().enumerate() {
            if v & (1 << (6 - r)) != 0 {
                *row |= 1 << (4 - col);
            }
        }
    }
    rows
}

fn from_rows(rows: &[u8; 7]) -> [u8; 5] {
    let mut c = [0u8; 5];
    for (col, cv) in c.iter_mut().enumerate() {
        for (r, &row) in rows.iter().enumerate() {
            if row & (1 << (4 - col)) != 0 {
                *cv |= 1 << (6 - r);
            }
        }
    }
    c
}

fn rot3(q: &[[u8; 3]; 3]) -> [[u8; 3]; 3] {
    let mut r = [[0u8; 3]; 3];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = q[2 - j][i];
        }
    }
    r
}

fn template_rows(qbits: u16, extrude: u8) -> [u8; 7] {
    let mut q = [[0u8; 3]; 3];
    for (r, row) in q.iter_mut().enumerate() {
        for (c, v) in row.iter_mut().enumerate() {
            *v = ((qbits >> (8 - (r * 3 + c))) & 1) as u8;
        }
    }
    let r1 = rot3(&q);
    let r2 = rot3(&r1);
    let r3 = rot3(&r2);
    let mut grid = [[0u8; 5]; 5];
    for (t, r0, c0) in [(&q, 0usize, 0usize), (&r1, 0, 2), (&r2, 2, 2), (&r3, 2, 0)] {
        for (r, row) in t.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                grid[r0 + r][c0 + c] = v;
            }
        }
    }
    let mut core = [0u8; 5];
    for (r, row) in grid.iter().enumerate() {
        for (c, &v) in row.iter().enumerate() {
            if v != 0 {
                core[r] |= 1 << (4 - c);
            }
        }
    }
    [
        if extrude & 1 != 0 { core[0] } else { 0 },
        core[0],
        core[1],
        core[2],
        core[3],
        core[4],
        if extrude & 2 != 0 { core[4] } else { 0 },
    ]
}

struct Store {
    rows: [[u8; 7]; N],
    dy: [u8; N],
}

struct StoreCell(UnsafeCell<Store>);

// Single-threaded wasm guest / test process.
unsafe impl Sync for StoreCell {}

static STORE: StoreCell = StoreCell(UnsafeCell::new(Store {
    rows: [[0; 7]; N],
    dy: [0; N],
}));
static READY: AtomicBool = AtomicBool::new(false);

fn decode_into(store: &mut Store) {
    let mut r = Reader::new(BLOB);
    let magic = r.get(8);
    let version = r.get(8);
    debug_assert_eq!(magic, 0x46);
    debug_assert_eq!(version, 2);
    let n = r.get(16) as usize;
    let norbits = r.get(8) as usize;
    debug_assert!(n <= N && norbits <= MAX_ORBITS);

    let mut protos = [0u8; MAX_ORBITS];
    for p in protos.iter_mut().take(norbits) {
        *p = r.get(7) as u8;
    }

    for i in 0..n {
        let mode = r.get(2) as u8;
        let rows: [u8; 7] = match mode {
            0 => {
                let mut c = [0u8; 5];
                for v in c.iter_mut() {
                    *v = col_from_word(r.get(8) as u8, &protos);
                }
                to_rows(&c)
            }
            1 => {
                let kind = r.get(1) as u8;
                let mut h = [0u8; 3];
                for v in h.iter_mut() {
                    *v = col_from_word(r.get(8) as u8, &protos);
                }
                let c = if kind == 0 {
                    [h[0], h[1], h[2], h[1], h[0]]
                } else {
                    [h[0], h[1], h[2], rev7(h[1]), rev7(h[0])]
                };
                to_rows(&c)
            }
            2 => {
                let j = r.get(10) as usize;
                let kind = r.get(2) as u8;
                debug_assert!(j < i);
                let base = from_rows(&store.rows[j]);
                to_rows(&fold(&base, kind))
            }
            _ => {
                let qbits = r.get(9) as u16;
                let extrude = r.get(2) as u8;
                template_rows(qbits, extrude)
            }
        };
        store.rows[i] = rows;
        store.dy[i] = r.get(2) as u8;
    }
}

/// Decode the whole blob into the static row table.
pub fn decode_all() {
    unsafe {
        decode_into(&mut *STORE.0.get());
    }
}

pub fn ensure_init() {
    if !READY.load(Ordering::Relaxed) {
        decode_all();
        READY.store(true, Ordering::Relaxed);
    }
}

pub fn rows(idx: usize) -> [u8; 7] {
    unsafe { (*STORE.0.get()).rows[idx] }
}

pub fn dy(idx: usize) -> u8 {
    unsafe { (*STORE.0.get()).dy[idx] }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph;

    #[test]
    fn folded_matches_packed() {
        ensure_init();
        for idx in 0..N {
            let p = glyph::packed_glyph(idx);
            assert_eq!(rows(idx), p.rows, "rows idx {}", idx);
            assert_eq!(dy(idx), p.dy, "dy idx {}", idx);
        }
    }
}
