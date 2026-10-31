//! Unified glyph store (`glyph.rs`).
//!
//! One codepoint space, two glyph formats:
//! - **FiveSeven** text glyphs: 5x7 cells, 1 bit per pixel, with per-glyph metrics
//!   (`dy` vertical bump for descenders, auto-trimmed left bearing / ink width for
//!   proportional spacing).
//! - **Icons32**: 32x32 cells, 2 bits per pixel. The four levels are coverage steps
//!   of a single ink color (0 = transparent, 1 = 33%, 2 = 66%, 3 = 100%), so an icon
//!   drawn black-on-white renders as white / light grey / dark grey / black.
//!
//! All tables are generated at build time by `build.rs` from the ASCII-art sources
//! in `v2/glyphs/` (see `glyphs/fiveseven/*.txt` and `glyphs/icons32/*.txt`).
//!
//! Lookup order for a codepoint: FiveSeven sheets -> Icons32 Private Use Area
//! (`U+E000..`) -> Unicode alias table (e.g. `U+2302 ⌂` -> Home) -> missing box.

use crate::fb::Framebuffer;

include!(concat!(env!("OUT_DIR"), "/glyph_tables.rs"));

/// First Private Use Area codepoint assigned to Icons32.
pub const ICON_PUA_BASE: u32 = 0xE000;
pub const ICON_SIZE: i32 = 32;

/// Missing-glyph box (5x7).
pub const MISSING_ROWS: [u8; 7] = [0x1F, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1F];

/// A 5x7 text glyph plus its metrics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextGlyph {
    pub rows: &'static [u8; 7],
    /// Rows to shift the glyph down (descender bump), 0..=3.
    pub dy: u8,
    /// Leftmost inked column (0..=4).
    pub lsb: u8,
    /// Inked column span (0 for blank glyphs such as SPACE).
    pub ink_w: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Text(TextGlyph),
    /// Index into the Icons32 table.
    Icon(u16),
    Missing,
}

/// Resolve a Unicode scalar value to a glyph.
pub fn resolve(cp: u32) -> Glyph {
    if let Some(t) = lookup_fiveseven(cp) {
        return Glyph::Text(t);
    }
    if let Some(i) = icon_index_for_cp(cp) {
        return Glyph::Icon(i);
    }
    Glyph::Missing
}

pub fn lookup_fiveseven(cp: u32) -> Option<TextGlyph> {
    // Binary search over contiguous codepoint runs.
    let (mut lo, mut hi) = (0usize, FIVESEVEN_RANGES.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        let (first, last, base) = FIVESEVEN_RANGES[mid];
        if cp < first {
            hi = mid;
        } else if cp > last {
            lo = mid + 1;
        } else {
            let idx = base as usize + (cp - first) as usize;
            let m = FIVESEVEN_METRICS[idx];
            return Some(TextGlyph {
                rows: &FIVESEVEN_ROWS[idx],
                dy: m >> 6,
                lsb: (m >> 3) & 0x7,
                ink_w: m & 0x7,
            });
        }
    }
    None
}

/// Real Unicode symbols that render with an Icons32 glyph when FiveSeven has no
/// glyph of its own. Sorted by codepoint for binary search.
static ICON_ALIASES: [(u32, u16); 32] = [
    (0x2139, 29),  // ℹ info
    (0x2190, 1),   // ← back
    (0x2191, 3),   // ↑ up
    (0x2192, 2),   // → forward
    (0x22EF, 7),   // ⋯ more
    (0x2302, 0),   // ⌂ home
    (0x23F8, 25),  // ⏸ pause
    (0x23F9, 26),  // ⏹ stop
    (0x25B6, 24),  // ▶ play
    (0x2630, 6),   // ☰ menu
    (0x2699, 12),  // ⚙ settings
    (0x26A0, 30),  // ⚠ warning
    (0x270E, 14),  // ✎ edit
    (0x2713, 5),   // ✓ check
    (0x2715, 4),   // ✕ close
    (0x2795, 8),   // ➕ plus
    (0x2796, 9),   // ➖ minus
    (0x1F3AE, 21), // 🎮 games
    (0x1F4BE, 15), // 💾 save
    (0x1F4C1, 17), // 📁 folder
    (0x1F4C4, 18), // 📄 document
    (0x1F504, 11), // 🔄 refresh
    (0x1F507, 28), // 🔇 mute
    (0x1F50A, 27), // 🔊 speaker
    (0x1F50D, 10), // 🔍 search
    (0x1F512, 31), // 🔒 lock
    (0x1F552, 23), // 🕒 clock
    (0x1F5A5, 22), // 🖥 system
    (0x1F5A9, 20), // 🖩 calculator
    (0x1F5BC, 19), // 🖼 image
    (0x1F5C2, 16), // 🗂 files
    (0x1F5D1, 13), // 🗑 trash
];

pub fn icon_index_for_cp(cp: u32) -> Option<u16> {
    if cp >= ICON_PUA_BASE && cp < ICON_PUA_BASE + ICON32_COUNT as u32 {
        return Some((cp - ICON_PUA_BASE) as u16);
    }
    ICON_ALIASES
        .binary_search_by_key(&cp, |&(c, _)| c)
        .ok()
        .map(|i| ICON_ALIASES[i].1)
}

/// 2-bit level (0..=3) of icon pixel `(x, y)`.
#[inline]
pub fn icon_level(index: u16, x: i32, y: i32) -> u32 {
    let d = &ICON32_DATA[index as usize];
    let b = d[(y * 8 + x / 4) as usize];
    ((b >> (6 - 2 * (x % 4))) & 0x3) as u32
}

/// Draw Icons32 glyph `index` into a `size x size` pixel box at `(x, y)` in `color`.
///
/// - `size == 32 * k`: each source pixel becomes a `k x k` block (exact 4-step levels).
/// - any other size: area-averaged (box filter) resampling, so downscaled icons keep
///   smooth edges instead of dropping pixels.
pub fn draw_icon_index(fb: &mut Framebuffer, x: i32, y: i32, index: u16, size: i32, color: u32) {
    if size <= 0 || index as usize >= ICON32_COUNT {
        return;
    }
    // Clip early: skip icons entirely outside the framebuffer.
    if x >= fb.w || y >= fb.h || x + size <= 0 || y + size <= 0 {
        return;
    }
    if size % ICON_SIZE == 0 {
        let k = size / ICON_SIZE;
        for sy in 0..ICON_SIZE {
            for sx in 0..ICON_SIZE {
                let lv = icon_level(index, sx, sy);
                if lv != 0 {
                    fb.blend(x + sx * k, y + sy * k, k, k, color, lv * 85);
                }
            }
        }
        return;
    }
    for dy in 0..size {
        let sy0 = dy * ICON_SIZE / size;
        let sy1 = ((dy + 1) * ICON_SIZE / size).max(sy0 + 1);
        for dx in 0..size {
            let sx0 = dx * ICON_SIZE / size;
            let sx1 = ((dx + 1) * ICON_SIZE / size).max(sx0 + 1);
            let mut sum = 0u32;
            for sy in sy0..sy1 {
                for sx in sx0..sx1 {
                    sum += icon_level(index, sx, sy);
                }
            }
            let n = ((sy1 - sy0) * (sx1 - sx0)) as u32;
            let alpha = (sum * 85 + n / 2) / n; // level 3 -> 255
            if alpha != 0 {
                fb.blend(x + dx, y + dy, 1, 1, color, alpha);
            }
        }
    }
}
