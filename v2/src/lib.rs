#![cfg_attr(target_arch = "wasm32", no_std)]

pub mod calc;
pub mod checkers;
pub mod checkers_sm;
pub mod chess;
pub mod chess_sm;
pub mod code;
pub mod fb;
pub mod files;
pub mod fmt;
pub mod font;
#[cfg(feature = "folded-glyphs")]
pub mod folded;
pub mod glyph;
pub mod icon;
pub mod input;
pub mod layout;
pub mod mine_sm;
pub mod mines;
pub mod smooth;
pub mod system;
pub mod theme;
pub mod ui;
pub mod wm;

use core::cell::UnsafeCell;
use fb::{Framebuffer, DEFAULT_H, DEFAULT_W, MAX_DIM, MAX_PIXELS};
use files::{MAX_CONTEXT_BYTES, MAX_TEXT_BYTES};
use input::{Key, Mods};
use wm::{AppSlot, WmState};

#[cfg(not(target_arch = "wasm32"))]
const FB_PIXELS: usize = MAX_PIXELS;

/// Single-threaded Wasm guest state container.
///
/// On `wasm32`, the framebuffer, the RGBA8888 network mount staging buffer,
/// the 16 KiB text/code staging buffer, and the 4 KiB context staging buffer
/// reside above `__heap_base` in linear memory, grown on demand inside
/// `app_init(width, height)` for the session's locked resolution
/// (from `360x800` up to `1920x1080`).
struct GuestMemory {
    #[cfg(not(target_arch = "wasm32"))]
    pixels: UnsafeCell<core::mem::MaybeUninit<[u32; FB_PIXELS]>>,
    #[cfg(not(target_arch = "wasm32"))]
    staging: UnsafeCell<core::mem::MaybeUninit<[u32; FB_PIXELS]>>,
    #[cfg(not(target_arch = "wasm32"))]
    text_staging: UnsafeCell<[u8; MAX_TEXT_BYTES]>,
    #[cfg(not(target_arch = "wasm32"))]
    context_staging: UnsafeCell<[u8; MAX_CONTEXT_BYTES]>,
    name_scratch: UnsafeCell<[u8; 64]>,
    wm: UnsafeCell<WmState>,
}

unsafe impl Sync for GuestMemory {}

/// Serialises tests that touch the global guest, theme, or render-mode state.
/// `cargo test` runs test functions on threads, and this appliance is
/// single-threaded by design.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

static GUEST: GuestMemory = GuestMemory {
    #[cfg(not(target_arch = "wasm32"))]
    pixels: UnsafeCell::new(core::mem::MaybeUninit::uninit()),
    #[cfg(not(target_arch = "wasm32"))]
    staging: UnsafeCell::new(core::mem::MaybeUninit::uninit()),
    #[cfg(not(target_arch = "wasm32"))]
    text_staging: UnsafeCell::new([0; MAX_TEXT_BYTES]),
    #[cfg(not(target_arch = "wasm32"))]
    context_staging: UnsafeCell::new([0; MAX_CONTEXT_BYTES]),
    name_scratch: UnsafeCell::new([0; 64]),
    wm: UnsafeCell::new(WmState::new()),
};

#[cfg(target_arch = "wasm32")]
extern "C" {
    static __heap_base: u8;
}

#[cfg(target_arch = "wasm32")]
#[inline]
unsafe fn session_pixels() -> usize {
    let wm = &*GUEST.wm.get();
    let w = wm.width.clamp(1, MAX_DIM) as usize;
    let h = wm.height.clamp(1, MAX_DIM) as usize;
    (w * h).min(MAX_PIXELS)
}

#[cfg(target_arch = "wasm32")]
#[inline]
unsafe fn ensure_wasm_memory(px_count: usize) {
    let base = (&raw const __heap_base as usize + 15) & !15;
    let needed_bytes = base + px_count * 4 * 2 + MAX_TEXT_BYTES + MAX_CONTEXT_BYTES + 65536;
    let needed_pages = (needed_bytes + 65535) / 65536;
    let cur = core::arch::wasm32::memory_size(0);
    if cur < needed_pages {
        let _ = core::arch::wasm32::memory_grow(0, needed_pages - cur);
    }
}

#[cfg(target_arch = "wasm32")]
#[inline]
unsafe fn fb_base_ptr() -> *mut u32 {
    ensure_wasm_memory(session_pixels());
    let base = (&raw const __heap_base as usize + 15) & !15;
    base as *mut u32
}

#[cfg(target_arch = "wasm32")]
#[inline]
unsafe fn staging_base_ptr() -> *mut u32 {
    let n = session_pixels();
    fb_base_ptr().add(n)
}

#[cfg(target_arch = "wasm32")]
#[inline]
unsafe fn text_staging_base_ptr() -> *mut u8 {
    let n = session_pixels();
    staging_base_ptr().add(n) as *mut u8
}

#[cfg(target_arch = "wasm32")]
#[inline]
unsafe fn context_staging_base_ptr() -> *mut u8 {
    text_staging_base_ptr().add(MAX_TEXT_BYTES)
}

#[cfg(not(target_arch = "wasm32"))]
#[inline]
unsafe fn fb_base_ptr() -> *mut u32 {
    (*GUEST.pixels.get()).as_mut_ptr() as *mut u32
}

#[cfg(not(target_arch = "wasm32"))]
#[inline]
unsafe fn staging_base_ptr() -> *mut u32 {
    (*GUEST.staging.get()).as_mut_ptr() as *mut u32
}

#[cfg(not(target_arch = "wasm32"))]
#[inline]
unsafe fn text_staging_base_ptr() -> *mut u8 {
    (*GUEST.text_staging.get()).as_mut_ptr()
}

#[cfg(not(target_arch = "wasm32"))]
#[inline]
unsafe fn context_staging_base_ptr() -> *mut u8 {
    (*GUEST.context_staging.get()).as_mut_ptr()
}

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

fn with_state<R>(f: impl FnOnce(&mut WmState, &mut Framebuffer, &[u32], &[u8], &[u8]) -> R) -> R {
    unsafe {
        let wm = &mut *GUEST.wm.get();
        let w = wm.width.clamp(1, MAX_DIM);
        let h = wm.height.clamp(1, MAX_DIM);
        let n_px = ((w as usize) * (h as usize)).min(MAX_PIXELS);
        let px_ptr = fb_base_ptr();
        let st_ptr = staging_base_ptr();
        let txt_ptr = text_staging_base_ptr();
        let ctx_ptr = context_staging_base_ptr();
        let px = core::slice::from_raw_parts_mut(px_ptr, n_px);
        let st = core::slice::from_raw_parts(st_ptr, n_px);
        let txt = core::slice::from_raw_parts(txt_ptr, MAX_TEXT_BYTES);
        let ctx = core::slice::from_raw_parts(ctx_ptr, MAX_CONTEXT_BYTES);
        let mut fb = Framebuffer::new(px, w, h);
        f(wm, &mut fb, st, txt, ctx)
    }
}

#[no_mangle]
pub extern "C" fn app_init(width: i32, height: i32) -> i32 {
    let mut w = if width <= 0 || width > MAX_DIM {
        DEFAULT_W
    } else {
        width
    };
    let mut h = if height <= 0 || height > MAX_DIM {
        DEFAULT_H
    } else {
        height
    };
    if (w as usize) * (h as usize) > MAX_PIXELS {
        w = DEFAULT_W;
        h = DEFAULT_H;
    }
    unsafe {
        let wm = &mut *GUEST.wm.get();
        wm.width = w;
        wm.height = h;
    }
    with_state(|wm, fb, st, txt, ctx| {
        wm.init(w, h, wm.session_id);
        wm.render(fb, st, txt, ctx);
        1
    })
}

#[no_mangle]
pub extern "C" fn app_set_session(session_id: u32) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.session_id = if session_id > 0 { session_id } else { 1 };
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

#[no_mangle]
pub extern "C" fn app_render() {
    with_state(|wm, fb, st, txt, ctx| {
        wm.render(fb, st, txt, ctx);
    });
}

#[no_mangle]
pub extern "C" fn app_click(x: i32, y: i32) -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        let rc = wm.click(x, y);
        if rc.is_changed() {
            wm.render(fb, st, txt, ctx);
        }
        rc.to_i32()
    })
}

#[no_mangle]
pub extern "C" fn app_key(key: i32) -> i32 {
    app_key_mods(key, 0)
}

/// Preferred entry point: `mods` is the bitmask from [`Mods`].
#[no_mangle]
pub extern "C" fn app_key_mods(key: i32, mods: i32) -> i32 {
    let Some(key) = Key::from_code(key) else {
        return 0;
    };
    let mods = Mods::from_bits(mods);
    with_state(|wm, fb, st, txt, ctx| {
        let rc = wm.key(key, mods);
        if rc.is_changed() {
            wm.render(fb, st, txt, ctx);
        }
        rc.to_i32()
    })
}

#[no_mangle]
pub extern "C" fn app_pointer_move(x: i32, y: i32) -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        let rc = wm.pointer_move(x, y);
        if rc.is_changed() {
            wm.render(fb, st, txt, ctx);
        }
        rc.to_i32()
    })
}

