//! Minimalist Appliance Window Manager (`wm.rs`).
//!
//! 1. 32px Title Bar (`y: 0..32`):
//!    - Top-left App Dock button (`x: 0..44`)
//!    - Context menus (`SYSTEM`/`CALC`/`FILES`/`MINES`, `VIEW`, `TESTING`)
//!    - Right-aligned user, session, and date/time (`TESTER · VM-01 · ...`)
//! 2. Left Overlay App Dock:
//!    - Four `128x72` tiles (1/10th scale), stacked vertically over the active
//!      surface without displacing it, auto-hiding after a launch.
//! 3. Maximized Client Window:
//!    - Flat surface directly below the title bar at the session resolution.
//!
//! All geometry lives in [`WmLayout`] so drawing and hit testing cannot drift.
//! Menu labels map to [`Command`] values rather than numeric item indices.

use crate::calc::{self, CalcApp};
use crate::fb::{rgb, Framebuffer};
use crate::files::{self, FilesApp};
use crate::fmt as f;
use crate::font::{draw_str_prop, text_width_prop};
use crate::icon::{draw_icon, Icon};
use crate::input::{InputResult, Key};
use crate::layout::{Metrics, Rect};
use crate::mines::{self, MinesApp};
use crate::system::{self, SystemApp, SystemSection};
use crate::theme as t;

pub const TITLE_H: i32 = 32;
pub const DOCK_BTN_W: i32 = 44;

// Left App Dock geometry: 128x72 icons (1/10th of 1280x720) stacked vertically.
pub const DOCK_ICON_W: i32 = 128;
pub const DOCK_ICON_H: i32 = 72;
pub const DOCK_GAP: i32 = 10;
pub const DOCK_X: i32 = 12;
pub const DOCK_Y: i32 = TITLE_H + 10;

// Menu Bar geometry.
pub const MENU_COUNT: usize = 3;
pub const MENU_X0: i32 = DOCK_BTN_W + 4;
pub const MENU_HEADER_W: i32 = 104;
pub const MENU_DROP_W: i32 = 224;
pub const MENU_ITEM_H: i32 = 30;

/// ABI-level input result codes. Kept numeric for `host.js`; internal code uses
/// [`InputResult`].
pub const CLICK_NOOP: i32 = InputResult::Noop as i32;
pub const CLICK_CHANGED: i32 = InputResult::Changed as i32;
pub const CLICK_SNAPSHOT: i32 = InputResult::Snapshot as i32;
pub const CLICK_TTS: i32 = InputResult::Tts as i32;

#[inline]
pub const fn dock_icon_y(slot_idx: usize) -> i32 {
    DOCK_Y + (slot_idx as i32) * (DOCK_ICON_H + DOCK_GAP)
}

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

/// App surfaces. `Desktop` is a real surface (wallpaper or backdrop), not a
/// null state; the four dock apps are the `APP_COUNT` launched surfaces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppSlot {
    Desktop = 0,
    System = 1,
    Calc = 2,
    Files = 3,
    Mines = 4,
}

impl AppSlot {
    /// Number of docked applications (excludes the desktop surface).
    pub const APP_COUNT: usize = 4;
    pub const DOCK_APPS: [AppSlot; 4] = [
        AppSlot::System,
        AppSlot::Calc,
        AppSlot::Files,
        AppSlot::Mines,
    ];

    pub const fn dock_index(self) -> Option<usize> {
        match self {
            AppSlot::System => Some(0),
            AppSlot::Calc => Some(1),
            AppSlot::Files => Some(2),
            AppSlot::Mines => Some(3),
            AppSlot::Desktop => None,
        }
    }

    pub const fn from_dock_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(AppSlot::System),
            1 => Some(AppSlot::Calc),
            2 => Some(AppSlot::Files),
            3 => Some(AppSlot::Mines),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    About,
    Locale,
    Fonts,
    Icons,
    CloseWindow,
    CalcClear,
    FilesHome,
    FilesGithub,
    FilesToggleContext,
    FilesSetWallpaper,
    MinesNewSeed,
    MinesReplay,
    MinesToggleFlag,
    MinesToggleRules,
    ToggleDock,
    TestPattern,
    Snapshot,
}

