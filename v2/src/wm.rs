//! Minimalist Appliance Window Manager (`wm.rs`).
//!
//! 1. 32px Title Bar (`y: 0..32`):
//!    - App Home menu (`x: 0..44`) with its app drawer as the dropdown
//!    - Context menus (`System`/`Calc`/`Files`/`Mines`, `View`, `Testing`)
//!    - Right-aligned user, session, and date/time (`Tester · VM-01 · ...`)
//! 2. App Home Drawer:
//!    - Four `128x72` tiles (1/10th scale), drawn as the Home menu's dropdown
//!      below the Home header.
//! 3. Maximized Client Window:
//!    - Flat surface directly below the title bar at the session resolution.
//!
//! Mouse and keyboard share one [`Focus`] state. The Home drawer, menus, and
//! client controls are all reachable with either input source, and activation
//! always goes through the same command path.

use crate::calc::{self, CalcApp};
use crate::fb::{rgb, Framebuffer};
use crate::files::{self, FilesApp};
use crate::fmt as f;
use crate::font::{draw_str_prop, text_width_prop};
use crate::icon::{draw_icon, Icon};
use crate::input::{InputResult, Key, Mods};
use crate::layout::{Metrics, Rect};
use crate::mines::{self, MinesApp};
use crate::system::{self, SystemApp, SystemSection};
use crate::theme as t;
use crate::ui;

pub const TITLE_H: i32 = 32;
pub const HOME_BTN_W: i32 = 44;

// App Home drawer geometry: 128x72 icons (1/10th of 1280x720) stacked
// vertically and drawn as the Home menu's dropdown.
pub const DOCK_ICON_W: i32 = 128;
pub const DOCK_ICON_H: i32 = 72;
pub const DOCK_GAP: i32 = 10;
pub const DOCK_X: i32 = 12;
pub const DOCK_Y: i32 = TITLE_H + 10;

// Menu Bar geometry (Home + app + View + Testing).
pub const MENU_COUNT: usize = 4;
pub const MENU_X0: i32 = HOME_BTN_W + 4;
pub const MENU_HEADER_W: i32 = 104;
pub const MENU_DROP_W: i32 = 224;
pub const MENU_ITEM_H: i32 = 30;

/// ABI-level input result codes. Kept numeric for `host.js`; internal code uses
/// [`InputResult`].
pub const CLICK_NOOP: i32 = InputResult::Noop as i32;
pub const CLICK_CHANGED: i32 = InputResult::Changed as i32;
pub const CLICK_SNAPSHOT: i32 = InputResult::Snapshot as i32;
pub const CLICK_TTS: i32 = InputResult::Tts as i32;
pub const CLICK_HANDLED: i32 = InputResult::Handled as i32;

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
/// null state; the four drawer apps are the `APP_COUNT` launched surfaces.
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

    pub const fn title(self) -> &'static str {
        match self {
            AppSlot::Desktop => "Desktop",
            AppSlot::System => "System",
            AppSlot::Calc => "Calc",
            AppSlot::Files => "Files",
            AppSlot::Mines => "Mines",
        }
    }
}

/// Menu-bar entries. `Home` is a first-class menu whose dropdown is the app
/// drawer; the app menu title changes with the active app.
#[repr(usize)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuId {
    Home = 0,
    App = 1,
    View = 2,
    Testing = 3,
}

impl MenuId {
    pub const ALL: [MenuId; 4] = [MenuId::Home, MenuId::App, MenuId::View, MenuId::Testing];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn from_index(idx: usize) -> Option<Self> {
        match idx {
            0 => Some(MenuId::Home),
            1 => Some(MenuId::App),
            2 => Some(MenuId::View),
            3 => Some(MenuId::Testing),
            _ => None,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            MenuId::Home => MenuId::App,
            MenuId::App => MenuId::View,
            MenuId::View => MenuId::Testing,
            MenuId::Testing => MenuId::Home,
        }
    }

    pub const fn prev(self) -> Self {
        match self {
            MenuId::Home => MenuId::Testing,
            MenuId::App => MenuId::Home,
            MenuId::View => MenuId::App,
            MenuId::Testing => MenuId::View,
        }
    }
}

/// Shared focus target for mouse and keyboard.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Client,
    MenuBar { menu: MenuId },
    Menu { menu: MenuId, item: usize },
}

