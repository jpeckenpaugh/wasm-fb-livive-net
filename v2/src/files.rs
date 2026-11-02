//! Two-Panel Document Finder & Preview Application (`files.rs`).
//!
//! Layout inside the maximized client surface (default `1280 x 688`):
//! - Left 1/3 (`x: 0..426`, `w: 426`): `~/` File Browser:
//!   - `~/wallpapers` (`wallpapers/*`)
//!   - `~/samples` (`samples/*`)
//!   - `gh://<owner>/<repo>/` (`gh/<owner>/<repo>/*`)
//!   - `web://<site>/` (`web/<site>/*` captured web snapshot)
//! - Right 2/3 (`x: 428..1280`, `w: 852`): Live read-only Preview of the selected item
//!   (folder summary at `~/`, scaled `RGBA8888` image preview inside `~/wallpapers`,
//!   syntax-highlighted source preview inside `~/samples` / `gh://<owner>/<repo>/`,
//!   or enriched `.context.xml` Knowledge Graph & Architectural Summary).

use crate::code::{count_total_lines, draw_code_preview, is_image_filename, lang_for_filename};
use crate::fb::Framebuffer;
use crate::fmt as f;
use crate::font::{
    char_advance, draw_str, draw_str_centered, draw_str_prop, glyph_height, line_height, text_width,
    text_width_prop,
};
use crate::icon::{draw_icon, Icon};
use crate::input::{InputResult, Key, Mods};
use crate::layout::{Metrics, Rect};
use crate::theme as t;
use crate::ui;

pub const MAX_MOUNT_FILES: usize = 128;
pub const MAX_NAME_LEN: usize = 64;
pub const MAX_TEXT_BYTES: usize = 768 * 1024;
pub const MAX_CONTEXT_BYTES: usize = 4096;
pub const WEB_META_BYTES: usize = 512;
pub const OPEN_TARGET_BYTES: usize = 128;

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
    pub row_icon: i32,
}

pub fn files_geom(client_w: i32, client_h: i32) -> FilesGeom {
    let m = Metrics::new(client_w, client_h);
    let ratio = if client_w < 700 { 48 } else { 37 };
    let left_w = (client_w * ratio / 100).max(120);
    let div_x = left_w;
    let right_x = left_w + 2;
    let right_w = (client_w - right_x).max(160);
    let compact_w = left_w < 240;
    let pad_x = if compact_w { 8 } else { 24 };
    let head_y = m.pick(12, 16, 24);
    let row_start_y = m.pick(48, 60, 76);
    let row_h = m.pick(32, 38, 46);
    let row_scale = 2;
    let row_icon = if compact_w { 12 } else { 16 };

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
        row_icon,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FolderLevel {
    HomeRoot = 0,
    HomeWallpapers = 1,
    HomeSamples = 2,
    GhRoot = 3,
    GhRepo = 4,
    WebRoot = 5,
    WebSite = 6,
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
        if full.starts_with(b"samples/")
            || full.starts_with(b"gh/")
            || full.starts_with(b"web/")
        {
            return is_image_filename(self.base_name());
        }
        is_image_filename(self.base_name())
    }

    pub fn is_web(&self) -> bool {
        self.name_str().as_bytes().starts_with(b"web/")
    }

    pub fn is_gh(&self) -> bool {
        self.name_str().as_bytes().starts_with(b"gh/")
    }

    /// `gh/<owner>/<repo>/...` -> `(owner, repo)`.
    pub fn gh_repo(&self) -> Option<(&str, &str)> {
        let rest = self.name_str().strip_prefix("gh/")?;
        let slash = rest.find('/')?;
        let owner = &rest[..slash];
        let rest = &rest[slash + 1..];
        let slash = rest.find('/')?;
        let repo = &rest[..slash];
        if owner.is_empty() || repo.is_empty() {
            None
        } else {
            Some((owner, repo))
        }
    }

    /// `web/<site>/...` -> `<site>`.
    pub fn web_site(&self) -> Option<&str> {
        let rest = self.name_str().strip_prefix("web/")?;
        let slash = rest.find('/')?;
        let site = &rest[..slash];
        if site.is_empty() {
            None
        } else {
            Some(site)
        }
    }

    pub fn in_folder(&self, level: FolderLevel) -> bool {
        let full = self.name_str().as_bytes();
        match level {
            FolderLevel::HomeRoot => false,
            FolderLevel::HomeWallpapers => {
                full.starts_with(b"wallpapers/")
                    || (self.is_image()
                        && !full.starts_with(b"samples/")
                        && !full.starts_with(b"gh/")
                        && !full.starts_with(b"web/"))
            }
            FolderLevel::HomeSamples => {
                full.starts_with(b"samples/")
                    || (!self.is_image()
                        && !full.starts_with(b"wallpapers/")
                        && !full.starts_with(b"gh/")
                        && !full.starts_with(b"web/"))
            }
            FolderLevel::GhRoot
            | FolderLevel::GhRepo
            | FolderLevel::WebRoot
            | FolderLevel::WebSite => false,
        }
    }
}

/// Keyboard focus inside the Files app.
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilesFocus {
    Browser,
    Mode,
    FontInc,
    FontDec,
    Tts,
    Open,
    Prev,
    Next,
}

impl FilesFocus {
    pub const CONTROLS: [FilesFocus; 7] = [
        FilesFocus::Mode,
        FilesFocus::FontInc,
        FilesFocus::FontDec,
        FilesFocus::Tts,
        FilesFocus::Open,
        FilesFocus::Prev,
        FilesFocus::Next,
    ];

    pub fn next(self, paging: bool, open: bool) -> Self {
        match self {
            FilesFocus::Browser => FilesFocus::Mode,
            FilesFocus::Mode => FilesFocus::FontInc,
            FilesFocus::FontInc => FilesFocus::FontDec,
            FilesFocus::FontDec => FilesFocus::Tts,
            FilesFocus::Tts => {
                if open {
                    FilesFocus::Open
                } else if paging {
                    FilesFocus::Prev
                } else {
                    FilesFocus::Browser
                }
            }
            FilesFocus::Open => {
                if paging {
                    FilesFocus::Prev
                } else {
                    FilesFocus::Browser
                }
            }
            FilesFocus::Prev => FilesFocus::Next,
            FilesFocus::Next => FilesFocus::Browser,
        }
    }

    pub fn prev(self, paging: bool, open: bool) -> Self {
        match self {
            FilesFocus::Browser => {
                if paging {
                    FilesFocus::Next
                } else if open {
                    FilesFocus::Open
                } else {
                    FilesFocus::Tts
                }
            }
            FilesFocus::Mode => FilesFocus::Browser,
            FilesFocus::FontInc => FilesFocus::Mode,
            FilesFocus::FontDec => FilesFocus::FontInc,
            FilesFocus::Tts => FilesFocus::FontDec,
            FilesFocus::Open => FilesFocus::Tts,
            FilesFocus::Prev => {
                if open {
                    FilesFocus::Open
                } else {
                    FilesFocus::Tts
                }
            }
            FilesFocus::Next => FilesFocus::Prev,
        }
    }
}

pub struct FilesApp {
    pub entries: [MountEntry; MAX_MOUNT_FILES],
    pub count: usize,
    /// Current folder (`~/`, `~/wallpapers`, `~/samples`, `gh://<owner>/<repo>/`, or `web://<site>/`).
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
    /// Keyboard focus inside the Files pane.
    pub focus: FilesFocus,
    pub focus_visible: bool,
    pub hover_row: Option<usize>,
    pub hover_summary_row: Option<usize>,
    pub hover_control: Option<FilesFocus>,
    /// Selected `gh/<owner>/<repo>` path.
    pub gh_repo: [u8; MAX_NAME_LEN],
    pub gh_repo_len: usize,
    /// Selected `web/<site>` path.
    pub web_site: [u8; MAX_NAME_LEN],
    pub web_site_len: usize,
    /// Host-provided `KEY=VALUE\n` capture metadata for the `web://` root.
    pub web_meta: [u8; WEB_META_BYTES],
    pub web_meta_len: usize,
    /// Guest asked the host to re-capture the site.
    pub web_refresh_requested: bool,
    /// Symbolic `web/...` or `gh/...` target for the host to open externally.
    pub open_target: [u8; OPEN_TARGET_BYTES],
    pub open_target_len: usize,
    /// Per-mount staging error (`-1` if none).
    pub error_idx: i32,
    pub error_code: i32,
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
            font_scale: 2,
            tts_active: false,
            staged_context_idx: -1,
            staged_context_len: 0,
            context_mode: false,
            wallpaper_active: false,
            wallpaper_idx: -1,
            focus: FilesFocus::Browser,
            focus_visible: false,
            hover_row: None,
            hover_summary_row: None,
            hover_control: None,
            gh_repo: [0; MAX_NAME_LEN],
            gh_repo_len: 0,
            web_site: [0; MAX_NAME_LEN],
            web_site_len: 0,
            web_meta: [0; WEB_META_BYTES],
            web_meta_len: 0,
            web_refresh_requested: false,
            open_target: [0; OPEN_TARGET_BYTES],
            open_target_len: 0,
            error_idx: -1,
            error_code: 0,
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
        self.focus = FilesFocus::Browser;
        self.focus_visible = false;
        self.hover_row = None;
        self.hover_summary_row = None;
        self.hover_control = None;
        self.gh_repo = [0; MAX_NAME_LEN];
        self.gh_repo_len = 0;
        self.web_site = [0; MAX_NAME_LEN];
        self.web_site_len = 0;
        self.web_meta = [0; WEB_META_BYTES];
        self.web_meta_len = 0;
        self.web_refresh_requested = false;
        self.open_target = [0; OPEN_TARGET_BYTES];
        self.open_target_len = 0;
        self.error_idx = -1;
        self.error_code = 0;
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

    pub fn web_field(&self, key: &str) -> Option<&str> {
        if self.web_meta_len == 0 {
            return None;
        }
        find_context_field(&self.web_meta[..self.web_meta_len], key.as_bytes())
    }

    pub fn web_state(&self) -> &str {
        self.web_field("STATE").unwrap_or("unconfigured")
    }