#[derive(Clone, Copy)]
pub struct MenuItem {
    pub label: &'static str,
    pub command: Command,
}

const MENU_DESKTOP: &[MenuItem] = &[
    MenuItem {
        label: "ABOUT",
        command: Command::About,
    },
    MenuItem {
        label: "LOCALE",
        command: Command::Locale,
    },
    MenuItem {
        label: "FONTS",
        command: Command::Fonts,
    },
    MenuItem {
        label: "ICONS",
        command: Command::Icons,
    },
];
const MENU_SYSTEM: &[MenuItem] = &[
    MenuItem {
        label: "ABOUT",
        command: Command::About,
    },
    MenuItem {
        label: "LOCALE",
        command: Command::Locale,
    },
    MenuItem {
        label: "FONTS",
        command: Command::Fonts,
    },
    MenuItem {
        label: "ICONS",
        command: Command::Icons,
    },
    MenuItem {
        label: "CLOSE WINDOW",
        command: Command::CloseWindow,
    },
];
const MENU_CALC: &[MenuItem] = &[
    MenuItem {
        label: "CLEAR (AC)",
        command: Command::CalcClear,
    },
    MenuItem {
        label: "CLOSE WINDOW",
        command: Command::CloseWindow,
    },
];
const MENU_FILES: &[MenuItem] = &[
    MenuItem {
        label: "HOME (~/)",
        command: Command::FilesHome,
    },
    MenuItem {
        label: "GITHUB (GH://)",
        command: Command::FilesGithub,
    },
    MenuItem {
        label: "TOGGLE CONTEXT",
        command: Command::FilesToggleContext,
    },
    MenuItem {
        label: "SET AS WALLPAPER",
        command: Command::FilesSetWallpaper,
    },
    MenuItem {
        label: "CLOSE WINDOW",
        command: Command::CloseWindow,
    },
];
const MENU_MINES: &[MenuItem] = &[
    MenuItem {
        label: "NEW SEED (N)",
        command: Command::MinesNewSeed,
    },
    MenuItem {
        label: "REPLAY SEED (R)",
        command: Command::MinesReplay,
    },
    MenuItem {
        label: "TOGGLE FLAG (F)",
        command: Command::MinesToggleFlag,
    },
    MenuItem {
        label: "TOGGLE SHIELD RULE",
        command: Command::MinesToggleRules,
    },
    MenuItem {
        label: "CLOSE WINDOW",
        command: Command::CloseWindow,
    },
];
const MENU_VIEW: &[MenuItem] = &[
    MenuItem {
        label: "TOGGLE DOCK",
        command: Command::ToggleDock,
    },
    MenuItem {
        label: "TEST PATTERN",
        command: Command::TestPattern,
    },
];
const MENU_TESTING: &[MenuItem] = &[MenuItem {
    label: "SNAPSHOT",
    command: Command::Snapshot,
}];

/// Title-bar / dropdown geometry derived once per frame from the session width.
#[derive(Clone, Copy, Debug)]
pub struct WmLayout {
    pub header_w: i32,
    pub menu_scale: i32,
    pub menu_text_y: i32,
    pub right_scale: i32,
    pub right_text_y: i32,
    pub right_compact: bool,
}

impl WmLayout {
    pub fn new(width: i32) -> Self {
        let m = Metrics::new(width, 720);
        let menu_scale = if m.is_compact() { 1 } else { 2 };
        Self {
            header_w: menu_header_w(width),
            menu_scale,
            menu_text_y: if menu_scale == 1 { 12 } else { 9 },
            right_scale: menu_scale,
            right_text_y: if menu_scale == 1 { 12 } else { 9 },
            right_compact: width < 680,
        }
    }

    pub const fn header_x(&self, idx: usize) -> i32 {
        MENU_X0 + (idx as i32) * self.header_w
    }

