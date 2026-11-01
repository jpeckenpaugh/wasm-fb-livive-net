//! Runtime semantic color palette for the v2 appliance (`theme.rs`).
//!
//! Tokens are sentinel values in the `0xFE......` range. `Framebuffer` resolves a
//! sentinel through the active [`THEMES`] entry at draw time, so UI code keeps
//! referring to `t::ACCENT` while themes can change without recompiling.

use crate::fb::rgb;
use core::sync::atomic::{AtomicUsize, Ordering};

const TOKEN_MARKER: u32 = 0xFE00_0000;
const TOKEN_MASK: u32 = 0xFF00_0000;

const fn token(index: u32) -> u32 {
    TOKEN_MARKER | index
}

// Token indices. Keep aliases at the bottom.
pub const BG: u32 = token(0);
pub const BG_DEEP: u32 = token(1);
pub const SURFACE: u32 = token(2);
pub const SURFACE_RAISED: u32 = token(3);
pub const SURFACE_ALT: u32 = token(4);
pub const SURFACE_ACTIVE: u32 = token(5);
pub const SURFACE_SELECTED: u32 = token(6);
pub const SURFACE_CONTROL: u32 = token(7);
pub const SURFACE_HOVER: u32 = token(8);
pub const BAR: u32 = token(9);
pub const TITLE_BG: u32 = token(10);
pub const OVERLAY: u32 = token(11);
pub const GRID: u32 = token(12);
pub const WHITE: u32 = token(13);
pub const BLACK: u32 = token(14);
pub const BORDER: u32 = token(15);
pub const BORDER_DARK: u32 = token(16);
pub const BORDER_SOFT: u32 = token(17);
pub const BORDER_STRONG: u32 = token(18);
pub const BORDER_CONTROL: u32 = token(19);
pub const TEXT: u32 = token(20);
pub const TEXT_BODY: u32 = token(21);
pub const TEXT_DIM: u32 = token(22);
pub const TEXT_MUTED: u32 = token(23);
pub const TEXT_SOFT: u32 = token(24);
pub const TEXT_MENU: u32 = token(25);
pub const TEXT_CODE: u32 = token(26);
pub const ACCENT: u32 = token(27);
pub const ACCENT_DIM: u32 = token(28);
pub const FOCUS: u32 = token(29);
pub const SUCCESS: u32 = token(30);
pub const WARNING: u32 = token(31);
pub const WARNING_BRIGHT: u32 = token(32);
pub const DANGER: u32 = token(33);
pub const DANGER_SOFT: u32 = token(34);
pub const INFO: u32 = token(35);
pub const PURPLE: u32 = token(36);
pub const CALC_CLEAR: u32 = token(37);
pub const CALC_OP: u32 = token(38);
pub const CALC_KEY_BORDER: u32 = token(39);
pub const MINES_HIDDEN: u32 = token(40);
pub const MINES_REVEALED_BORDER: u32 = token(41);
pub const MINES_DETONATED: u32 = token(42);
pub const MINES_DETONATED_BORDER: u32 = token(43);
pub const MINES_FLAG: u32 = token(44);
pub const MINES_TERMINAL_MINE: u32 = token(45);
pub const MINES_TERMINAL_FLAG: u32 = token(46);
pub const MINES_FLAG_MODE: u32 = token(47);
pub const MINES_REVEAL_MODE: u32 = token(48);
pub const TTS_ACTIVE: u32 = token(49);
pub const CODE_GUTTER: u32 = token(50);
pub const CODE_LINE_NO: u32 = token(51);
pub const SYN_KEYWORD: u32 = token(52);
pub const SYN_TYPE: u32 = token(53);
pub const SYN_STRING: u32 = token(54);
pub const SYN_NUMBER: u32 = token(55);
pub const SYN_COMMENT: u32 = token(56);
pub const SYN_PUNCT: u32 = token(57);
pub const SYN_PREPROC: u32 = token(58);

pub const CALC_EQ: u32 = ACCENT_DIM;
pub const MINES_HIDDEN_BORDER: u32 = BORDER_CONTROL;
pub const MINES_REVEALED: u32 = GRID;
pub const SYN_PLAIN: u32 = TEXT_CODE;

pub const TOKEN_COUNT: usize = 59;
pub const THEME_COUNT: usize = 10;

pub const THEME_NAMES: [&str; THEME_COUNT] = [
    "Midnight",
    "Slate",
    "Ocean",
    "Forest",
    "Ember",
    "Amethyst",
    "Mono",
    "Solarized",
    "Rose",
    "Copper",
];

