#![cfg_attr(target_arch = "wasm32", no_std)]

pub mod about;
pub mod calc;
pub mod fb;
pub mod font;
pub mod wm;

use core::cell::UnsafeCell;
use fb::{Framebuffer, SCREEN_H, SCREEN_W};
use wm::{AppSlot, WmState};

#[cfg(not(target_arch = "wasm32"))]
const FB_PIXELS: usize = SCREEN_W * SCREEN_H;

/// Single-threaded Wasm guest state container.
///
/// On `wasm32`, the 3.52 MiB framebuffer resides at a fixed linear memory offset
/// (`0x100000` = 1 MiB) and we grow linear memory to 8 MiB on `app_init` so the
/// `.wasm` binary stays ~15 KB with zero static data segments for pixels.
struct GuestMemory {
    #[cfg(not(target_arch = "wasm32"))]
    pixels: UnsafeCell<core::mem::MaybeUninit<[u32; FB_PIXELS]>>,
    wm: UnsafeCell<WmState>,
}

unsafe impl Sync for GuestMemory {}

static GUEST: GuestMemory = GuestMemory {
    #[cfg(not(target_arch = "wasm32"))]
    pixels: UnsafeCell::new(core::mem::MaybeUninit::uninit()),
    wm: UnsafeCell::new(WmState::new()),
};

#[cfg(target_arch = "wasm32")]
extern "C" {
    static __heap_base: u8;
}

#[cfg(target_arch = "wasm32")]
const WASM_MIN_PAGES: usize = 128; // 128 * 64 KiB = 8 MiB reserved linear memory

#[cfg(target_arch = "wasm32")]
#[inline]
unsafe fn fb_base_ptr() -> *mut u32 {
    let cur = core::arch::wasm32::memory_size(0);
    if cur < WASM_MIN_PAGES {
        let _ = core::arch::wasm32::memory_grow(0, WASM_MIN_PAGES - cur);
    }
    let base = (&raw const __heap_base as usize + 15) & !15;
    base as *mut u32
}

#[cfg(not(target_arch = "wasm32"))]
#[inline]
unsafe fn fb_base_ptr() -> *mut u32 {
    (*GUEST.pixels.get()).as_mut_ptr() as *mut u32
}

#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

fn with_state<R>(f: impl FnOnce(&mut WmState, &mut Framebuffer) -> R) -> R {
    unsafe {
        let wm = &mut *GUEST.wm.get();
        let px_ptr = fb_base_ptr();
        let w = wm.width.clamp(1, SCREEN_W as i32);
        let h = wm.height.clamp(1, SCREEN_H as i32);
        let px = core::slice::from_raw_parts_mut(px_ptr, (w as usize) * (h as usize));
        let mut fb = Framebuffer::new(px, w, h);
        f(wm, &mut fb)
    }
}

#[no_mangle]
pub extern "C" fn app_init(width: i32, height: i32) -> i32 {
    let w = if width <= 0 || width > SCREEN_W as i32 {
        SCREEN_W as i32
    } else {
        width
    };
    let h = if height <= 0 || height > SCREEN_H as i32 {
        SCREEN_H as i32
    } else {
        height
    };
    with_state(|wm, fb| {
        wm.init(w, h, wm.session_id);
        wm.render(fb);
        1
    })
}

#[no_mangle]
pub extern "C" fn app_set_session(session_id: u32) {
    with_state(|wm, fb| {
        wm.session_id = if session_id > 0 { session_id } else { 1 };
        if wm.ready {
            wm.render(fb);
        }
    });
}

#[no_mangle]
pub extern "C" fn app_render() {
    with_state(|wm, fb| {
        wm.render(fb);
    });
}

/// Returns:
/// - `0`: no change
/// - `1`: screen changed
/// - `2`: screen changed AND a snapshot capture was requested via `Testing -> Snapshot`
#[no_mangle]
pub extern "C" fn app_click(x: i32, y: i32) -> i32 {
    with_state(|wm, fb| {
        let rc = wm.click(x, y);
        if rc != 0 {
            wm.render(fb);
        }
        rc
    })
}