    pub fn header_rect(&self, idx: usize) -> Rect {
        Rect::new(self.header_x(idx), 0, self.header_w, TITLE_H)
    }

    pub fn dropdown_rect(&self, idx: usize, item_count: usize, fb_w: i32) -> Rect {
        let w = MENU_DROP_W.min(fb_w - 16);
        let x = self.header_rect(idx).x.min(fb_w - w - 4).max(0);
        let h = (item_count as i32) * MENU_ITEM_H + 8;
        Rect::new(x, TITLE_H, w, h)
    }
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

    pub fn menu_items(&self, menu_idx: usize) -> &'static [MenuItem] {
        match menu_idx {
            0 => match self.active_app {
                AppSlot::Desktop => MENU_DESKTOP,
                AppSlot::System => MENU_SYSTEM,
                AppSlot::Calc => MENU_CALC,
                AppSlot::Files => MENU_FILES,
                AppSlot::Mines => MENU_MINES,
            },
            1 => MENU_VIEW,
            2 => MENU_TESTING,
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
            return;
        }

        // 1. Maximized client surface below the title bar.
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
                AppSlot::Files => files::draw_window(
                    &mut client_fb,
                    &self.files,
                    staged_px,
                    staged_text,
                    staged_context,
                ),
                AppSlot::Mines => mines::draw_window(&mut client_fb, &self.mines),
            }
        }

        // 2. Left overlay dock.
        if self.dock_visible {
            self.draw_left_dock(fb);
        }

        // 3. Title bar.
        self.draw_title_bar(fb);

        // 4. Open dropdown.
        if let Some(menu_idx) = self.open_menu {
            self.draw_dropdown_menu(fb, menu_idx);
        }
    }

    fn draw_title_bar(&self, fb: &mut Framebuffer) {
        let layout = WmLayout::new(fb.w);
        fb.fill(0, 0, fb.w, TITLE_H, t::TITLE_BG);
        fb.fill(0, TITLE_H - 1, fb.w, 1, t::BORDER);

        // Top-left dock button.
        let btn_bg = if self.dock_visible {
            t::ACCENT
        } else {
            t::SURFACE_RAISED
        };
        let icon_col = if self.dock_visible {
            t::OVERLAY
        } else {
            t::ACCENT
        };
        fb.fill(0, 0, DOCK_BTN_W, TITLE_H - 1, btn_bg);
        fb.fill(DOCK_BTN_W - 1, 0, 1, TITLE_H - 1, t::BORDER);
        draw_icon(
            fb,
            (DOCK_BTN_W - 20) / 2,
            (TITLE_H - 1 - 20) / 2,
            Icon::Menu,
            20,
            icon_col,
        );

        // Menu headers.
        for i in 0..MENU_COUNT {
            let r = layout.header_rect(i);
            let is_open = self.open_menu == Some(i);
            if is_open {
                fb.fill(r.x, 2, r.w - 4, TITLE_H - 3, t::SURFACE_HOVER);
                fb.fill(r.x, TITLE_H - 3, r.w - 4, 2, t::ACCENT);
            }
            let fg = if is_open { t::TEXT } else { t::TEXT_SOFT };
            draw_str_prop(
                fb,
                r.x + 10,
                layout.menu_text_y,
                self.menu_title(i),
                layout.menu_scale,
                fg,
            );
        }

        // Right side: one formatted session/date/time string.
        let mut right_buf = [b' '; 64];
        let right_str = f::title_right(
            &mut right_buf,
            self.session_id,
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
            layout.right_compact,
        );
        let rw = text_width_prop(right_str, layout.right_scale);
        draw_str_prop(
            fb,
            fb.w - rw - 10,
            layout.right_text_y,
            right_str,
            layout.right_scale,
            t::TEXT_MUTED,
        );
    }

    fn draw_left_dock(&self, fb: &mut Framebuffer) {
        for (idx, &slot) in AppSlot::DOCK_APPS.iter().enumerate() {
            let y = dock_icon_y(idx);
            let active = self.active_app == slot;
            match slot {
                AppSlot::System => system::draw_dock_tile(
                    fb,
                    &self.system,
                    DOCK_X,
                    y,
                    DOCK_ICON_W,
                    DOCK_ICON_H,
                    active,
                ),
                AppSlot::Calc => calc::draw_dock_tile(
                    fb,
                    &self.calc,
                    DOCK_X,
                    y,
                    DOCK_ICON_W,
                    DOCK_ICON_H,
                    active,
                ),
                AppSlot::Files => files::draw_dock_tile(
                    fb,
                    &self.files,
                    DOCK_X,
                    y,
                    DOCK_ICON_W,
                    DOCK_ICON_H,
                    active,
                ),
                AppSlot::Mines => mines::draw_dock_tile(
                    fb,
                    &self.mines,
                    DOCK_X,
                    y,
                    DOCK_ICON_W,
                    DOCK_ICON_H,
                    active,
                ),
                AppSlot::Desktop => {}
            }
        }
    }

    fn draw_dropdown_menu(&self, fb: &mut Framebuffer, menu_idx: usize) {
        let items = self.menu_items(menu_idx);
        if items.is_empty() {
            return;
        }
        let layout = WmLayout::new(fb.w);
        let r = layout.dropdown_rect(menu_idx, items.len(), fb.w);

        fb.shade(r.x + 4, r.y + 4, r.w, r.h, 80);
        fb.fill(r.x, r.y, r.w, r.h, t::SURFACE);
        fb.border(r.x, r.y, r.w, r.h, 2, t::ACCENT);

        for (idx, item) in items.iter().enumerate() {
            let iy = r.y + 4 + (idx as i32) * MENU_ITEM_H;
            if idx > 0 {
                fb.fill(r.x + 8, iy, r.w - 16, 1, t::BORDER_SOFT);
            }
            draw_str_prop(fb, r.x + 14, iy + 8, item.label, 2, t::TEXT_MENU);
        }
    }

    pub fn click(&mut self, x: i32, y: i32) -> InputResult {
        if !self.ready {
            return InputResult::Noop;
        }

        if self.test_pattern {
            self.test_pattern = false;
            return InputResult::Changed;
        }

        let layout = WmLayout::new(self.width);

        // Layer 1: an open dropdown consumes the click.
        if let Some(menu_idx) = self.open_menu {
            let items = self.menu_items(menu_idx);
            let r = layout.dropdown_rect(menu_idx, items.len(), self.width);

            if r.contains(x, y) {
                let rel_y = y - (r.y + 4);
                if rel_y >= 0 {
                    let item_idx = (rel_y / MENU_ITEM_H) as usize;
                    if item_idx < items.len() {
                        self.open_menu = None;
                        return self.execute_command(items[item_idx].command);
                    }
                }
            }

            if (0..TITLE_H).contains(&y) {
                if (0..DOCK_BTN_W).contains(&x) {
                    self.open_menu = None;
                    self.dock_visible = !self.dock_visible;
                    return InputResult::Changed;
                }
                for i in 0..MENU_COUNT {
                    if layout.header_rect(i).contains(x, y) {
                        self.open_menu = if i == menu_idx { None } else { Some(i) };
                        return InputResult::Changed;
                    }
                }
            }

            self.open_menu = None;
            return InputResult::Changed;
        }

        // Layer 2: title bar.
        if (0..TITLE_H).contains(&y) {
            if (0..DOCK_BTN_W).contains(&x) {
                self.dock_visible = !self.dock_visible;
                return InputResult::Changed;
            }
            for i in 0..MENU_COUNT {
                if layout.header_rect(i).contains(x, y) {
                    self.open_menu = Some(i);
                    return InputResult::Changed;
                }
            }
            return InputResult::Noop;
        }

        // Layer 3: left dock.
        if self.dock_visible && (DOCK_X..DOCK_X + DOCK_ICON_W).contains(&x) {
            for (idx, &slot) in AppSlot::DOCK_APPS.iter().enumerate() {
                let r = Rect::new(DOCK_X, dock_icon_y(idx), DOCK_ICON_W, DOCK_ICON_H);
                if r.contains(x, y) {
                    if slot == AppSlot::Files {
                        self.files.ensure_selected_staged();
                    }
                    self.active_app = slot;
                    self.dock_visible = false;
                    return InputResult::Changed;
                }
            }
        }

        // Layer 4: client surface.
        let cx = x;
        let cy = y - TITLE_H;
        let client_w = self.width;
        let client_h = (self.height - TITLE_H).max(1);
        match self.active_app {
            AppSlot::Desktop => InputResult::Noop,
            AppSlot::System => system::click_window(&mut self.system, cx, cy, client_w, client_h),
            AppSlot::Calc => calc::click_window(&mut self.calc, cx, cy, client_w, client_h),
            AppSlot::Files => files::click_window(&mut self.files, cx, cy, client_w, client_h),
            AppSlot::Mines => mines::click_window(&mut self.mines, cx, cy, client_w, client_h),
        }
    }

    fn execute_command(&mut self, command: Command) -> InputResult {
        match command {
            Command::About => self.open_system(SystemSection::About),
            Command::Locale => self.open_system(SystemSection::Locale),
            Command::Fonts => self.open_system(SystemSection::Fonts),
            Command::Icons => self.open_system(SystemSection::Icons),
            Command::CloseWindow => {
                self.active_app = AppSlot::Desktop;
                InputResult::Changed
            }
            Command::CalcClear => {
                self.calc.clear();
                InputResult::Changed
            }
            Command::FilesHome => {
                self.files.open_subfolder(files::FolderLevel::HomeRoot);
                InputResult::Changed
            }
            Command::FilesGithub => {
                self.files.open_subfolder(files::FolderLevel::GithubRepo);
                InputResult::Changed
            }
            Command::FilesToggleContext => {
                self.files.context_mode = !self.files.context_mode;
                InputResult::Changed
            }
            Command::FilesSetWallpaper => {
                if let Some(sel) = self.files.active_file_idx() {
                    if self.files.entries[sel].is_image() {
                        self.files.wallpaper_active = true;
                        self.files.wallpaper_idx = sel as i32;
                        self.files.ensure_selected_staged();
                        self.active_app = AppSlot::Desktop;
                    }
                }
                InputResult::Changed
            }
            Command::MinesNewSeed => {
                self.mines.next_seed();
                InputResult::Changed
            }
            Command::MinesReplay => {
                self.mines.reset_seed(self.mines.board.seed);
                InputResult::Changed
            }
            Command::MinesToggleFlag => {
                self.mines.toggle_mode();
                InputResult::Changed
            }
            Command::MinesToggleRules => {
                self.mines.toggle_rules();
                InputResult::Changed
            }
            Command::ToggleDock => {
                self.dock_visible = !self.dock_visible;
                InputResult::Changed
            }
            Command::TestPattern => {
                self.test_pattern = !self.test_pattern;
                InputResult::Changed
            }
            Command::Snapshot => {
                self.snapshot_seq = self.snapshot_seq.wrapping_add(1);
                self.system.snapshots_taken = self.snapshot_seq;
                InputResult::Snapshot
            }
        }
    }

    fn open_system(&mut self, section: SystemSection) -> InputResult {
        self.active_app = AppSlot::System;
        self.system.section = section;
        self.dock_visible = false;
        InputResult::Changed
    }

    pub fn key(&mut self, key: Key) -> InputResult {
        if !self.ready {
            return InputResult::Noop;
        }
        if key == Key::Escape {
            if self.open_menu.is_some() {
                self.open_menu = None;
                return InputResult::Changed;
            }
            if self.dock_visible {
                self.dock_visible = false;
                return InputResult::Changed;
            }
            if self.test_pattern {
                self.test_pattern = false;
                return InputResult::Changed;
            }
        }
        if key == Key::Space {
            self.dock_visible = !self.dock_visible;
            return InputResult::Changed;
        }

        match self.active_app {
            AppSlot::System => self.key_system(key),
            AppSlot::Calc => calc::key_window(&mut self.calc, key),
            AppSlot::Files => files::key_window(&mut self.files, key),
            AppSlot::Mines => mines::key_window(&mut self.mines, key),
            AppSlot::Desktop => InputResult::Noop,
        }
    }

    fn key_system(&mut self, key: Key) -> InputResult {
        match key {
            Key::Digit1 | Key::Digit2 | Key::Digit3 | Key::Digit4 => {
                let idx = key as i32 - Key::Digit1 as i32;
                let target = SystemSection::from_index(idx as usize);
                if self.system.section != target {
                    self.system.section = target;
                    InputResult::Changed
                } else {
                    InputResult::Noop
                }
            }
            Key::Left | Key::Right => {
                let cur = self.system.section as usize;
                let next = if key == Key::Left {
                    (cur + SystemSection::COUNT - 1) % SystemSection::COUNT
                } else {
                    (cur + 1) % SystemSection::COUNT
                };
                self.system.section = SystemSection::from_index(next);
                InputResult::Changed
            }
            _ => InputResult::Noop,
        }
    }
}