const DARK_BASE: [u32; TOKEN_COUNT] = [
    rgb(0x0b, 0x10, 0x16), // BG
    rgb(0x09, 0x0d, 0x12), // BG_DEEP
    rgb(0x10, 0x17, 0x20), // SURFACE
    rgb(0x14, 0x1d, 0x26), // SURFACE_RAISED
    rgb(0x14, 0x1f, 0x2a), // SURFACE_ALT
    rgb(0x13, 0x22, 0x2b), // SURFACE_ACTIVE
    rgb(0x13, 0x22, 0x2e), // SURFACE_SELECTED
    rgb(0x18, 0x26, 0x33), // SURFACE_CONTROL
    rgb(0x1b, 0x28, 0x36), // SURFACE_HOVER
    rgb(0x0e, 0x16, 0x20), // BAR
    rgb(0x0c, 0x11, 0x17), // TITLE_BG
    rgb(0x08, 0x10, 0x14), // OVERLAY
    rgb(0x0f, 0x16, 0x1f), // GRID
    rgb(0xff, 0xff, 0xff), // WHITE
    rgb(0x00, 0x00, 0x00), // BLACK
    rgb(0x22, 0x30, 0x3e), // BORDER
    rgb(0x18, 0x22, 0x2d), // BORDER_DARK
    rgb(0x1f, 0x2c, 0x38), // BORDER_SOFT
    rgb(0x2e, 0x3f, 0x50), // BORDER_STRONG
    rgb(0x26, 0x38, 0x4a), // BORDER_CONTROL
    rgb(0xf0, 0xf6, 0xfc), // TEXT
    rgb(0xe6, 0xf1, 0xf8), // TEXT_BODY
    rgb(0x7b, 0x8b, 0x98), // TEXT_DIM
    rgb(0x9f, 0xb3, 0xc8), // TEXT_MUTED
    rgb(0xc8, 0xd7, 0xe2), // TEXT_SOFT
    rgb(0xd4, 0xe2, 0xea), // TEXT_MENU
    rgb(0xe4, 0xec, 0xf2), // TEXT_CODE
    rgb(0x2d, 0xd4, 0xbf), // ACCENT
    rgb(0x10, 0x34, 0x37), // ACCENT_DIM
    rgb(0xff, 0xd1, 0x66), // FOCUS
    rgb(0x34, 0xd3, 0x99), // SUCCESS
    rgb(0xf5, 0x9e, 0x0b), // WARNING
    rgb(0xfb, 0xbf, 0x24), // WARNING_BRIGHT
    rgb(0xf8, 0x71, 0x71), // DANGER
    rgb(0xff, 0xb4, 0xb4), // DANGER_SOFT
    rgb(0x38, 0xbd, 0xf8), // INFO
    rgb(0xc0, 0x84, 0xfc), // PURPLE
    rgb(0x2e, 0x17, 0x1d), // CALC_CLEAR
    rgb(0x12, 0x26, 0x2c), // CALC_OP
    rgb(0x24, 0x32, 0x40), // CALC_KEY_BORDER
    rgb(0x16, 0x22, 0x2f), // MINES_HIDDEN
    rgb(0x1b, 0x26, 0x33), // MINES_REVEALED_BORDER
    rgb(0x4c, 0x14, 0x1d), // MINES_DETONATED
    rgb(0x5c, 0x2b, 0x35), // MINES_DETONATED_BORDER
    rgb(0x16, 0x26, 0x34), // MINES_FLAG
    rgb(0x28, 0x15, 0x1d), // MINES_TERMINAL_MINE
    rgb(0x14, 0x33, 0x2e), // MINES_TERMINAL_FLAG
    rgb(0x36, 0x26, 0x10), // MINES_FLAG_MODE
    rgb(0x10, 0x2c, 0x2e), // MINES_REVEAL_MODE
    rgb(0x1a, 0x2e, 0x22), // TTS_ACTIVE
    rgb(0x1e, 0x2b, 0x38), // CODE_GUTTER
    rgb(0x52, 0x64, 0x74), // CODE_LINE_NO
    rgb(0x4c, 0xc2, 0xff), // SYN_KEYWORD
    rgb(0xc9, 0xa0, 0xff), // SYN_TYPE
    rgb(0x8f, 0xd8, 0xa8), // SYN_STRING
    rgb(0xf0, 0xc6, 0x74), // SYN_NUMBER
    rgb(0x6d, 0x7d, 0x8c), // SYN_COMMENT
    rgb(0x9a, 0xa7, 0xb4), // SYN_PUNCT
    rgb(0xf0, 0xb8, 0x6a), // SYN_PREPROC
];