#[no_mangle]
pub extern "C" fn app_pointer_leave() -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        let rc = wm.pointer_leave();
        if rc.is_changed() {
            wm.render(fb, st, txt, ctx);
        }
        rc.to_i32()
    })
}

/// Encoded focus target for tests and the host status line.
#[no_mangle]
pub extern "C" fn app_focus() -> i32 {
    unsafe { (*GUEST.wm.get()).focus_code() }
}

/// Keyboard focus inside the Files pane (`FilesFocus` discriminant).
#[no_mangle]
pub extern "C" fn app_files_focus() -> i32 {
    unsafe { (*GUEST.wm.get()).files.focus as i32 }
}

#[no_mangle]
pub extern "C" fn app_files_context_mode() -> i32 {
    unsafe { i32::from((*GUEST.wm.get()).files.context_mode) }
}

#[no_mangle]
pub extern "C" fn app_preview_scale() -> i32 {
    unsafe { (*GUEST.wm.get()).system.preview_idx as i32 + 1 }
}

#[no_mangle]
pub extern "C" fn app_theme() -> i32 {
    crate::theme::theme_index() as i32
}

#[no_mangle]
pub extern "C" fn app_render_mode() -> i32 {
    crate::font::render_mode() as i32
}

/// Deprecated alias kept for older hosts/snapshots: `0` native, `1` smooth.
#[no_mangle]
pub extern "C" fn app_font_family() -> i32 {
    app_render_mode()
}

#[no_mangle]
pub extern "C" fn app_font() -> i32 {
    crate::glyph::font() as i32
}

#[no_mangle]
pub extern "C" fn app_set_time(hour: i32, minute: i32, second: i32, _msec: i32) {
    unsafe {
        let wm = &mut *GUEST.wm.get();
        wm.hour = (hour.clamp(0, 23)) as u8;
        wm.minute = (minute.clamp(0, 59)) as u8;
        wm.second = (second.clamp(0, 59)) as u8;
    }
}

#[no_mangle]
pub extern "C" fn app_set_date(year: i32, month: i32, day: i32, _weekday: i32) {
    unsafe {
        let wm = &mut *GUEST.wm.get();
        wm.year = (year.clamp(1970, 9999)) as u16;
        wm.month = (month.clamp(1, 12)) as u8;
        wm.day = (day.clamp(1, 31)) as u8;
    }
}

#[no_mangle]
pub extern "C" fn app_set_utc(
    year: i32,
    month: i32,
    day: i32,
    hour: i32,
    minute: i32,
    second: i32,
    tz_offset_min: i32,
) {
    unsafe {
        let s = &mut (*GUEST.wm.get()).system;
        s.utc_year = (year.clamp(1970, 9999)) as u16;
        s.utc_month = (month.clamp(1, 12)) as u8;
        s.utc_day = (day.clamp(1, 31)) as u8;
        s.utc_hour = (hour.clamp(0, 23)) as u8;
        s.utc_min = (minute.clamp(0, 59)) as u8;
        s.utc_sec = (second.clamp(0, 59)) as u8;
        s.tz_offset_min = (tz_offset_min.clamp(-840, 840)) as i16;
    }
}

#[no_mangle]
pub extern "C" fn app_tick() -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        if !wm.ready || wm.test_pattern {
            return 0;
        }
        wm.render(fb, st, txt, ctx);
        1
    })
}

#[no_mangle]
pub extern "C" fn app_record_snapshot() -> u32 {
    with_state(|wm, fb, st, txt, ctx| {
        wm.snapshot_seq = wm.snapshot_seq.wrapping_add(1);
        wm.system.snapshots_taken = wm.snapshot_seq;
        wm.render(fb, st, txt, ctx);
        fb.fnv1a_32()
    })
}

// --- Network Mount (`~/`) & RGBA8888 / Source Code Upstream Bridge ---

#[no_mangle]
pub extern "C" fn app_mount_name_buf() -> *mut u8 {
    unsafe { (*GUEST.name_scratch.get()).as_mut_ptr() }
}

#[no_mangle]
pub extern "C" fn app_mount_clear() {
    unsafe {
        (*GUEST.wm.get()).files.clear_mounts();
    }
}

#[no_mangle]
pub extern "C" fn app_mount_add(name_len: usize, byte_size: u32) -> i32 {
    unsafe {
        let len = name_len.min(64);
        let ptr = (*GUEST.name_scratch.get()).as_ptr();
        let name = core::slice::from_raw_parts(ptr, len);
        (*GUEST.wm.get()).files.add_mount(name, byte_size)
    }
}

#[no_mangle]
pub extern "C" fn app_mount_update_size(idx: i32, byte_size: u32) -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        if idx < 0 || idx as usize >= wm.files.count {
            return -1;
        }
        wm.files.entries[idx as usize].byte_size = byte_size;
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
        0
    })
}

/// Host-side staging failure for a mount. The Files preview renders an error
/// card instead of waiting forever on "Loading preview...".
#[no_mangle]
pub extern "C" fn app_mount_commit_error(idx: i32, code: i32) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.files.complete_error(idx, code);
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

/// `KEY=VALUE\n` capture metadata for `web://` (512 bytes).
#[no_mangle]
pub extern "C" fn app_web_meta_ptr() -> *mut u8 {
    unsafe { (*GUEST.wm.get()).files.web_meta.as_mut_ptr() }
}

#[no_mangle]
pub extern "C" fn app_web_commit_meta(byte_len: usize) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.files.complete_web_meta(byte_len);
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

/// Symbolic `web/...` or `gh/...` target for the host to open externally.
#[no_mangle]
pub extern "C" fn app_open_target_ptr() -> *mut u8 {
    unsafe { (*GUEST.wm.get()).files.open_target.as_mut_ptr() }
}

#[no_mangle]
pub extern "C" fn app_open_target_len() -> usize {
    unsafe { (*GUEST.wm.get()).files.open_target_len }
}

/// Returns 1 once when the guest requested a web re-capture, then clears.
#[no_mangle]
pub extern "C" fn app_web_take_refresh() -> i32 {
    unsafe {
        let files = &mut (*GUEST.wm.get()).files;
        let requested = files.web_refresh_requested;
        files.web_refresh_requested = false;
        i32::from(requested)
    }
}

#[no_mangle]
pub extern "C" fn app_mount_staging_ptr() -> *mut u8 {
    unsafe { staging_base_ptr() as *mut u8 }
}

#[no_mangle]
pub extern "C" fn app_mount_text_ptr() -> *mut u8 {
    unsafe { text_staging_base_ptr() }
}

#[no_mangle]
pub extern "C" fn app_mount_context_ptr() -> *mut u8 {
    unsafe { context_staging_base_ptr() }
}

#[no_mangle]
pub extern "C" fn app_mount_pending_idx() -> i32 {
    unsafe { (*GUEST.wm.get()).files.pending_fetch_idx }
}

