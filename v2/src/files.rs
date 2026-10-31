//! Two-Panel Document Finder & Preview Application (`files.rs`).
//!
//! Layout inside the maximized client surface (default `1280 x 688`):
//! - Left 1/3 (`x: 0..426`, `w: 426`): `~/` File Browser:
//!   - `~/wallpapers` (`wallpapers/*`)
//!   - `~/samples` (`samples/*`)
//!   - `gh://mines-sm` (`github/*`)
//! - Right 2/3 (`x: 428..1280`, `w: 852`): Live read-only Preview of the selected item
//!   (folder summary at `~/`, scaled `RGBA8888` image preview inside `~/wallpapers`,
//!   syntax-highlighted source preview inside `~/samples` / `gh://mines-sm`,
//!   or enriched `.context.xml` Knowledge Graph & Architectural Summary).

use crate::code::{count_total_lines, draw_code_preview, is_image_filename, lang_for_filename};
use crate::fb::Framebuffer;
use crate::fmt as f;
use crate::font::{draw_str, draw_str_centered, draw_str_prop, text_width};
use crate::icon::{draw_icon, Icon};
use crate::input::{InputResult, Key};
use crate::layout::{Metrics, Rect};
use crate::theme as t;
use crate::ui;

pub const MAX_MOUNT_FILES: usize = 24;
pub const MAX_NAME_LEN: usize = 48;
pub const MAX_TEXT_BYTES: usize = 16384;
pub const MAX_CONTEXT_BYTES: usize = 4096;

pub const CODE_CHUNK_LINES: usize = 30;

pub struct FilesGeom {
    pub left_w: i32,
    pub div_x: i32,
    pub right_x: i32,
    pub right_w: i32,
    pub pad_x: i32,
    pub head_y: i32,
    pub row_start_y: i32,
    pub row_h: i32,
    pub row_scale: i32,
}