const fn index_of(token: u32) -> usize {
    (token & 0xFF) as usize
}

#[allow(clippy::too_many_arguments)]
const fn variant(
    bg: u32,
    deep: u32,
    surface: u32,
    raised: u32,
    accent: u32,
    accent_dim: u32,
    text: u32,
    dim: u32,
    border: u32,
) -> [u32; TOKEN_COUNT] {
    let mut out = DARK_BASE;
    out[index_of(BG)] = bg;
    out[index_of(BG_DEEP)] = deep;
    out[index_of(SURFACE)] = surface;
    out[index_of(SURFACE_RAISED)] = raised;
    out[index_of(SURFACE_ALT)] = raised;
    out[index_of(SURFACE_ACTIVE)] = raised;
    out[index_of(SURFACE_SELECTED)] = surface;
    out[index_of(SURFACE_CONTROL)] = raised;
    out[index_of(SURFACE_HOVER)] = raised;
    out[index_of(BAR)] = deep;
    out[index_of(TITLE_BG)] = deep;
    out[index_of(OVERLAY)] = rgb(0x00, 0x00, 0x00);
    out[index_of(GRID)] = deep;
    out[index_of(BORDER)] = border;
    out[index_of(BORDER_DARK)] = border;
    out[index_of(BORDER_SOFT)] = border;
    out[index_of(BORDER_STRONG)] = border;
    out[index_of(BORDER_CONTROL)] = border;
    out[index_of(TEXT)] = text;
    out[index_of(TEXT_BODY)] = text;
    out[index_of(TEXT_DIM)] = dim;
    out[index_of(TEXT_MUTED)] = dim;
    out[index_of(TEXT_SOFT)] = text;
    out[index_of(TEXT_MENU)] = text;
    out[index_of(TEXT_CODE)] = text;
    out[index_of(ACCENT)] = accent;
    out[index_of(ACCENT_DIM)] = accent_dim;
    out[index_of(CALC_OP)] = accent_dim;
    out[index_of(CALC_KEY_BORDER)] = border;
    out
}

pub static THEMES: [[u32; TOKEN_COUNT]; THEME_COUNT] = [
    DARK_BASE,
    variant(
        rgb(0x0c, 0x11, 0x18),
        rgb(0x07, 0x0b, 0x10),
        rgb(0x12, 0x1a, 0x24),
        rgb(0x1a, 0x24, 0x30),
        rgb(0x6c, 0xc4, 0xff),
        rgb(0x12, 0x32, 0x44),
        rgb(0xf2, 0xf7, 0xfb),
        rgb(0x83, 0x93, 0xa3),
        rgb(0x2a, 0x3a, 0x4a),
    ),
    variant(
        rgb(0x06, 0x12, 0x1b),
        rgb(0x03, 0x0b, 0x12),
        rgb(0x0a, 0x1a, 0x26),
        rgb(0x0e, 0x22, 0x30),
        rgb(0x38, 0xbd, 0xf8),
        rgb(0x0d, 0x34, 0x46),
        rgb(0xea, 0xf6, 0xff),
        rgb(0x7f, 0x9d, 0xb3),
        rgb(0x24, 0x40, 0x4f),
    ),
    variant(
        rgb(0x08, 0x14, 0x0f),
        rgb(0x05, 0x0b, 0x08),
        rgb(0x0b, 0x1a, 0x14),
        rgb(0x10, 0x25, 0x1b),
        rgb(0x4a, 0xde, 0x80),
        rgb(0x10, 0x3a, 0x24),
        rgb(0xea, 0xff, 0xf3),
        rgb(0x7f, 0x9c, 0x8a),
        rgb(0x24, 0x44, 0x33),
    ),
    variant(
        rgb(0x16, 0x0f, 0x0a),
        rgb(0x0d, 0x08, 0x05),
        rgb(0x1c, 0x13, 0x0d),
        rgb(0x27, 0x1b, 0x12),
        rgb(0xf5, 0x9e, 0x0b),
        rgb(0x3a, 0x26, 0x08),
        rgb(0xff, 0xf4, 0xe8),
        rgb(0xb3, 0x9a, 0x82),
        rgb(0x4a, 0x35, 0x24),
    ),
    variant(
        rgb(0x12, 0x0d, 0x1a),
        rgb(0x09, 0x06, 0x0f),
        rgb(0x19, 0x12, 0x23),
        rgb(0x23, 0x1a, 0x30),
        rgb(0xc0, 0x84, 0xfc),
        rgb(0x2e, 0x1b, 0x45),
        rgb(0xf7, 0xef, 0xff),
        rgb(0xa1, 0x8c, 0xaf),
        rgb(0x3c, 0x2a, 0x4d),
    ),
    variant(
        rgb(0x0b, 0x0b, 0x0d),
        rgb(0x07, 0x07, 0x08),
        rgb(0x11, 0x11, 0x14),
        rgb(0x18, 0x18, 0x1c),
        rgb(0xc8, 0xd7, 0xe2),
        rgb(0x2a, 0x30, 0x38),
        rgb(0xf5, 0xf5, 0xf5),
        rgb(0x9a, 0x9a, 0xa0),
        rgb(0x33, 0x34, 0x3a),
    ),
    variant(
        rgb(0x00, 0x2b, 0x36),
        rgb(0x00, 0x1f, 0x27),
        rgb(0x07, 0x36, 0x42),
        rgb(0x0b, 0x45, 0x52),
        rgb(0x2a, 0xa1, 0x98),
        rgb(0x0d, 0x4a, 0x4a),
        rgb(0xe6, 0xf1, 0xf1),
        rgb(0x93, 0xa1, 0xa1),
        rgb(0x2a, 0x4c, 0x57),
    ),
    variant(
        rgb(0x18, 0x0f, 0x14),
        rgb(0x0f, 0x08, 0x0c),
        rgb(0x21, 0x15, 0x1b),
        rgb(0x2c, 0x1c, 0x24),
        rgb(0xfb, 0x71, 0x85),
        rgb(0x43, 0x17, 0x25),
        rgb(0xff, 0xf0, 0xf3),
        rgb(0xb3, 0x8a, 0x95),
        rgb(0x4a, 0x2b, 0x36),
    ),
    variant(
        rgb(0x14, 0x10, 0x0d),
        rgb(0x0c, 0x09, 0x07),
        rgb(0x1b, 0x16, 0x11),
        rgb(0x25, 0x1e, 0x17),
        rgb(0xf0, 0xa8, 0x68),
        rgb(0x3a, 0x26, 0x14),
        rgb(0xff, 0xf5, 0xea),
        rgb(0xb0, 0x9a, 0x84),
        rgb(0x43, 0x34, 0x29),
    ),
];

