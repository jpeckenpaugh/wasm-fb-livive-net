//! Minimalist Appliance Window Manager (`wm.rs`).
//!
//! 1. 32px Title Bar (`y: 0..32`):
//!    - Top-left App Dock button (`x: 0..44`)
//!    - Context menus (`SYSTEM`/`CALC`/`FILES`, `VIEW`, `TESTING`)
//!    - Right-aligned User, Session, and Date/Time (`TESTER · VM-01 · 2026-10-30 07:12:00`)
//! 2. Left Overlay App Dock:
//!    - `128x72` App Icons (1/10th of `1280x720`) stacked vertically on the left edge
//!      (`SYSTEM`, `CALC`, `FILES`), rendered on top of the active window without displacing it,
//!      auto-hiding when clicked.
//! 3. Maximized Client Window (`1280 x 688`):
//!    - Flat surface directly below the Title Bar.

use crate::calc::{self, CalcApp};
use crate::fb::{rgb, Framebuffer};
use crate::files::{self, FilesApp};
use crate::font::draw_str;
use crate::mines::{self, MinesApp};
use crate::system::{self, SystemApp, SystemSection};

pub const TITLE_H: i32 = 32;
pub const DOCK_BTN_W: i32 = 44;

// Left App Dock geometry: 128x72 icons (1/10th of 1280x720) stacked vertically
pub const DOCK_ICON_W: i32 = 128;
pub const DOCK_ICON_H: i32 = 72;
pub const DOCK_GAP: i32 = 10;
pub const DOCK_X: i32 = 12;
pub const DOCK_Y: i32 = TITLE_H + 10;
pub const APP_COUNT: usize = 4; // 0 = System, 1 = Calc, 2 = Files, 3 = Mines

#[inline]
pub const fn dock_icon_y(slot_idx: usize) -> i32 {
    DOCK_Y + (slot_idx as i32) * (DOCK_ICON_H + DOCK_GAP)
}

// Menu Bar geometry
pub const MENU_COUNT: usize = 3;
pub const MENU_X0: i32 = DOCK_BTN_W + 4;
pub const MENU_HEADER_W: i32 = 104;
pub const MENU_DROP_W: i32 = 224;
pub const MENU_ITEM_H: i32 = 30;

#[inline]
pub const fn menu_header_w(fb_w: i32) -> i32 {
    if fb_w < 540 {
        72
    } else if fb_w < 720 {
        88
    } else {
        MENU_HEADER_W
    }
}

// Click return codes
pub const CLICK_NOOP: i32 = 0;
pub const CLICK_CHANGED: i32 = 1;
pub const CLICK_SNAPSHOT: i32 = 2;
pub const CLICK_TTS: i32 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppSlot {
    Desktop = 0,
    System = 1,
    Calc = 2,
    Files = 3,
    Mines = 4,
}

pub struct WmState {
    pub ready: bool,
    pub width: i32,
    pub height: i32,
    pub session_id: u32,
    pub active_app: AppSlot,
    pub dock_visible: bool,
    pub open_menu: Option<usize>,
    pub test_pattern: bool,
    pub snapshot_seq: u32,
    pub last_fb_hash: u32,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub system: SystemApp,
    pub calc: CalcApp,
    pub files: FilesApp,
    pub mines: MinesApp,
}

impl WmState {
    pub const fn new() -> Self {
        Self {
            ready: false,
            width: 1280,
            height: 720,
            session_id: 1,
            active_app: AppSlot::Desktop,
            dock_visible: false,
            open_menu: None,
            test_pattern: false,
            snapshot_seq: 0,
            last_fb_hash: 0,
            hour: 6,
            minute: 45,
            second: 0,
            year: 2026,
            month: 10,
            day: 30,
            system: SystemApp::new(),
            calc: CalcApp::new(),
            files: FilesApp::new(),
            mines: MinesApp::new_const(),
        }
    }

    pub fn init(&mut self, w: i32, h: i32, session_id: u32) {
        self.width = if w > 0 { w } else { 1280 };
        self.height = if h > 0 { h } else { 720 };
        self.session_id = if session_id > 0 { session_id } else { 1 };
        self.active_app = AppSlot::Desktop;
        self.dock_visible = false;
        self.open_menu = None;
        self.test_pattern = false;
        self.snapshot_seq = 0;
        self.system = SystemApp::new();
        self.calc = CalcApp::new();
        self.files = FilesApp::new();
        self.mines.init_default();
        self.ready = true;
    }