/// Called by `host.js` once it has decoded a PNG/JPEG into `RGBA8888` and written
/// `w * h * 4` bytes into `app_mount_staging_ptr()`.
#[no_mangle]
pub extern "C" fn app_mount_commit_rgba(idx: i32, w: i32, h: i32) {
    with_state(|wm, fb, st, txt, ctx| {
        let clamped_w = w.clamp(0, wm.width);
        let clamped_h = h.clamp(0, wm.height);
        wm.files.complete_stage(idx, clamped_w, clamped_h);
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

/// Called by `host.js` once it has fetched a source code / text file and written
/// `byte_len` UTF-8 bytes into `app_mount_text_ptr()`.
#[no_mangle]
pub extern "C" fn app_mount_commit_text(idx: i32, byte_len: usize) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.files.complete_stage_text(idx, byte_len);
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

/// Called by `host.js` once it has decoded an enriched `.context.xml` file and written
/// `byte_len` UTF-8 bytes (`KEY=VALUE\n` format) into `app_mount_context_ptr()`.
#[no_mangle]
pub extern "C" fn app_mount_commit_context(idx: i32, byte_len: usize) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.files.complete_stage_context(idx, byte_len);
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

#[no_mangle]
pub extern "C" fn app_fb_hash() -> u32 {
    with_state(|wm, fb, _, _, _| if wm.ready { fb.fnv1a_32() } else { 0 })
}

#[no_mangle]
pub extern "C" fn app_client_fb_hash() -> u32 {
    with_state(|wm, fb, _, _, _| {
        if wm.ready {
            fb.fnv1a_32_sub(wm::TITLE_H, fb.h - wm::TITLE_H)
        } else {
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn app_snapshot_count() -> u32 {
    unsafe { (*GUEST.wm.get()).snapshot_seq }
}

#[no_mangle]
pub extern "C" fn app_dock_visible() -> i32 {
    unsafe { i32::from((*GUEST.wm.get()).drawer_open()) }
}

#[no_mangle]
pub extern "C" fn app_active_page() -> i32 {
    unsafe {
        let wm = &*GUEST.wm.get();
        match wm.active_app {
            AppSlot::Desktop => -1,
            AppSlot::System => wm.system.section as i32,
            AppSlot::Calc => 0,
            AppSlot::Files => wm.files.active_file_idx().map(|i| i as i32).unwrap_or(-1),
            AppSlot::Mines => wm.mines.board.seed as i32,
            AppSlot::Checkers => wm.checkers.state.move_count as i32,
            AppSlot::Chess => wm.chess.state.fullmove_number as i32,
        }
    }
}

#[no_mangle]
pub extern "C" fn app_framebuffer() -> *const u8 {
    unsafe { fb_base_ptr() as *const u8 }
}

#[no_mangle]
pub extern "C" fn app_width() -> i32 {
    unsafe { (*GUEST.wm.get()).width }
}

#[no_mangle]
pub extern "C" fn app_height() -> i32 {
    unsafe { (*GUEST.wm.get()).height }
}

#[no_mangle]
pub extern "C" fn app_selected() -> i32 {
    unsafe { (*GUEST.wm.get()).active_app as i32 }
}

#[no_mangle]
pub extern "C" fn app_count() -> i32 {
    AppSlot::APP_COUNT as i32
}

#[no_mangle]
pub extern "C" fn app_semantic_hit(x: i32, y: i32) -> u32 {
    unsafe {
        let wm = &*GUEST.wm.get();
        if !wm.ready {
            return 0;
        }

        let layout = wm::WmLayout::new(wm.width);

        // 1. If Drawer is open and clicked on home menu items
        if let wm::Focus::Menu { menu, .. } = wm.focus {
            if menu == wm::MenuId::Home {
                let items = wm.menu_items(menu);
                let r = layout.dropdown_rect(menu, items.len(), wm.width);
                if r.contains(x, y) {
                    for idx in 0..items.len() {
                        let ir = layout.menu_item_rect(menu, idx, items.len(), wm.width);
                        if ir.contains(x, y) {
                            return (1 << 24) | (idx as u32);
                        }
                    }
                }
            }
        }

        // 2. Title Bar: Home button or header rect
        if (0..wm::TITLE_H).contains(&y) {
            if layout.header_rect(wm::MenuId::Home).contains(x, y) {
                return (1 << 24) | 0; // Home toggle
            }
            if layout.header_rect(wm::MenuId::App).contains(x, y) {
                return (1 << 24) | 100; // App menu
            }
            return (1 << 24) | 0xFF; // Title bar generic
        }

        // 3. Client surface hit
        let cx = x;
        let cy = y - wm::TITLE_H;
        let client_w = wm.width;
        let client_h = (wm.height - wm::TITLE_H).max(1);

        match wm.active_app {
            AppSlot::Chess => {
                let g = chess::chess_geom(client_w, client_h);
                if layout::Rect::new(g.board_x, g.board_y, g.grid_px, g.grid_px).contains(cx, cy) {
                    let v_col = (cx - g.board_x) / g.cell;
                    let v_row = (cy - g.board_y) / g.cell;
                    if (0..8).contains(&v_col) && (0..8).contains(&v_row) {
                        let f = if wm.chess.flipped { 7 - v_col } else { v_col };
                        let r = if wm.chess.flipped { v_row } else { 7 - v_row };
                        if let Some(sq) = chess_sm::coord_to_square(f as i8, r as i8) {
                            return (2 << 24) | (sq as u32);
                        }
                    }
                }
                if g.btn_new.contains(cx, cy) {
                    return (5 << 24) | 0; // New Game
                }
                if g.btn_mode.contains(cx, cy) {
                    return (5 << 24) | 1; // Mode
                }
                if g.btn_diff.contains(cx, cy) {
                    return (5 << 24) | 2; // Diff
                }
                if g.btn_flip.contains(cx, cy) {
                    return (5 << 24) | 3; // Flip
                }
            }
            AppSlot::Checkers => {
                let g = checkers::checkers_geom(client_w, client_h);
                if layout::Rect::new(g.board_x, g.board_y, g.grid_px, g.grid_px).contains(cx, cy) {
                    let v_col = (cx - g.board_x) / g.cell;
                    let v_row = (cy - g.board_y) / g.cell;
                    if (0..8).contains(&v_col) && (0..8).contains(&v_row) {
                        let row = if wm.checkers.flipped { 7 - v_row } else { v_row };
                        let col = if wm.checkers.flipped { 7 - v_col } else { v_col };
                        if let Some(sq) = checkers_sm::coord_to_index(row, col) {
                            return (3 << 24) | (sq as u32);
                        }
                    }
                }
                if g.btn_new.contains(cx, cy) {
                    return (5 << 24) | 0; // New Game
                }
                if g.btn_mode.contains(cx, cy) {
                    return (5 << 24) | 1; // Mode
                }
                if g.btn_diff.contains(cx, cy) {
                    return (5 << 24) | 2; // Diff
                }
                if g.btn_flip.contains(cx, cy) {
                    return (5 << 24) | 3; // Flip
                }
            }
            AppSlot::Mines => {
                let g = mines::mines_geom(client_w, client_h);
                if layout::Rect::new(g.board_x, g.board_y, g.grid_px, g.grid_px).contains(cx, cy) {
                    let rel_x = cx - g.board_x;
                    let rel_y = cy - g.board_y;
                    let stride = g.cell + g.gap;
                    let col = rel_x / stride;
                    let row = rel_y / stride;
                    if (0..8).contains(&col)
                        && (0..8).contains(&row)
                        && (rel_x % stride) < g.cell
                        && (rel_y % stride) < g.cell
                    {
                        let cell = (row * 8 + col) as u32;
                        return (4 << 24) | cell;
                    }
                }
                if g.btn_mode.contains(cx, cy) {
                    return (5 << 24) | 1; // Mode
                }
                if g.btn_seed.contains(cx, cy) {
                    return (5 << 24) | 0; // Seed / New Game
                }
                if g.btn_rule.contains(cx, cy) {
                    return (5 << 24) | 4; // Rule
                }
                if g.btn_mp.contains(cx, cy) {
                    return (5 << 24) | 5; // MP
                }
            }
            _ => {}
        }

        0
    }
}

#[no_mangle]
pub extern "C" fn app_semantic_coord(kind: u32, id: u32) -> u32 {
    unsafe {
        let wm = &*GUEST.wm.get();
        let client_w = if wm.width > 0 { wm.width } else { 1280 };
        let client_h = if wm.height > 0 { (wm.height - wm::TITLE_H).max(1) } else { 720 - wm::TITLE_H };

        match kind {
            1 => {
                // TitleBar / Home Menu
                let layout = wm::WmLayout::new(client_w);
                if id == 0 {
                    // Home button
                    let r = layout.header_rect(wm::MenuId::Home);
                    let cx = (r.x + r.w / 2) as u32;
                    let cy = (r.y + r.h / 2) as u32;
                    return (cx << 16) | (cy & 0xFFFF);
                }
                // Drawer menu item
                let items_len = wm::AppSlot::APP_COUNT + 1; // apps + test pattern
                if (id as usize) < items_len {
                    let ir = layout.menu_item_rect(wm::MenuId::Home, id as usize, items_len, client_w);
                    let cx = (ir.x + ir.w / 2) as u32;
                    let cy = (ir.y + ir.h / 2) as u32;
                    return (cx << 16) | (cy & 0xFFFF);
                }
            }
            2 => {
                // Chess square (0..63)
                if id < 64 {
                    let g = chess::chess_geom(client_w, client_h);
                    let sq = id as u8;
                    let f = chess_sm::square_file(sq) as i32;
                    let r = chess_sm::square_rank(sq) as i32;
                    let v_col = if wm.chess.flipped { 7 - f } else { f };
                    let v_row = if wm.chess.flipped { r } else { 7 - r };
                    let cx = (g.board_x + v_col * g.cell + g.cell / 2) as u32;
                    let cy = (wm::TITLE_H + g.board_y + v_row * g.cell + g.cell / 2) as u32;
                    return (cx << 16) | (cy & 0xFFFF);
                }
            }
            3 => {
                // Checkers square (0..31)
                if id < 32 {
                    let g = checkers::checkers_geom(client_w, client_h);
                    let (row, col) = checkers_sm::index_to_coord(id as u8);
                    let v_row = if wm.checkers.flipped { 7 - row } else { row };
                    let v_col = if wm.checkers.flipped { 7 - col } else { col };
                    let cx = (g.board_x + v_col * g.cell + g.cell / 2) as u32;
                    let cy = (wm::TITLE_H + g.board_y + v_row * g.cell + g.cell / 2) as u32;
                    return (cx << 16) | (cy & 0xFFFF);
                }
            }
            4 => {
                // Mines cell (0..63)
                if id < 64 {
                    let g = mines::mines_geom(client_w, client_h);
                    let row = (id / 8) as i32;
                    let col = (id % 8) as i32;
                    let stride = g.cell + g.gap;
                    let cx = (g.board_x + col * stride + g.cell / 2) as u32;
                    let cy = (wm::TITLE_H + g.board_y + row * stride + g.cell / 2) as u32;
                    return (cx << 16) | (cy & 0xFFFF);
                }
            }
            5 => {
                // Buttons: 0=NewGame/Seed, 1=Mode, 2=Diff, 3=Flip, 4=Rule, 5=Mp
                match wm.active_app {
                    AppSlot::Chess => {
                        let g = chess::chess_geom(client_w, client_h);
                        let r = match id {
                            0 => g.btn_new,
                            1 => g.btn_mode,
                            2 => g.btn_diff,
                            3 => g.btn_flip,
                            _ => g.btn_new,
                        };
                        let cx = (r.x + r.w / 2) as u32;
                        let cy = (wm::TITLE_H + r.y + r.h / 2) as u32;
                        return (cx << 16) | (cy & 0xFFFF);
                    }
                    AppSlot::Checkers => {
                        let g = checkers::checkers_geom(client_w, client_h);
                        let r = match id {
                            0 => g.btn_new,
                            1 => g.btn_mode,
                            2 => g.btn_diff,
                            3 => g.btn_flip,
                            _ => g.btn_new,
                        };
                        let cx = (r.x + r.w / 2) as u32;
                        let cy = (wm::TITLE_H + r.y + r.h / 2) as u32;
                        return (cx << 16) | (cy & 0xFFFF);
                    }
                    AppSlot::Mines => {
                        let g = mines::mines_geom(client_w, client_h);
                        let r = match id {
                            0 => g.btn_seed,
                            1 => g.btn_mode,
                            4 => g.btn_rule,
                            5 => g.btn_mp,
                            _ => g.btn_seed,
                        };
                        let cx = (r.x + r.w / 2) as u32;
                        let cy = (wm::TITLE_H + r.y + r.h / 2) as u32;
                        return (cx << 16) | (cy & 0xFFFF);
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        0
    }
}

#[no_mangle]
pub extern "C" fn app_set_test_pattern(enabled: i32) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.test_pattern = enabled != 0;
        wm.render(fb, st, txt, ctx);
    });
}

#[no_mangle]
pub extern "C" fn app_tts_active() -> i32 {
    unsafe { i32::from((*GUEST.wm.get()).files.tts_active) }
}

#[no_mangle]
pub extern "C" fn app_tts_set_active(active: i32) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.files.tts_active = active != 0;
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

// --- 2-Player Turn-Based Minesweeper Host Mailbox ABI ---

#[no_mangle]
pub extern "C" fn app_mines_set_mp(
    enabled: i32,
    match_id: u32,
    seat: i32,
    seed: u32,
    max_strikes: i32,
) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.mines.configure_mp(
            enabled != 0,
            match_id,
            seat.clamp(0, 2) as u8,
            seed as u64,
            max_strikes.clamp(1, 3) as u8,
        );
        wm.active_app = AppSlot::Mines;
        wm.focus = wm::Focus::Client;
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

#[no_mangle]
pub extern "C" fn app_mines_tx_pending() -> i32 {
    unsafe { i32::from((*GUEST.wm.get()).mines.tx_pending) }
}

#[no_mangle]
pub extern "C" fn app_mines_take_tx() -> u32 {
    unsafe {
        let m = &mut (*GUEST.wm.get()).mines;
        m.tx_pending = false;
        (m.tx_action as u32)
            | ((m.tx_cell as u32) << 8)
            | ((m.local_seat as u32) << 16)
            | ((m.board.rules.max_strikes as u32) << 24)
    }
}

#[no_mangle]
pub extern "C" fn app_mines_seed() -> u32 {
    unsafe { (*GUEST.wm.get()).mines.board.seed as u32 }
}

#[no_mangle]
pub extern "C" fn app_mines_seq() -> u32 {
    unsafe { (*GUEST.wm.get()).mines.mp_seq }
}

#[no_mangle]
pub extern "C" fn app_mines_turn() -> i32 {
    unsafe { (*GUEST.wm.get()).mines.active_turn as i32 }
}

#[no_mangle]
pub extern "C" fn app_mines_p1_score() -> i32 {
    unsafe { (*GUEST.wm.get()).mines.p1_score }
}

#[no_mangle]
pub extern "C" fn app_mines_p2_score() -> i32 {
    unsafe { (*GUEST.wm.get()).mines.p2_score }
}

#[no_mangle]
pub extern "C" fn app_mines_winner() -> i32 {
    unsafe { (*GUEST.wm.get()).mines.winner_seat as i32 }
}

#[no_mangle]
pub extern "C" fn app_mines_apply_rx(
    seq: u32,
    seat: i32,
    action: i32,
    cell: i32,
    seed: u32,
    max_strikes: i32,
) -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        if action == mines::TX_NEW_MATCH as i32 {
            let strikes = if max_strikes > 1 { 3 } else { 1 };
            wm.mines.board = mine_sm::BoardState::new(
                seed as u64,
                mines::DEFAULT_BOMBS,
                mine_sm::Rules {
                    max_strikes: strikes,
                },
            );
            wm.mines.active_turn = 1;
            wm.mines.p1_score = 0;
            wm.mines.p2_score = 0;
            wm.mines.winner_seat = 0;
            wm.mines.mp_seq = seq;
            wm.mines.tx_pending = false;
            if wm.ready {
                wm.render(fb, st, txt, ctx);
            }
            return 1;
        }

        if seq > wm.mines.mp_seq {
            let act_type = if action == mines::TX_FLAG as i32 {
                mine_sm::ActionType::Flag
            } else {
                mine_sm::ActionType::Reveal
            };
            let _ = wm
                .mines
                .apply_move(seat.clamp(1, 2) as u8, act_type, cell.clamp(0, 63) as u8);
            wm.mines.mp_seq = seq;
            wm.mines.tx_pending = false;
            if wm.ready {
                wm.render(fb, st, txt, ctx);
            }
            return 1;
        }
        0
    })
}

// --- Checkers AI & 2-Player Match Broker Host Mailbox ABI ---

#[no_mangle]
pub extern "C" fn app_checkers_set_mp(enabled: i32, match_id: u32, seat: i32, difficulty: i32) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.checkers.configure_mp(
            enabled != 0,
            match_id,
            seat.clamp(1, 2) as u8,
            checkers_sm::Difficulty::from_i32(difficulty),
        );
        wm.active_app = AppSlot::Checkers;
        wm.focus = wm::Focus::Client;
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

#[no_mangle]
pub extern "C" fn app_checkers_ai_pending() -> i32 {
    unsafe { i32::from((*GUEST.wm.get()).checkers.ai_pending) }
}

#[no_mangle]
pub extern "C" fn app_checkers_step_ai() -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        if wm.checkers.step_ai() {
            if wm.ready {
                wm.render(fb, st, txt, ctx);
            }
            1
        } else {
            0
        }
    })
}

#[no_mangle]
pub extern "C" fn app_checkers_tx_pending() -> i32 {
    unsafe { i32::from((*GUEST.wm.get()).checkers.tx_pending) }
}

#[no_mangle]
pub extern "C" fn app_checkers_take_tx_lo() -> u32 {
    unsafe { (*GUEST.wm.get()).checkers.tx_lo }
}

#[no_mangle]
pub extern "C" fn app_checkers_take_tx_hi() -> u32 {
    unsafe {
        let c = &mut (*GUEST.wm.get()).checkers;
        c.tx_pending = false;
        c.tx_hi
    }
}

#[no_mangle]
pub extern "C" fn app_checkers_apply_rx(
    seq: u32,
    seat: i32,
    action: i32,
    lo: u32,
    hi: u32,
) -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        let ok = wm.checkers.apply_rx(
            seq,
            seat.clamp(1, 2) as u8,
            action.clamp(0, 2) as u8,
            lo,
            hi,
        );
        if ok && wm.ready {
            wm.render(fb, st, txt, ctx);
        }
        i32::from(ok)
    })
}