#[no_mangle]
pub extern "C" fn app_key(key: i32) -> i32 {
    with_state(|wm, fb| {
        let rc = wm.key(key);
        if rc != 0 {
            wm.render(fb);
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
pub extern "C" fn app_tick() -> i32 {
    with_state(|wm, fb| {
        if !wm.ready || wm.test_pattern {
            return 0;
        }
        wm.render(fb);
        1
    })
}

#[no_mangle]
pub extern "C" fn app_record_snapshot() -> u32 {
    with_state(|wm, fb| {
        wm.snapshot_seq = wm.snapshot_seq.wrapping_add(1);
        wm.about.snapshots_taken = wm.snapshot_seq;
        wm.render(fb);
        wm.last_fb_hash
    })
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
            AppSlot::About | AppSlot::Calc => 0,
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
    2 // 2 bundled apps: About, Calc
}

#[no_mangle]
pub extern "C" fn app_set_test_pattern(enabled: i32) {
    with_state(|wm, fb| {
        wm.test_pattern = enabled != 0;
        wm.render(fb);
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
        dock_icon_y, CLICK_CHANGED, CLICK_SNAPSHOT, DOCK_ICON_H, DOCK_ICON_W, DOCK_X, DOCK_Y,
        TITLE_H,
    };

    #[test]
    fn step1_pipeline_calc_and_test_pattern_contracts() {
        app_set_session(1);
        app_set_date(2026, 10, 30, 5);
        app_set_time(6, 45, 0, 0);
        assert_eq!(app_init(1280, 720), 1);

        // 1. Starts on empty Desktop with Dock hidden
        assert_eq!(app_selected(), AppSlot::Desktop as i32);
        assert_eq!(app_dock_visible(), 0);
        let desktop_hash = app_fb_hash();
        assert_ne!(desktop_hash, 0);

        // 2. Click top-left Title Bar button (`(16, 16)`) to show Left App Dock
        assert_eq!(app_click(16, 16), CLICK_CHANGED);
        assert_eq!(app_dock_visible(), 1);
        let dock_open_hash = app_fb_hash();
        assert_ne!(dock_open_hash, desktop_hash);

        // 3. Click the 128x72 About App Icon (slot 0) -> launches About & auto-hides Dock
        let icon_x = DOCK_X + DOCK_ICON_W / 2;
        assert_eq!(app_click(icon_x, DOCK_Y + DOCK_ICON_H / 2), CLICK_CHANGED);
        assert_eq!(app_selected(), AppSlot::About as i32);
        assert_eq!(app_dock_visible(), 0);
        let about_hash = app_fb_hash();
        assert_ne!(about_hash, desktop_hash);

        // 4. Open Dock again and click Calculator App Icon (slot 1) -> launches Calc & auto-hides Dock
        assert_eq!(app_click(16, 16), CLICK_CHANGED);
        assert_eq!(app_dock_visible(), 1);
        assert_eq!(
            app_click(icon_x, dock_icon_y(1) + DOCK_ICON_H / 2),
            CLICK_CHANGED
        );
        assert_eq!(app_selected(), AppSlot::Calc as i32);
        assert_eq!(app_dock_visible(), 0);
        let calc_zero_hash = app_fb_hash();
        assert_ne!(calc_zero_hash, about_hash);

        // 5. Exercise Calculator arithmetic (`12 + 30 = 42`) via key codes
        assert_eq!(app_key(11), CLICK_CHANGED); // '1'
        assert_eq!(app_key(12), CLICK_CHANGED); // '2'
        assert_eq!(app_key(20), CLICK_CHANGED); // '+'
        assert_eq!(app_key(13), CLICK_CHANGED); // '3'
        assert_eq!(app_key(10), CLICK_CHANGED); // '0'
        assert_eq!(app_key(24), CLICK_CHANGED); // '='
        let mut disp = [0u8; 24];
        let text = unsafe { (*GUEST.wm.get()).calc.format_display(&mut disp) };
        assert_eq!(text, "42");
        assert_ne!(app_fb_hash(), calc_zero_hash);

        // Clear via `CALC -> CLEAR (AC)` menu item
        let calc_menu_x = wm::MENU_X0 + 20;
        assert_eq!(app_click(calc_menu_x, 16), CLICK_CHANGED);
        assert_eq!(app_click(calc_menu_x, TITLE_H + 16), CLICK_CHANGED);
        let text_cleared = unsafe { (*GUEST.wm.get()).calc.format_display(&mut disp) };
        assert_eq!(text_cleared, "0");
        assert_eq!(app_fb_hash(), calc_zero_hash);

        // 6. Open "Testing" menu (menu index 2) in Title Bar and click "SNAPSHOT"
        let testing_menu_x = wm::MENU_X0 + 2 * wm::MENU_HEADER_W + 20;
        assert_eq!(app_click(testing_menu_x, 16), CLICK_CHANGED);
        assert_eq!(app_click(testing_menu_x, TITLE_H + 16), CLICK_SNAPSHOT);
        assert_eq!(app_snapshot_count(), 1);

        // 7. Verify diagnostic test pattern channel & orientation contract
        app_set_test_pattern(1);
        let ptr = app_framebuffer();
        assert!(!ptr.is_null());

        let px = unsafe { core::slice::from_raw_parts(ptr as *const u32, FB_PIXELS) };
        assert_eq!(px[8 * 1280 + 8], rgb(0, 0, 0));
        assert_eq!(px[50 * 1280 + 100], rgb(255, 0, 0));
        assert_eq!(unsafe { *ptr.add((50 * 1280 + 100) * 4) }, 255);
        assert_eq!(px[50 * 1280 + 900], rgb(0, 255, 0));
        assert_eq!(px[500 * 1280 + 100], rgb(0, 0, 255));
        assert_eq!(px[(720 - 20) * 1280 + (1280 - 20)], rgb(255, 0, 255));
    }
}