    pub fn web_configured(&self) -> bool {
        self.web_state() != "unconfigured" || self.web_site_count() > 0
    }

    pub fn gh_repo_name(&self) -> &str {
        core::str::from_utf8(&self.gh_repo[..self.gh_repo_len]).unwrap_or("")
    }

    pub fn web_site_name(&self) -> &str {
        core::str::from_utf8(&self.web_site[..self.web_site_len]).unwrap_or("")
    }

    pub fn gh_repo_count(&self) -> usize {
        let mut n = 0;
        for i in 0..self.count {
            let Some((owner, repo)) = self.entries[i].gh_repo() else {
                continue;
            };
            let mut duplicate = false;
            for j in 0..i {
                if let Some((prev_owner, prev_repo)) = self.entries[j].gh_repo() {
                    if prev_owner == owner && prev_repo == repo {
                        duplicate = true;
                        break;
                    }
                }
            }
            if !duplicate {
                n += 1;
            }
        }
        n
    }

    pub fn gh_repo_at(&self, ord: usize) -> Option<&str> {
        let mut seen = 0;
        for i in 0..self.count {
            let Some((owner, repo)) = self.entries[i].gh_repo() else {
                continue;
            };
            let mut duplicate = false;
            for j in 0..i {
                if let Some((prev_owner, prev_repo)) = self.entries[j].gh_repo() {
                    if prev_owner == owner && prev_repo == repo {
                        duplicate = true;
                        break;
                    }
                }
            }
            if duplicate {
                continue;
            }
            if seen == ord {
                let full = self.entries[i].name_str();
                let start = 3;
                let len = owner.len() + 1 + repo.len();
                return Some(&full[start..start + len]);
            }
            seen += 1;
        }
        None
    }

    pub fn gh_repo_row(&self, repo: &str) -> Option<usize> {
        for ord in 0..self.gh_repo_count() {
            if self.gh_repo_at(ord) == Some(repo) {
                return Some(ord + 1);
            }
        }
        None
    }

    pub fn open_gh_repo_at(&mut self, ord: usize) -> bool {
        let (len, buf) = {
            let Some(repo) = self.gh_repo_at(ord) else {
                return false;
            };
            let len = repo.len().min(MAX_NAME_LEN);
            let mut buf = [0u8; MAX_NAME_LEN];
            buf[..len].copy_from_slice(&repo.as_bytes()[..len]);
            (len, buf)
        };
        self.gh_repo = buf;
        self.gh_repo_len = len;
        self.hover_summary_row = None;
        self.level = FolderLevel::GhRepo;
        self.row_selected = if self.folder_file_count(FolderLevel::GhRepo) > 0 {
            1
        } else {
            0
        };
        self.code_scroll = 0;
        self.code_col_scroll = 0;
        self.context_scroll = 0;
        self.ensure_selected_staged();
        true
    }

    pub fn web_site_count(&self) -> usize {
        let mut n = 0;
        for i in 0..self.count {
            let Some(site) = self.entries[i].web_site() else {
                continue;
            };
            let mut duplicate = false;
            for j in 0..i {
                if self.entries[j].web_site() == Some(site) {
                    duplicate = true;
                    break;
                }
            }
            if !duplicate {
                n += 1;
            }
        }
        if let Some(list) = self.web_field("SITES_LIST") {
            for part in list.split(',') {
                let s = part.trim();
                if s.is_empty() {
                    continue;
                }
                let mut duplicate = false;
                for i in 0..self.count {
                    if self.entries[i].web_site() == Some(s) {
                        duplicate = true;
                        break;
                    }
                }
                if !duplicate {
                    n += 1;
                }
            }
        }
        n
    }

    pub fn web_site_at(&self, ord: usize) -> Option<&str> {
        let mut seen = 0;
        for i in 0..self.count {
            let Some(site) = self.entries[i].web_site() else {
                continue;
            };
            let mut duplicate = false;
            for j in 0..i {
                if self.entries[j].web_site() == Some(site) {
                    duplicate = true;
                    break;
                }
            }
            if duplicate {
                continue;
            }
            if seen == ord {
                return Some(site);
            }
            seen += 1;
        }
        if let Some(list) = self.web_field("SITES_LIST") {
            for part in list.split(',') {
                let s = part.trim();
                if s.is_empty() {
                    continue;
                }
                let mut duplicate = false;
                for i in 0..self.count {
                    if self.entries[i].web_site() == Some(s) {
                        duplicate = true;
                        break;
                    }
                }
                if duplicate {
                    continue;
                }
                if seen == ord {
                    return Some(s);
                }
                seen += 1;
            }
        }
        None
    }

    pub fn web_site_row(&self, site: &str) -> Option<usize> {
        for ord in 0..self.web_site_count() {
            if self.web_site_at(ord) == Some(site) {
                return Some(ord + 1);
            }
        }
        None
    }

    pub fn open_web_site_at(&mut self, ord: usize) -> bool {
        let (len, buf) = {
            let Some(site) = self.web_site_at(ord) else {
                return false;
            };
            let len = site.len().min(MAX_NAME_LEN);
            let mut buf = [0u8; MAX_NAME_LEN];
            buf[..len].copy_from_slice(&site.as_bytes()[..len]);
            (len, buf)
        };
        self.web_site = buf;
        self.web_site_len = len;
        self.hover_summary_row = None;
        self.level = FolderLevel::WebSite;
        self.row_selected = if self.folder_file_count(FolderLevel::WebSite) > 0 {
            1
        } else {
            0
        };
        self.code_scroll = 0;
        self.code_col_scroll = 0;
        self.context_scroll = 0;
        self.ensure_selected_staged();
        true
    }

    fn repo_matches(entry: &MountEntry, repo: &str) -> bool {
        let Some((owner, name)) = entry.gh_repo() else {
            return false;
        };
        let mut parts = repo.splitn(2, '/');
        parts.next() == Some(owner) && parts.next() == Some(name)
    }

    pub fn repo_file_count(&self, repo: &str) -> usize {
        (0..self.count)
            .filter(|&i| Self::repo_matches(&self.entries[i], repo))
            .count()
    }

    pub fn site_file_count(&self, site: &str) -> usize {
        (0..self.count)
            .filter(|&i| self.entries[i].web_site() == Some(site))
            .count()
    }

    pub fn entry_in_folder(&self, entry: &MountEntry, folder: FolderLevel) -> bool {
        match folder {
            FolderLevel::GhRepo => Self::repo_matches(entry, self.gh_repo_name()),
            FolderLevel::WebSite => entry.web_site() == Some(self.web_site_name()),
            FolderLevel::GhRoot | FolderLevel::WebRoot | FolderLevel::HomeRoot => false,
            _ => entry.in_folder(folder),
        }
    }