#[no_mangle]
pub extern "C" fn app_checkers_turn() -> i32 {
    unsafe { (*GUEST.wm.get()).checkers.active_turn_seat() as i32 }
}

#[no_mangle]
pub extern "C" fn app_checkers_winner() -> i32 {
    unsafe { (*GUEST.wm.get()).checkers.state.winner as i32 }
}

#[no_mangle]
pub extern "C" fn app_checkers_seq() -> u32 {
    unsafe { (*GUEST.wm.get()).checkers.mp_seq }
}

#[no_mangle]
pub extern "C" fn app_checkers_state_hash() -> u32 {
    unsafe { (*GUEST.wm.get()).checkers.state.state_hash() }
}

#[no_mangle]
pub extern "C" fn app_checkers_red_count() -> i32 {
    unsafe { (*GUEST.wm.get()).checkers.state.piece_counts().0 as i32 }
}

#[no_mangle]
pub extern "C" fn app_checkers_black_count() -> i32 {
    unsafe { (*GUEST.wm.get()).checkers.state.piece_counts().1 as i32 }
}

// --- Chess AI & 2-Player Match Broker Host Mailbox ABI ---

#[no_mangle]
pub extern "C" fn app_chess_set_mp(enabled: i32, match_id: u32, seat: i32, difficulty: i32) {
    with_state(|wm, fb, st, txt, ctx| {
        wm.chess.configure_mp(
            enabled != 0,
            match_id,
            seat.clamp(1, 2) as u8,
            checkers_sm::Difficulty::from_i32(difficulty),
        );
        wm.active_app = AppSlot::Chess;
        wm.focus = wm::Focus::Client;
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
    });
}

#[no_mangle]
pub extern "C" fn app_chess_turn() -> i32 {
    unsafe { (*GUEST.wm.get()).chess.active_turn_seat() as i32 }
}

