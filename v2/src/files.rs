//! Two-Panel Document Finder & Preview Application (`files.rs`).
//!
//! Layout inside the maximized `1280 x 688` client surface:
//! - Left 1/3 (`x: 0..426`, `w: 426`): `~/` File Browser with 2 sub-folders:
//!   - `~/wallpapers` (`wallpapers/*`)
//!   - `~/samples` (`samples/*`)
//! - Right 2/3 (`x: 428..1280`, `w: 852`): Live read-only Preview of the selected item
//!   (folder summary at `~/`, scaled `RGBA8888` image preview inside `~/wallpapers`,
//!   or syntax-highlighted source preview inside `~/samples`).

use crate::code::{draw_code_preview, is_image_filename, lang_for_filename};
use crate::fb::{rgb, Framebuffer};
use crate::font::{draw_str, draw_str_centered, text_width};

pub const MAX_MOUNT_FILES: usize = 24;
pub const MAX_NAME_LEN: usize = 48;
pub const MAX_TEXT_BYTES: usize = 16384;

// 1/3 Left Browser + 2/3 Right Preview split across 1280px
pub const LEFT_W: i32 = 426; // 1280 / 3
pub const DIV_X: i32 = LEFT_W;
pub const RIGHT_X: i32 = LEFT_W + 2;
pub const RIGHT_W: i32 = 1280 - RIGHT_X; // 852px

const PAD_X: i32 = 24;
const HEAD_Y: i32 = 24;
const ROW_START_Y: i32 = 76;
const ROW_H: i32 = 46;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FolderLevel {
    Root = 0,
    Wallpapers = 1,
    Samples = 2,
}

#[derive(Clone, Copy)]
pub struct MountEntry {
    pub used: bool,
    pub name: [u8; MAX_NAME_LEN],
    pub name_len: usize,
    pub byte_size: u32,
}

impl MountEntry {
    pub const fn empty() -> Self {
        Self {
            used: false,
            name: [0; MAX_NAME_LEN],
            name_len: 0,
            byte_size: 0,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }

    /// Returns the display basename (stripping `wallpapers/` or `samples/` if present).
    pub fn base_name(&self) -> &str {
        let full = self.name_str();
        let bytes = full.as_bytes();
        let mut i = bytes.len();
        while i > 0 {
            i -= 1;
            if bytes[i] == b'/' {
                return &full[i + 1..];
            }
        }
        full
    }

    pub fn is_image(&self) -> bool {
        let full = self.name_str().as_bytes();
        if full.starts_with(b"wallpapers/") {
            return true;
        }
        if full.starts_with(b"samples/") {
            return false;
        }
        is_image_filename(self.base_name())
    }

    pub fn in_folder(&self, level: FolderLevel) -> bool {
        match level {
            FolderLevel::Root => false,
            FolderLevel::Wallpapers => self.is_image(),
            FolderLevel::Samples => !self.is_image(),
        }
    }
}

pub struct FilesApp {
    pub entries: [MountEntry; MAX_MOUNT_FILES],
    pub count: usize,
    /// Current folder (`~/`, `~/wallpapers`, or `~/samples`).
    pub level: FolderLevel,
    /// Selected row index inside the current folder view.
    pub row_selected: usize,
    /// Global mount index (`0..count`) of the currently selected file (`0` if on a folder row).
    pub selected: usize,
    /// Index of the file requested from the `host.js` upstream (`-1` if none).
    pub pending_fetch_idx: i32,
    /// Index of the image currently loaded in the guest's RGBA staging buffer (`-1` if none).
    pub staged_idx: i32,
    pub staged_w: i32,
    pub staged_h: i32,
    /// Index of the text/code file currently loaded in the guest's text staging buffer (`-1` if none).
    pub staged_text_idx: i32,
    pub staged_text_len: usize,
    pub code_scroll: usize,
    /// Whether the currently staged image is also pinned as the Desktop Wallpaper.
    pub wallpaper_active: bool,
    pub wallpaper_idx: i32,
}

impl FilesApp {
    pub const fn new() -> Self {
        Self {
            entries: [MountEntry::empty(); MAX_MOUNT_FILES],
            count: 0,
            level: FolderLevel::Root,
            row_selected: 0,
            selected: 0,
            pending_fetch_idx: -1,
            staged_idx: -1,
            staged_w: 0,
            staged_h: 0,
            staged_text_idx: -1,
            staged_text_len: 0,
            code_scroll: 0,
            wallpaper_active: false,
            wallpaper_idx: -1,
        }
    }