    pub fn gh_title<'a>(&self, buf: &'a mut [u8; 64]) -> &'a str {
        let mut pos = 0usize;
        for &b in b"gh://" {
            if pos < buf.len() {
                buf[pos] = b;
                pos += 1;
            }
        }
        for &b in self.gh_repo_name().as_bytes() {
            if pos < buf.len() {
                buf[pos] = b;
                pos += 1;
            }
        }
        if pos < buf.len() {
            buf[pos] = b'/';
            pos += 1;
        }
        core::str::from_utf8(&buf[..pos]).unwrap_or("gh://")
    }

    pub fn web_site_title<'a>(&self, buf: &'a mut [u8; 64]) -> &'a str {
        let mut pos = 0usize;
        for &b in b"web://" {
            if pos < buf.len() {
                buf[pos] = b;
                pos += 1;
            }
        }
        for &b in self.web_site_name().as_bytes() {
            if pos < buf.len() {
                buf[pos] = b;
                pos += 1;
            }
        }
        if pos < buf.len() {
            buf[pos] = b'/';
            pos += 1;
        }
        core::str::from_utf8(&buf[..pos]).unwrap_or("web://")
    }

    pub fn web_url<'a>(&self, buf: &'a mut [u8; 64]) -> &'a str {
        let mut pos = 0usize;
        for &b in b"https://" {
            if pos < buf.len() {
                buf[pos] = b;
                pos += 1;
            }
        }
        for &b in self.web_site_name().as_bytes() {
            if pos < buf.len() {
                buf[pos] = b;
                pos += 1;
            }
        }
        if pos < buf.len() {
            buf[pos] = b'/';
            pos += 1;
        }
        core::str::from_utf8(&buf[..pos]).unwrap_or("https://")
    }

    pub fn can_open_selection(&self) -> bool {
        if let Some(idx) = self.active_file_idx() {
            let entry = &self.entries[idx];
            if entry.is_web() || entry.is_gh() {
                return true;
            }
        }
        match self.level {
            FolderLevel::WebSite | FolderLevel::GhRepo => true,
            FolderLevel::WebRoot | FolderLevel::GhRoot => self.row_selected > 0,
            _ => false,
        }
    }

    fn set_open_target(&mut self, bytes: &[u8]) -> bool {
        if bytes.is_empty() {
            return false;
        }
        let len = bytes.len().min(OPEN_TARGET_BYTES);
        self.open_target[..len].copy_from_slice(&bytes[..len]);
        self.open_target_len = len;
        true
    }

    /// Build the symbolic target for the current selection. The host validates
    /// and resolves it; the guest never hands over an arbitrary URL.
    pub fn request_open(&mut self) -> InputResult {
        let mut buf = [0u8; OPEN_TARGET_BYTES];
        let mut len = 0usize;
        if let Some(idx) = self.active_file_idx() {
            let entry = &self.entries[idx];
            if entry.is_web() || entry.is_gh() {
                let name = entry.name_str().as_bytes();
                len = name.len().min(buf.len());
                buf[..len].copy_from_slice(&name[..len]);
            }
        }
        if len == 0 {
            let prefix = |bytes: &[u8], buf: &mut [u8; OPEN_TARGET_BYTES], len: &mut usize| {
                for &b in bytes {
                    if *len < buf.len() {
                        buf[*len] = b;
                        *len += 1;
                    }
                }
            };
            match self.level {
                FolderLevel::WebSite => {
                    prefix(b"web/", &mut buf, &mut len);
                    prefix(self.web_site_name().as_bytes(), &mut buf, &mut len);
                }
                FolderLevel::GhRepo => {
                    prefix(b"gh/", &mut buf, &mut len);
                    prefix(self.gh_repo_name().as_bytes(), &mut buf, &mut len);
                }
                FolderLevel::WebRoot if self.row_selected > 0 => {
                    if let Some(site) = self.web_site_at(self.row_selected - 1) {
                        prefix(b"web/", &mut buf, &mut len);
                        prefix(site.as_bytes(), &mut buf, &mut len);
                    }
                }
                FolderLevel::GhRoot if self.row_selected > 0 => {
                    if let Some(repo) = self.gh_repo_at(self.row_selected - 1) {
                        prefix(b"gh/", &mut buf, &mut len);
                        prefix(repo.as_bytes(), &mut buf, &mut len);
                    }
                }
                _ => {}
            }
        }
        if len == 0 {
            return InputResult::Noop;
        }
        if self.set_open_target(&buf[..len]) {
            InputResult::Open
        } else {
            InputResult::Noop
        }
    }

    pub fn complete_web_meta(&mut self, byte_len: usize) {
        self.web_meta_len = byte_len.min(WEB_META_BYTES);
        if self.level == FolderLevel::WebRoot
            && self.row_selected == 0
            && self.web_site_count() > 0
        {
            self.row_selected = 1;
        }
    }

    pub fn complete_error(&mut self, idx: i32, code: i32) {
        self.error_idx = idx;
        self.error_code = code;
        if self.pending_fetch_idx == idx {
            self.pending_fetch_idx = -1;
        }
    }

    pub fn folder_file_count(&self, folder: FolderLevel) -> usize {
        let mut n = 0;
        for i in 0..self.count {
            if self.entry_in_folder(&self.entries[i], folder) {
                n += 1;
            }
        }
        n
    }

    /// Folder currently summarized in the right pane when no file is active.
    pub fn target_folder(&self) -> FolderLevel {
        match self.level {
            FolderLevel::HomeRoot => match self.row_selected {
                0 => FolderLevel::HomeWallpapers,
                1 => FolderLevel::HomeSamples,
                2 => FolderLevel::GhRoot,
                _ => FolderLevel::WebRoot,
            },
            sub => sub,
        }
    }

    pub fn summary_item_count(&self, folder: FolderLevel) -> usize {
        match folder {
            FolderLevel::GhRoot => self.gh_repo_count(),
            FolderLevel::WebRoot => self.web_site_count(),
            FolderLevel::HomeRoot => 0,
            _ => self.folder_file_count(folder),
        }
    }

    pub fn summary_entry_at(&self, folder: FolderLevel, ord: usize) -> Option<&MountEntry> {
        let mut seen = 0;
        for i in 0..self.count {
            if self.entry_in_folder(&self.entries[i], folder) {
                if seen == ord {
                    return Some(&self.entries[i]);
                }
                seen += 1;
            }
        }
        None
    }

    pub fn open_summary_row(&mut self, folder: FolderLevel, row: usize) -> bool {
        match folder {
            FolderLevel::GhRoot => self.open_gh_repo_at(row),
            FolderLevel::WebRoot => self.open_web_site_at(row),
            FolderLevel::HomeWallpapers
            | FolderLevel::HomeSamples
            | FolderLevel::GhRepo
            | FolderLevel::WebSite => {
                if row >= self.folder_file_count(folder) {
                    return false;
                }
                if self.level == FolderLevel::HomeRoot {
                    self.hover_summary_row = None;
                    self.level = folder;
                }
                self.row_selected = row + 1;
                self.code_scroll = 0;
                self.code_col_scroll = 0;
                self.context_scroll = 0;
                self.ensure_selected_staged();
                true
            }
            FolderLevel::HomeRoot => false,
        }
    }

    /// Number of clickable rows in the Left 1/3 panel for the current `level`.
    pub fn visible_row_count(&self) -> usize {
        match self.level {
            FolderLevel::HomeRoot => 4,
            FolderLevel::GhRoot => 1 + self.gh_repo_count(),
            FolderLevel::WebRoot => 1 + self.web_site_count(),
            FolderLevel::HomeWallpapers
            | FolderLevel::HomeSamples
            | FolderLevel::GhRepo
            | FolderLevel::WebSite => 1 + self.folder_file_count(self.level),
        }
    }

    /// Maps a `1`-based file row inside a subfolder to its global mount index (`0..count`).
    pub fn global_idx_for_row(&self, level: FolderLevel, row: usize) -> Option<usize> {
        if matches!(
            level,
            FolderLevel::HomeRoot | FolderLevel::GhRoot | FolderLevel::WebRoot
        ) || row == 0
        {
            return None;
        }
        let target_ord = row - 1;
        let mut ord = 0;
        for i in 0..self.count {
            if self.entry_in_folder(&self.entries[i], level) {
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
        if self.level != folder {
            self.hover_summary_row = None;
        }
        self.level = folder;
        self.row_selected = match folder {
            FolderLevel::HomeRoot => 0,
            _ if self.visible_row_count() > 1 => 1,
            _ => 0,
        };
        self.code_scroll = 0;
        self.code_col_scroll = 0;
        self.context_scroll = 0;
        self.ensure_selected_staged();
        true
    }

    pub fn go_parent(&mut self) -> bool {
        self.hover_summary_row = None;
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
            FolderLevel::GhRoot => {
                self.level = FolderLevel::HomeRoot;
                self.row_selected = 2;
                true
            }
            FolderLevel::WebRoot => {
                self.level = FolderLevel::HomeRoot;
                self.row_selected = 3;
                true
            }
            FolderLevel::GhRepo => {
                let row = self.gh_repo_row(self.gh_repo_name()).unwrap_or(1);
                self.level = FolderLevel::GhRoot;
                self.row_selected = row;
                true
            }
            FolderLevel::WebSite => {
                let row = self.web_site_row(self.web_site_name()).unwrap_or(1);
                self.level = FolderLevel::WebRoot;
                self.row_selected = row;
                true
            }
        }
    }

    pub fn complete_stage(&mut self, idx: i32, w: i32, h: i32) {
        self.staged_idx = idx;
        self.staged_w = w;
        self.staged_h = h;
        if self.error_idx == idx {
            self.error_idx = -1;
        }
        if self.pending_fetch_idx == idx {
            self.pending_fetch_idx = -1;
        }
    }

    pub fn complete_stage_text(&mut self, idx: i32, byte_len: usize) {
        self.staged_text_idx = idx;
        self.staged_text_len = byte_len.min(MAX_TEXT_BYTES);
        self.code_scroll = 0;
        self.code_col_scroll = 0;
        if self.error_idx == idx {
            self.error_idx = -1;
        }
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
    let mut gh_title_buf = [0u8; 64];
    let mut web_title_buf = [0u8; 64];
    let path_title = match app.level {
        FolderLevel::HomeRoot => "~/",
        FolderLevel::HomeWallpapers => "~/Wallpapers",
        FolderLevel::HomeSamples => "~/Samples",
        FolderLevel::GhRoot => "gh://",
        FolderLevel::GhRepo => app.gh_title(&mut gh_title_buf),
        FolderLevel::WebRoot => "web://",
        FolderLevel::WebSite => app.web_site_title(&mut web_title_buf),
    };
    let head_scale = if g.left_w < 240
        || matches!(app.level, FolderLevel::GhRepo | FolderLevel::WebSite)
    {
        2
    } else {
        3
    };
    draw_str(fb, g.pad_x, g.head_y, path_title, head_scale, t::TEXT);
    fb.fill(
        g.pad_x,
        g.head_y + line_height(head_scale) + 2,
        g.left_w - 2 * g.pad_x,
        2,
        accent,
    );

    let row_w = g.left_w - 2 * g.pad_x;
    let text_pad_y = (g.row_h - 4 - glyph_height(g.row_scale)) / 2;
    let show_meta = g.left_w >= 320;

    match app.level {
        FolderLevel::HomeRoot => {
            let wp_count = app.folder_file_count(FolderLevel::HomeWallpapers) as u32;
            let sm_count = app.folder_file_count(FolderLevel::HomeSamples) as u32;
            let gh_count = app.gh_repo_count() as u32;
            let web_count = app.web_site_count() as u32;
            let folders = [
                ("wallpapers/", wp_count),
                ("samples/", sm_count),
                ("gh://", gh_count),
                ("web://", web_count),
            ];

            for (r, &(fname, fcount)) in folders.iter().enumerate() {
                let ry = g.row_start_y + (r as i32) * g.row_h;
                let selected = r == app.row_selected;
                let hovered = app.hover_row == Some(r);

                if selected {
                    fb.fill(g.pad_x, ry, row_w, g.row_h - 4, t::SURFACE_SELECTED);
                    fb.fill(g.pad_x, ry, 4, g.row_h - 4, accent);
                } else if hovered {
                    fb.fill(g.pad_x, ry, row_w, g.row_h - 4, t::SURFACE_HOVER);
                }

                let isz = g.row_icon;
                draw_icon(
                    fb,
                    g.pad_x + 8,
                    ry + (g.row_h - 4 - isz) / 2,
                    if r == 3 { Icon::Search } else { Icon::Folder },
                    isz,
                    if selected { accent } else { label_col },
                );

                let max_name = ((row_w - 20 - isz) / char_advance(g.row_scale)).max(4) as usize;
                draw_str(
                    fb,
                    g.pad_x + 12 + isz,
                    ry + text_pad_y,
                    ui::truncate_chars(fname, max_name),
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
        FolderLevel::GhRoot | FolderLevel::WebRoot => {
            let is_gh = app.level == FolderLevel::GhRoot;
            let count = if is_gh {
                app.gh_repo_count()
            } else {
                app.web_site_count()
            };
            for r in 0..(1 + count) {
                let ry = g.row_start_y + (r as i32) * g.row_h;
                if ry + g.row_h > fb.h {
                    break;
                }
                let selected = r == app.row_selected;
                let hovered = app.hover_row == Some(r);
                if selected {
                    fb.fill(g.pad_x, ry, row_w, g.row_h - 4, t::SURFACE_SELECTED);
                    fb.fill(g.pad_x, ry, 4, g.row_h - 4, accent);
                } else if hovered {
                    fb.fill(g.pad_x, ry, row_w, g.row_h - 4, t::SURFACE_HOVER);
                }
                let isz = g.row_icon;
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
                } else {
                    let name = if is_gh {
                        app.gh_repo_at(r - 1).unwrap_or("")
                    } else {
                        app.web_site_at(r - 1).unwrap_or("")
                    };
                    let display = if is_gh {
                        name.rsplit('/').next().unwrap_or(name)
                    } else {
                        name
                    };
                    let mut name_buf = [0u8; 80];
                    let mut pos = 0usize;
                    for &b in display.as_bytes() {
                        if pos < name_buf.len() - 1 {
                            name_buf[pos] = b;
                            pos += 1;
                        }
                    }
                    name_buf[pos] = b'/';
                    pos += 1;
                    let row_name = core::str::from_utf8(&name_buf[..pos]).unwrap_or("");
                    draw_icon(
                        fb,
                        g.pad_x + 8,
                        ry + (g.row_h - 4 - isz) / 2,
                        Icon::Folder,
                        isz,
                        if selected { accent } else { label_col },
                    );
                    let max_name =
                        ((row_w - 20 - isz) / char_advance(g.row_scale)).max(4) as usize;
                    draw_str(
                        fb,
                        g.pad_x + 12 + isz,
                        ry + text_pad_y,
                        ui::truncate_chars(row_name, max_name),
                        g.row_scale,
                        if selected { t::TEXT } else { text_col },
                    );
                }
                fb.fill(g.pad_x, ry + g.row_h - 4, row_w, 1, t::BORDER_DARK);
            }
        }
        FolderLevel::HomeWallpapers
        | FolderLevel::HomeSamples
        | FolderLevel::GhRepo
        | FolderLevel::WebSite => {
            let total_rows = app.visible_row_count();
            for r in 0..total_rows {
                let ry = g.row_start_y + (r as i32) * g.row_h;
                if ry + g.row_h > fb.h {
                    break;
                }
                let selected = r == app.row_selected;
                let hovered = app.hover_row == Some(r);

                if selected {
                    fb.fill(g.pad_x, ry, row_w, g.row_h - 4, t::SURFACE_SELECTED);
                    fb.fill(g.pad_x, ry, 4, g.row_h - 4, accent);
                } else if hovered {
                    fb.fill(g.pad_x, ry, row_w, g.row_h - 4, t::SURFACE_HOVER);
                }

                let isz = g.row_icon;
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
                        let up_lbl = "Up";
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
                        ((row_w - 20 - isz) / char_advance(g.row_scale)).max(8) as usize
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

    // Keyboard focus ring on the active browser row.
    if app.focus_visible && app.focus == FilesFocus::Browser {
        let vis = app.visible_row_count();
        if app.row_selected < vis {
            let ry = g.row_start_y + app.row_selected as i32 * g.row_h;
            if ry + g.row_h - 4 <= fb.h {
                ui::focus_ring(fb, Rect::new(g.pad_x, ry, row_w, g.row_h - 4));
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
        } else if app.error_idx == sel_i32
            && app.staged_idx != sel_i32
            && app.staged_text_idx != sel_i32
        {
            let (title, detail) = match app.error_code {
                1 => ("Network error", "The host could not fetch this artifact."),
                2 => ("Capture timed out", "The site did not settle in time."),
                3 => ("Blocked", "The capture service refused this URL."),
                4 => ("Artifact too large", "The preview exceeds the staging limit."),
                5 => ("Unsupported", "This artifact type cannot be previewed."),
                _ => ("Preview unavailable", "Try another artifact or refresh the capture."),
            };
            draw_str_centered(fb, g.right_x, body_y, g.right_w, body_h / 2, title, 3, t::WARNING);
            draw_str_centered(
                fb,
                g.right_x,
                body_y,
                g.right_w,
                body_h / 2 + 34,
                detail,
                2,
                t::TEXT_DIM,
            );
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
                    "Loading preview...",
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
                "Loading preview...",
                g.row_scale,
                accent,
            );
        }
    } else {
        // Folder or `../` selected: show a clean directory summary in the right pane
        draw_folder_summary(fb, app, &g, app.target_folder());
    }
}

struct PreviewControls {
    mode: Rect,
    ap: Rect,
    am: Rect,
    tts: Rect,
    open: Option<Rect>,
    prev: Option<Rect>,
    next: Option<Rect>,
    compact: bool,
}

impl PreviewControls {
    fn rect_for(&self, focus: FilesFocus) -> Option<Rect> {
        match focus {
            FilesFocus::Mode => Some(self.mode),
            FilesFocus::FontInc => Some(self.ap),
            FilesFocus::FontDec => Some(self.am),
            FilesFocus::Tts => Some(self.tts),
            FilesFocus::Open => self.open,
            FilesFocus::Prev => self.prev,
            FilesFocus::Next => self.next,
            FilesFocus::Browser => None,
        }
    }

    fn leftmost_x(&self) -> i32 {
        let mut left = self.mode.x.min(self.ap.x).min(self.am.x).min(self.tts.x);
        if let Some(open) = self.open {
            left = left.min(open.x);
        }
        if let Some(prev) = self.prev {
            left = left.min(prev.x);
        }
        if let Some(next) = self.next {
            left = left.min(next.x);
        }
        left
    }
}

fn show_paging(app: &FilesApp, is_image: bool) -> bool {
    !app.context_mode && !is_image && app.staged_text_idx == app.selected as i32
}

/// One source of truth for the right-pane control bar geometry. Draw and hit
/// testing both use this layout so they cannot drift apart.
struct ControlLabels {
    mode: &'static str,
    tts: &'static str,
    open: &'static str,
    ap: &'static str,
    am: &'static str,
    prev: &'static str,
    next: &'static str,
}

fn control_labels(
    app: &FilesApp,
    compact: bool,
    narrow: bool,
    entry: &MountEntry,
) -> ControlLabels {
    let is_web = entry.is_web();
    let is_gh = entry.is_gh();
    if compact {
        ControlLabels {
            mode: if app.context_mode {
                "Raw"
            } else if narrow {
                "Ctx"
            } else {
                "Context"
            },
            tts: if app.tts_active { "Stop" } else { "TTS" },
            open: if narrow {
                if is_web || is_gh { "\u{2197}" } else { "" }
            } else if is_web {
                "\u{2197} Live"
            } else if is_gh {
                "\u{2197} GitHub"
            } else {
                ""
            },
            ap: "A+",
            am: "A-",
            prev: "<",
            next: ">",
        }
    } else {
        ControlLabels {
            mode: if app.context_mode {
                "[ Raw View ]"
            } else {
                "[ Context ]"
            },
            tts: if app.tts_active {
                "[\u{23f9} Stop ]"
            } else {
                "[\u{25b6} TTS ]"
            },
            open: if is_web {
                "[ \u{2197} Live ]"
            } else if is_gh {
                "[ \u{2197} GitHub ]"
            } else {
                ""
            },
            ap: "[ A+ ]",
            am: "[ A- ]",
            prev: "[\u{25b2} Prev]",
            next: "[\u{25bc} Next]",
        }
    }
}

fn preview_controls_with_compact(
    app: &FilesApp,
    g: &FilesGeom,
    entry: &MountEntry,
    compact: bool,
) -> PreviewControls {
    let narrow = g.right_w < 420;
    let ultra_narrow = g.right_w < 300;
    let labels = control_labels(app, compact, narrow, entry);
    let text_sc = if compact { 1 } else { 2 };
    let gap = if narrow { 4 } else { 8 };
    let pad = if narrow { 8 } else { 12 };

    let mode_w = text_width_prop(labels.mode, text_sc) + pad;
    let mode_x = g.right_x + g.right_w - mode_w - if narrow { 6 } else { 12 };

    let ap_w = text_width_prop(labels.ap, text_sc) + if narrow { 6 } else { 8 };
    let am_w = text_width_prop(labels.am, text_sc) + if narrow { 6 } else { 8 };
    let ap_x = mode_x - ap_w - gap;
    let am_x = ap_x - am_w - 4;

    let tts_w = text_width_prop(labels.tts, text_sc) + if narrow { 8 } else { 10 };
    let tts_x = am_x - tts_w - gap;

    let has_open = entry.is_web() || entry.is_gh();
    let open_w = if has_open {
        text_width_prop(labels.open, text_sc) + if narrow { 8 } else { 10 }
    } else {
        0
    };
    let open_x = tts_x - open_w - if has_open { gap } else { 0 };
    let open = if has_open {
        Some(Rect::new(open_x, 8, open_w, 24))
    } else {
        None
    };
    let paging_right = open.map(|r| r.x).unwrap_or(tts_x);

    let (prev, next) = if !ultra_narrow && show_paging(app, entry.is_image()) {
        let next_w = text_width_prop(labels.next, text_sc) + 8;
        let prev_w = text_width_prop(labels.prev, text_sc) + 8;
        let next_x = paging_right - next_w - gap;
        let prev_x = next_x - prev_w - 4;
        if prev_x >= g.right_x + 48 {
            (
                Some(Rect::new(prev_x, 8, prev_w, 24)),
                Some(Rect::new(next_x, 8, next_w, 24)),
            )
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    PreviewControls {
        mode: Rect::new(mode_x, 7, mode_w, 26),
        ap: Rect::new(ap_x, 8, ap_w, 24),
        am: Rect::new(am_x, 8, am_w, 24),
        tts: Rect::new(tts_x, 8, tts_w, 24),
        open,
        prev,
        next,
        compact,
    }
}

fn preview_controls(app: &FilesApp, g: &FilesGeom, entry: &MountEntry) -> PreviewControls {
    if g.right_w < 620 {
        return preview_controls_with_compact(app, g, entry, true);
    }
    let full = preview_controls_with_compact(app, g, entry, false);
    if full.leftmost_x() - (g.right_x + 12) < 160 {
        preview_controls_with_compact(app, g, entry, true)
    } else {
        full
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

    let c = preview_controls(app, g, entry);
    let labels = control_labels(app, c.compact, g.right_w < 420, entry);
    let controls_left = c.leftmost_x();
    let mut title_buf = [0u8; 64];
    let title = fmt_file_title(&mut title_buf, app, entry.base_name());
    let max_chars = (((controls_left - (g.right_x + 12) - 8) / char_advance(2)).max(3)) as usize;
    let title = ui::truncate_chars(title, max_chars);
    draw_str(fb, g.right_x + 12, 14, title, 2, t::TEXT);
    ui::button(fb, c.mode, labels.mode, ui::Tone::Accent, 1);
    ui::button(fb, c.ap, labels.ap, ui::Tone::Neutral, 1);
    ui::button(fb, c.am, labels.am, ui::Tone::Neutral, 1);
    ui::button(
        fb,
        c.tts,
        labels.tts,
        if app.tts_active {
            ui::Tone::Success
        } else {
            ui::Tone::Neutral
        },
        1,
    );
    if let Some(open) = c.open {
        ui::button(fb, open, labels.open, ui::Tone::Accent, 1);
    }

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
        let rw = text_width(range_str, 2);
        let range_x = prev.x - rw - 8;
        if range_x > g.right_x + text_width(title, 2) + 16 {
            draw_str(fb, range_x, 14, range_str, 2, t::TEXT_DIM);
        }
        ui::button(fb, prev, labels.prev, ui::Tone::Neutral, 1);
        ui::button(fb, next, labels.next, ui::Tone::Neutral, 1);
    }

    if let Some(hover) = app.hover_control {
        if let Some(r) = c.rect_for(hover) {
            ui::hover_ring(fb, r);
        }
    }
    if app.focus_visible {
        if let Some(r) = c.rect_for(app.focus) {
            ui::focus_ring(fb, r);
        }
    }
}

fn fmt_file_title<'a>(buf: &'a mut [u8; 64], app: &FilesApp, base_name: &str) -> &'a str {
    let mut prefix_buf = [0u8; 64];
    let prefix = match app.level {
        FolderLevel::GhRepo => app.gh_title(&mut prefix_buf),
        FolderLevel::WebSite => app.web_site_title(&mut prefix_buf),
        FolderLevel::HomeWallpapers => "~/Wallpapers/",
        FolderLevel::HomeSamples => "~/Samples/",
        FolderLevel::HomeRoot => "~/",
        FolderLevel::GhRoot => "gh://",
        FolderLevel::WebRoot => "web://",
    };
    let mut pos = 0;
    for &b in prefix.as_bytes() {
        if pos < buf.len() {
            buf[pos] = b;
            pos += 1;
        }
    }
    for &b in base_name.as_bytes() {
        if pos < buf.len() {
            buf[pos] = b;
            pos += 1;
        }
    }
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

fn context_label(key: &str) -> &str {
    match key {
        "FILE" => "File",
        "SCOPE" => "Scope",
        "RUNTIME" => "Runtime",
        "ROLE" => "Role",
        "IMPORTS" => "Imports",
        "EXPORTS" => "Exports",
        "DEPENDENTS" => "Dependents",
        "INVARIANTS" => "Invariants",
        "URL" => "Url",
        "STATUS" => "Status",
        "VIEWPORT" => "Viewport",
        "CAPTURED" => "Captured",
        "RENDER_MS" => "Render",
        "ARTIFACT" => "Artifact",
        "BYTES" => "Bytes",
        "TRUNCATED" => "Truncated",
        other => other,
    }
}

fn draw_context_kv_row(
    fb: &mut Framebuffer,
    rx: i32,
    mut y: i32,
    key_w: i32,
    content_w: i32,
    label: &str,
    val: &str,
    key_col: u32,
    val_col: u32,
) -> i32 {
    if y >= 40 && y + 16 <= fb.h {
        draw_str(fb, rx, y, label, 2, key_col);
    }
    let val_w = (content_w - key_w).max(40);
    let max_chars = (val_w / char_advance(2)).max(6) as usize;
    let bytes = val.as_bytes();
    if bytes.is_empty() {
        return y + 20;
    }
    let mut start = 0;
    let mut lines_drawn = 0;
    while start < bytes.len() && lines_drawn < 3 {
        while start < bytes.len() && bytes[start] == b' ' {
            start += 1;
        }
        if start >= bytes.len() {
            break;
        }
        let rem = bytes.len() - start;
        let line_len = if rem <= max_chars {
            rem
        } else {
            let mut break_at = None;
            for i in (start..start + max_chars).rev() {
                if bytes[i] == b' ' || bytes[i] == b',' || bytes[i] == b'/' {
                    break_at = Some(i + 1);
                    break;
                }
            }
            break_at.map(|b| b - start).unwrap_or(max_chars)
        };
        if let Ok(seg) = core::str::from_utf8(&bytes[start..start + line_len]) {
            if y >= 40 && y + 16 <= fb.h {
                draw_str(fb, rx + key_w, y, seg, 2, val_col);
            }
        }
        y += 20;
        start += line_len;
        lines_drawn += 1;
    }
    y
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
    let key_w = if g.right_w < 400 { 112 } else { 150 };

    // Always show the full file path at the top so narrow viewports never hide it.
    y = draw_context_kv_row(
        fb,
        rx,
        y,
        key_w,
        content_w,
        context_label("FILE"),
        entry.name_str(),
        key_col,
        t::TEXT,
    );

    // Captured web metadata header: URL, STATUS, VIEWPORT, CAPTURED, RENDER_MS.
    if entry.is_web() {
        let web_keys = ["URL", "STATUS", "VIEWPORT", "CAPTURED", "RENDER_MS", "BYTES", "TRUNCATED"];
        for &key in &web_keys {
            let val = find_context_field(context_data, key.as_bytes()).unwrap_or("—");
            y = draw_context_kv_row(
                fb,
                rx,
                y,
                key_w,
                content_w,
                context_label(key),
                val,
                key_col,
                text_col,
            );
        }
        y += 6;
        if y >= 40 && y < fb.h {
            fb.fill(rx, y, content_w, 1, t::BORDER);
        }
        y += 12;
    }

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

    for &key in &keys {
        let val = find_context_field(context_data, key.as_bytes()).unwrap_or(if key == "SCOPE" {
            entry.name_str()
        } else {
            "—"
        });
        y = draw_context_kv_row(
            fb,
            rx,
            y,
            key_w,
            content_w,
            context_label(key),
            val,
            key_col,
            text_col,
        );
    }

    y += 6;
    if y >= 40 && y < fb.h {
        fb.fill(rx, y, content_w, 1, t::BORDER);
    }
    y += 12;

    let font_scale = app.font_scale.clamp(2, 4);

    // TECHNICAL SUMMARY
    if y + 16 >= 40 && y < fb.h {
        draw_str(fb, rx, y, "Technical Summary", 2, accent);
    }
    y += 20;
    let summary_val = find_context_field(context_data, b"SUMMARY")
        .unwrap_or("No context metadata staged for this file.");
    y = draw_word_wrapped_paragraph(fb, rx, y, content_w, summary_val, text_col, font_scale);
    y += 10;

    // ARCHITECTURAL FIT
    if let Some(arch_val) = find_context_field(context_data, b"ARCHITECTURE") {
        if y + 16 >= 40 && y < fb.h {
            draw_str(fb, rx, y, "Architectural Fit", 2, accent);
        }
        y += 20;
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
    let char_w = char_advance(scale);
    let line_step = line_height(scale) + 4;
    let max_chars_per_line = (max_w / char_w).max(8) as usize;
    let bytes = text.as_bytes();
    let mut start = 0;

    while start < bytes.len() {
        if y + line_height(scale) > fb.h {
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

struct SummaryGeom {
    rx: i32,
    row_w: i32,
    head_sc: i32,
    body_sc: i32,
    rule_y: i32,
    list_y: i32,
    step: i32,
    row_h: i32,
    count: usize,
    retry_btn: Option<Rect>,
    open_btn: Option<Rect>,
}

impl SummaryGeom {
    fn row_at(&self, cx: i32, cy: i32, client_h: i32) -> Option<usize> {
        if cx < self.rx || cx >= self.rx + self.row_w || cy < self.list_y {
            return None;
        }
        let rel = cy - self.list_y;
        let idx = (rel / self.step) as usize;
        let in_row = (rel % self.step) < self.row_h;
        let ry = self.list_y + (idx as i32) * self.step;
        if idx < self.count && in_row && ry + self.row_h <= client_h {
            Some(idx)
        } else {
            None
        }
    }
}

fn summary_geom(app: &FilesApp, g: &FilesGeom, folder: FolderLevel) -> SummaryGeom {
    let pad = if g.right_w < 400 { 16 } else { 48 };
    let rx = g.right_x + pad;
    let row_w = (g.right_w - 2 * pad).max(40);
    let head_sc = if g.right_w < 360 { 2 } else { 3 };
    let body_sc = 2;
    let rule_y = g.head_y + line_height(head_sc) + 4 + line_height(body_sc) + 4;
    let list_y = if folder == FolderLevel::WebSite {
        rule_y + 44
    } else {
        rule_y + 18
    };
    let step = 34;
    let row_h = 30;
    let count = app.summary_item_count(folder);

    let needs_web_diag = matches!(folder, FolderLevel::WebRoot | FolderLevel::WebSite)
        && (matches!(
            app.web_state(),
            "failed" | "error" | "partial" | "unconfigured" | "stale"
        ) || (folder == FolderLevel::WebSite && count == 0));

    let retry_w = if row_w < 260 { 108 } else { 156 };
    let retry_btn = if needs_web_diag {
        Some(Rect::new(
            (rx + row_w - retry_w).max(rx),
            g.head_y + 2,
            retry_w.min(row_w),
            26,
        ))
    } else {
        None
    };

    let open_w = if row_w < 240 { 76 } else { 96 };
    let open_btn = if folder == FolderLevel::WebSite {
        Some(Rect::new(
            (rx + row_w - open_w).max(rx),
            rule_y + 10,
            open_w.min(row_w),
            24,
        ))
    } else {
        None
    };

    SummaryGeom {
        rx,
        row_w,
        head_sc,
        body_sc,
        rule_y,
        list_y,
        step,
        row_h,
        count,
        retry_btn,
        open_btn,
    }
}

fn draw_folder_summary(fb: &mut Framebuffer, app: &FilesApp, g: &FilesGeom, folder: FolderLevel) {
    let accent = t::ACCENT;
    let label_col = t::TEXT_DIM;
    let text_col = t::TEXT_BODY;
    let sg = summary_geom(app, g, folder);
    let rx = sg.rx;
    let row_w = sg.row_w;
    let head_sc = sg.head_sc;
    let body_sc = sg.body_sc;
    let rule_y = sg.rule_y;

    let mut gh_title_buf = [0u8; 64];
    let mut web_title_buf = [0u8; 64];
    let title = match folder {
        FolderLevel::HomeWallpapers => "~/Wallpapers",
        FolderLevel::HomeSamples => "~/Samples",
        FolderLevel::GhRoot => "gh://",
        FolderLevel::GhRepo => app.gh_title(&mut gh_title_buf),
        FolderLevel::WebRoot => "web://",
        FolderLevel::WebSite => app.web_site_title(&mut web_title_buf),
        FolderLevel::HomeRoot => "~/",
    };
    let (subtitle, sub_col) = match folder {
        FolderLevel::HomeWallpapers => ("RGBA8888 Upstream Images", accent),
        FolderLevel::HomeSamples => ("UTF-8 General Code Samples", accent),
        FolderLevel::GhRoot => ("GitHub Repositories", accent),
        FolderLevel::GhRepo => ("GitHub Repository", accent),
        FolderLevel::WebRoot => match app.web_state() {
            "ready" => ("Captured Web Snapshots", accent),
            "capturing" => ("Capturing Web Snapshots...", accent),
            "partial" => ("Partial Web Captures", t::WARNING),
            "failed" | "error" => ("Capture Service Unavailable (0 Captured)", t::WARNING),
            "stale" => ("Stale Web Captures", t::WARNING),
            "unconfigured" => ("Web Capture Not Configured", label_col),
            _ => ("Captured Web Snapshots", accent),
        },
        FolderLevel::WebSite => {
            if sg.count == 0 {
                ("Live Web Site (No Local Snapshot)", t::WARNING)
            } else {
                ("Captured Web Snapshot", accent)
            }
        }
        FolderLevel::HomeRoot => ("", accent),
    };

    let title_avail = sg
        .retry_btn
        .map(|b| (b.x - rx - 8).max(48))
        .unwrap_or(row_w);
    let max_title = (title_avail / char_advance(head_sc)).max(4) as usize;
    let max_sub = (row_w / char_advance(body_sc)).max(4) as usize;
    draw_str(
        fb,
        rx,
        g.head_y,
        ui::truncate_chars(title, max_title),
        head_sc,
        t::TEXT,
    );
    draw_str(
        fb,
        rx,
        g.head_y + line_height(head_sc) + 4,
        ui::truncate_chars(subtitle, max_sub),
        body_sc,
        sub_col,
    );
    if let Some(retry) = sg.retry_btn {
        let lbl = if retry.w < 140 {
            "Retry (F5)"
        } else {
            "[ Retry Capture ]"
        };
        ui::button(fb, retry, lbl, ui::Tone::Accent, 1);
    }
    fb.fill(rx, rule_y, row_w, 1, t::BORDER);

    if folder == FolderLevel::WebSite {
        let mut url_buf = [0u8; 64];
        let url = app.web_url(&mut url_buf);
        let url_avail = sg
            .open_btn
            .map(|b| (b.x - rx - 8).max(48))
            .unwrap_or(row_w);
        let max_url = (url_avail / char_advance(body_sc)).max(4) as usize;
        draw_str(
            fb,
            rx,
            rule_y + 16,
            ui::truncate_chars(url, max_url),
            body_sc,
            t::TEXT_DIM,
        );
        if let Some(open) = sg.open_btn {
            let lbl = if open.w < 90 {
                "\u{2197} Live"
            } else {
                "[ \u{2197} Live ]"
            };
            ui::button(fb, open, lbl, ui::Tone::Accent, 1);
        }
    }

    let text_pad_y = (sg.row_h - glyph_height(body_sc)) / 2;
    let mut rows_end_y = sg.list_y;

    if folder == FolderLevel::GhRoot || folder == FolderLevel::WebRoot {
        let is_gh = folder == FolderLevel::GhRoot;
        for r in 0..sg.count {
            let ry = sg.list_y + (r as i32) * sg.step;
            if ry + sg.row_h > fb.h {
                break;
            }
            let hovered = app.hover_summary_row == Some(r);
            if hovered {
                fb.fill(rx, ry, row_w, sg.row_h, t::SURFACE_HOVER);
            }
            let name = if is_gh {
                app.gh_repo_at(r).unwrap_or("")
            } else {
                app.web_site_at(r).unwrap_or("")
            };
            let files = if is_gh {
                app.repo_file_count(name)
            } else {
                app.site_file_count(name)
            };
            draw_icon(
                fb,
                rx + 6,
                ry + (sg.row_h - 16) / 2,
                Icon::Folder,
                16,
                if hovered { accent } else { label_col },
            );
            let mut cnt_buf = [b' '; 16];
            let cnt_str = if !is_gh && files == 0 {
                "Live (\u{2197} O)"
            } else {
                f::items_str(&mut cnt_buf, files as u32)
            };
            let cw = text_width(cnt_str, body_sc);
            let max_name = ((row_w - 36 - cw - 12) / char_advance(body_sc)).max(4) as usize;
            draw_str(
                fb,
                rx + 28,
                ry + text_pad_y,
                ui::truncate_chars(name, max_name),
                body_sc,
                if hovered { t::TEXT } else { text_col },
            );
            draw_str(
                fb,
                rx + row_w - cw - 6,
                ry + text_pad_y,
                cnt_str,
                body_sc,
                if !is_gh && files == 0 {
                    accent
                } else {
                    label_col
                },
            );
            fb.fill(rx, ry + sg.row_h, row_w, 1, t::BORDER_DARK);
            rows_end_y = ry + sg.step;
        }
    } else {
        for r in 0..sg.count {
            let ry = sg.list_y + (r as i32) * sg.step;
            if ry + sg.row_h > fb.h {
                break;
            }
            let Some(entry) = app.summary_entry_at(folder, r) else {
                break;
            };
            let hovered = app.hover_summary_row == Some(r);
            if hovered {
                fb.fill(rx, ry, row_w, sg.row_h, t::SURFACE_HOVER);
            }
            let icon = if entry.is_image() {
                Icon::Image
            } else {
                Icon::Document
            };
            draw_icon(
                fb,
                rx + 6,
                ry + (sg.row_h - 16) / 2,
                icon,
                16,
                if hovered { accent } else { label_col },
            );

            let mut sz_buf = [b' '; 16];
            let sz_str = f::size_str(&mut sz_buf, entry.byte_size);
            let sw = text_width(sz_str, body_sc);
            let max_name = ((row_w - 36 - sw - 12) / char_advance(body_sc)).max(4) as usize;
            draw_str(
                fb,
                rx + 28,
                ry + text_pad_y,
                ui::truncate_chars(entry.base_name(), max_name),
                body_sc,
                if hovered { t::TEXT } else { text_col },
            );
            draw_str(
                fb,
                rx + row_w - sw - 6,
                ry + text_pad_y,
                sz_str,
                body_sc,
                label_col,
            );
            fb.fill(rx, ry + sg.row_h, row_w, 1, t::BORDER_DARK);
            rows_end_y = ry + sg.step;
        }
    }

    if matches!(folder, FolderLevel::WebRoot | FolderLevel::WebSite)
        && (matches!(
            app.web_state(),
            "failed" | "error" | "partial" | "unconfigured"
        ) || (folder == FolderLevel::WebSite && sg.count == 0))
    {
        let mut dy = rows_end_y + 12;
        if dy + 24 <= fb.h {
            let endpoint = app
                .web_field("ENDPOINT")
                .unwrap_or("http://127.0.0.1:8791");
            let detail = app
                .web_field("DETAIL")
                .unwrap_or("Capture service unreachable or not started.");
            let key_w = if row_w < 360 { 88 } else { 112 };
            dy = draw_context_kv_row(
                fb,
                rx,
                dy,
                key_w,
                row_w,
                "Endpoint",
                endpoint,
                t::WARNING,
                text_col,
            );
            dy = draw_context_kv_row(
                fb,
                rx,
                dy,
                key_w,
                row_w,
                "Status",
                detail,
                t::WARNING,
                text_col,
            );
            dy += 4;
            let help = "Start `make web-capture` locally and allow local-network access in your browser, then press F5 or click Retry Capture. Press O or click a site to open the live URL in a new tab.";
            draw_word_wrapped_paragraph(fb, rx, dy, row_w, help, label_col, 2);
        }
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
        app.focus = FilesFocus::Browser;
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
                } else if row == 2 {
                    FolderLevel::GhRoot
                } else {
                    FolderLevel::WebRoot
                };
                return InputResult::from(app.open_subfolder(folder));
            }
            FolderLevel::GhRoot => {
                if row == 0 {
                    return InputResult::from(app.go_parent());
                }
                return InputResult::from(app.open_gh_repo_at(row - 1));
            }
            FolderLevel::WebRoot => {
                if row == 0 {
                    return InputResult::from(app.go_parent());
                }
                return InputResult::from(app.open_web_site_at(row - 1));
            }
            FolderLevel::HomeWallpapers
            | FolderLevel::HomeSamples
            | FolderLevel::GhRepo
            | FolderLevel::WebSite => {
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
            let c = preview_controls(app, &g, &app.entries[sel_idx]);

            if let Some(open) = c.open {
                if open.contains(cx, cy) {
                    app.focus = FilesFocus::Open;
                    return activate_control(app, FilesFocus::Open);
                }
            }
            if c.mode.contains(cx, cy) {
                app.focus = FilesFocus::Mode;
                return activate_control(app, FilesFocus::Mode);
            }
            if c.ap.contains(cx, cy) {
                app.focus = FilesFocus::FontInc;
                return activate_control(app, FilesFocus::FontInc);
            }
            if c.am.contains(cx, cy) {
                app.focus = FilesFocus::FontDec;
                return activate_control(app, FilesFocus::FontDec);
            }
            if c.tts.contains(cx, cy) {
                app.focus = FilesFocus::Tts;
                return activate_control(app, FilesFocus::Tts);
            }
            if let Some(prev) = c.prev {
                if prev.contains(cx, cy) {
                    app.focus = FilesFocus::Prev;
                    return activate_control(app, FilesFocus::Prev);
                }
            }
            if let Some(next) = c.next {
                if next.contains(cx, cy) {
                    app.focus = FilesFocus::Next;
                    return activate_control(app, FilesFocus::Next);
                }
            }
        }
    }

    // Right panel: folder summary rows & web action buttons when no file is active.
    if cx >= g.right_x && app.active_file_idx().is_none() {
        app.focus = FilesFocus::Browser;
        let folder = app.target_folder();
        let sg = summary_geom(app, &g, folder);
        if let Some(retry) = sg.retry_btn {
            if retry.contains(cx, cy) {
                app.web_refresh_requested = true;
                return InputResult::Changed;
            }
        }
        if let Some(open) = sg.open_btn {
            if open.contains(cx, cy) {
                return app.request_open();
            }
        }
        if let Some(r) = sg.row_at(cx, cy, client_h) {
            return InputResult::from(app.open_summary_row(folder, r));
        }
    }

    app.focus = FilesFocus::Browser;
    InputResult::Noop
}

fn activate_control(app: &mut FilesApp, focus: FilesFocus) -> InputResult {
    match focus {
        FilesFocus::Mode => {
            app.context_mode = !app.context_mode;
            InputResult::Changed
        }
        FilesFocus::FontInc => {
            app.font_scale = (app.font_scale + 1).min(4);
            InputResult::Changed
        }
        FilesFocus::FontDec => {
            app.font_scale = (app.font_scale - 1).max(2);
            InputResult::Changed
        }
        FilesFocus::Tts => {
            app.tts_active = !app.tts_active;
            InputResult::Tts
        }
        FilesFocus::Open => app.request_open(),
        FilesFocus::Prev => {
            app.code_scroll = app.code_scroll.saturating_sub(CODE_CHUNK_LINES);
            InputResult::Changed
        }
        FilesFocus::Next => {
            app.code_scroll = app.code_scroll.saturating_add(CODE_CHUNK_LINES);
            InputResult::Changed
        }
        FilesFocus::Browser => InputResult::Noop,
    }
}

pub fn key_window(app: &mut FilesApp, key: Key, mods: Mods) -> InputResult {
    let vis = app.visible_row_count();
    let paging = app
        .active_file_idx()
        .map(|idx| show_paging(app, app.entries[idx].is_image()))
        .unwrap_or(false);
    let has_open = app.can_open_selection();

    // Focus traversal and app-level toggles work from any pane.
    if key == Key::Tab {
        let next = if mods.shift() {
            app.focus.prev(paging, has_open)
        } else {
            app.focus.next(paging, has_open)
        };
        if next != app.focus {
            app.focus = next;
            return InputResult::Changed;
        }
        return InputResult::Handled;
    }
    if key == Key::Info {
        app.context_mode = !app.context_mode;
        return InputResult::Changed;
    }
    if key == Key::Refresh && app.web_configured() {
        app.web_refresh_requested = true;
        return InputResult::Changed;
    }
    if key == Key::Open {
        if app.can_open_selection() {
            return app.request_open();
        }
        return InputResult::Noop;
    }

    // Global-ish preview keys remain available while a control is focused.
    match key {
        Key::Plus => {
            app.font_scale = (app.font_scale + 1).min(4);
            return InputResult::Changed;
        }
        Key::Minus => {
            app.font_scale = (app.font_scale - 1).max(2);
            return InputResult::Changed;
        }
        Key::PageUp => {
            if app.code_scroll > 0 {
                app.code_scroll = app.code_scroll.saturating_sub(CODE_CHUNK_LINES);
                return InputResult::Changed;
            }
            return InputResult::Noop;
        }
        Key::PageDown => {
            app.code_scroll = app.code_scroll.saturating_add(CODE_CHUNK_LINES);
            return InputResult::Changed;
        }
        Key::ScrollLeft => {
            app.code_col_scroll = app.code_col_scroll.saturating_sub(8);
            return InputResult::Changed;
        }
        Key::ScrollRight => {
            app.code_col_scroll = app.code_col_scroll.saturating_add(8);
            return InputResult::Changed;
        }
        Key::ScrollDown => {
            app.code_scroll = app.code_scroll.saturating_add(5);
            app.context_scroll = app.context_scroll.saturating_add(2);
            return InputResult::Changed;
        }
        Key::ScrollUp => {
            app.code_scroll = app.code_scroll.saturating_sub(5);
            app.context_scroll = app.context_scroll.saturating_sub(2);
            return InputResult::Changed;
        }
        _ => {}
    }

    if app.focus != FilesFocus::Browser {
        return match key {
            Key::Left => {
                app.focus = app.focus.prev(paging, has_open);
                InputResult::Changed
            }
            Key::Right | Key::Tab => {
                app.focus = app.focus.next(paging, has_open);
                InputResult::Changed
            }
            Key::Enter | Key::Space => activate_control(app, app.focus),
            Key::Escape | Key::Up | Key::Down | Key::Home | Key::End => {
                app.focus = FilesFocus::Browser;
                InputResult::Changed
            }
            _ => InputResult::Noop,
        };
    }

    if vis == 0 {
        return InputResult::Noop;
    }

    match key {
        Key::Up => {
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
        Key::Down => {
            app.row_selected = (app.row_selected + 1) % vis;
            app.code_scroll = 0;
            app.code_col_scroll = 0;
            app.context_scroll = 0;
            app.ensure_selected_staged();
            InputResult::Changed
        }
        Key::Home => {
            app.row_selected = 0;
            app.code_scroll = 0;
            app.code_col_scroll = 0;
            app.context_scroll = 0;
            app.ensure_selected_staged();
            InputResult::Changed
        }
        Key::End => {
            app.row_selected = vis - 1;
            app.code_scroll = 0;
            app.code_col_scroll = 0;
            app.context_scroll = 0;
            app.ensure_selected_staged();
            InputResult::Changed
        }
        Key::Left | Key::Backspace => {
            if app.go_parent() {
                InputResult::Changed
            } else {
                InputResult::Handled
            }
        }
        Key::Right => {
            if app.active_file_idx().is_some() {
                app.focus = FilesFocus::Mode;
                InputResult::Changed
            } else {
                InputResult::Handled
            }
        }
        Key::Enter | Key::Space => enter_browser_row(app),
        Key::Escape => InputResult::Handled,
        _ => InputResult::Noop,
    }
}

fn enter_browser_row(app: &mut FilesApp) -> InputResult {
    match app.level {
        FolderLevel::HomeRoot => {
            let folder = if app.row_selected == 0 {
                FolderLevel::HomeWallpapers
            } else if app.row_selected == 1 {
                FolderLevel::HomeSamples
            } else if app.row_selected == 2 {
                FolderLevel::GhRoot
            } else {
                FolderLevel::WebRoot
            };
            InputResult::from(app.open_subfolder(folder))
        }
        FolderLevel::GhRoot => {
            if app.row_selected == 0 {
                return InputResult::from(app.go_parent());
            }
            InputResult::from(app.open_gh_repo_at(app.row_selected - 1))
        }
        FolderLevel::WebRoot => {
            if app.row_selected == 0 {
                return InputResult::from(app.go_parent());
            }
            InputResult::from(app.open_web_site_at(app.row_selected - 1))
        }
        FolderLevel::HomeWallpapers
        | FolderLevel::HomeSamples
        | FolderLevel::GhRepo
        | FolderLevel::WebSite => {
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
    }
}

pub fn pointer_move(app: &mut FilesApp, cx: i32, cy: i32, client_w: i32, client_h: i32) -> bool {
    let g = files_geom(client_w, client_h);
    let mut row = None;
    let mut control = None;
    let mut summary_row = None;

    if cx < g.left_w && cy >= g.row_start_y {
        let candidate = ((cy - g.row_start_y) / g.row_h) as usize;
        if candidate < app.visible_row_count() {
            row = Some(candidate);
        }
    } else if cx >= g.right_x {
        if let Some(sel_idx) = app.active_file_idx() {
            if cy < 40 {
                let c = preview_controls(app, &g, &app.entries[sel_idx]);
                for focus in FilesFocus::CONTROLS {
                    if let Some(r) = c.rect_for(focus) {
                        if r.contains(cx, cy) {
                            control = Some(focus);
                            break;
                        }
                    }
                }
            }
        } else {
            let folder = app.target_folder();
            let sg = summary_geom(app, &g, folder);
            summary_row = sg.row_at(cx, cy, client_h);
        }
    }

    let changed = app.hover_row != row
        || app.hover_control != control
        || app.hover_summary_row != summary_row;
    app.hover_row = row;
    app.hover_control = control;
    app.hover_summary_row = summary_row;
    changed
}

pub fn pointer_leave(app: &mut FilesApp) -> bool {
    let changed = app.hover_row.is_some()
        || app.hover_control.is_some()
        || app.hover_summary_row.is_some();
    app.hover_row = None;
    app.hover_control = None;
    app.hover_summary_row = None;
    changed
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
    draw_str_prop(fb, x + 48, y + 18, "Files", 2, t::TEXT);

    let mut sub_buf = [b' '; 16];
    let sub_str = f::mount_count(&mut sub_buf, app.count as u32);
    draw_str_prop(fb, x + 48, y + 42, sub_str, 2, t::ACCENT);
}

#[cfg(test)]
mod web_tests {
    use super::*;

    fn app_with_web() -> FilesApp {
        let mut app = FilesApp::new();
        app.add_mount(b"wallpapers/01.png", 10);
        app.add_mount(b"samples/app.js", 10);
        app.add_mount(b"gh/jpeckenpaugh/mines-sm/lib.rs", 10);
        app.add_mount(b"gh/jpeckenpaugh/mines-sm/Cargo.toml", 10);
        app.add_mount(b"web/livive.net/index.html", 10);
        app.add_mount(b"web/livive.net/page.png", 10);
        app.add_mount(b"web/gm.livive.net/index.html", 10);
        app
    }

    #[test]
    fn namespaces_do_not_leak_into_home_folders() {
        let app = app_with_web();
        assert_eq!(app.gh_repo_count(), 1);
        assert_eq!(app.gh_repo_at(0), Some("jpeckenpaugh/mines-sm"));
        assert_eq!(app.web_site_count(), 2);
        assert_eq!(app.web_site_at(0), Some("livive.net"));
        assert!(!app.entries[2].in_folder(FolderLevel::HomeSamples));
        assert!(!app.entries[4].in_folder(FolderLevel::HomeSamples));
        assert!(!app.entries[5].in_folder(FolderLevel::HomeWallpapers));
        assert!(app.entries[5].is_image());
        assert!(!app.entries[4].is_image());
    }

    #[test]
    fn nested_gh_and_web_folders_open_and_parent_correctly() {
        let mut app = app_with_web();
        assert!(app.open_gh_repo_at(0));
        assert_eq!(app.level, FolderLevel::GhRepo);
        assert_eq!(app.gh_repo_name(), "jpeckenpaugh/mines-sm");
        assert_eq!(app.active_file_idx(), Some(2));
        assert!(app.go_parent());
        assert_eq!(app.level, FolderLevel::GhRoot);
        assert_eq!(app.row_selected, 1);
        assert!(app.go_parent());
        assert_eq!(app.level, FolderLevel::HomeRoot);
        assert_eq!(app.row_selected, 2);

        app.row_selected = 3;
        assert!(app.open_subfolder(FolderLevel::WebRoot));
        assert_eq!(app.row_selected, 1);
        assert!(app.open_web_site_at(0));
        assert_eq!(app.level, FolderLevel::WebSite);
        assert_eq!(app.web_site_name(), "livive.net");
        assert_eq!(app.active_file_idx(), Some(4));
        assert!(app.go_parent());
        assert_eq!(app.level, FolderLevel::WebRoot);
        assert_eq!(app.row_selected, 1);
    }

    #[test]
    fn open_targets_cover_web_and_gh_selections() {
        let mut app = app_with_web();
        assert!(app.open_web_site_at(0));
        app.row_selected = 1;
        assert!(app.can_open_selection());
        assert_eq!(app.request_open(), InputResult::Open);
        assert_eq!(&app.open_target[..app.open_target_len], b"web/livive.net/index.html");

        assert!(app.open_gh_repo_at(0));
        app.row_selected = 1;
        assert!(app.can_open_selection());
        assert_eq!(app.request_open(), InputResult::Open);
        assert_eq!(
            &app.open_target[..app.open_target_len],
            b"gh/jpeckenpaugh/mines-sm/lib.rs"
        );

        app.row_selected = 0;
        assert_eq!(app.request_open(), InputResult::Open);
        assert_eq!(
            &app.open_target[..app.open_target_len],
            b"gh/jpeckenpaugh/mines-sm"
        );

        app.level = FolderLevel::HomeWallpapers;
        assert!(!app.can_open_selection());
        assert_eq!(app.request_open(), InputResult::Noop);
    }

    #[test]
    fn web_meta_does_not_change_home_row_count() {
        let mut app = app_with_web();
        assert_eq!(app.visible_row_count(), 4);
        let meta = b"STATE=ready
SITES=2
";
        app.complete_web_meta(meta.len());
        app.web_meta[..meta.len()].copy_from_slice(meta);
        app.complete_web_meta(meta.len());
        assert!(app.web_configured());
        assert_eq!(app.visible_row_count(), 4);
        app.level = FolderLevel::WebRoot;
        app.row_selected = 0;
        app.complete_web_meta(meta.len());
        assert_eq!(app.row_selected, 1);
    }

    #[test]
    fn right_pane_summary_clicks_activate_files_and_folders() {
        let mut app = FilesApp::new();
        app.add_mount(b"wallpapers/01.png", 100);
        app.add_mount(b"wallpapers/02.png", 200);
        app.add_mount(b"samples/app.js", 300);
        app.add_mount(b"samples/lib.rs", 400);
        app.add_mount(b"gh/jpeckenpaugh/mines-sm/lib.rs", 500);
        app.add_mount(b"gh/jpeckenpaugh/wasm-fb/main.rs", 600);
        app.add_mount(b"web/livive.net/index.html", 700);
        app.add_mount(b"web/gm.livive.net/index.html", 800);

        let cw = 1280;
        let ch = 684;
        let g = files_geom(cw, ch);

        // 1. At HomeRoot with row_selected = 0 (wallpapers/), click 1st and 2nd right-pane summary entries.
        assert_eq!(app.level, FolderLevel::HomeRoot);
        assert_eq!(app.row_selected, 0);
        let sg = summary_geom(&app, &g, app.target_folder());
        let rx = sg.rx + 40;
        let row0_y = sg.list_y + 10;
        let row1_y = sg.list_y + sg.step + 10;

        assert!(pointer_move(&mut app, rx, row1_y, cw, ch));
        assert_eq!(app.hover_summary_row, Some(1));

        // Clicking blank space below the list returns Noop.
        let blank_y = sg.list_y + 5 * sg.step + 10;
        assert_eq!(click_window(&mut app, rx, blank_y, cw, ch), InputResult::Noop);
        assert_eq!(app.level, FolderLevel::HomeRoot);

        // Clicking 2nd wallpaper opens HomeWallpapers with row_selected = 2 (index 1).
        assert_eq!(click_window(&mut app, rx, row1_y, cw, ch), InputResult::Changed);
        assert_eq!(app.level, FolderLevel::HomeWallpapers);
        assert_eq!(app.row_selected, 2);
        assert_eq!(app.active_file_idx(), Some(1));
        assert_eq!(app.pending_fetch_idx, 1);

        // Go back to HomeRoot, select samples/ (row 1), click 1st sample in right pane.
        assert!(app.go_parent());
        app.row_selected = 1;
        assert_eq!(click_window(&mut app, rx, row0_y, cw, ch), InputResult::Changed);
        assert_eq!(app.level, FolderLevel::HomeSamples);
        assert_eq!(app.row_selected, 1);
        assert_eq!(app.active_file_idx(), Some(2));

        // Go to GhRoot, select "../" (row 0) so right pane summarizes GhRoot, click 2nd repo.
        app.level = FolderLevel::GhRoot;
        app.row_selected = 0;
        assert_eq!(click_window(&mut app, rx, row1_y, cw, ch), InputResult::Changed);
        assert_eq!(app.level, FolderLevel::GhRepo);
        assert_eq!(app.gh_repo_name(), "jpeckenpaugh/wasm-fb");
        assert_eq!(app.active_file_idx(), Some(5));

        // Go to WebRoot, select "../" (row 0) so right pane summarizes WebRoot, click 2nd site.
        app.level = FolderLevel::WebRoot;
        app.row_selected = 0;
        assert_eq!(click_window(&mut app, rx, row1_y, cw, ch), InputResult::Changed);
        assert_eq!(app.level, FolderLevel::WebSite);
        assert_eq!(app.web_site_name(), "gm.livive.net");
        assert_eq!(app.active_file_idx(), Some(7));
    }

    #[test]
    fn web_failed_state_lists_configured_sites_and_retry_button_works() {
        let mut app = FilesApp::new();
        let meta = b"STATE=failed\nSITES=4\nREADY=0/4\nENDPOINT=http://127.0.0.1:8791\nSITES_LIST=livive.net,gm.livive.net,jpeckenpaugh.com,jeapostrophy.github.io\nDETAIL=0/4 captured\n";
        app.web_meta[..meta.len()].copy_from_slice(meta);
        app.complete_web_meta(meta.len());

        assert_eq!(app.web_state(), "failed");
        assert_eq!(app.web_site_count(), 4);
        assert_eq!(app.web_site_at(0), Some("livive.net"));
        assert_eq!(app.web_site_at(2), Some("jpeckenpaugh.com"));

        app.level = FolderLevel::WebRoot;
        app.row_selected = 0;
        let cw = 1280;
        let ch = 684;
        let g = files_geom(cw, ch);
        let sg = summary_geom(&app, &g, app.target_folder());
        let retry = sg.retry_btn.expect("failed web state should expose retry button");
        assert_eq!(
            click_window(&mut app, retry.x + 8, retry.y + 8, cw, ch),
            InputResult::Changed
        );
        assert!(app.web_refresh_requested);

        // Clicking a site row in the right pane opens WebSite and allows live URL open (`O`).
        let row2_y = sg.list_y + 2 * sg.step + 10;
        assert_eq!(
            click_window(&mut app, sg.rx + 40, row2_y, cw, ch),
            InputResult::Changed
        );
        assert_eq!(app.level, FolderLevel::WebSite);
        assert_eq!(app.web_site_name(), "jpeckenpaugh.com");
        assert_eq!(app.request_open(), InputResult::Open);
        assert_eq!(
            &app.open_target[..app.open_target_len],
            b"web/jpeckenpaugh.com"
        );
    }

    #[test]
    fn preview_controls_switch_to_compact_before_overflowing_file_title() {
        let mut app = app_with_web();
        assert!(app.open_web_site_at(0));
        app.row_selected = 1; // web/livive.net/index.html (text file)
        let idx = app.active_file_idx().unwrap();
        app.complete_stage_text(idx as i32, 100);

        for &(cw, ch) in &[(1280, 684), (960, 564), (390, 808), (360, 764)] {
            let g = files_geom(cw, ch);
            let c = preview_controls(&app, &g, &app.entries[idx]);
            assert!(
                c.leftmost_x() > g.right_x + 48,
                "controls overflowed at {}x{}: leftmost={} right_x={}",
                cw,
                ch,
                c.leftmost_x(),
                g.right_x
            );
        }
    }
}