pub fn files_geom(client_w: i32, client_h: i32) -> FilesGeom {
    let m = Metrics::new(client_w, client_h);
    let left_w = (client_w / 3).max(120);
    let div_x = left_w;
    let right_x = left_w + 2;
    let right_w = (client_w - right_x).max(160);
    let compact_w = left_w < 240;
    let pad_x = if compact_w { 8 } else { 24 };
    let head_y = m.pick(12, 16, 24);
    let row_start_y = m.pick(48, 60, 76);
    let row_h = m.pick(32, 38, 46);
    let row_scale = if compact_w { 1 } else { 2 };

    FilesGeom {
        left_w,
        div_x,
        right_x,
        right_w,
        pad_x,
        head_y,
        row_start_y,
        row_h,
        row_scale,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FolderLevel {
    HomeRoot = 0,
    HomeWallpapers = 1,
    HomeSamples = 2,
    GithubRepo = 3,
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

    /// Returns the display basename (stripping folder prefix if present).
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
        if full.starts_with(b"samples/") || full.starts_with(b"github/") {
            return false;
        }
        is_image_filename(self.base_name())
    }

    pub fn in_folder(&self, level: FolderLevel) -> bool {
        let full = self.name_str().as_bytes();
        match level {
            FolderLevel::HomeRoot => false,
            FolderLevel::HomeWallpapers => {
                full.starts_with(b"wallpapers/")
                    || (self.is_image()
                        && !full.starts_with(b"samples/")
                        && !full.starts_with(b"github/"))
            }
            FolderLevel::HomeSamples => {
                full.starts_with(b"samples/")
                    || (!self.is_image()
                        && !full.starts_with(b"wallpapers/")
                        && !full.starts_with(b"github/"))
            }
            FolderLevel::GithubRepo => full.starts_with(b"github/"),
        }
    }
}

pub struct FilesApp {
    pub entries: [MountEntry; MAX_MOUNT_FILES],
    pub count: usize,
    /// Current folder (`~/`, `~/wallpapers`, `~/samples`, or `gh://mines-sm`).
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
    pub code_col_scroll: usize,
    pub context_scroll: usize,
    pub font_scale: i32,
    pub tts_active: bool,
    /// Index of the context file currently loaded in the guest's context staging buffer (`-1` if none).
    pub staged_context_idx: i32,
    pub staged_context_len: usize,
    /// Whether the Right 2/3 pane shows Context metadata or Raw content.
    pub context_mode: bool,
    /// Whether the currently staged image is also pinned as the Desktop Wallpaper.
    pub wallpaper_active: bool,
    pub wallpaper_idx: i32,
}

impl FilesApp {
    pub const fn new() -> Self {
        Self {
            entries: [MountEntry::empty(); MAX_MOUNT_FILES],
            count: 0,
            level: FolderLevel::HomeRoot,
            row_selected: 0,
            selected: 0,
            pending_fetch_idx: -1,
            staged_idx: -1,
            staged_w: 0,
            staged_h: 0,
            staged_text_idx: -1,
            staged_text_len: 0,
            code_scroll: 0,
            code_col_scroll: 0,
            context_scroll: 0,
            font_scale: 1,
            tts_active: false,
            staged_context_idx: -1,
            staged_context_len: 0,
            context_mode: false,
            wallpaper_active: false,
            wallpaper_idx: -1,
        }
    }

    pub fn clear_mounts(&mut self) {
        self.entries = [MountEntry::empty(); MAX_MOUNT_FILES];
        self.count = 0;
        self.level = FolderLevel::HomeRoot;
        self.row_selected = 0;
        self.selected = 0;
        self.pending_fetch_idx = -1;
        self.staged_idx = -1;
        self.staged_w = 0;
        self.staged_h = 0;
        self.staged_text_idx = -1;
        self.staged_text_len = 0;
        self.code_scroll = 0;
        self.code_col_scroll = 0;
        self.context_scroll = 0;
        self.staged_context_idx = -1;
        self.staged_context_len = 0;
        self.context_mode = false;
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
            FolderLevel::HomeRoot => 3, // 0 = wallpapers/, 1 = samples/, 2 = gh://mines-sm/
            FolderLevel::HomeWallpapers | FolderLevel::HomeSamples | FolderLevel::GithubRepo => {
                1 + self.folder_file_count(self.level) // row 0 = `../`, rows 1..=N = files
            }
        }
    }

    /// Maps a `1`-based file row inside a subfolder to its global mount index (`0..count`).
    pub fn global_idx_for_row(&self, level: FolderLevel, row: usize) -> Option<usize> {
        if level == FolderLevel::HomeRoot || row == 0 {
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
        // Pre-select the first file (`row 1`) when opening a non-empty subfolder so preview appears immediately!
        self.row_selected = if fcount > 0 { 1 } else { 0 };
        self.code_scroll = 0;
        self.code_col_scroll = 0;
        self.context_scroll = 0;
        self.ensure_selected_staged();
        true
    }

    pub fn go_parent(&mut self) -> bool {
        match self.level {
            FolderLevel::HomeRoot => false,
            FolderLevel::HomeWallpapers => {
                self.level = FolderLevel::HomeRoot;
                self.row_selected = 0;
                true
            }
            FolderLevel::HomeSamples => {
                self.level = FolderLevel::HomeRoot;
                self.row_selected = 1;
                true
            }
            FolderLevel::GithubRepo => {
                self.level = FolderLevel::HomeRoot;
                self.row_selected = 2;
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
        self.code_col_scroll = 0;
        if self.pending_fetch_idx == idx {
            self.pending_fetch_idx = -1;
        }
    }

    pub fn complete_stage_context(&mut self, idx: i32, byte_len: usize) {
        self.staged_context_idx = idx;
        self.staged_context_len = byte_len.min(MAX_CONTEXT_BYTES);
        self.context_scroll = 0;
    }
}

impl Default for FilesApp {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_window(
    fb: &mut Framebuffer,
    app: &FilesApp,
    staged_px: &[u32],
    staged_text: &[u8],
    staged_context: &[u8],
) {
    fb.clear(t::BG);
    let g = files_geom(fb.w, fb.h);

    let accent = t::ACCENT;
    let label_col = t::TEXT_DIM;
    let text_col = t::TEXT_BODY;

    // --- 1. Left 1/3 Panel: Folder / File Browser ---
    let path_title = match app.level {
        FolderLevel::HomeRoot => "~/",
        FolderLevel::HomeWallpapers => "~/WALLPAPERS",
        FolderLevel::HomeSamples => "~/SAMPLES",
        FolderLevel::GithubRepo => "GH://MINES-SM",
    };
    let head_scale = if g.left_w < 240 { 2 } else { 3 };
    draw_str(fb, g.pad_x, g.head_y, path_title, head_scale, t::TEXT);
    fb.fill(
        g.pad_x,
        g.head_y + 10 * head_scale + 2,
        g.left_w - 2 * g.pad_x,
        2,
        accent,
    );

    let row_w = g.left_w - 2 * g.pad_x;
    let text_pad_y = (g.row_h - 4 - 7 * g.row_scale) / 2;
    let show_meta = g.left_w >= 260;

    match app.level {
        FolderLevel::HomeRoot => {
            let wp_count = app.folder_file_count(FolderLevel::HomeWallpapers) as u32;
            let sm_count = app.folder_file_count(FolderLevel::HomeSamples) as u32;
            let gh_count = app.folder_file_count(FolderLevel::GithubRepo) as u32;
            let folders = [
                ("wallpapers/", wp_count),
                ("samples/", sm_count),
                ("gh://mines-sm/", gh_count),
            ];

            for (r, &(fname, fcount)) in folders.iter().enumerate() {
                let ry = g.row_start_y + (r as i32) * g.row_h;
                let selected = r == app.row_selected;

                if selected {
                    fb.fill(g.pad_x, ry, row_w, g.row_h - 4, t::SURFACE_SELECTED);
                    fb.fill(g.pad_x, ry, 4, g.row_h - 4, accent);
                }

                let isz = if g.row_scale == 1 { 12 } else { 16 };
                draw_icon(
                    fb,
                    g.pad_x + 8,
                    ry + (g.row_h - 4 - isz) / 2,
                    Icon::Folder,
                    isz,
                    if selected { accent } else { label_col },
                );

                draw_str(
                    fb,
                    g.pad_x + 12 + isz,
                    ry + text_pad_y,
                    fname,
                    g.row_scale,
                    if selected { t::TEXT } else { text_col },
                );

                if show_meta {
                    let mut cnt_buf = [b' '; 16];
                    let cnt_str = f::items_str(&mut cnt_buf, fcount);
                    let cw = text_width(cnt_str, g.row_scale);
                    draw_str(
                        fb,
                        g.pad_x + row_w - cw - 8,
                        ry + text_pad_y,
                        cnt_str,
                        g.row_scale,
                        if selected { accent } else { label_col },
                    );
                }

                fb.fill(g.pad_x, ry + g.row_h - 4, row_w, 1, t::BORDER_DARK);
            }
        }
        FolderLevel::HomeWallpapers | FolderLevel::HomeSamples | FolderLevel::GithubRepo => {
            let total_rows = app.visible_row_count();
            for r in 0..total_rows {
                let ry = g.row_start_y + (r as i32) * g.row_h;
                if ry + g.row_h > fb.h {
                    break;
                }
                let selected = r == app.row_selected;

                if selected {
                    fb.fill(g.pad_x, ry, row_w, g.row_h - 4, t::SURFACE_SELECTED);
                    fb.fill(g.pad_x, ry, 4, g.row_h - 4, accent);
                }

                let isz = if g.row_scale == 1 { 12 } else { 16 };
                if r == 0 {
                    draw_icon(
                        fb,
                        g.pad_x + 8,
                        ry + (g.row_h - 4 - isz) / 2,
                        Icon::Up,
                        isz,
                        if selected { accent } else { label_col },
                    );
                    draw_str(
                        fb,
                        g.pad_x + 12 + isz,
                        ry + text_pad_y,
                        "../",
                        g.row_scale,
                        if selected { t::TEXT } else { text_col },
                    );
                    if show_meta {
                        let up_lbl = "UP";
                        let uw = text_width(up_lbl, g.row_scale);
                        draw_str(
                            fb,
                            g.pad_x + row_w - uw - 8,
                            ry + text_pad_y,
                            up_lbl,
                            g.row_scale,
                            if selected { accent } else { label_col },
                        );
                    }
                } else if let Some(gidx) = app.global_idx_for_row(app.level, r) {
                    let entry = &app.entries[gidx];
                    let row_icon = if entry.is_image() {
                        Icon::Image
                    } else {
                        Icon::Document
                    };
                    draw_icon(
                        fb,
                        g.pad_x + 8,
                        ry + (g.row_h - 4 - isz) / 2,
                        row_icon,
                        isz,
                        if selected { accent } else { label_col },
                    );
                    let name = entry.base_name();
                    let max_name = if show_meta {
                        18
                    } else {
                        ((row_w - 20 - isz) / (6 * g.row_scale)).max(8) as usize
                    };
                    let name_clamped = if name.len() > max_name {
                        &name[..max_name]
                    } else {
                        name
                    };
                    draw_str(
                        fb,
                        g.pad_x + 12 + isz,
                        ry + text_pad_y,
                        name_clamped,
                        g.row_scale,
                        if selected { t::TEXT } else { text_col },
                    );

                    if show_meta {
                        let mut sz_buf = [b' '; 16];
                        let sz_str = f::size_str(&mut sz_buf, entry.byte_size);
                        let sw = text_width(sz_str, g.row_scale);
                        draw_str(
                            fb,
                            g.pad_x + row_w - sw - 8,
                            ry + text_pad_y,
                            sz_str,
                            g.row_scale,
                            if selected { accent } else { label_col },
                        );
                    }
                }

                fb.fill(g.pad_x, ry + g.row_h - 4, row_w, 1, t::BORDER_DARK);
            }
        }
    }

    // --- 2. Vertical Split Divider ---
    fb.fill(g.div_x, 0, 2, fb.h, t::BORDER);

    // --- 3. Right 2/3 Panel: Control Bar + Preview ---
    if let Some(sel_idx) = app.active_file_idx() {
        let sel_entry = &app.entries[sel_idx];
        let sel_i32 = sel_idx as i32;

        // 40px Top Control Bar
        draw_preview_control_bar(fb, app, &g, sel_entry, staged_text);

        let body_y = 40;
        let body_h = (fb.h - body_y).max(0);

        if app.context_mode {
            let ctx_len = app.staged_context_len.min(staged_context.len());
            draw_context_preview(fb, app, &g, sel_entry, &staged_context[..ctx_len]);
        } else if sel_entry.is_image() {
            if app.staged_idx == sel_i32 && app.staged_w > 0 && app.staged_h > 0 {
                fb.blit_scaled(
                    g.right_x,
                    body_y,
                    g.right_w,
                    body_h,
                    staged_px,
                    app.staged_w,
                    app.staged_h,
                );
            } else {
                draw_str_centered(
                    fb,
                    g.right_x,
                    body_y,
                    g.right_w,
                    body_h,
                    "LOADING PREVIEW...",
                    g.row_scale,
                    accent,
                );
            }
        } else if app.staged_text_idx == sel_i32 {
            let len = app.staged_text_len.min(staged_text.len());
            let lang = lang_for_filename(sel_entry.base_name());
            draw_code_preview(
                fb,
                &staged_text[..len],
                g.right_x,
                body_y,
                g.right_w,
                body_h,
                lang,
                app.code_scroll,
                app.font_scale,
                app.code_col_scroll,
            );
        } else {
            draw_str_centered(
                fb,
                g.right_x,
                body_y,
                g.right_w,
                body_h,
                "LOADING PREVIEW...",
                g.row_scale,
                accent,
            );
        }
    } else {
        // Folder or `../` selected: show a clean directory summary in the right pane
        let target_folder = match app.level {
            FolderLevel::HomeRoot => {
                if app.row_selected == 0 {
                    FolderLevel::HomeWallpapers
                } else if app.row_selected == 1 {
                    FolderLevel::HomeSamples
                } else {
                    FolderLevel::GithubRepo
                }
            }
            sub => sub,
        };
        draw_folder_summary(fb, app, &g, target_folder);
    }
}

struct PreviewControls {
    mode: Rect,
    ap: Rect,
    am: Rect,
    tts: Rect,
    prev: Option<Rect>,
    next: Option<Rect>,
}

/// One source of truth for the right-pane control bar geometry. Draw and hit
/// testing both use this layout so they cannot drift apart.
fn preview_controls(app: &FilesApp, g: &FilesGeom, is_image: bool) -> PreviewControls {
    let mode_w = text_width(
        if app.context_mode {
            "[ RAW VIEW ]"
        } else {
            "[ CONTEXT ]"
        },
        1,
    ) + 12;
    let mode_x = g.right_x + g.right_w - mode_w - 12;

    let ap_w = text_width("[ A+ ]", 1) + 8;
    let am_w = text_width("[ A- ]", 1) + 8;
    let ap_x = mode_x - ap_w - 8;
    let am_x = ap_x - am_w - 4;

    let tts_w = text_width(
        if app.tts_active {
            "[ \u{23f9} STOP ]"
        } else {
            "[ \u{25b6} TTS ]"
        },
        1,
    ) + 10;
    let tts_x = am_x - tts_w - 8;

    let show_paging = !app.context_mode && !is_image && app.staged_text_idx == app.selected as i32;
    let (prev, next) = if show_paging {
        let next_w = text_width("[\u{25bc} NEXT]", 1) + 8;
        let prev_w = text_width("[\u{25b2} PREV]", 1) + 8;
        let next_x = tts_x - next_w - 8;
        let prev_x = next_x - prev_w - 4;
        (
            Some(Rect::new(prev_x, 8, prev_w, 24)),
            Some(Rect::new(next_x, 8, next_w, 24)),
        )
    } else {
        (None, None)
    };

    PreviewControls {
        mode: Rect::new(mode_x, 7, mode_w, 26),
        ap: Rect::new(ap_x, 8, ap_w, 24),
        am: Rect::new(am_x, 8, am_w, 24),
        tts: Rect::new(tts_x, 8, tts_w, 24),
        prev,
        next,
    }
}

fn draw_preview_control_bar(
    fb: &mut Framebuffer,
    app: &FilesApp,
    g: &FilesGeom,
    entry: &MountEntry,
    staged_text: &[u8],
) {
    ui::toolbar(fb, Rect::new(g.right_x, 0, g.right_w, 40));

    let mut title_buf = [0u8; 64];
    let title = fmt_file_title(&mut title_buf, app.level, entry.base_name());
    draw_str(fb, g.right_x + 12, 14, title, 1, t::TEXT);

    let c = preview_controls(app, g, entry.is_image());
    ui::button(
        fb,
        c.mode,
        if app.context_mode {
            "[ RAW VIEW ]"
        } else {
            "[ CONTEXT ]"
        },
        ui::Tone::Accent,
        1,
    );
    ui::button(fb, c.ap, "[ A+ ]", ui::Tone::Neutral, 1);
    ui::button(fb, c.am, "[ A- ]", ui::Tone::Neutral, 1);
    ui::button(
        fb,
        c.tts,
        if app.tts_active {
            "[ \u{23f9} STOP ]"
        } else {
            "[ \u{25b6} TTS ]"
        },
        if app.tts_active {
            ui::Tone::Success
        } else {
            ui::Tone::Neutral
        },
        1,
    );

    if let (Some(prev), Some(next)) = (c.prev, c.next) {
        let total_lines = count_total_lines(&staged_text[..app.staged_text_len]);
        let start_line = app.code_scroll + 1;
        let end_line = (app.code_scroll + CODE_CHUNK_LINES).min(total_lines.max(1));
        let mut range_buf = [b' '; 24];
        let range_str = f::line_range(
            &mut range_buf,
            start_line as u32,
            end_line as u32,
            total_lines as u32,
        );
        let rw = text_width(range_str, 1);
        let range_x = prev.x - rw - 8;
        if range_x > g.right_x + text_width(title, 1) + 16 {
            draw_str(fb, range_x, 14, range_str, 1, t::TEXT_DIM);
        }
        ui::button(fb, prev, "[\u{25b2} PREV]", ui::Tone::Neutral, 1);
        ui::button(fb, next, "[\u{25bc} NEXT]", ui::Tone::Neutral, 1);
    }
}

fn fmt_file_title<'a>(buf: &'a mut [u8; 64], level: FolderLevel, base_name: &str) -> &'a str {
    let mut pos = 0;
    let prefix = match level {
        FolderLevel::GithubRepo => "GH://MINES-SM/",
        FolderLevel::HomeWallpapers => "~/WALLPAPERS/",
        FolderLevel::HomeSamples => "~/SAMPLES/",
        FolderLevel::HomeRoot => "~/",
    };
    for &b in prefix.as_bytes() {
        if pos < buf.len() {
            buf[pos] = b;
            pos += 1;
        }
    }
    for &b in base_name.as_bytes() {
        if pos < buf.len() {
            // uppercase for title banner aesthetic
            let ch = if b.is_ascii_lowercase() {
                b.to_ascii_uppercase()
            } else {
                b
            };
            buf[pos] = ch;
            pos += 1;
        }
    }
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

fn draw_context_preview(
    fb: &mut Framebuffer,
    app: &FilesApp,
    g: &FilesGeom,
    entry: &MountEntry,
    context_data: &[u8],
) {
    let pad = if g.right_w < 400 { 16 } else { 32 };
    let rx = g.right_x + pad;
    let content_w = (g.right_w - 2 * pad).max(40);

    let accent = t::ACCENT;
    let key_col = t::WARNING; // warm amber for KG keys
    let label_col = t::TEXT_DIM;
    let text_col = t::TEXT_BODY;

    let scroll_px = (app.context_scroll as i32) * 14;
    let mut y = 50 - scroll_px;

    // Mini Knowledge Graph rows: SCOPE, RUNTIME, ROLE, IMPORTS, EXPORTS, DEPENDENTS, INVARIANTS
    let keys = [
        "SCOPE",
        "RUNTIME",
        "ROLE",
        "IMPORTS",
        "EXPORTS",
        "DEPENDENTS",
        "INVARIANTS",
    ];

    let key_w = 90;
    for &key in &keys {
        if y >= 40 && y + 16 <= fb.h {
            draw_str(fb, rx, y, key, 1, key_col);

            let val =
                find_context_field(context_data, key.as_bytes()).unwrap_or(if key == "SCOPE" {
                    entry.name_str()
                } else {
                    "—"
                });

            // Max characters fitting in remaining width
            let val_w = content_w - key_w;
            let max_chars = (val_w / 6).max(4) as usize;
            let val_clamped = if val.len() > max_chars {
                &val[..max_chars]
            } else {
                val
            };

            draw_str(fb, rx + key_w, y, val_clamped, 1, text_col);
        }
        y += 18;
    }

    y += 6;
    if y >= 40 && y < fb.h {
        fb.fill(rx, y, content_w, 1, t::BORDER);
    }
    y += 12;

    let font_scale = app.font_scale.clamp(1, 3);

    // TECHNICAL SUMMARY
    if y + 16 >= 40 && y < fb.h {
        draw_str(fb, rx, y, "TECHNICAL SUMMARY", 1, accent);
    }
    y += 16;
    let summary_val = find_context_field(context_data, b"SUMMARY")
        .unwrap_or("No context metadata staged for this file.");
    y = draw_word_wrapped_paragraph(fb, rx, y, content_w, summary_val, text_col, font_scale);
    y += 10;

    // ARCHITECTURAL FIT
    if let Some(arch_val) = find_context_field(context_data, b"ARCHITECTURE") {
        if y + 16 >= 40 && y < fb.h {
            draw_str(fb, rx, y, "ARCHITECTURAL FIT", 1, accent);
        }
        y += 16;
        draw_word_wrapped_paragraph(fb, rx, y, content_w, arch_val, label_col, font_scale);
    }
}

/// Look up a `KEY=VALUE\n` field from context wire data.
pub fn find_context_field<'a>(data: &'a [u8], key: &[u8]) -> Option<&'a str> {
    let mut pos = 0;
    let n = data.len();

    while pos < n {
        // Skip leading newlines / whitespace
        while pos < n && (data[pos] == b'\n' || data[pos] == b'\r') {
            pos += 1;
        }
        if pos >= n {
            break;
        }

        // Line start
        let line_start = pos;
        while pos < n && data[pos] != b'\n' && data[pos] != b'\r' {
            pos += 1;
        }
        let line = &data[line_start..pos];

        if line.starts_with(key) && line.len() > key.len() && line[key.len()] == b'=' {
            let val = &line[key.len() + 1..];
            return core::str::from_utf8(val).ok();
        }
    }
    None
}

/// Word-wraps text into lines fitting `max_w` at the given font `scale` and renders to framebuffer.
pub fn draw_word_wrapped_paragraph(
    fb: &mut Framebuffer,
    x: i32,
    mut y: i32,
    max_w: i32,
    text: &str,
    col: u32,
    scale: i32,
) -> i32 {
    let scale = scale.clamp(1, 3);
    let char_w = 6 * scale;
    let line_step = 10 * scale + 4;
    let max_chars_per_line = (max_w / char_w).max(8) as usize;
    let bytes = text.as_bytes();
    let mut start = 0;

    while start < bytes.len() {
        if y + 10 * scale > fb.h {
            break;
        }

        // Skip leading spaces
        while start < bytes.len() && bytes[start] == b' ' {
            start += 1;
        }
        if start >= bytes.len() {
            break;
        }

        let remaining = bytes.len() - start;
        let line_len = if remaining <= max_chars_per_line {
            remaining
        } else {
            // Find last space before max_chars_per_line
            let mut last_space = None;
            for i in (start..start + max_chars_per_line).rev() {
                if bytes[i] == b' ' {
                    last_space = Some(i);
                    break;
                }
            }
            match last_space {
                Some(sp) => sp - start,
                None => max_chars_per_line,
            }
        };

        if let Ok(line_str) = core::str::from_utf8(&bytes[start..start + line_len]) {
            if y >= 40 {
                draw_str(fb, x, y, line_str, scale, col);
            }
        }
        y += line_step;
        start += line_len;
    }
    y
}

fn draw_folder_summary(fb: &mut Framebuffer, app: &FilesApp, g: &FilesGeom, folder: FolderLevel) {
    let accent = t::ACCENT;
    let label_col = t::TEXT_DIM;
    let text_col = t::TEXT_BODY;

    let pad = if g.right_w < 400 { 16 } else { 48 };
    let rx = g.right_x + pad;
    let title = match folder {
        FolderLevel::HomeWallpapers => "~/WALLPAPERS",
        FolderLevel::HomeSamples => "~/SAMPLES",
        FolderLevel::GithubRepo => "GH://MINES-SM",
        FolderLevel::HomeRoot => "~/",
    };
    let subtitle = match folder {
        FolderLevel::HomeWallpapers => "RGBA8888 UPSTREAM IMAGES",
        FolderLevel::HomeSamples => "UTF-8 GENERAL CODE SAMPLES",
        FolderLevel::GithubRepo => "BITBOARD MINESWEEPER STATE MACHINE",
        FolderLevel::HomeRoot => "",
    };

    let head_sc = if g.right_w < 360 { 2 } else { 3 };
    let body_sc = if g.right_w < 360 { 1 } else { 2 };
    draw_str(fb, rx, g.head_y, title, head_sc, t::TEXT);
    draw_str(
        fb,
        rx,
        g.head_y + 10 * head_sc + 4,
        subtitle,
        body_sc,
        accent,
    );
    let rule_y = g.head_y + 10 * head_sc + 4 + 10 * body_sc + 4;
    fb.fill(rx, rule_y, (g.right_w - 2 * pad).max(40), 1, t::BORDER);

    let mut row = 0i32;
    let step = if body_sc == 1 { 22 } else { 34 };
    for i in 0..app.count {
        if !app.entries[i].in_folder(folder) {
            continue;
        }
        let ry = rule_y + 18 + row * step;
        if ry + 16 > fb.h {
            break;
        }
        let entry = &app.entries[i];
        draw_str(fb, rx, ry, entry.base_name(), body_sc, text_col);

        let mut sz_buf = [b' '; 16];
        let sz_str = f::size_str(&mut sz_buf, entry.byte_size);
        let sw = text_width(sz_str, body_sc);
        draw_str(
            fb,
            rx + (g.right_w - 2 * pad) - sw,
            ry,
            sz_str,
            body_sc,
            label_col,
        );
        row += 1;
    }
}

pub fn click_window(
    app: &mut FilesApp,
    cx: i32,
    cy: i32,
    client_w: i32,
    client_h: i32,
) -> InputResult {
    let g = files_geom(client_w, client_h);

    // Left panel: folder / file browser rows.
    if cx < g.left_w && cy >= g.row_start_y {
        let row = ((cy - g.row_start_y) / g.row_h) as usize;
        let vis = app.visible_row_count();
        if row >= vis {
            return InputResult::Noop;
        }
        match app.level {
            FolderLevel::HomeRoot => {
                let folder = if row == 0 {
                    FolderLevel::HomeWallpapers
                } else if row == 1 {
                    FolderLevel::HomeSamples
                } else {
                    FolderLevel::GithubRepo
                };
                return InputResult::from(app.open_subfolder(folder));
            }
            FolderLevel::HomeWallpapers | FolderLevel::HomeSamples | FolderLevel::GithubRepo => {
                if row == 0 {
                    return InputResult::from(app.go_parent());
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
                    app.code_col_scroll = 0;
                    app.context_scroll = 0;
                    app.ensure_selected_staged();
                    return InputResult::from(changed);
                }
            }
        }
    }

    // Right panel: shared control-bar geometry.
    if cx >= g.right_x && cy < 40 {
        if let Some(sel_idx) = app.active_file_idx() {
            let c = preview_controls(app, &g, app.entries[sel_idx].is_image());

            if c.mode.contains(cx, cy) {
                app.context_mode = !app.context_mode;
                return InputResult::Changed;
            }
            if c.ap.contains(cx, cy) {
                app.font_scale = (app.font_scale + 1).min(3);
                return InputResult::Changed;
            }
            if c.am.contains(cx, cy) {
                app.font_scale = (app.font_scale - 1).max(1);
                return InputResult::Changed;
            }
            if c.tts.contains(cx, cy) {
                app.tts_active = !app.tts_active;
                return InputResult::Tts;
            }
            if let Some(prev) = c.prev {
                if prev.contains(cx, cy) {
                    app.code_scroll = app.code_scroll.saturating_sub(CODE_CHUNK_LINES);
                    return InputResult::Changed;
                }
            }
            if let Some(next) = c.next {
                if next.contains(cx, cy) {
                    app.code_scroll = app.code_scroll.saturating_add(CODE_CHUNK_LINES);
                    return InputResult::Changed;
                }
            }
        }
    }

    InputResult::Noop
}

pub fn key_window(app: &mut FilesApp, key: Key) -> InputResult {
    let vis = app.visible_row_count();
    if vis == 0 {
        return InputResult::Noop;
    }
    match key {
        Key::Up | Key::Left => {
            app.row_selected = if app.row_selected == 0 {
                vis - 1
            } else {
                app.row_selected - 1
            };
            app.code_scroll = 0;
            app.code_col_scroll = 0;
            app.context_scroll = 0;
            app.ensure_selected_staged();
            InputResult::Changed
        }
        Key::Down | Key::Right => {
            app.row_selected = (app.row_selected + 1) % vis;
            app.code_scroll = 0;
            app.code_col_scroll = 0;
            app.context_scroll = 0;
            app.ensure_selected_staged();
            InputResult::Changed
        }
        Key::Backspace => InputResult::from(app.go_parent()),
        Key::Tab => {
            app.context_mode = !app.context_mode;
            InputResult::Changed
        }
        Key::Plus => {
            app.font_scale = (app.font_scale + 1).min(3);
            InputResult::Changed
        }
        Key::Minus => {
            app.font_scale = (app.font_scale - 1).max(1);
            InputResult::Changed
        }
        Key::ScrollLeft => {
            app.code_col_scroll = app.code_col_scroll.saturating_sub(8);
            InputResult::Changed
        }
        Key::ScrollRight => {
            app.code_col_scroll = app.code_col_scroll.saturating_add(8);
            InputResult::Changed
        }
        Key::ScrollDown => {
            app.code_scroll = app.code_scroll.saturating_add(5);
            app.context_scroll = app.context_scroll.saturating_add(2);
            InputResult::Changed
        }
        Key::ScrollUp => {
            app.code_scroll = app.code_scroll.saturating_sub(5);
            app.context_scroll = app.context_scroll.saturating_sub(2);
            InputResult::Changed
        }
        Key::PageUp => {
            if app.code_scroll > 0 {
                app.code_scroll = app.code_scroll.saturating_sub(CODE_CHUNK_LINES);
                InputResult::Changed
            } else {
                InputResult::Noop
            }
        }
        Key::PageDown => {
            app.code_scroll = app.code_scroll.saturating_add(CODE_CHUNK_LINES);
            InputResult::Changed
        }
        Key::Enter => match app.level {
            FolderLevel::HomeRoot => {
                let folder = if app.row_selected == 0 {
                    FolderLevel::HomeWallpapers
                } else if app.row_selected == 1 {
                    FolderLevel::HomeSamples
                } else {
                    FolderLevel::GithubRepo
                };
                InputResult::from(app.open_subfolder(folder))
            }
            FolderLevel::HomeWallpapers | FolderLevel::HomeSamples | FolderLevel::GithubRepo => {
                if app.row_selected == 0 {
                    return InputResult::from(app.go_parent());
                }
                if app.selected_is_image() {
                    if let Some(gidx) = app.active_file_idx() {
                        app.wallpaper_active = true;
                        app.wallpaper_idx = gidx as i32;
                        app.ensure_selected_staged();
                        return InputResult::Changed;
                    }
                }
                InputResult::Noop
            }
        },
        _ => InputResult::Noop,
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
    ui::dock_tile(fb, Rect::new(x, y, w, h), active);
    draw_icon(fb, x + 10, y + 24, Icon::Files, 32, t::ACCENT);
    draw_str_prop(fb, x + 48, y + 18, "FILES", 2, t::TEXT);

    let mut sub_buf = [b' '; 16];
    let sub_str = f::mount_count(&mut sub_buf, app.count as u32);
    draw_str_prop(fb, x + 48, y + 42, sub_str, 1, t::ACCENT);
}