    pub fn menu_title(&self, menu_idx: usize) -> &'static str {
        match menu_idx {
            0 => match self.active_app {
                AppSlot::Desktop | AppSlot::System => "SYSTEM",
                AppSlot::Calc => "CALC",
                AppSlot::Files => "FILES",
                AppSlot::Mines => "MINES",
            },
            1 => "VIEW",
            2 => "TESTING",
            _ => "",
        }
    }

    pub fn menu_items(&self, menu_idx: usize) -> &'static [&'static str] {
        match menu_idx {
            0 => match self.active_app {
                AppSlot::Desktop => &["ABOUT", "LOCALE", "FONTS"],
                AppSlot::System => &["ABOUT", "LOCALE", "FONTS", "CLOSE WINDOW"],
                AppSlot::Calc => &["CLEAR (AC)", "CLOSE WINDOW"],
                AppSlot::Files => &[
                    "HOME (~/)",
                    "GITHUB (GH://)",
                    "TOGGLE CONTEXT",
                    "SET AS WALLPAPER",
                    "CLOSE WINDOW",
                ],
                AppSlot::Mines => &[
                    "NEW SEED (N)",
                    "REPLAY SEED (R)",
                    "TOGGLE FLAG (F)",
                    "TOGGLE SHIELD RULE",
                    "CLOSE WINDOW",
                ],
            },
            1 => &["TOGGLE DOCK", "TEST PATTERN"],
            2 => &["SNAPSHOT"],
            _ => &[],
        }
    }

    pub fn render(
        &mut self,
        fb: &mut Framebuffer,
        staged_px: &[u32],
        staged_text: &[u8],
        staged_context: &[u8],
    ) {
        if !self.ready {
            return;
        }
        if self.test_pattern {
            draw_test_pattern(fb);
            self.last_fb_hash = fb.fnv1a_32();
            return;
        }

        // 1. Maximized Client Surface (`1280 x 688`, below the 32px Title Bar)
        {
            let client_h = (fb.h - TITLE_H).max(0);
            let mut client_fb = fb.sub_rows(TITLE_H, client_h);
            match self.active_app {
                AppSlot::Desktop => {
                    if self.files.wallpaper_active
                        && self.files.staged_idx == self.files.wallpaper_idx
                        && self.files.staged_w > 0
                        && self.files.staged_h > 0
                    {
                        client_fb.blit_scaled(
                            0,
                            0,
                            client_fb.w,
                            client_fb.h,
                            staged_px,
                            self.files.staged_w,
                            self.files.staged_h,
                        );
                    } else {
                        system::draw_desktop_backdrop(&mut client_fb);
                    }
                }
                AppSlot::System => {
                    system::draw_window(&mut client_fb, &self.system, self.width, self.height)
                }
                AppSlot::Calc => calc::draw_window(&mut client_fb, &self.calc),
                AppSlot::Files => {
                    files::draw_window(
                        &mut client_fb,
                        &self.files,
                        staged_px,
                        staged_text,
                        staged_context,
                    )
                }
                AppSlot::Mines => mines::draw_window(&mut client_fb, &self.mines),
            }
        }

        // 2. Left Overlay App Dock (`128x72` icons stacked on the left edge)
        if self.dock_visible {
            self.draw_left_dock(fb);
        }

        // 3. Top Title Bar (`y: 0..32`)
        self.draw_title_bar(fb);

        // 4. Active Drop-Down Menu
        if let Some(menu_idx) = self.open_menu {
            self.draw_dropdown_menu(fb, menu_idx);
        }

        self.last_fb_hash = fb.fnv1a_32();
    }

    fn draw_title_bar(&self, fb: &mut Framebuffer) {
        fb.fill(0, 0, fb.w, TITLE_H, rgb(0x0c, 0x11, 0x17));
        fb.fill(0, TITLE_H - 1, fb.w, 1, rgb(0x22, 0x30, 0x3e));

        // 1. Top-Left App Dock Button (`x: 0..DOCK_BTN_W`)
        let btn_bg = if self.dock_visible {
            rgb(0x2d, 0xd4, 0xbf)
        } else {
            rgb(0x14, 0x1d, 0x26)
        };
        let icon_col = if self.dock_visible {
            rgb(0x08, 0x10, 0x14)
        } else {
            rgb(0x2d, 0xd4, 0xbf)
        };
        fb.fill(0, 0, DOCK_BTN_W, TITLE_H - 1, btn_bg);
        fb.fill(DOCK_BTN_W - 1, 0, 1, TITLE_H - 1, rgb(0x22, 0x30, 0x3e));

        for row in 0..3 {
            let py = 9 + row * 5;
            fb.fill(12, py, 6, 3, icon_col);
            fb.fill(20, py, 12, 3, icon_col);
        }

        // 2. Menu Headers
        let mhw = menu_header_w(fb.w);
        let menu_sc = if fb.w < 540 { 1 } else { 2 };
        let menu_ty = if menu_sc == 1 { 12 } else { 9 };
        for i in 0..MENU_COUNT {
            let mx = MENU_X0 + (i as i32) * mhw;
            let is_open = self.open_menu == Some(i);
            if is_open {
                fb.fill(mx, 2, mhw - 4, TITLE_H - 3, rgb(0x1b, 0x28, 0x36));
                fb.fill(mx, TITLE_H - 3, mhw - 4, 2, rgb(0x2d, 0xd4, 0xbf));
            }
            let fg = if is_open {
                rgb(0xf0, 0xf6, 0xfc)
            } else {
                rgb(0xc8, 0xd7, 0xe2)
            };
            draw_str(fb, mx + 8, menu_ty, self.menu_title(i), menu_sc, fg);
        }

        // 3. Right Side: User, Session, and Full Timestamp shown ONCE
        let mut right_buf = [b' '; 48];
        let right_str = fmt_title_right(
            &mut right_buf,
            self.session_id,
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            fb.w < 680,
        );
        let right_sc = if fb.w < 540 { 1 } else { 2 };
        let right_ty = if right_sc == 1 { 12 } else { 9 };
        let rw = crate::font::text_width(right_str, right_sc);
        draw_str(fb, fb.w - rw - 10, right_ty, right_str, right_sc, rgb(0x9f, 0xb3, 0xc8));
    }

    fn draw_left_dock(&self, fb: &mut Framebuffer) {
        // Tile 0: System (About | Locale | Fonts)
        let y0 = dock_icon_y(0);
        fb.shade(DOCK_X - 4, y0 - 4, DOCK_ICON_W + 8, DOCK_ICON_H + 8, 64);
        system::draw_dock_tile(
            fb,
            &self.system,
            DOCK_X,
            y0,
            DOCK_ICON_W,
            DOCK_ICON_H,
            self.active_app == AppSlot::System,
        );

        // Tile 1: Calculator (shows live value)
        let y1 = dock_icon_y(1);
        fb.shade(DOCK_X - 4, y1 - 4, DOCK_ICON_W + 8, DOCK_ICON_H + 8, 64);
        calc::draw_dock_tile(
            fb,
            &self.calc,
            DOCK_X,
            y1,
            DOCK_ICON_W,
            DOCK_ICON_H,
            self.active_app == AppSlot::Calc,
        );

        // Tile 2: Files (`~/` network mount)
        let y2 = dock_icon_y(2);
        fb.shade(DOCK_X - 4, y2 - 4, DOCK_ICON_W + 8, DOCK_ICON_H + 8, 64);
        files::draw_dock_tile(
            fb,
            &self.files,
            DOCK_X,
            y2,
            DOCK_ICON_W,
            DOCK_ICON_H,
            self.active_app == AppSlot::Files,
        );

        // Tile 3: Mines (`mines-sm` 8x8 state machine)
        let y3 = dock_icon_y(3);
        fb.shade(DOCK_X - 4, y3 - 4, DOCK_ICON_W + 8, DOCK_ICON_H + 8, 64);
        mines::draw_dock_tile(
            fb,
            &self.mines,
            DOCK_X,
            y3,
            DOCK_ICON_W,
            DOCK_ICON_H,
            self.active_app == AppSlot::Mines,
        );
    }

    fn draw_dropdown_menu(&self, fb: &mut Framebuffer, menu_idx: usize) {
        let items = self.menu_items(menu_idx);
        if items.is_empty() {
            return;
        }
        let mhw = menu_header_w(fb.w);
        let drop_w = MENU_DROP_W.min(fb.w - 16);
        let mx = (MENU_X0 + (menu_idx as i32) * mhw).min(fb.w - drop_w - 4);
        let my = TITLE_H;
        let mh = (items.len() as i32) * MENU_ITEM_H + 8;

        fb.shade(mx + 4, my + 4, drop_w, mh, 80);
        fb.fill(mx, my, drop_w, mh, rgb(0x10, 0x17, 0x20));
        fb.border(mx, my, drop_w, mh, 2, rgb(0x2d, 0xd4, 0xbf));

        for (idx, &label) in items.iter().enumerate() {
            let iy = my + 4 + (idx as i32) * MENU_ITEM_H;
            if idx > 0 {
                fb.fill(mx + 8, iy, drop_w - 16, 1, rgb(0x1f, 0x2c, 0x38));
            }
            draw_str(fb, mx + 14, iy + 8, label, 2, rgb(0xd4, 0xe2, 0xea));
        }
    }

    pub fn click(&mut self, x: i32, y: i32) -> i32 {
        if !self.ready {
            return CLICK_NOOP;
        }

        if self.test_pattern {
            self.test_pattern = false;
            return CLICK_CHANGED;
        }

        let mhw = menu_header_w(self.width);
        let drop_w = MENU_DROP_W.min(self.width - 16);

        // Layer 1: Active Drop-Down Menu
        if let Some(menu_idx) = self.open_menu {
            let items = self.menu_items(menu_idx);
            let mx = (MENU_X0 + (menu_idx as i32) * mhw).min(self.width - drop_w - 4);
            let my = TITLE_H;
            let mh = (items.len() as i32) * MENU_ITEM_H + 8;

            if x >= mx && x < mx + drop_w && y >= my && y < my + mh {
                let rel_y = y - (my + 4);
                if rel_y >= 0 {
                    let item_idx = (rel_y / MENU_ITEM_H) as usize;
                    if item_idx < items.len() {
                        self.open_menu = None;
                        return self.execute_menu_item(menu_idx, item_idx);
                    }
                }
            }

            if y >= 0 && y < TITLE_H {
                if x >= 0 && x < DOCK_BTN_W {
                    self.open_menu = None;
                    self.dock_visible = !self.dock_visible;
                    return CLICK_CHANGED;
                }
                for i in 0..MENU_COUNT {
                    let hx = MENU_X0 + (i as i32) * mhw;
                    if x >= hx && x < hx + mhw {
                        self.open_menu = if i == menu_idx { None } else { Some(i) };
                        return CLICK_CHANGED;
                    }
                }
            }

            self.open_menu = None;
            return CLICK_CHANGED;
        }

        // Layer 2: Title Bar (`y < TITLE_H`)
        if y >= 0 && y < TITLE_H {
            if x >= 0 && x < DOCK_BTN_W {
                self.dock_visible = !self.dock_visible;
                return CLICK_CHANGED;
            }
            for i in 0..MENU_COUNT {
                let hx = MENU_X0 + (i as i32) * mhw;
                if x >= hx && x < hx + mhw {
                    self.open_menu = Some(i);
                    return CLICK_CHANGED;
                }
            }
            return CLICK_NOOP;
        }

        // Layer 3: Left Overlay App Dock (`128x72` icons stacked vertically)
        if self.dock_visible && x >= DOCK_X && x < DOCK_X + DOCK_ICON_W {
            for slot in 0..APP_COUNT {
                let iy = dock_icon_y(slot);
                if y >= iy && y < iy + DOCK_ICON_H {
                    self.active_app = match slot {
                        0 => AppSlot::System,
                        1 => AppSlot::Calc,
                        2 => {
                            self.files.ensure_selected_staged();
                            AppSlot::Files
                        }
                        _ => AppSlot::Mines,
                    };
                    self.dock_visible = false;
                    return CLICK_CHANGED;
                }
            }
        }

        // Layer 4: Maximized Client Window (`y >= TITLE_H`)
        let cx = x;
        let cy = y - TITLE_H;
        let client_w = self.width;
        let client_h = (self.height - TITLE_H).max(1);
        match self.active_app {
            AppSlot::Calc => {
                if calc::click_window(&mut self.calc, cx, cy, client_w, client_h) {
                    CLICK_CHANGED
                } else {
                    CLICK_NOOP
                }
            }
            AppSlot::Files => {
                let rc = files::click_window(&mut self.files, cx, cy, client_w, client_h);
                if rc == 3 {
                    CLICK_TTS
                } else if rc != 0 {
                    CLICK_CHANGED
                } else {
                    CLICK_NOOP
                }
            }
            AppSlot::Mines => {
                if mines::click_window(&mut self.mines, cx, cy, client_w, client_h) {
                    CLICK_CHANGED
                } else {
                    CLICK_NOOP
                }
            }
            _ => CLICK_NOOP,
        }
    }

    fn execute_menu_item(&mut self, menu_idx: usize, item_idx: usize) -> i32 {
        match (menu_idx, item_idx) {
            (0, idx) => match (self.active_app, idx) {
                (AppSlot::Desktop | AppSlot::System, 0..=2) => {
                    self.active_app = AppSlot::System;
                    self.system.section = SystemSection::from_index(idx);
                    self.dock_visible = false;
                    CLICK_CHANGED
                }
                (AppSlot::System, 3) => {
                    self.active_app = AppSlot::Desktop;
                    CLICK_CHANGED
                }
                (AppSlot::Calc, 0) => {
                    self.calc.clear();
                    CLICK_CHANGED
                }
                (AppSlot::Calc, 1) => {
                    self.active_app = AppSlot::Desktop;
                    CLICK_CHANGED
                }
                (AppSlot::Files, 0) => {
                    self.files.open_subfolder(files::FolderLevel::HomeRoot);
                    CLICK_CHANGED
                }
                (AppSlot::Files, 1) => {
                    self.files.open_subfolder(files::FolderLevel::GithubRepo);
                    CLICK_CHANGED
                }
                (AppSlot::Files, 2) => {
                    self.files.context_mode = !self.files.context_mode;
                    CLICK_CHANGED
                }
                (AppSlot::Files, 3) => {
                    if let Some(sel) = self.files.active_file_idx() {
                        if self.files.entries[sel].is_image() {
                            self.files.wallpaper_active = true;
                            self.files.wallpaper_idx = sel as i32;
                            self.files.ensure_selected_staged();
                            self.active_app = AppSlot::Desktop;
                        }
                    }
                    CLICK_CHANGED
                }
                (AppSlot::Files, 4) => {
                    self.active_app = AppSlot::Desktop;
                    CLICK_CHANGED
                }
                (AppSlot::Mines, 0) => {
                    self.mines.next_seed();
                    CLICK_CHANGED
                }
                (AppSlot::Mines, 1) => {
                    self.mines.reset_seed(self.mines.board.seed);
                    CLICK_CHANGED
                }
                (AppSlot::Mines, 2) => {
                    self.mines.toggle_mode();
                    CLICK_CHANGED
                }
                (AppSlot::Mines, 3) => {
                    self.mines.toggle_rules();
                    CLICK_CHANGED
                }
                (AppSlot::Mines, 4) => {
                    self.active_app = AppSlot::Desktop;
                    CLICK_CHANGED
                }
                _ => CLICK_NOOP,
            },
            // Menu 1: VIEW
            (1, 0) => {
                self.dock_visible = !self.dock_visible;
                CLICK_CHANGED
            }
            (1, 1) => {
                self.test_pattern = !self.test_pattern;
                CLICK_CHANGED
            }
            // Menu 2: TESTING -> SNAPSHOT
            (2, 0) => {
                self.snapshot_seq = self.snapshot_seq.wrapping_add(1);
                self.system.snapshots_taken = self.snapshot_seq;
                CLICK_SNAPSHOT
            }
            _ => CLICK_NOOP,
        }
    }

    pub fn key(&mut self, code: i32) -> i32 {
        if !self.ready {
            return CLICK_NOOP;
        }
        if code == 25 {
            if self.open_menu.is_some() {
                self.open_menu = None;
                return CLICK_CHANGED;
            }
            if self.dock_visible {
                self.dock_visible = false;
                return CLICK_CHANGED;
            }
            if self.test_pattern {
                self.test_pattern = false;
                return CLICK_CHANGED;
            }
        }
        if code == 3 {
            self.dock_visible = !self.dock_visible;
            return CLICK_CHANGED;
        }

        match self.active_app {
            AppSlot::System => match code {
                11..=13 => {
                    let target = SystemSection::from_index((code - 11) as usize);
                    if self.system.section != target {
                        self.system.section = target;
                        return CLICK_CHANGED;
                    }
                    CLICK_NOOP
                }
                1 | 2 => {
                    let cur = self.system.section as usize;
                    let next = if code == 1 {
                        (cur + 2) % 3
                    } else {
                        (cur + 1) % 3
                    };
                    self.system.section = SystemSection::from_index(next);
                    CLICK_CHANGED
                }
                _ => CLICK_NOOP,
            },
            AppSlot::Calc => {
                if calc::key_window(&mut self.calc, code) {
                    CLICK_CHANGED
                } else {
                    CLICK_NOOP
                }
            }
            AppSlot::Files => {
                if files::key_window(&mut self.files, code) {
                    CLICK_CHANGED
                } else {
                    CLICK_NOOP
                }
            }
            AppSlot::Mines => {
                if mines::key_window(&mut self.mines, code) {
                    CLICK_CHANGED
                } else {
                    CLICK_NOOP
                }
            }
            AppSlot::Desktop => CLICK_NOOP,
        }
    }
}

