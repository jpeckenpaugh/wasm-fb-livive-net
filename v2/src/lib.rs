#![cfg_attr(target_arch = "wasm32", no_std)]

pub mod calc;
pub mod code;
pub mod fb;
pub mod files;
pub mod font;
pub mod mine_sm;
pub mod mines;
pub mod system;
pub mod wm;

use core::cell::UnsafeCell;
use fb::{Framebuffer, DEFAULT_H, DEFAULT_W, MAX_DIM, MAX_PIXELS};
use files::{MAX_CONTEXT_BYTES, MAX_TEXT_BYTES};
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

fn with_state<R>(
    f: impl FnOnce(&mut WmState, &mut Framebuffer, &[u32], &[u8], &[u8]) -> R,
) -> R {
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
        if rc != 0 {
            wm.render(fb, st, txt, ctx);
        }
        rc
    })
}

#[no_mangle]
pub extern "C" fn app_key(key: i32) -> i32 {
    with_state(|wm, fb, st, txt, ctx| {
        let rc = wm.key(key);
        if rc != 0 {
            wm.render(fb, st, txt, ctx);
        }
        rc
    })
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
        wm.last_fb_hash
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
    unsafe { (*GUEST.wm.get()).last_fb_hash }
}

#[no_mangle]
pub extern "C" fn app_snapshot_count() -> u32 {
    unsafe { (*GUEST.wm.get()).snapshot_seq }
}

#[no_mangle]
pub extern "C" fn app_dock_visible() -> i32 {
    unsafe { i32::from((*GUEST.wm.get()).dock_visible) }
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
    4 // 4 bundled apps: System, Calc, Files, Mines
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
    use crate::wm::{
        dock_icon_y, CLICK_CHANGED, CLICK_SNAPSHOT, CLICK_TTS, DOCK_ICON_H, DOCK_ICON_W, DOCK_X,
        DOCK_Y, TITLE_H,
    };

    #[test]
    fn step1_pipeline_system_calc_files_mount_and_test_pattern() {
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
            let st = core::slice::from_raw_parts_mut(app_mount_staging_ptr() as *mut u32, 1280 * 720);
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
            app_click(files_menu_x, TITLE_H + 16 + 3 * wm::MENU_ITEM_H),
            CLICK_CHANGED
        );
        assert_eq!(app_selected(), AppSlot::Desktop as i32);
        assert_ne!(app_fb_hash(), desktop_hash); // Desktop now renders the mounted wallpaper!

        // 6. Open "Testing" menu (menu index 2) and click "SNAPSHOT"
        let testing_menu_x = wm::MENU_X0 + 2 * wm::MENU_HEADER_W + 20;
        assert_eq!(app_click(testing_menu_x, 16), CLICK_CHANGED);
        assert_eq!(app_click(testing_menu_x, TITLE_H + 16), CLICK_SNAPSHOT);
        assert_eq!(app_snapshot_count(), 1);

        // 7. Verify diagnostic test pattern channel & orientation contract
        app_set_test_pattern(1);
        let ptr = app_framebuffer();
        let px = unsafe { core::slice::from_raw_parts(ptr as *const u32, 1280 * 720) };
        assert_eq!(px[8 * 1280 + 8], rgb(0, 0, 0));
        assert_eq!(px[50 * 1280 + 100], rgb(255, 0, 0));
        assert_eq!(px[50 * 1280 + 900], rgb(0, 255, 0));
        assert_eq!(px[500 * 1280 + 100], rgb(0, 0, 255));
        assert_eq!(px[(720 - 20) * 1280 + (1280 - 20)], rgb(255, 0, 255));

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
        app_set_session(1);
        assert_eq!(app_init(1280, 720), 1);

        app_mount_clear();
        let name_buf = unsafe { core::slice::from_raw_parts_mut(app_mount_name_buf(), 64) };

        // Mount a GitHub file: "github/lib.rs"
        let f_gh = b"github/lib.rs";
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
        assert_eq!(app_active_page(), 0); // Active file is github/lib.rs (index 0)
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

        // Toggle context mode via key 33 (Tab / I)
        assert_eq!(app_key(33), CLICK_CHANGED);
        let ctx_hash = app_fb_hash();
        assert_ne!(ctx_hash, raw_hash);

        // Toggle back to raw mode via key 33
        assert_eq!(app_key(33), CLICK_CHANGED);
        assert_eq!(app_fb_hash(), raw_hash);

        // Toggle via menu item 2 (TOGGLE CONTEXT)
        assert_eq!(app_click(files_menu_x, 16), CLICK_CHANGED);
        assert_eq!(
            app_click(files_menu_x, TITLE_H + 16 + 2 * wm::MENU_ITEM_H),
            CLICK_CHANGED
        );
        assert_eq!(app_fb_hash(), ctx_hash);

        // Toggle back via menu item 2
        assert_eq!(app_click(files_menu_x, 16), CLICK_CHANGED);
        assert_eq!(
            app_click(files_menu_x, TITLE_H + 16 + 2 * wm::MENU_ITEM_H),
            CLICK_CHANGED
        );
        assert_eq!(app_fb_hash(), raw_hash);

        // Test paging keys: PageDown (35) and PageUp (34)
        assert_eq!(app_key(35), CLICK_CHANGED); // scroll chunk down
        assert_eq!(app_key(34), CLICK_CHANGED); // scroll chunk up

        // Test new font scaling and horizontal/vertical scroll keys
        assert_eq!(app_key(20), CLICK_CHANGED); // font_scale + 1 -> 2
        assert_eq!(app_key(20), CLICK_CHANGED); // font_scale + 1 -> 3
        assert_eq!(app_key(20), CLICK_CHANGED); // font_scale clamped at 3
        assert_eq!(app_key(21), CLICK_CHANGED); // font_scale - 1 -> 2
        assert_eq!(app_key(21), CLICK_CHANGED); // font_scale - 1 -> 1
        assert_eq!(app_key(21), CLICK_CHANGED); // font_scale clamped at 1

        assert_eq!(app_key(37), CLICK_CHANGED); // col_scroll + 8
        assert_eq!(app_key(36), CLICK_CHANGED); // col_scroll - 8
        assert_eq!(app_key(38), CLICK_CHANGED); // scroll down: code + 5, ctx + 2
        assert_eq!(app_key(39), CLICK_CHANGED); // scroll up: code - 5, ctx - 2

        // Test TTS button click and C export
        assert_eq!(app_tts_active(), 0);
        // Compute click on TTS button in Control bar:
        // mode_w = text_width("[ RAW VIEW ]", 1) + 12 = 12 * 6 + 12 = 84
        // mode_x = 1280 - 84 - 12 = 1184
        // ap_w = 44, ap_x = 1184 - 44 - 8 = 1132
        // am_w = 44, am_x = 1132 - 44 - 4 = 1084
        // tts_w = 10 * 6 + 10 = 70, tts_x = 1084 - 70 - 8 = 1006
        // Click inside TTS button: x = 1020, y = TITLE_H + 16 = 48
        let tts_click_x = 1020;
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
}
