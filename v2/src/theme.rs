//! Semantic color tokens for the v2 appliance (`theme.rs`).
//!
//! All widgets and apps should reference these tokens instead of literal
//! `rgb(0x..)` values. Keep hue names out of app code: pick the role
//! (`SURFACE`, `TEXT_DIM`, `DANGER`, ...) so a palette change stays local.

use crate::fb::rgb;

// --- Base surfaces ---------------------------------------------------------
pub const BG: u32 = rgb(0x0b, 0x10, 0x16);
pub const BG_DEEP: u32 = rgb(0x09, 0x0d, 0x12);
pub const SURFACE: u32 = rgb(0x10, 0x17, 0x20);
pub const SURFACE_RAISED: u32 = rgb(0x14, 0x1d, 0x26);
pub const SURFACE_ALT: u32 = rgb(0x14, 0x1f, 0x2a);
pub const SURFACE_ACTIVE: u32 = rgb(0x13, 0x22, 0x2b);
pub const SURFACE_SELECTED: u32 = rgb(0x13, 0x22, 0x2e);
pub const SURFACE_CONTROL: u32 = rgb(0x18, 0x26, 0x33);
pub const SURFACE_HOVER: u32 = rgb(0x1b, 0x28, 0x36);
pub const BAR: u32 = rgb(0x0e, 0x16, 0x20);
pub const TITLE_BG: u32 = rgb(0x0c, 0x11, 0x17);
pub const OVERLAY: u32 = rgb(0x08, 0x10, 0x14);
pub const GRID: u32 = rgb(0x0f, 0x16, 0x1f);

pub const WHITE: u32 = rgb(0xff, 0xff, 0xff);
pub const BLACK: u32 = rgb(0x00, 0x00, 0x00);

// --- Borders and rules -----------------------------------------------------
pub const BORDER: u32 = rgb(0x22, 0x30, 0x3e);
pub const BORDER_DARK: u32 = rgb(0x18, 0x22, 0x2d);
pub const BORDER_SOFT: u32 = rgb(0x1f, 0x2c, 0x38);
pub const BORDER_STRONG: u32 = rgb(0x2e, 0x3f, 0x50);
pub const BORDER_CONTROL: u32 = rgb(0x26, 0x38, 0x4a);

// --- Text ------------------------------------------------------------------
pub const TEXT: u32 = rgb(0xf0, 0xf6, 0xfc);
pub const TEXT_BODY: u32 = rgb(0xe6, 0xf1, 0xf8);
pub const TEXT_DIM: u32 = rgb(0x7b, 0x8b, 0x98);
pub const TEXT_MUTED: u32 = rgb(0x9f, 0xb3, 0xc8);
pub const TEXT_SOFT: u32 = rgb(0xc8, 0xd7, 0xe2);
pub const TEXT_MENU: u32 = rgb(0xd4, 0xe2, 0xea);
pub const TEXT_CODE: u32 = rgb(0xe4, 0xec, 0xf2);

// --- Accent and status -----------------------------------------------------
pub const ACCENT: u32 = rgb(0x2d, 0xd4, 0xbf);
pub const ACCENT_DIM: u32 = rgb(0x10, 0x34, 0x37);
pub const SUCCESS: u32 = rgb(0x34, 0xd3, 0x99);
pub const WARNING: u32 = rgb(0xf5, 0x9e, 0x0b);
pub const WARNING_BRIGHT: u32 = rgb(0xfb, 0xbf, 0x24);
pub const DANGER: u32 = rgb(0xf8, 0x71, 0x71);
pub const DANGER_SOFT: u32 = rgb(0xff, 0xb4, 0xb4);
pub const INFO: u32 = rgb(0x38, 0xbd, 0xf8);
pub const PURPLE: u32 = rgb(0xc0, 0x84, 0xfc);

// --- Calculator ------------------------------------------------------------
pub const CALC_CLEAR: u32 = rgb(0x2e, 0x17, 0x1d);
pub const CALC_OP: u32 = rgb(0x12, 0x26, 0x2c);
pub const CALC_EQ: u32 = ACCENT_DIM;
pub const CALC_KEY_BORDER: u32 = rgb(0x24, 0x32, 0x40);

// --- Minesweeper -----------------------------------------------------------
pub const MINES_HIDDEN: u32 = rgb(0x16, 0x22, 0x2f);
pub const MINES_HIDDEN_BORDER: u32 = BORDER_CONTROL;
pub const MINES_REVEALED: u32 = GRID;
pub const MINES_REVEALED_BORDER: u32 = rgb(0x1b, 0x26, 0x33);
pub const MINES_DETONATED: u32 = rgb(0x4c, 0x14, 0x1d);
pub const MINES_DETONATED_BORDER: u32 = rgb(0x5c, 0x2b, 0x35);
pub const MINES_FLAG: u32 = rgb(0x16, 0x26, 0x34);
pub const MINES_TERMINAL_MINE: u32 = rgb(0x28, 0x15, 0x1d);
pub const MINES_TERMINAL_FLAG: u32 = rgb(0x14, 0x33, 0x2e);
pub const MINES_FLAG_MODE: u32 = rgb(0x36, 0x26, 0x10);
pub const MINES_REVEAL_MODE: u32 = rgb(0x10, 0x2c, 0x2e);

// --- Preview / TTS ---------------------------------------------------------
pub const TTS_ACTIVE: u32 = rgb(0x1a, 0x2e, 0x22);
pub const CODE_GUTTER: u32 = rgb(0x1e, 0x2b, 0x38);
pub const CODE_LINE_NO: u32 = rgb(0x52, 0x64, 0x74);

// --- Syntax highlighting ---------------------------------------------------
pub const SYN_PLAIN: u32 = TEXT_CODE;
pub const SYN_KEYWORD: u32 = rgb(0x4c, 0xc2, 0xff);
pub const SYN_TYPE: u32 = rgb(0xc9, 0xa0, 0xff);
pub const SYN_STRING: u32 = rgb(0x8f, 0xd8, 0xa8);
pub const SYN_NUMBER: u32 = rgb(0xf0, 0xc6, 0x74);
pub const SYN_COMMENT: u32 = rgb(0x6d, 0x7d, 0x8c);
pub const SYN_PUNCT: u32 = rgb(0x9a, 0xa7, 0xb4);
pub const SYN_PREPROC: u32 = rgb(0xf0, 0xb8, 0x6a);