    pub fn clear_mounts(&mut self) {
        self.entries = [MountEntry::empty(); MAX_MOUNT_FILES];
        self.count = 0;
        self.level = FolderLevel::Root;
        self.row_selected = 0;
        self.selected = 0;
        self.pending_fetch_idx = -1;
        self.staged_text_idx = -1;
        self.staged_text_len = 0;
        self.code_scroll = 0;
    }

    pub fn add_mount(&mut self, name_bytes: &[u8], byte_size: u32) -> i32 {
        if self.count >= MAX_MOUNT_FILES {
            return -1;
        }
        let idx = self.count;
        let len = name_bytes.len().min(MAX_NAME_LEN);
        let mut entry = MountEntry::empty();
        entry.used = true;
        entry.name[..len].copy_from_slice(&name_bytes[..len]);
        entry.name_len = len;
        entry.byte_size = byte_size;
        self.entries[idx] = entry;
        self.count += 1;
        idx as i32
    }

    pub fn folder_file_count(&self, folder: FolderLevel) -> usize {
        let mut n = 0;
        for i in 0..self.count {
            if self.entries[i].in_folder(folder) {
                n += 1;
            }
        }
        n
    }

    /// Number of clickable rows in the Left 1/3 panel for the current `level`.
    pub fn visible_row_count(&self) -> usize {
        match self.level {
            FolderLevel::Root => 2, // 0 = wallpapers/, 1 = samples/
            FolderLevel::Wallpapers | FolderLevel::Samples => {
                1 + self.folder_file_count(self.level) // row 0 = `../`, rows 1..=N = files
            }
        }
    }

    /// Maps a `1`-based file row inside `Wallpapers` or `Samples` to its global mount index (`0..count`).
    pub fn global_idx_for_row(&self, level: FolderLevel, row: usize) -> Option<usize> {
        if level == FolderLevel::Root || row == 0 {
            return None;
        }
        let target_ord = row - 1;
        let mut ord = 0;
        for i in 0..self.count {
            if self.entries[i].in_folder(level) {
                if ord == target_ord {
                    return Some(i);
                }
                ord += 1;
            }
        }
        None
    }

    /// Returns the global mount index if a file (not `../` or a folder) is actively selected.
    pub fn active_file_idx(&self) -> Option<usize> {
        self.global_idx_for_row(self.level, self.row_selected)
    }

    pub fn selected_is_image(&self) -> bool {
        if let Some(idx) = self.active_file_idx() {
            self.entries[idx].is_image()
        } else {
            false
        }
    }

    /// Ensure the currently selected file (if inside a subfolder) is staged for the right preview pane.
    pub fn ensure_selected_staged(&mut self) {
        let Some(idx) = self.active_file_idx() else {
            return;
        };
        self.selected = idx;
        let idx_i32 = idx as i32;
        if self.entries[idx].is_image() {
            if self.staged_idx != idx_i32 {
                self.pending_fetch_idx = idx_i32;
            }
        } else if self.staged_text_idx != idx_i32 {
            self.pending_fetch_idx = idx_i32;
        }
    }

    pub fn open_subfolder(&mut self, folder: FolderLevel) -> bool {
        self.level = folder;
        let fcount = self.folder_file_count(folder);
        // Pre-select the first file (`row 1`) when opening a non-empty subfolder so its preview appears immediately!
        self.row_selected = if fcount > 0 { 1 } else { 0 };
        self.code_scroll = 0;
        self.ensure_selected_staged();
        true
    }

    pub fn go_parent(&mut self) -> bool {
        match self.level {
            FolderLevel::Root => false,
            FolderLevel::Wallpapers => {
                self.level = FolderLevel::Root;
                self.row_selected = 0;
                true
            }
            FolderLevel::Samples => {
                self.level = FolderLevel::Root;
                self.row_selected = 1;
                true
            }
        }
    }

    pub fn complete_stage(&mut self, idx: i32, w: i32, h: i32) {
        self.staged_idx = idx;
        self.staged_w = w;
        self.staged_h = h;
        if self.pending_fetch_idx == idx {
            self.pending_fetch_idx = -1;
        }
    }