pub fn draw_test_pattern(fb: &mut Framebuffer) {
    let half_w = fb.w / 2;
    let half_h = fb.h / 2;

    fb.fill(0, 0, half_w, half_h, rgb(255, 0, 0));
    fb.fill(half_w, 0, fb.w - half_w, half_h, rgb(0, 255, 0));
    fb.fill(0, half_h, half_w, fb.h - half_h, rgb(0, 0, 255));
    fb.fill(
        half_w,
        half_h,
        fb.w - half_w,
        fb.h - half_h,
        rgb(255, 255, 255),
    );

    for i in 0..fb.w {
        fb.fill(i, (i * fb.h) / fb.w, 3, 3, rgb(255, 255, 0));
    }

    fb.fill(8, 8, 32, 32, rgb(0, 0, 0));
    fb.fill(fb.w - 40, fb.h - 40, 32, 32, rgb(255, 0, 255));
}

#[allow(clippy::too_many_arguments)]
fn fmt_title_right(
    buf: &mut [u8; 48],
    session_id: u32,
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    min: u8,
    sec: u8,
    compact: bool,
) -> &str {
    let mut pos = 0;
    let prefix: &[u8] = if compact { b"VM-" } else { b"TESTER  VM-" };
    for &b in prefix {
        buf[pos] = b;
        pos += 1;
    }
    buf[pos] = b'0' + ((session_id / 10) % 10) as u8;
    buf[pos + 1] = b'0' + (session_id % 10) as u8;
    pos += 2;
    buf[pos] = b' ';
    pos += 1;

    if !compact {
        buf[pos] = b' ';
        pos += 1;
        buf[pos] = b'0' + ((year / 1000) % 10) as u8;
        buf[pos + 1] = b'0' + ((year / 100) % 10) as u8;
        buf[pos + 2] = b'0' + ((year / 10) % 10) as u8;
        buf[pos + 3] = b'0' + (year % 10) as u8;
        buf[pos + 4] = b'-';
        buf[pos + 5] = b'0' + (month / 10) % 10;
        buf[pos + 6] = b'0' + (month % 10);
        buf[pos + 7] = b'-';
        buf[pos + 8] = b'0' + (day / 10) % 10;
        buf[pos + 9] = b'0' + (day % 10);
        buf[pos + 10] = b' ';
        pos += 11;
    }

    buf[pos] = b'0' + (hour / 10) % 10;
    buf[pos + 1] = b'0' + (hour % 10);
    buf[pos + 2] = b':';
    buf[pos + 3] = b'0' + (min / 10) % 10;
    buf[pos + 4] = b'0' + (min % 10);
    buf[pos + 5] = b':';
    buf[pos + 6] = b'0' + (sec / 10) % 10;
    buf[pos + 7] = b'0' + (sec % 10);
    pos += 8;

    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}