impl Default for WmState {
    fn default() -> Self {
        Self::new()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_slots_round_trip() {
        assert_eq!(AppSlot::DOCK_APPS.len(), AppSlot::APP_COUNT);
        for (idx, slot) in AppSlot::DOCK_APPS.iter().enumerate() {
            assert_eq!(slot.dock_index(), Some(idx));
            assert_eq!(AppSlot::from_dock_index(idx), Some(*slot));
        }
        assert_eq!(AppSlot::Desktop.dock_index(), None);
        assert_eq!(AppSlot::from_dock_index(AppSlot::APP_COUNT), None);
    }

    #[test]
    fn menu_labels_map_to_commands() {
        let wm = WmState::new();
        for app in AppSlot::DOCK_APPS {
            let mut wm = WmState::new();
            wm.active_app = app;
            let items = wm.menu_items(0);
            assert!(!items.is_empty());
            for pair in items.windows(2) {
                assert_ne!(pair[0].label, pair[1].label);
            }
        }
        let mut files = WmState::new();
        files.active_app = AppSlot::Files;
        assert_eq!(files.menu_items(0)[2].command, Command::FilesToggleContext);
        assert_eq!(files.menu_items(0)[4].command, Command::CloseWindow);
        assert_eq!(wm.menu_items(1)[1].command, Command::TestPattern);
        assert_eq!(wm.menu_items(2)[0].command, Command::Snapshot);
    }

    #[test]
    fn commands_update_state() {
        let mut wm = WmState::new();
        wm.ready = true;
        assert_eq!(wm.execute_command(Command::Icons), InputResult::Changed);
        assert_eq!(wm.active_app, AppSlot::System);
        assert_eq!(wm.system.section, SystemSection::Icons);
        assert_eq!(wm.execute_command(Command::Snapshot), InputResult::Snapshot);
        assert_eq!(wm.snapshot_seq, 1);
        assert_eq!(wm.system.snapshots_taken, 1);
        assert_eq!(
            wm.execute_command(Command::CloseWindow),
            InputResult::Changed
        );
        assert_eq!(wm.active_app, AppSlot::Desktop);
    }

    #[test]
    fn layout_geometry_is_consistent() {
        let full = WmLayout::new(1280);
        let compact = WmLayout::new(390);
        assert_eq!(full.header_w, MENU_HEADER_W);
        assert_eq!(compact.header_w, 72);
        assert_eq!(full.menu_scale, 2);
        assert_eq!(compact.menu_scale, 1);
        assert!(!full.right_compact);
        assert!(compact.right_compact);

        for idx in 0..MENU_COUNT {
            let r = full.dropdown_rect(idx, 5, 1280);
            assert!(r.x >= 0 && r.right() <= 1280);
            assert!(r.contains(r.x + 1, r.y + 1));
        }
        let r = compact.dropdown_rect(2, 1, 390);
        assert!(r.x >= 0 && r.right() <= 390);
    }
}