    pub fn complete_stage_text(&mut self, idx: i32, byte_len: usize) {
        self.staged_text_idx = idx;
        self.staged_text_len = byte_len.min(MAX_TEXT_BYTES);
        self.code_scroll = 0;
        if self.pending_fetch_idx == idx {
            self.pending_fetch_idx = -1;
        }
    }
}

pub fn draw_window(
    fb: &mut Framebuffer,
    app: &FilesApp,
    staged_px: &[u32],
    staged_text: &[u8],
) {
    fb.clear(rgb(0x0b, 0x10, 0x16));

    let accent = rgb(0x2d, 0xd4, 0xbf);
    let label_col = rgb(0x7b, 0x8b, 0x98);
    let text_col = rgb(0xe6, 0xf1, 0xf8);

    // --- 1. Left 1/3 Panel: Folder / File Browser (`x: 0..426`) ---
    let path_title = match app.level {
        FolderLevel::Root => "~/",
        FolderLevel::Wallpapers => "~/WALLPAPERS",
        FolderLevel::Samples => "~/SAMPLES",
    };
    draw_str(fb, PAD_X, HEAD_Y, path_title, 3, rgb(0xf0, 0xf6, 0xfc));
    fb.fill(PAD_X, HEAD_Y + 32, LEFT_W - 2 * PAD_X, 2, accent);

    let row_w = LEFT_W - 2 * PAD_X;

    match app.level {
        FolderLevel::Root => {
            let wp_count = app.folder_file_count(FolderLevel::Wallpapers) as u32;
            let sm_count = app.folder_file_count(FolderLevel::Samples) as u32;
            let folders = [("wallpapers/", wp_count), ("samples/", sm_count)];

            for (r, &(fname, fcount)) in folders.iter().enumerate() {
                let ry = ROW_START_Y + (r as i32) * ROW_H;
                let selected = r == app.row_selected;

                if selected {
                    fb.fill(PAD_X, ry, row_w, ROW_H - 4, rgb(0x13, 0x22, 0x2e));
                    fb.fill(PAD_X, ry, 4, ROW_H - 4, accent);
                }

                draw_str(
                    fb,
                    PAD_X + 14,
                    ry + 14,
                    fname,
                    2,
                    if selected { rgb(0xf0, 0xf6, 0xfc) } else { text_col },
                );

                let mut cnt_buf = [b' '; 16];
                let cnt_str = fmt_items(&mut cnt_buf, fcount);
                let cw = text_width(cnt_str, 2);
                draw_str(
                    fb,
                    PAD_X + row_w - cw - 10,
                    ry + 14,
                    cnt_str,
                    2,
                    if selected { accent } else { label_col },
                );

                fb.fill(PAD_X, ry + ROW_H - 4, row_w, 1, rgb(0x18, 0x22, 0x2d));
            }
        }
        FolderLevel::Wallpapers | FolderLevel::Samples => {
            let total_rows = app.visible_row_count();
            for r in 0..total_rows {
                let ry = ROW_START_Y + (r as i32) * ROW_H;
                if ry + ROW_H > fb.h {
                    break;
                }
                let selected = r == app.row_selected;

                if selected {
                    fb.fill(PAD_X, ry, row_w, ROW_H - 4, rgb(0x13, 0x22, 0x2e));
                    fb.fill(PAD_X, ry, 4, ROW_H - 4, accent);
                }

                if r == 0 {
                    draw_str(
                        fb,
                        PAD_X + 14,
                        ry + 14,
                        "../",
                        2,
                        if selected { rgb(0xf0, 0xf6, 0xfc) } else { text_col },
                    );
                    let up_lbl = "UP";
                    let uw = text_width(up_lbl, 2);
                    draw_str(
                        fb,
                        PAD_X + row_w - uw - 10,
                        ry + 14,
                        up_lbl,
                        2,
                        if selected { accent } else { label_col },
                    );
                } else if let Some(gidx) = app.global_idx_for_row(app.level, r) {
                    let entry = &app.entries[gidx];
                    let name = entry.base_name();
                    let max_name = 21;
                    let name_clamped = if name.len() > max_name {
                        &name[..max_name]
                    } else {
                        name
                    };
                    draw_str(
                        fb,
                        PAD_X + 14,
                        ry + 14,
                        name_clamped,
                        2,
                        if selected { rgb(0xf0, 0xf6, 0xfc) } else { text_col },
                    );

                    let mut sz_buf = [b' '; 16];
                    let sz_str = fmt_size(&mut sz_buf, entry.byte_size);
                    let sw = text_width(sz_str, 2);
                    draw_str(
                        fb,
                        PAD_X + row_w - sw - 10,
                        ry + 14,
                        sz_str,
                        2,
                        if selected { accent } else { label_col },
                    );
                }

                fb.fill(PAD_X, ry + ROW_H - 4, row_w, 1, rgb(0x18, 0x22, 0x2d));
            }
        }
    }

    // --- 2. Vertical Split Divider (`x: 426`, `w: 2px`) ---
    fb.fill(DIV_X, 0, 2, fb.h, rgb(0x22, 0x30, 0x3e));

    // --- 3. Right 2/3 Panel: Live Read-Only Preview (`x: 428..1280`) ---
    if let Some(sel_idx) = app.active_file_idx() {
        let sel_entry = &app.entries[sel_idx];
        let sel_i32 = sel_idx as i32;

        if sel_entry.is_image() {
            if app.staged_idx == sel_i32 && app.staged_w > 0 && app.staged_h > 0 {
                fb.blit_scaled(
                    RIGHT_X,
                    0,
                    RIGHT_W,
                    fb.h,
                    staged_px,
                    app.staged_w,
                    app.staged_h,
                );
            } else {
                draw_str_centered(
                    fb,
                    RIGHT_X,
                    0,
                    RIGHT_W,
                    fb.h,
                    "LOADING PREVIEW...",
                    2,
                    accent,
                );
            }
        } else if app.staged_text_idx == sel_i32 {
            let len = app.staged_text_len.min(staged_text.len());
            let lang = lang_for_filename(sel_entry.base_name());
            draw_code_preview(
                fb,
                &staged_text[..len],
                RIGHT_X,
                0,
                RIGHT_W,
                fb.h,
                lang,
                app.code_scroll,
            );
        } else {
            draw_str_centered(
                fb,
                RIGHT_X,
                0,
                RIGHT_W,
                fb.h,
                "LOADING PREVIEW...",
                2,
                accent,
            );
        }
    } else {
        // Folder or `../` selected: show a clean directory summary in the right pane
        let target_folder = match app.level {
            FolderLevel::Root => {
                if app.row_selected == 0 {
                    FolderLevel::Wallpapers
                } else {
                    FolderLevel::Samples
                }
            }
            sub => sub,
        };
        draw_folder_summary(fb, app, target_folder);
    }
}

fn draw_folder_summary(fb: &mut Framebuffer, app: &FilesApp, folder: FolderLevel) {
    let accent = rgb(0x2d, 0xd4, 0xbf);
    let label_col = rgb(0x7b, 0x8b, 0x98);
    let text_col = rgb(0xe6, 0xf1, 0xf8);

    let rx = RIGHT_X + 48;
    let title = match folder {
        FolderLevel::Wallpapers => "~/WALLPAPERS",
        FolderLevel::Samples => "~/SAMPLES",
        FolderLevel::Root => "~/",
    };
    let subtitle = match folder {
        FolderLevel::Wallpapers => "NETWORK SHARE  RGBA8888 UPSTREAM IMAGES",
        FolderLevel::Samples => "NETWORK SHARE  UTF-8 SOURCE CODE MOUNTS",
        FolderLevel::Root => "",
    };

    draw_str(fb, rx, 24, title, 3, rgb(0xf0, 0xf6, 0xfc));
    draw_str(fb, rx, 56, subtitle, 2, accent);
    fb.fill(rx, 80, RIGHT_W - 96, 1, rgb(0x22, 0x30, 0x3e));

    let mut row = 0i32;
    for i in 0..app.count {
        if !app.entries[i].in_folder(folder) {
            continue;
        }
        let ry = 104 + row * 36;
        if ry + 32 > fb.h {
            break;
        }
        let entry = &app.entries[i];
        draw_str(fb, rx, ry, entry.base_name(), 2, text_col);

        let mut sz_buf = [b' '; 16];
        let sz_str = fmt_size(&mut sz_buf, entry.byte_size);
        draw_str(fb, rx + 360, ry, sz_str, 2, label_col);
        row += 1;
    }
}

pub fn click_window(app: &mut FilesApp, cx: i32, cy: i32) -> bool {
    if cx < LEFT_W && cy >= ROW_START_Y {
        let row = ((cy - ROW_START_Y) / ROW_H) as usize;
        let vis = app.visible_row_count();
        if row >= vis {
            return false;
        }
        match app.level {
            FolderLevel::Root => {
                let folder = if row == 0 {
                    FolderLevel::Wallpapers
                } else {
                    FolderLevel::Samples
                };
                return app.open_subfolder(folder);
            }
            FolderLevel::Wallpapers | FolderLevel::Samples => {
                if row == 0 {
                    return app.go_parent();
                }
                if let Some(gidx) = app.global_idx_for_row(app.level, row) {
                    let is_img = app.entries[gidx].is_image();
                    let already_staged = if is_img {
                        app.staged_idx == gidx as i32
                    } else {
                        app.staged_text_idx == gidx as i32
                    };
                    let changed = app.row_selected != row || !already_staged;
                    app.row_selected = row;
                    app.code_scroll = 0;
                    app.ensure_selected_staged();
                    return changed;
                }
            }
        }
    }
    false
}

pub fn key_window(app: &mut FilesApp, code: i32) -> bool {
    let vis = app.visible_row_count();
    if vis == 0 {
        return false;
    }
    match code {
        // Up arrow or Left arrow selects previous row
        50 | 1 => {
            app.row_selected = if app.row_selected == 0 {
                vis - 1
            } else {
                app.row_selected - 1
            };
            app.code_scroll = 0;
            app.ensure_selected_staged();
            true
        }
        // Down arrow or Right arrow selects next row
        51 | 2 => {
            app.row_selected = (app.row_selected + 1) % vis;
            app.code_scroll = 0;
            app.ensure_selected_staged();
            true
        }
        // Backspace (26) goes up to `~/`
        26 => app.go_parent(),
        // Enter (24) opens folder, goes up on `../`, or pins wallpaper on an image
        24 => match app.level {
            FolderLevel::Root => {
                let folder = if app.row_selected == 0 {
                    FolderLevel::Wallpapers
                } else {
                    FolderLevel::Samples
                };
                app.open_subfolder(folder)
            }
            FolderLevel::Wallpapers | FolderLevel::Samples => {
                if app.row_selected == 0 {
                    return app.go_parent();
                }
                if app.selected_is_image() {
                    if let Some(gidx) = app.active_file_idx() {
                        app.wallpaper_active = true;
                        app.wallpaper_idx = gidx as i32;
                        app.ensure_selected_staged();
                        return true;
                    }
                }
                false
            }
        },
        _ => false,
    }
}

/// Draw the `128 x 72` live icon in the Left App Dock.
pub fn draw_dock_tile(
    fb: &mut Framebuffer,
    app: &FilesApp,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    active: bool,
) {
    let bg = if active {
        rgb(0x13, 0x22, 0x2b)
    } else {
        rgb(0x10, 0x17, 0x20)
    };
    let border = if active {
        rgb(0x2d, 0xd4, 0xbf)
    } else {
        rgb(0x2e, 0x3f, 0x50)
    };
    fb.fill(x, y, w, h, bg);
    fb.border(x, y, w, h, 2, border);
    fb.fill(x + 2, y + 2, w - 4, 4, rgb(0x2d, 0xd4, 0xbf));

    draw_str_centered(fb, x, y + 14, w, 24, "FILES", 2, rgb(0xf0, 0xf6, 0xfc));

    let mut sub_buf = [b' '; 16];
    let sub_str = fmt_mount_count(&mut sub_buf, app.count as u32);
    draw_str_centered(fb, x, y + 42, w, 16, sub_str, 1, rgb(0x2d, 0xd4, 0xbf));
}

fn fmt_items(buf: &mut [u8; 16], count: u32) -> &str {
    let mut pos = write_u32(buf, 0, count);
    for &b in b" ITEMS" {
        if pos < buf.len() {
            buf[pos] = b;
            pos += 1;
        }
    }
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

fn fmt_size(buf: &mut [u8; 16], bytes: u32) -> &str {
    if bytes < 1024 {
        let mut pos = write_u32(buf, 0, bytes);
        if pos < buf.len() {
            buf[pos] = b'B';
            pos += 1;
        }
        return core::str::from_utf8(&buf[..pos]).unwrap_or("");
    }
    let kib = (bytes + 512) / 1024;
    let mut pos = write_u32(buf, 0, kib.max(1));
    if pos < buf.len() {
        buf[pos] = b'K';
        pos += 1;
    }
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

fn fmt_mount_count(buf: &mut [u8; 16], count: u32) -> &str {
    let mut pos = 0;
    for &b in b"~/ (" {
        buf[pos] = b;
        pos += 1;
    }
    pos = write_u32(buf, pos, count);
    if pos < buf.len() {
        buf[pos] = b')';
        pos += 1;
    }
    core::str::from_utf8(&buf[..pos]).unwrap_or("~/")
}

fn write_u32(buf: &mut [u8], mut pos: usize, mut val: u32) -> usize {
    if val == 0 {
        if pos < buf.len() {
            buf[pos] = b'0';
            pos += 1;
        }
        return pos;
    }
    let mut tmp = [0u8; 10];
    let mut n = 0;
    while val > 0 {
        tmp[n] = b'0' + (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    while n > 0 && pos < buf.len() {
        n -= 1;
        buf[pos] = tmp[n];
        pos += 1;
    }
    pos
}