#[no_mangle]
pub extern "C" fn app_chess_status() -> i32 {
    unsafe {
        match (*GUEST.wm.get()).chess.state.status {
            chess_sm::GameStatus::InPlay => 0,
            chess_sm::GameStatus::Check => 1,
            chess_sm::GameStatus::Checkmate => 2,
            chess_sm::GameStatus::Stalemate => 3,
            chess_sm::GameStatus::Draw50Move => 4,
        }
    }
}

#[no_mangle]
pub extern "C" fn app_chess_white_count() -> i32 {
    unsafe { (*GUEST.wm.get()).chess.piece_counts().0 as i32 }
}

#[no_mangle]
pub extern "C" fn app_chess_black_count() -> i32 {
    unsafe { (*GUEST.wm.get()).chess.piece_counts().1 as i32 }
}

#[no_mangle]
pub extern "C" fn app_chess_state_hash() -> u32 {
    unsafe { (*GUEST.wm.get()).chess.state.state_hash() }
}

#[no_mangle]
pub extern "C" fn app_chess_tx_pending() -> i32 {
    unsafe { i32::from((*GUEST.wm.get()).chess.tx_pending) }
}

#[no_mangle]
pub extern "C" fn app_chess_take_tx() -> u32 {
    unsafe { (*GUEST.wm.get()).chess.take_tx_word() }
}

#[no_mangle]
pub extern "C" fn app_chess_apply_rx(
    seq: u32,
    seat: i32,
    action: i32,
    from: i32,
    to: i32,
    promo: i32,
) -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        let ok = wm.chess.apply_rx(
            seq,
            seat.clamp(1, 2) as u8,
            action.clamp(0, 2) as u8,
            from.clamp(0, 63) as u8,
            to.clamp(0, 63) as u8,
            promo.clamp(0, 7) as u8,
        );
        if ok && wm.ready {
            wm.render(fb, st, txt, ctx);
        }
        i32::from(ok)
    })
}

#[no_mangle]
pub extern "C" fn app_chess_ai_pending() -> i32 {
    unsafe { i32::from((*GUEST.wm.get()).chess.ai_pending) }
}

#[no_mangle]
pub extern "C" fn app_chess_step_ai() -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        if wm.chess.step_ai() {
            if wm.ready {
                wm.render(fb, st, txt, ctx);
            }
            1
        } else {
            0
        }
    })
}

// --- Player DiceBear Avatar ABI (16x16 RGBA) ---

#[no_mangle]
pub extern "C" fn app_avatar_staging_ptr() -> *mut u8 {
    unsafe { staging_base_ptr() as *mut u8 }
}

#[no_mangle]
pub extern "C" fn app_set_player_avatar(seat: i32, w: i32, h: i32) -> i32 {
    if w != 16 || h != 16 {
        return 0;
    }
    with_state(|wm, fb, st, txt, ctx| {
        let pixels = &st[..256];
        if seat == 1 {
            wm.checkers.p1_avatar.copy_from_slice(pixels);
            wm.checkers.has_p1_avatar = true;
            wm.chess.p1_avatar.copy_from_slice(pixels);
            wm.chess.has_p1_avatar = true;
            wm.mines.p1_avatar.copy_from_slice(pixels);
            wm.mines.has_p1_avatar = true;
        } else if seat == 2 {
            wm.checkers.p2_avatar.copy_from_slice(pixels);
            wm.checkers.has_p2_avatar = true;
            wm.chess.p2_avatar.copy_from_slice(pixels);
            wm.chess.has_p2_avatar = true;
            wm.mines.p2_avatar.copy_from_slice(pixels);
            wm.mines.has_p2_avatar = true;
        } else {
            return 0;
        }
        if wm.ready {
            wm.render(fb, st, txt, ctx);
        }
        1
    })
}