/// ABI-friendly focus codes for the host and test harness.
pub const FOCUS_CLIENT: i32 = 0;
pub const FOCUS_MENU_BAR: i32 = 100;
pub const FOCUS_MENU: i32 = 200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Command {
    About,
    Locale,
    Fonts,
    Icons,
    CloseWindow,
    Launch(AppSlot),
    CalcClear,
    FilesHome,
    FilesGithub,
    FilesToggleContext,
    FilesSetWallpaper,
    MinesNewSeed,
    MinesReplay,
    MinesToggleFlag,
    MinesToggleRules,
    TestPattern,
    Snapshot,
}

#[derive(Clone, Copy)]
pub struct MenuItem {
    pub label: &'static str,
    pub command: Command,
}

const MENU_HOME: &[MenuItem] = &[
    MenuItem {
        label: "System",
        command: Command::Launch(AppSlot::System),
    },
    MenuItem {
        label: "Calc",
        command: Command::Launch(AppSlot::Calc),
    },
    MenuItem {
        label: "Files",
        command: Command::Launch(AppSlot::Files),
    },
    MenuItem {
        label: "Mines",
        command: Command::Launch(AppSlot::Mines),
    },
];
const MENU_DESKTOP: &[MenuItem] = &[
    MenuItem {
        label: "About",
        command: Command::About,
    },
    MenuItem {
        label: "Locale",
        command: Command::Locale,
    },
    MenuItem {
        label: "Fonts",
        command: Command::Fonts,
    },
    MenuItem {
        label: "Icons",
        command: Command::Icons,
    },
];
const MENU_SYSTEM: &[MenuItem] = &[
    MenuItem {
        label: "About",
        command: Command::About,
    },
    MenuItem {
        label: "Locale",
        command: Command::Locale,
    },
    MenuItem {
        label: "Fonts",
        command: Command::Fonts,
    },
    MenuItem {
        label: "Icons",
        command: Command::Icons,
    },
    MenuItem {
        label: "Close Window",
        command: Command::CloseWindow,
    },
];
const MENU_CALC: &[MenuItem] = &[
    MenuItem {
        label: "Clear (AC)",
        command: Command::CalcClear,
    },
    MenuItem {
        label: "Close Window",
        command: Command::CloseWindow,
    },
];
const MENU_FILES: &[MenuItem] = &[
    MenuItem {
        label: "Home (~/)",
        command: Command::FilesHome,
    },
    MenuItem {
        label: "GitHub (gh://)",
        command: Command::FilesGithub,
    },
    MenuItem {
        label: "Toggle Context",
        command: Command::FilesToggleContext,
    },
    MenuItem {
        label: "Set as Wallpaper",
        command: Command::FilesSetWallpaper,
    },
    MenuItem {
        label: "Close Window",
        command: Command::CloseWindow,
    },
];
const MENU_MINES: &[MenuItem] = &[
    MenuItem {
        label: "New Seed (N)",
        command: Command::MinesNewSeed,
    },
    MenuItem {
        label: "Replay Seed (R)",
        command: Command::MinesReplay,
    },
    MenuItem {
        label: "Toggle Flag (F)",
        command: Command::MinesToggleFlag,
    },
    MenuItem {
        label: "Toggle Shield Rule",
        command: Command::MinesToggleRules,
    },
    MenuItem {
        label: "Close Window",
        command: Command::CloseWindow,
    },
];
const MENU_VIEW: &[MenuItem] = &[MenuItem {
    label: "Test Pattern",
    command: Command::TestPattern,
}];
const MENU_TESTING: &[MenuItem] = &[MenuItem {
    label: "Snapshot",
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

    pub fn header_rect(&self, menu: MenuId) -> Rect {
        match menu {
            MenuId::Home => Rect::new(0, 0, HOME_BTN_W, TITLE_H),
            _ => Rect::new(
                MENU_X0 + (menu.index() as i32 - 1) * self.header_w,
                0,
                self.header_w,
                TITLE_H,
            ),
        }
    }

    pub fn dropdown_rect(&self, menu: MenuId, item_count: usize, fb_w: i32) -> Rect {
        if menu == MenuId::Home {
            let h = (item_count as i32) * (DOCK_ICON_H + DOCK_GAP) - DOCK_GAP;
            return Rect::new(DOCK_X, DOCK_Y, DOCK_ICON_W, h.max(DOCK_ICON_H));
        }
        let w = MENU_DROP_W.min(fb_w - 16);
        let x = self.header_rect(menu).x.min(fb_w - w - 4).max(0);
        let h = (item_count as i32) * MENU_ITEM_H + 8;
        Rect::new(x, TITLE_H, w, h)
    }

    pub fn menu_item_rect(&self, menu: MenuId, item: usize, item_count: usize, fb_w: i32) -> Rect {
        if menu == MenuId::Home {
            return Rect::new(DOCK_X, dock_icon_y(item), DOCK_ICON_W, DOCK_ICON_H);
        }
        let r = self.dropdown_rect(menu, item_count, fb_w);
        Rect::new(r.x, r.y + 4 + (item as i32) * MENU_ITEM_H, r.w, MENU_ITEM_H)
    }
}

pub struct WmState {
    pub ready: bool,
    pub width: i32,
    pub height: i32,
    pub session_id: u32,
    pub active_app: AppSlot,
    pub focus: Focus,
    pub hover: Option<Focus>,
    pub focus_visible: bool,
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
            focus: Focus::Client,
            hover: None,
            focus_visible: false,
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
        self.focus = Focus::Client;
        self.hover = None;
        self.focus_visible = false;
        self.test_pattern = false;
        self.snapshot_seq = 0;
        self.system = SystemApp::new();
        self.calc = CalcApp::new();
        self.files = FilesApp::new();
        self.mines.init_default();
        self.ready = true;
    }

    /// Whether the App Home drawer (the Home menu dropdown) is open.
    pub fn drawer_open(&self) -> bool {
        matches!(
            self.focus,
            Focus::Menu {
                menu: MenuId::Home,
                ..
            }
        )
    }

    pub fn menu_title(&self, menu: MenuId) -> &'static str {
        match menu {
            MenuId::Home => "",
            MenuId::App => self.active_app.title(),
            MenuId::View => "View",
            MenuId::Testing => "Testing",
        }
    }

    pub fn menu_items(&self, menu: MenuId) -> &'static [MenuItem] {
        match menu {
            MenuId::Home => MENU_HOME,
            MenuId::App => match self.active_app {
                AppSlot::Desktop => MENU_DESKTOP,
                AppSlot::System => MENU_SYSTEM,
                AppSlot::Calc => MENU_CALC,
                AppSlot::Files => MENU_FILES,
                AppSlot::Mines => MENU_MINES,
            },
            MenuId::View => MENU_VIEW,
            MenuId::Testing => MENU_TESTING,
        }
    }

    pub fn focus_code(&self) -> i32 {
        match self.focus {
            Focus::Client => FOCUS_CLIENT,
            Focus::MenuBar { menu } => FOCUS_MENU_BAR + menu.index() as i32,
            Focus::Menu { menu, item } => FOCUS_MENU + menu.index() as i32 * 10 + item as i32,
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
                    self.system.focus_visible = self.focus_visible && self.focus == Focus::Client;
                    system::draw_window(&mut client_fb, &self.system, self.width, self.height)
                }
                AppSlot::Calc => calc::draw_window(&mut client_fb, &self.calc),
                AppSlot::Files => {
                    self.files.focus_visible = self.focus_visible && self.focus == Focus::Client;
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

        // 2. App Home drawer (Home menu dropdown).
        if self.drawer_open() {
            self.draw_home_menu(fb);
        }

        // 3. Title bar.
        self.draw_title_bar(fb);

        // 4. Other open dropdowns.
        if let Focus::Menu { menu, .. } = self.focus {
            if menu != MenuId::Home {
                self.draw_dropdown_menu(fb, menu);
            }
        }
    }

    fn draw_title_bar(&self, fb: &mut Framebuffer) {
        let layout = WmLayout::new(fb.w);
        fb.fill(0, 0, fb.w, TITLE_H, t::TITLE_BG);
        fb.fill(0, TITLE_H - 1, fb.w, 1, t::BORDER);

        // App Home header, drawn as an icon button.
        let home = layout.header_rect(MenuId::Home);
        let home_open = self.drawer_open();
        let home_hover = matches!(
            self.hover,
            Some(Focus::MenuBar { menu: MenuId::Home })
                | Some(Focus::Menu {
                    menu: MenuId::Home,
                    ..
                })
        );
        let home_bg = if home_open {
            t::ACCENT
        } else if home_hover {
            t::SURFACE_HOVER
        } else {
            t::SURFACE_RAISED
        };
        let home_icon = if home_open { t::OVERLAY } else { t::ACCENT };
        fb.fill(home.x, home.y, home.w, TITLE_H - 1, home_bg);
        fb.fill(home.right() - 1, 0, 1, TITLE_H - 1, t::BORDER);
        draw_icon(
            fb,
            (HOME_BTN_W - 20) / 2,
            (TITLE_H - 1 - 20) / 2,
            Icon::Menu,
            20,
            home_icon,
        );
        if self.focus_visible && matches!(self.focus, Focus::MenuBar { menu: MenuId::Home }) {
            ui::focus_ring(fb, home);
        }

        // Text menu headers.
        for menu in MenuId::ALL {
            if menu == MenuId::Home {
                continue;
            }
            let r = layout.header_rect(menu);
            let is_open = matches!(
                self.focus,
                Focus::Menu { menu: m, .. } | Focus::MenuBar { menu: m } if m == menu
            );
            let hovered = matches!(
                self.hover,
                Some(Focus::MenuBar { menu: m }) | Some(Focus::Menu { menu: m, .. }) if m == menu
            );
            if is_open {
                fb.fill(r.x, 2, r.w - 4, TITLE_H - 3, t::SURFACE_HOVER);
                fb.fill(r.x, TITLE_H - 3, r.w - 4, 2, t::ACCENT);
            } else if hovered {
                fb.fill(r.x, 2, r.w - 4, TITLE_H - 3, t::SURFACE);
            }
            let fg = if is_open {
                t::TEXT
            } else if hovered {
                t::TEXT_BODY
            } else {
                t::TEXT_SOFT
            };
            draw_str_prop(
                fb,
                r.x + 10,
                layout.menu_text_y,
                self.menu_title(menu),
                layout.menu_scale,
                fg,
            );
            if self.focus_visible && matches!(self.focus, Focus::MenuBar { menu: m } if m == menu) {
                ui::focus_ring(fb, r);
            }
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

    fn draw_home_menu(&self, fb: &mut Framebuffer) {
        let items = self.menu_items(MenuId::Home);
        let layout = WmLayout::new(fb.w);
        let r = layout.dropdown_rect(MenuId::Home, items.len(), fb.w);

        fb.shade(r.x - 6, r.y - 6, r.w + 12, r.h + 12, 80);
        fb.fill(r.x - 6, r.y - 6, r.w + 12, r.h + 12, t::SURFACE);
        fb.border(r.x - 6, r.y - 6, r.w + 12, r.h + 12, 2, t::ACCENT);

        for (idx, slot) in AppSlot::DOCK_APPS.iter().enumerate() {
            let tr = layout.menu_item_rect(MenuId::Home, idx, items.len(), fb.w);
            let active = self.active_app == *slot;
            match slot {
                AppSlot::System => {
                    system::draw_dock_tile(fb, &self.system, tr.x, tr.y, tr.w, tr.h, active)
                }
                AppSlot::Calc => {
                    calc::draw_dock_tile(fb, &self.calc, tr.x, tr.y, tr.w, tr.h, active)
                }
                AppSlot::Files => {
                    files::draw_dock_tile(fb, &self.files, tr.x, tr.y, tr.w, tr.h, active)
                }
                AppSlot::Mines => {
                    mines::draw_dock_tile(fb, &self.mines, tr.x, tr.y, tr.w, tr.h, active)
                }
                AppSlot::Desktop => {}
            }
            let focused = self.focus_visible
                && matches!(self.focus, Focus::Menu { menu: MenuId::Home, item } if item == idx);
            let hovered = matches!(
                self.hover,
                Some(Focus::Menu { menu: MenuId::Home, item }) if item == idx
            );
            if hovered && !focused {
                ui::hover_ring(fb, tr);
            }
            if focused {
                ui::focus_ring(fb, tr);
            }
        }
    }

    fn draw_dropdown_menu(&self, fb: &mut Framebuffer, menu: MenuId) {
        let items = self.menu_items(menu);
        if items.is_empty() {
            return;
        }
        let layout = WmLayout::new(fb.w);
        let r = layout.dropdown_rect(menu, items.len(), fb.w);
        let selected = match self.focus {
            Focus::Menu { item, .. } => item,
            _ => 0,
        };

        fb.shade(r.x + 4, r.y + 4, r.w, r.h, 80);
        fb.fill(r.x, r.y, r.w, r.h, t::SURFACE);
        fb.border(r.x, r.y, r.w, r.h, 2, t::ACCENT);

        for (idx, item) in items.iter().enumerate() {
            let ir = layout.menu_item_rect(menu, idx, items.len(), fb.w);
            let hovered = matches!(
                self.hover,
                Some(Focus::Menu { menu: m, item }) if m == menu && item == idx
            );
            if idx == selected || hovered {
                fb.fill(ir.x + 2, ir.y, ir.w - 4, ir.h, t::SURFACE_HOVER);
            }
            if idx > 0 {
                fb.fill(r.x + 8, ir.y, r.w - 16, 1, t::BORDER_SOFT);
            }
            draw_str_prop(
                fb,
                ir.x + 14,
                ir.y + 8,
                item.label,
                2,
                if idx == selected {
                    t::TEXT
                } else {
                    t::TEXT_MENU
                },
            );
            if self.focus_visible && idx == selected {
                ui::focus_ring(fb, ir.inset(1));
            }
        }
    }

    pub fn click(&mut self, x: i32, y: i32) -> InputResult {
        if !self.ready {
            return InputResult::Noop;
        }

        if self.test_pattern {
            self.test_pattern = false;
            self.focus = Focus::Client;
            self.focus_visible = false;
            return InputResult::Changed;
        }

        let layout = WmLayout::new(self.width);
        self.focus_visible = false;
        self.files.hover_row = None;
        self.files.hover_control = None;
        self.system.hover_tab = None;

        // Layer 1: an open dropdown consumes the click.
        if let Focus::Menu { menu, .. } = self.focus {
            let items = self.menu_items(menu);
            let r = layout.dropdown_rect(menu, items.len(), self.width);

            if r.contains(x, y) {
                if menu == MenuId::Home {
                    for idx in 0..items.len() {
                        let ir = layout.menu_item_rect(menu, idx, items.len(), self.width);
                        if ir.contains(x, y) {
                            self.focus = Focus::Client;
                            return self.execute_command(items[idx].command);
                        }
                    }
                } else {
                    let rel_y = y - (r.y + 4);
                    if rel_y >= 0 {
                        let item_idx = (rel_y / MENU_ITEM_H) as usize;
                        if item_idx < items.len() {
                            self.focus = Focus::Client;
                            return self.execute_command(items[item_idx].command);
                        }
                    }
                }
            }

            if (0..TITLE_H).contains(&y) {
                for candidate in MenuId::ALL {
                    if layout.header_rect(candidate).contains(x, y) {
                        self.focus = if candidate == menu {
                            Focus::MenuBar { menu }
                        } else {
                            Focus::Menu {
                                menu: candidate,
                                item: 0,
                            }
                        };
                        return InputResult::Changed;
                    }
                }
            }

            self.focus = Focus::Client;
            return InputResult::Changed;
        }

        // Layer 2: title bar.
        if (0..TITLE_H).contains(&y) {
            for menu in MenuId::ALL {
                if layout.header_rect(menu).contains(x, y) {
                    self.focus = Focus::Menu { menu, item: 0 };
                    return InputResult::Changed;
                }
            }
            return InputResult::Noop;
        }

        // Layer 3: client surface.
        self.focus = Focus::Client;
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

    pub fn pointer_move(&mut self, x: i32, y: i32) -> InputResult {
        if !self.ready || self.test_pattern {
            return InputResult::Noop;
        }
        let layout = WmLayout::new(self.width);
        let previous = self.hover;
        let mut hover = None;
        let mut over_chrome = false;

        if let Focus::Menu { menu, .. } = self.focus {
            // An open dropdown is modal: suppress client hover behind it.
            over_chrome = true;
            let items = self.menu_items(menu);
            let r = layout.dropdown_rect(menu, items.len(), self.width);
            if r.contains(x, y) {
                if menu == MenuId::Home {
                    for idx in 0..items.len() {
                        let ir = layout.menu_item_rect(menu, idx, items.len(), self.width);
                        if ir.contains(x, y) {
                            hover = Some(Focus::Menu { menu, item: idx });
                        }
                    }
                } else {
                    let rel_y = y - (r.y + 4);
                    if rel_y >= 0 {
                        let item = (rel_y / MENU_ITEM_H) as usize;
                        if item < items.len() {
                            hover = Some(Focus::Menu { menu, item });
                        }
                    }
                }
            } else if (0..TITLE_H).contains(&y) {
                over_chrome = true;
                for candidate in MenuId::ALL {
                    if layout.header_rect(candidate).contains(x, y) {
                        hover = Some(Focus::MenuBar { menu: candidate });
                    }
                }
            }
        } else if (0..TITLE_H).contains(&y) {
            over_chrome = true;
            for menu in MenuId::ALL {
                if layout.header_rect(menu).contains(x, y) {
                    hover = Some(Focus::MenuBar { menu });
                }
            }
        }

        let client_hover = if !over_chrome {
            let cx = x;
            let cy = y - TITLE_H;
            let client_w = self.width;
            let client_h = (self.height - TITLE_H).max(1);
            match self.active_app {
                AppSlot::Files => files::pointer_move(&mut self.files, cx, cy, client_w, client_h),
                AppSlot::System => {
                    system::pointer_move(&mut self.system, cx, cy, client_w, client_h)
                }
                _ => false,
            }
        } else {
            // Moving into chrome must clear stale client hover highlights.
            files::pointer_leave(&mut self.files) || system::pointer_leave(&mut self.system)
        };

        self.hover = hover;
        if previous != self.hover || client_hover {
            InputResult::Changed
        } else {
            InputResult::Noop
        }
    }

    pub fn pointer_leave(&mut self) -> InputResult {
        if !self.ready {
            return InputResult::Noop;
        }
        let changed = self.hover.is_some()
            || files::pointer_leave(&mut self.files)
            || system::pointer_leave(&mut self.system);
        self.hover = None;
        if changed {
            InputResult::Changed
        } else {
            InputResult::Noop
        }
    }

    fn launch_app(&mut self, slot: AppSlot) -> InputResult {
        if slot == AppSlot::Files {
            self.files.ensure_selected_staged();
        }
        self.active_app = slot;
        self.focus = Focus::Client;
        self.hover = None;
        self.focus_visible = false;
        self.files.hover_row = None;
        self.files.hover_control = None;
        self.system.hover_tab = None;
        InputResult::Changed
    }

    fn execute_command(&mut self, command: Command) -> InputResult {
        match command {
            Command::About => self.open_system(SystemSection::About),
            Command::Locale => self.open_system(SystemSection::Locale),
            Command::Fonts => self.open_system(SystemSection::Fonts),
            Command::Icons => self.open_system(SystemSection::Icons),
            Command::CloseWindow => {
                self.active_app = AppSlot::Desktop;
                self.focus = Focus::Client;
                InputResult::Changed
            }
            Command::Launch(slot) => self.launch_app(slot),
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
                        self.focus = Focus::Client;
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
            Command::TestPattern => {
                self.test_pattern = !self.test_pattern;
                self.focus = Focus::Client;
                InputResult::Changed
            }
            Command::Snapshot => {
                self.snapshot_seq = self.snapshot_seq.wrapping_add(1);
                self.system.snapshots_taken = self.snapshot_seq;
                self.focus = Focus::Client;
                InputResult::Snapshot
            }
        }
    }

    fn open_system(&mut self, section: SystemSection) -> InputResult {
        self.active_app = AppSlot::System;
        self.system.section = section;
        self.focus = Focus::Client;
        InputResult::Changed
    }

    pub fn key(&mut self, key: Key, mods: Mods) -> InputResult {
        if !self.ready {
            return InputResult::Noop;
        }

        // Ctrl/Cmd + 1..4 launches an app without opening the Home drawer.
        if mods.command() {
            if let Some(slot) = match key {
                Key::Digit1 => Some(AppSlot::System),
                Key::Digit2 => Some(AppSlot::Calc),
                Key::Digit3 => Some(AppSlot::Files),
                Key::Digit4 => Some(AppSlot::Mines),
                _ => None,
            } {
                let rc = self.launch_app(slot);
                self.focus_visible = true;
                return rc;
            }
        }

        // App Home shortcut opens the Home menu (drawer), like clicking it.
        if key == Key::Dock {
            self.focus = Focus::Menu {
                menu: MenuId::Home,
                item: 0,
            };
            self.hover = None;
            self.focus_visible = true;
            return InputResult::Changed;
        }

        if key == Key::MenuBar {
            self.focus = Focus::MenuBar { menu: MenuId::Home };
            self.hover = None;
            self.focus_visible = true;
            return InputResult::Changed;
        }

        // Escape pops one layer. From the client it focuses App Home without
        // opening the drawer.
        if key == Key::Escape {
            self.focus = match self.focus {
                Focus::Menu { menu, .. } => Focus::MenuBar { menu },
                Focus::MenuBar { menu: MenuId::Home } => Focus::Client,
                Focus::MenuBar { .. } => Focus::MenuBar { menu: MenuId::Home },
                Focus::Client => Focus::MenuBar { menu: MenuId::Home },
            };
            self.hover = None;
            self.focus_visible = true;
            return InputResult::Changed;
        }

        let rc = match self.focus {
            Focus::MenuBar { menu } => self.key_menu_bar(key, mods, menu),
            Focus::Menu { menu, item } => self.key_menu(key, mods, menu, item),
            Focus::Client => self.key_client(key, mods),
        };
        if rc.is_consumed() {
            self.focus_visible = true;
        }
        rc
    }

    fn key_menu_bar(&mut self, key: Key, mods: Mods, menu: MenuId) -> InputResult {
        match key {
            Key::Left => {
                self.focus = Focus::MenuBar { menu: menu.prev() };
                InputResult::Changed
            }
            Key::Right => {
                self.focus = Focus::MenuBar { menu: menu.next() };
                InputResult::Changed
            }
            Key::Tab => {
                self.focus = Focus::MenuBar {
                    menu: if mods.shift() {
                        menu.prev()
                    } else {
                        menu.next()
                    },
                };
                InputResult::Changed
            }
            Key::Down | Key::Enter | Key::Space => {
                self.focus = Focus::Menu { menu, item: 0 };
                InputResult::Changed
            }
            Key::Up => {
                let items = self.menu_items(menu).len();
                self.focus = Focus::Menu {
                    menu,
                    item: items.saturating_sub(1),
                };
                InputResult::Changed
            }
            Key::Home => {
                self.focus = Focus::MenuBar { menu: MenuId::Home };
                InputResult::Changed
            }
            Key::End => {
                self.focus = Focus::MenuBar {
                    menu: MenuId::Testing,
                };
                InputResult::Changed
            }
            _ => InputResult::Handled,
        }
    }

    fn key_menu(&mut self, key: Key, mods: Mods, menu: MenuId, item: usize) -> InputResult {
        let items = self.menu_items(menu);
        if items.is_empty() {
            self.focus = Focus::MenuBar { menu };
            return InputResult::Changed;
        }
        match key {
            Key::Up => {
                self.focus = Focus::Menu {
                    menu,
                    item: if item == 0 { items.len() - 1 } else { item - 1 },
                };
                InputResult::Changed
            }
            Key::Down => {
                self.focus = Focus::Menu {
                    menu,
                    item: (item + 1) % items.len(),
                };
                InputResult::Changed
            }
            Key::Left => {
                self.focus = Focus::Menu {
                    menu: menu.prev(),
                    item: 0,
                };
                InputResult::Changed
            }
            Key::Right => {
                self.focus = Focus::Menu {
                    menu: menu.next(),
                    item: 0,
                };
                InputResult::Changed
            }
            Key::Tab => {
                self.focus = Focus::Menu {
                    menu: if mods.shift() {
                        menu.prev()
                    } else {
                        menu.next()
                    },
                    item: 0,
                };
                InputResult::Changed
            }
            Key::PageDown => {
                self.focus = Focus::Menu {
                    menu: menu.next(),
                    item: 0,
                };
                InputResult::Changed
            }
            Key::PageUp => {
                self.focus = Focus::Menu {
                    menu: menu.prev(),
                    item: 0,
                };
                InputResult::Changed
            }
            Key::Home => {
                self.focus = Focus::Menu { menu, item: 0 };
                InputResult::Changed
            }
            Key::End => {
                self.focus = Focus::Menu {
                    menu,
                    item: items.len() - 1,
                };
                InputResult::Changed
            }
            Key::Enter | Key::Space => {
                let cmd = items[item.min(items.len() - 1)].command;
                self.focus = Focus::Client;
                self.execute_command(cmd)
            }
            _ => InputResult::Handled,
        }
    }

    fn key_client(&mut self, key: Key, mods: Mods) -> InputResult {
        let rc = match self.active_app {
            AppSlot::Desktop => match key {
                Key::Left
                | Key::Right
                | Key::Up
                | Key::Down
                | Key::PageUp
                | Key::PageDown
                | Key::Home
                | Key::End
                | Key::Space => InputResult::Handled,
                _ => InputResult::Noop,
            },
            AppSlot::System => self.key_system(key, mods),
            AppSlot::Calc => calc::key_window(&mut self.calc, key),
            AppSlot::Files => files::key_window(&mut self.files, key, mods),
            AppSlot::Mines => mines::key_window(&mut self.mines, key),
        };
        if key == Key::Space && !rc.is_consumed() {
            // Avoid page-scrolling while a client surface has focus.
            InputResult::Handled
        } else {
            rc
        }
    }

    fn key_system(&mut self, key: Key, mods: Mods) -> InputResult {
        match key {
            Key::Digit1 | Key::Digit2 | Key::Digit3 | Key::Digit4 => {
                let idx = key as i32 - Key::Digit1 as i32;
                let target = SystemSection::from_index(idx as usize);
                if self.system.section != target {
                    self.system.section = target;
                    InputResult::Changed
                } else {
                    InputResult::Handled
                }
            }
            Key::Left | Key::Right | Key::Tab | Key::Up | Key::Down => {
                let cur = self.system.section as usize;
                let forward =
                    key == Key::Right || key == Key::Down || (key == Key::Tab && !mods.shift());
                let next = if forward {
                    (cur + 1) % SystemSection::COUNT
                } else {
                    (cur + SystemSection::COUNT - 1) % SystemSection::COUNT
                };
                self.system.section = SystemSection::from_index(next);
                InputResult::Changed
            }
            Key::Home => {
                self.system.section = SystemSection::About;
                InputResult::Changed
            }
            Key::End => {
                self.system.section = SystemSection::Icons;
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
        assert_eq!(AppSlot::Desktop.title(), "Desktop");
    }

    #[test]
    fn menu_labels_map_to_commands() {
        for app in AppSlot::DOCK_APPS {
            let mut wm = WmState::new();
            wm.active_app = app;
            let items = wm.menu_items(MenuId::App);
            assert!(!items.is_empty());
            for pair in items.windows(2) {
                assert_ne!(pair[0].label, pair[1].label);
            }
        }
        let mut files = WmState::new();
        files.active_app = AppSlot::Files;
        assert_eq!(
            files.menu_items(MenuId::App)[2].command,
            Command::FilesToggleContext
        );
        assert_eq!(
            files.menu_items(MenuId::App)[4].command,
            Command::CloseWindow
        );
        assert_eq!(
            WmState::new().menu_items(MenuId::View)[0].command,
            Command::TestPattern
        );
        assert_eq!(
            WmState::new().menu_items(MenuId::Testing)[0].command,
            Command::Snapshot
        );
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
        assert_eq!(
            wm.execute_command(Command::Launch(AppSlot::Mines)),
            InputResult::Changed
        );
        assert_eq!(wm.active_app, AppSlot::Mines);
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

        assert_eq!(
            full.header_rect(MenuId::Home),
            Rect::new(0, 0, HOME_BTN_W, TITLE_H)
        );
        for menu in MenuId::ALL {
            let r = full.dropdown_rect(menu, 5, 1280);
            assert!(r.x >= 0 && r.right() <= 1280);
            assert!(r.contains(r.x + 1, r.y + 1));
            let item = full.menu_item_rect(menu, 3, 5, 1280);
            assert!(item.x >= r.x && item.right() <= r.right());
        }
        let r = compact.dropdown_rect(MenuId::Testing, 1, 390);
        assert!(r.x >= 0 && r.right() <= 390);
    }
}