static CURRENT: AtomicUsize = AtomicUsize::new(0);

pub fn theme_index() -> usize {
    CURRENT.load(Ordering::Relaxed).min(THEME_COUNT - 1)
}

pub fn set_theme(index: usize) -> bool {
    if index >= THEME_COUNT {
        return false;
    }
    CURRENT.store(index, Ordering::Relaxed);
    true
}

pub fn theme_name(index: usize) -> &'static str {
    THEME_NAMES[index.min(THEME_COUNT - 1)]
}

/// Resolve a theme sentinel for the active theme. Non-sentinel colors pass
/// through unchanged (test patterns, decoded images, explicit colors).
pub fn resolve(color: u32) -> u32 {
    if color & TOKEN_MASK == TOKEN_MARKER {
        THEMES[theme_index()][index_of(color)]
    } else {
        color
    }
}

/// Actual color for a token in a specific theme (used by the Theme preview).
pub fn color(theme: usize, token: u32) -> u32 {
    if token & TOKEN_MASK == TOKEN_MARKER {
        THEMES[theme.min(THEME_COUNT - 1)][index_of(token)]
    } else {
        token
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn themes_are_distinct_and_resolvable() {
        assert_eq!(THEME_COUNT, THEME_NAMES.len());
        set_theme(0);
        assert_eq!(resolve(ACCENT), THEMES[0][index_of(ACCENT)]);
        assert_eq!(resolve(rgb(1, 2, 3)), rgb(1, 2, 3));
        assert_ne!(THEMES[0], THEMES[1]);
        // Out-of-range selection is rejected and keeps the current theme.
        assert!(set_theme(1));
        assert_eq!(theme_index(), 1);
        assert!(!set_theme(THEME_COUNT));
        assert_eq!(theme_index(), 1);
        set_theme(0);
    }

    #[test]
    fn theme_values_are_resolved_colors() {
        for theme in THEMES.iter() {
            for &color in theme.iter() {
                assert_ne!(color & TOKEN_MASK, TOKEN_MARKER);
            }
        }
    }

    #[test]
    fn preview_colors_ignore_active_theme() {
        let a = color(3, ACCENT);
        set_theme(0);
        assert_eq!(a, THEMES[3][index_of(ACCENT)]);
        set_theme(0);
    }
}