#[no_mangle]
pub extern "C" fn app_destroy() {
    unsafe {
        let wm = &mut *GUEST.wm.get();
        wm.ready = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fb::rgb;
    use crate::font::RenderMode;
    use crate::wm::{
        dock_icon_y, CLICK_CHANGED, CLICK_NOOP, CLICK_TTS, DOCK_ICON_H, DOCK_ICON_W, DOCK_X,
        DOCK_Y, TITLE_H,
    };

    #[test]
    fn step1_pipeline_system_calc_files_mount_and_test_pattern() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::font::set_render_mode(RenderMode::Smooth);
        app_set_session(1);
        app_set_date(2026, 10, 30, 5);
        app_set_time(6, 45, 0, 0);
        app_set_utc(2026, 10, 30, 10, 45, 0, -240);
        assert_eq!(app_init(1280, 720), 1);

        // Mount 1 image file (`wallpapers/*`) and 1 source code file (`samples/*`)
        app_mount_clear();
        let name_buf = unsafe { core::slice::from_raw_parts_mut(app_mount_name_buf(), 64) };
        let f0 = b"wallpapers/01_aurora.png";
        name_buf[..f0.len()].copy_from_slice(f0);
        assert_eq!(app_mount_add(f0.len(), 493_568), 0);

        let f1 = b"samples/app.js";
        name_buf[..f1.len()].copy_from_slice(f1);
        assert_eq!(app_mount_add(f1.len(), 1_045), 1);

        // 1. Starts on empty Desktop with Dock hidden
        assert_eq!(app_selected(), AppSlot::Desktop as i32);
        assert_eq!(app_dock_visible(), 0);
        let desktop_hash = app_fb_hash();

        // 2. Open Dock and launch System (slot 0)
        let icon_x = DOCK_X + DOCK_ICON_W / 2;
        assert_eq!(app_click(16, 16), CLICK_CHANGED);
        assert_eq!(app_click(icon_x, DOCK_Y + DOCK_ICON_H / 2), CLICK_CHANGED);
        assert_eq!(app_selected(), AppSlot::System as i32);
        let about_hash = app_fb_hash();
        assert_ne!(about_hash, desktop_hash);

        // 3. Open Dock and launch Files (slot 2) -> opens at `~/` Root showing `wallpapers/` and `samples/`
        assert_eq!(app_click(16, 16), CLICK_CHANGED);
        assert_eq!(
            app_click(icon_x, dock_icon_y(2) + DOCK_ICON_H / 2),
            CLICK_CHANGED
        );
        assert_eq!(app_selected(), AppSlot::Files as i32);
        assert_eq!(app_active_page(), -1); // At `~/` root folder level
        assert_eq!(app_mount_pending_idx(), -1);
        let files_root_hash = app_fb_hash();
        assert_ne!(files_root_hash, about_hash);

        // 4. Click row 0 (`wallpapers/`) in the left 1/3 browser panel -> opens `~/WALLPAPERS` and selects file 0
        assert_eq!(app_click(100, TITLE_H + 76 + 10), CLICK_CHANGED);
        assert_eq!(app_active_page(), 0);
        assert_eq!(app_mount_pending_idx(), 0);
        let files_loading_hash = app_fb_hash();
        assert_ne!(files_loading_hash, files_root_hash);

        // Simulate `host.js` writing a 1280x688 RGBA8888 image into staging memory
        unsafe {
            let st =
                core::slice::from_raw_parts_mut(app_mount_staging_ptr() as *mut u32, 1280 * 720);
            st.fill(rgb(0x12, 0x77, 0xaa));
        }
        app_mount_commit_rgba(0, 1280, 688);
        assert_eq!(app_mount_pending_idx(), -1);
        let files_preview0_hash = app_fb_hash();
        assert_ne!(files_preview0_hash, files_loading_hash);

        // 5. Click row 0 (`../`) to return to `~/`, then click row 1 (`samples/`) to open `~/SAMPLES`
        assert_eq!(app_click(100, TITLE_H + 76 + 10), CLICK_CHANGED);
        assert_eq!(app_active_page(), -1);
        assert_eq!(app_click(100, TITLE_H + 76 + 46 + 10), CLICK_CHANGED);
        assert_eq!(app_active_page(), 1);
        assert_eq!(app_mount_pending_idx(), 1);
        let sample_js = b"export class TaskBoard {\n  constructor() {\n    return 42;\n  }\n}\n";
        unsafe {
            let txt = core::slice::from_raw_parts_mut(app_mount_text_ptr(), MAX_TEXT_BYTES);
            txt[..sample_js.len()].copy_from_slice(sample_js);
        }
        app_mount_commit_text(1, sample_js.len());
        assert_eq!(app_mount_pending_idx(), -1);
        let files_code_hash = app_fb_hash();
        assert_ne!(files_code_hash, files_preview0_hash);

        // 6. Switch to `~/WALLPAPERS` via Left Panel click (or we can navigate to Wallpapers), then `SET AS WALLPAPER` (item 3)
        // First return to `~/` by clicking row 0 (`../`)
        assert_eq!(app_click(100, TITLE_H + 76 + 10), CLICK_CHANGED);
        assert_eq!(app_active_page(), -1);
        // Click row 0 (`wallpapers/`) to open `~/WALLPAPERS` (active_page = 0)
        assert_eq!(app_click(100, TITLE_H + 76 + 10), CLICK_CHANGED);
        assert_eq!(app_active_page(), 0);

        let files_menu_x = wm::MENU_X0 + 20;
        assert_eq!(app_click(files_menu_x, 16), CLICK_CHANGED);
        assert_eq!(
            app_click(files_menu_x, TITLE_H + 16 + 6 * wm::MENU_ITEM_H),
            CLICK_CHANGED
        );
        assert_eq!(app_selected(), AppSlot::Desktop as i32);
        assert_ne!(app_fb_hash(), desktop_hash); // Desktop now renders the mounted wallpaper!

        // 6. Record a snapshot via the host ABI (`app_record_snapshot`) and toggle Test Pattern from the Home drawer
        let snap_hash = app_record_snapshot();
        assert_ne!(snap_hash, 0);
        assert_eq!(app_snapshot_count(), 1);

        // Open Home drawer and click item 6 ("Test Pattern")
        assert_eq!(app_click(16, 16), CLICK_CHANGED);
        assert_eq!(app_click(icon_x, dock_icon_y(6) + 14), CLICK_CHANGED);

        // 7. Verify diagnostic test pattern channel & orientation contract
        let ptr = app_framebuffer();
        let px = unsafe { core::slice::from_raw_parts(ptr as *const u32, 1280 * 720) };
        assert_eq!(px[8 * 1280 + 8], rgb(0, 0, 0));
        assert_eq!(px[50 * 1280 + 100], rgb(255, 0, 0));
        assert_eq!(px[50 * 1280 + 900], rgb(0, 255, 0));
        assert_eq!(px[500 * 1280 + 100], rgb(0, 0, 255));
        assert_eq!(px[(720 - 20) * 1280 + (1280 - 20)], rgb(255, 0, 255));
        app_set_test_pattern(0);

        // 8. Verify per-session resolution constraint for mobile portrait (390x844) and 1080p (1920x1080)
        assert_eq!(app_init(390, 844), 1);
        assert_eq!(app_width(), 390);
        assert_eq!(app_height(), 844);
        assert_eq!(app_click(16, 16), CLICK_CHANGED);
        assert_eq!(app_click(icon_x, DOCK_Y + DOCK_ICON_H / 2), CLICK_CHANGED);
        assert_eq!(app_selected(), AppSlot::System as i32);

        assert_eq!(app_init(1920, 1080), 1);
        assert_eq!(app_width(), 1920);
        assert_eq!(app_height(), 1080);
    }

    #[test]
    fn test_github_mount_and_context_staging_and_key_paging() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::font::set_render_mode(RenderMode::Smooth);
        app_set_session(1);
        assert_eq!(app_init(1280, 720), 1);

        app_mount_clear();
        let name_buf = unsafe { core::slice::from_raw_parts_mut(app_mount_name_buf(), 64) };

        // Mount a nested GitHub file: "gh/jpeckenpaugh/mines-sm/lib.rs"
        let f_gh = b"gh/jpeckenpaugh/mines-sm/lib.rs";
        name_buf[..f_gh.len()].copy_from_slice(f_gh);
        let gh_idx = app_mount_add(f_gh.len(), 4096);
        assert_eq!(gh_idx, 0);

        // Switch to Files app
        let icon_x = DOCK_X + DOCK_ICON_W / 2;
        assert_eq!(app_click(16, 16), CLICK_CHANGED); // Open dock
        assert_eq!(
            app_click(icon_x, dock_icon_y(2) + DOCK_ICON_H / 2),
            CLICK_CHANGED
        );
        assert_eq!(app_selected(), AppSlot::Files as i32);
        assert_eq!(app_active_page(), -1); // At HomeRoot

        // Navigate to GITHUB via menu (FILES -> GITHUB (GH://), item 1)
        let files_menu_x = wm::MENU_X0 + 20;
        assert_eq!(app_click(files_menu_x, 16), CLICK_CHANGED);
        assert_eq!(
            app_click(files_menu_x, TITLE_H + 16 + wm::MENU_ITEM_H),
            CLICK_CHANGED
        );
        assert_eq!(app_active_page(), -1); // At gh:// root
        // Open the repository row -> gh://jpeckenpaugh/mines-sm/
        assert_eq!(app_click(100, TITLE_H + 76 + 46 + 10), CLICK_CHANGED);
        assert_eq!(app_active_page(), 0); // Active file is lib.rs (index 0)
        assert_eq!(app_mount_pending_idx(), 0);

        // Stage Rust source code into text buffer
        let rs_code = b"//! Mines State Machine\npub struct BoardState {\n    pub seed: u64,\n}\n";
        unsafe {
            let txt = core::slice::from_raw_parts_mut(app_mount_text_ptr(), MAX_TEXT_BYTES);
            txt[..rs_code.len()].copy_from_slice(rs_code);
        }
        app_mount_commit_text(0, rs_code.len());
        assert_eq!(app_mount_pending_idx(), -1);
        let raw_hash = app_fb_hash();

        // Stage wire context into context buffer
        let ctx_wire = b"SCOPE=gh://jpeckenpaugh/mines-sm (main)\nRUNTIME=Rust (#![no_std])\nROLE=Core Bitboard State Machine\nIMPORTS=core::ops\nEXPORTS=BoardState\nDEPENDENTS=v2/src/mines.rs\nINVARIANTS=SplitMix64\nSUMMARY=Deterministic 8x8 mines state machine.\nARCHITECTURE=Pure kernel.\n";
        unsafe {
            let ctx = core::slice::from_raw_parts_mut(app_mount_context_ptr(), MAX_CONTEXT_BYTES);
            ctx[..ctx_wire.len()].copy_from_slice(ctx_wire);
        }
        app_mount_commit_context(0, ctx_wire.len());

        // Toggle context mode via `I` (40). Tab (33) is now focus traversal.
        assert_eq!(app_files_context_mode(), 0);
        assert_eq!(app_key(40), CLICK_CHANGED);
        assert_eq!(app_files_context_mode(), 1);
        let ctx_hash = app_fb_hash();
        assert_ne!(ctx_hash, raw_hash);

        // Toggle back to raw mode with the same key.
        assert_eq!(app_key(40), CLICK_CHANGED);
        assert_eq!(app_files_context_mode(), 0);
        assert_ne!(app_fb_hash(), ctx_hash);

        // Toggle via menu item 2 (TOGGLE CONTEXT)
        assert_eq!(app_click(files_menu_x, 16), CLICK_CHANGED);
        assert_eq!(
            app_click(files_menu_x, TITLE_H + 16 + 4 * wm::MENU_ITEM_H),
            CLICK_CHANGED
        );
        assert_eq!(app_files_context_mode(), 1);

        // Toggle back via menu item 2
        assert_eq!(app_click(files_menu_x, 16), CLICK_CHANGED);
        assert_eq!(
            app_click(files_menu_x, TITLE_H + 16 + 4 * wm::MENU_ITEM_H),
            CLICK_CHANGED
        );
        assert_eq!(app_files_context_mode(), 0);

        // Test paging keys: PageDown (35) and PageUp (34)
        assert_eq!(app_key(35), CLICK_CHANGED); // scroll chunk down
        assert_eq!(app_key(34), CLICK_CHANGED); // scroll chunk up

        // Test new font scaling and horizontal/vertical scroll keys
        assert_eq!(app_key(20), CLICK_CHANGED); // font_scale + 1 -> 3
        assert_eq!(app_key(20), CLICK_CHANGED); // font_scale + 1 -> 4
        assert_eq!(app_key(20), CLICK_CHANGED); // clamped at 4 (no visual change)
        assert_eq!(app_key(21), CLICK_CHANGED); // font_scale - 1 -> 3
        assert_eq!(app_key(21), CLICK_CHANGED); // font_scale - 1 -> 2
        assert_eq!(app_key(21), CLICK_CHANGED); // clamped at 2 (no visual change)

        assert_eq!(app_key(37), CLICK_CHANGED); // col_scroll + 8
        assert_eq!(app_key(36), CLICK_CHANGED); // col_scroll - 8
        assert_eq!(app_key(38), CLICK_CHANGED); // scroll down: code + 5, ctx + 2
        assert_eq!(app_key(39), CLICK_CHANGED); // scroll up: code - 5, ctx - 2

        // Test TTS button click and C export
        assert_eq!(app_tts_active(), 0);
        // Compute click on TTS button in Control bar:
        let tts_click_x = 1135;
        let tts_click_y = TITLE_H + 16;
        assert_eq!(app_click(tts_click_x, tts_click_y), CLICK_TTS);
        assert_eq!(app_tts_active(), 1);

        // Click again to stop TTS
        assert_eq!(app_click(tts_click_x, tts_click_y), CLICK_TTS);
        assert_eq!(app_tts_active(), 0);

        // Test host setting TTS active directly via C ABI
        app_tts_set_active(1);
        assert_eq!(app_tts_active(), 1);
        app_tts_set_active(0);
        assert_eq!(app_tts_active(), 0);
    }

    #[test]
    fn keyboard_focus_traversal_and_activation() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::font::set_render_mode(RenderMode::Smooth);
        app_set_session(1);
        assert_eq!(app_init(1280, 720), 1);
        assert_eq!(app_focus(), 0);

        // F10 -> Home header, Down -> Home drawer, Enter -> System.
        assert_eq!(app_key(61), CLICK_CHANGED);
        assert_eq!(app_focus(), 100);
        assert_eq!(app_key(51), CLICK_CHANGED);
        assert_eq!(app_focus(), 200);
        assert_eq!(app_key(24), CLICK_CHANGED);
        assert_eq!(app_focus(), 0);
        assert_eq!(app_selected(), 1);
        assert_eq!(app_active_page(), 0);

        // F9 -> Home drawer, Down -> Calc, Enter -> launch.
        assert_eq!(app_key(60), CLICK_CHANGED);
        assert_eq!(app_focus(), 200);
        assert_eq!(app_dock_visible(), 1);
        assert_eq!(app_key(51), CLICK_CHANGED);
        assert_eq!(app_focus(), 201);
        assert_eq!(app_key(24), CLICK_CHANGED);
        assert_eq!(app_focus(), 0);
        assert_eq!(app_selected(), 2);
        assert_eq!(app_dock_visible(), 0);

        // Esc closes the drawer to the Home header; a second Esc returns to
        // the client.
        assert_eq!(app_key(60), CLICK_CHANGED);
        assert_eq!(app_focus(), 200);
        assert_eq!(app_key(25), CLICK_CHANGED);
        assert_eq!(app_focus(), 100);
        assert_eq!(app_key(25), CLICK_CHANGED);
        assert_eq!(app_focus(), 0);
        assert_eq!(app_dock_visible(), 0);

        // Ctrl+3 launches Files without opening the dock.
        assert_eq!(app_key_mods(13, 2), CLICK_CHANGED);
        assert_eq!(app_selected(), 3);
        assert_eq!(app_focus(), 0);

        // Tab cycles Files controls; Shift+Tab reverses.
        assert_eq!(app_files_focus(), 0);
        assert_eq!(app_key(33), CLICK_CHANGED);
        assert_eq!(app_files_focus(), 1);
        assert_eq!(app_key_mods(33, 1), CLICK_CHANGED);
        assert_eq!(app_files_focus(), 0);
    }

    #[test]
    fn fonts_page_switches_render_mode() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::font::set_render_mode(RenderMode::Smooth);
        app_set_session(1);
        assert_eq!(app_init(1280, 720), 1);
        assert_eq!(app_key_mods(11, 2), CLICK_CHANGED); // Ctrl+1 -> System
        assert_eq!(app_key(13), CLICK_CHANGED); // 3 -> Fonts
        assert_eq!(app_active_page(), 2);
        assert_eq!(app_render_mode(), 1);
        assert_eq!(app_font_family(), 1); // deprecated alias

        assert_eq!(app_key(1), CLICK_CHANGED); // Left -> Native
        assert_eq!(app_render_mode(), 0);
        assert_eq!(app_key(2), CLICK_CHANGED); // Right -> Smooth
        assert_eq!(app_render_mode(), 1);

        // Render mode never changes layout.
        assert_eq!(crate::font::char_advance(2), 13);
        assert_eq!(crate::font::line_height(2), 20);
        assert_eq!(crate::font::glyph_height(2), 16);
    }

    #[test]
    fn fonts_page_selects_preview_size() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        app_set_session(1);
        assert_eq!(app_init(1280, 720), 1);
        assert_eq!(app_key_mods(11, 2), CLICK_CHANGED); // Ctrl+1 -> System
        assert_eq!(app_key(13), CLICK_CHANGED); // 3 -> Fonts
        assert_eq!(app_preview_scale(), 4); // default 4x

        assert_eq!(app_key(51), CLICK_CHANGED); // Down -> 5x
        assert_eq!(app_preview_scale(), 5);
        assert_eq!(app_key(50), CLICK_CHANGED); // Up -> 4x
        assert_eq!(app_preview_scale(), 4);

        // Click the 2x ladder row (content y 265..285, plus the title bar).
        assert_eq!(app_click(200, TITLE_H + 270), CLICK_CHANGED);
        assert_eq!(app_preview_scale(), 2);
        let h = app_fb_hash();
        assert_eq!(app_key(51), CLICK_CHANGED); // Down -> 3x
        assert_ne!(app_fb_hash(), h);
    }

    #[test]
    fn theme_page_applies_palette_only() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        app_set_session(1);
        assert_eq!(app_init(1280, 720), 1);
        assert_eq!(app_key_mods(11, 2), CLICK_CHANGED); // Ctrl+1 -> System
        assert_eq!(app_key(15), CLICK_CHANGED); // 5 -> Theme
        assert_eq!(app_active_page(), 4);
        assert_eq!(app_key(51), CLICK_CHANGED); // Down -> next theme
        assert_eq!(app_key(24), CLICK_CHANGED); // Enter -> apply
        assert_eq!(app_theme(), 1);
        assert_eq!(app_key(41), CLICK_CHANGED); // Home -> first theme
        assert_eq!(app_key(24), CLICK_CHANGED); // apply
        assert_eq!(app_theme(), 0);
    }

    #[test]
    fn pointer_hover_is_reported_once() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(app_init(1280, 720), 1);
        assert_eq!(app_pointer_move(56, 16), CLICK_CHANGED); // first menu header
        assert_eq!(app_pointer_move(56, 16), CLICK_NOOP);
        assert_eq!(app_pointer_move(10, 400), CLICK_CHANGED); // leaves the header
        assert_eq!(app_pointer_leave(), CLICK_NOOP);
    }

    #[test]
    fn mines_multiplayer_abi_roundtrip() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(app_init(1280, 720), 1);

        // Configure Seat 1 on match #99, seed 42, classic rules (max_strikes = 1)
        app_mines_set_mp(1, 99, 1, 42, 1);
        assert_eq!(app_selected(), AppSlot::Mines as i32);
        assert_eq!(app_mines_seed(), 42);
        assert_eq!(app_mines_seq(), 0);
        assert_eq!(app_mines_turn(), 1);
        assert_eq!(app_mines_tx_pending(), 0);

        // Click top-left cell (cell 0) on the 1280x720 Mines board
        // At 1280x688 client area, board_x = 150, board_y = 54, cell = 68
        assert_eq!(app_click(170, TITLE_H + 70), CLICK_CHANGED);
        assert_eq!(app_mines_tx_pending(), 1);
        let word = app_mines_take_tx();
        assert_eq!(app_mines_tx_pending(), 0);

        let tx_action = (word & 0xff) as i32;
        let tx_cell = ((word >> 8) & 0xff) as i32;
        let tx_seat = ((word >> 16) & 0xff) as i32;
        let tx_strikes = ((word >> 24) & 0xff) as i32;
        assert_eq!(tx_action, 0); // Reveal
        assert_eq!(tx_cell, 0);
        assert_eq!(tx_seat, 1);
        assert_eq!(tx_strikes, 1);
        assert_eq!(app_mines_seq(), 1);
        assert_eq!(app_mines_turn(), 2);
        let p1_after_move1 = app_mines_p1_score();
        assert!(p1_after_move1 > 0);

        // Simulate Seat 2 guest receiving P1's move via app_mines_apply_rx
        app_mines_set_mp(1, 99, 2, 42, 1);
        assert_eq!(app_mines_seq(), 0);
        assert_eq!(
            app_mines_apply_rx(1, tx_seat, tx_action, tx_cell, 42, tx_strikes),
            1
        );
        assert_eq!(app_mines_seq(), 1);
        assert_eq!(app_mines_turn(), 2);
        assert_eq!(app_mines_p1_score(), p1_after_move1);
    }

    #[test]
    fn checkers_multiplayer_and_ai_abi_roundtrip() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(app_init(1280, 720), 1);

        // 1. Configure Checkers in 1P vs AI mode (enabled = 0, difficulty = 1 Medium)
        app_checkers_set_mp(0, 1, 1, 1);
        assert_eq!(app_selected(), AppSlot::Checkers as i32);
        assert_eq!(app_checkers_turn(), 1);
        assert_eq!(app_checkers_red_count(), 12);
        assert_eq!(app_checkers_black_count(), 12);
        assert_eq!(app_checkers_ai_pending(), 0);

        // At 1280x688 client area:
        // max_board_h = 644, max_board_w = 724 -> avail = 644 -> cell = 74, grid_px = 592
        // info_w = 360, total_w = 1000 -> board_x = 140, board_y = 48
        // Square 20 is row 5, col 0 -> x = 140 + 37, y = 48 + 5*74 + 37
        // Square 16 is row 4, col 1 -> x = 140 + 74 + 37, y = 48 + 4*74 + 37
        let sq20_x = 140 + 37;
        let sq20_y = TITLE_H + 48 + 5 * 74 + 37;
        let sq16_x = 140 + 74 + 37;
        let sq16_y = TITLE_H + 48 + 4 * 74 + 37;

        assert_eq!(app_click(sq20_x, sq20_y), CLICK_CHANGED);
        assert_eq!(app_click(sq16_x, sq16_y), CLICK_CHANGED);
        assert_eq!(app_checkers_turn(), 2);
        assert_eq!(app_checkers_ai_pending(), 1);

        // Step AI
        assert_eq!(app_checkers_step_ai(), 1);
        assert_eq!(app_checkers_ai_pending(), 0);
        assert_eq!(app_checkers_turn(), 1);

        // 2. Configure Checkers in 2P Broker mode on Seat 1 (Red)
        app_checkers_set_mp(1, 77, 1, 1);
        assert_eq!(app_checkers_turn(), 1);
        assert_eq!(app_checkers_seq(), 0);
        let initial_hash = app_checkers_state_hash();

        // Click Red piece at square 20 -> destination square 16
        assert_eq!(app_click(sq20_x, sq20_y), CLICK_CHANGED);
        assert_eq!(app_click(sq16_x, sq16_y), CLICK_CHANGED);
        assert_eq!(app_checkers_tx_pending(), 1);
        let lo = app_checkers_take_tx_lo();
        let hi = app_checkers_take_tx_hi();
        assert_eq!(app_checkers_tx_pending(), 0);
        assert_eq!(app_checkers_turn(), 2);
        let p1_hash_after = app_checkers_state_hash();
        assert_ne!(p1_hash_after, initial_hash);

        // Out-of-turn click on Seat 1 is rejected
        let sq21_x = 140 + 2 * 74 + 37;
        let sq21_y = TITLE_H + 48 + 5 * 74 + 37;
        assert_eq!(app_click(sq21_x, sq21_y), CLICK_NOOP);

        // Simulate Seat 2 receiving P1's move via app_checkers_apply_rx
        app_checkers_set_mp(1, 77, 2, 1);
        assert_eq!(app_checkers_state_hash(), initial_hash);
        let tx_action = (lo & 0xF) as i32;
        let tx_seat = ((lo >> 4) & 0xF) as i32;
        assert_eq!(tx_action, 1);
        assert_eq!(tx_seat, 1);
        assert_eq!(app_checkers_apply_rx(1, tx_seat, tx_action, lo, hi), 1);
        assert_eq!(app_checkers_seq(), 1);
        assert_eq!(app_checkers_turn(), 2);
        assert_eq!(app_checkers_state_hash(), p1_hash_after);
    }

    #[test]
    fn chess_multiplayer_and_ai_abi_roundtrip() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(app_init(1280, 720), 1);

        // 1. Configure Chess in 1P vs AI mode (enabled = 0, difficulty = 0 Easy)
        app_chess_set_mp(0, 1, 1, 0);
        assert_eq!(app_selected(), AppSlot::Chess as i32);
        assert_eq!(app_chess_turn(), 1);
        assert_eq!(app_chess_status(), 0); // InPlay
        assert_eq!(app_chess_white_count(), 16);
        assert_eq!(app_chess_black_count(), 16);
        assert_eq!(app_chess_ai_pending(), 0);

        // Board layout at 1280x688:
        // max_board_h = 644, max_board_w = 724 -> cell = 74, grid_px = 592
        // info_w = 360, total_w = 1000 -> board_x = 140, board_y = 48
        // Square e2: file 4, rank 1 -> col 4, row 6 (since row 0 is rank 7 at top, row 7 is rank 0 at bottom)
        // v_col = 4, v_row = 7 - 1 = 6
        // Square e4: file 4, rank 3 -> col 4, row 4
        // v_col = 4, v_row = 7 - 3 = 4
        let sq_e2_x = 140 + 4 * 74 + 37;
        let sq_e2_y = TITLE_H + 48 + 6 * 74 + 37;
        let sq_e4_x = 140 + 4 * 74 + 37;
        let sq_e4_y = TITLE_H + 48 + 4 * 74 + 37;

        assert_eq!(app_click(sq_e2_x, sq_e2_y), CLICK_CHANGED);
        assert_eq!(app_click(sq_e4_x, sq_e4_y), CLICK_CHANGED);
        assert_eq!(app_chess_turn(), 2);
        assert_eq!(app_chess_ai_pending(), 1);

        // Step AI
        assert_eq!(app_chess_step_ai(), 1);
        assert_eq!(app_chess_ai_pending(), 0);
        assert_eq!(app_chess_turn(), 1);

        // 2. Configure Chess in 2P Broker mode on Seat 1 (White)
        app_chess_set_mp(1, 88, 1, 0);
        assert_eq!(app_chess_turn(), 1);
        let initial_hash = app_chess_state_hash();

        // White plays e2 -> e4
        assert_eq!(app_click(sq_e2_x, sq_e2_y), CLICK_CHANGED);
        assert_eq!(app_click(sq_e4_x, sq_e4_y), CLICK_CHANGED);
        assert_eq!(app_chess_tx_pending(), 1);
        let word = app_chess_take_tx();
        assert_eq!(app_chess_tx_pending(), 0);
        assert_eq!(app_chess_turn(), 2);

        let action = (word & 0xFF) as i32;
        let seat = ((word >> 8) & 0xFF) as i32;
        let from = ((word >> 16) & 0xFF) as i32;
        let to = ((word >> 24) & 0xFF) as i32;
        let promo = ((word >> 4) & 0x07) as i32;

        assert_eq!(action, 1);
        assert_eq!(seat, 1);
        assert_eq!(from, 12); // e2
        assert_eq!(to, 28);   // e4
        let p1_hash_after = app_chess_state_hash();
        assert_ne!(p1_hash_after, initial_hash);

        // Out-of-turn click on Seat 1 is rejected
        assert_eq!(app_click(sq_e4_x, sq_e4_y), CLICK_NOOP);

        // Simulate Seat 2 receiving P1's move via app_chess_apply_rx
        app_chess_set_mp(1, 88, 2, 0);
        assert_eq!(app_chess_state_hash(), initial_hash);
        assert_eq!(app_chess_apply_rx(1, seat, action, from, to, promo), 1);
        assert_eq!(app_chess_turn(), 2);
        assert_eq!(app_chess_state_hash(), p1_hash_after);
    }

    #[test]
    fn semantic_reflection_queries_across_resolutions() {
        let _g = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        app_set_session(1);

        // 1. Test at 1280x720
        assert_eq!(app_init(1280, 720), 1);

        // Title bar Home button
        let hit_home = app_semantic_hit(20, 16);
        assert_eq!(hit_home >> 24, 1);
        let coord_home = app_semantic_coord(1, 0);
        let cx = (coord_home >> 16) as i32;
        let cy = (coord_home & 0xFFFF) as i32;
        assert_eq!(cy, wm::TITLE_H / 2);
        assert_eq!(cx, wm::HOME_BTN_W / 2);

        // Open Chess (Ctrl+6)
        assert_eq!(app_key_mods(16, 2), CLICK_CHANGED);
        assert_eq!(app_selected(), 6);

        // e2 square: square 12
        let coord_e2 = app_semantic_coord(2, 12);
        let e2_x = (coord_e2 >> 16) as i32;
        let e2_y = (coord_e2 & 0xFFFF) as i32;
        assert_eq!(app_semantic_hit(e2_x, e2_y), (2 << 24) | 12);

        // New Game button: kind 5, id 0
        let coord_btn = app_semantic_coord(5, 0);
        let btn_x = (coord_btn >> 16) as i32;
        let btn_y = (coord_btn & 0xFFFF) as i32;
        assert_eq!(app_semantic_hit(btn_x, btn_y), (5 << 24) | 0);

        // 2. Test at Mobile Portrait 390x844
        assert_eq!(app_init(390, 844), 1);
        assert_eq!(app_key_mods(16, 2), CLICK_CHANGED); // Open Chess
        let coord_e2_mob = app_semantic_coord(2, 12);
        let mob_x = (coord_e2_mob >> 16) as i32;
        let mob_y = (coord_e2_mob & 0xFFFF) as i32;
        assert_eq!(app_semantic_hit(mob_x, mob_y), (2 << 24) | 12);

        // 3. Test Mines at 1280x720
        assert_eq!(app_init(1280, 720), 1);
        assert_eq!(app_key_mods(14, 2), CLICK_CHANGED); // Open Mines
        let coord_cell0 = app_semantic_coord(4, 0);
        let c0_x = (coord_cell0 >> 16) as i32;
        let c0_y = (coord_cell0 & 0xFFFF) as i32;
        assert_eq!(app_semantic_hit(c0_x, c0_y), (4 << 24) | 0);

        // 4. Test Checkers at 1280x720
        assert_eq!(app_init(1280, 720), 1);
        assert_eq!(app_key_mods(15, 2), CLICK_CHANGED); // Open Checkers
        let coord_chk0 = app_semantic_coord(3, 0);
        let chk0_x = (coord_chk0 >> 16) as i32;
        let chk0_y = (coord_chk0 & 0xFFFF) as i32;
        assert_eq!(app_semantic_hit(chk0_x, chk0_y), (3 << 24) | 0);

        // 5. Test at 1080p (1920x1080) dynamic scaling
        assert_eq!(app_init(1920, 1080), 1);
        let g_chess_1080 = chess::chess_geom(1920, 1080 - wm::TITLE_H);
        assert_eq!(g_chess_1080.cell, 116);
        let g_chk_1080 = checkers::checkers_geom(1920, 1080 - wm::TITLE_H);
        assert_eq!(g_chk_1080.cell, 116);
        let g_mines_1080 = mines::mines_geom(1920, 1080 - wm::TITLE_H);
        assert_eq!(g_mines_1080.cell, 112);
    }
}
