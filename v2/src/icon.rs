//! Typed Icons32 API (`icon.rs`).
//!
//! Every [`Icon`] has a stable Private Use Area codepoint (`U+E000 + discriminant`)
//! so it can also be embedded inline in text drawn by `font::draw_str*`, e.g.
//! `"\u{E000} HOME"`. Discriminants match the sorted `glyphs/icons32/NN_name.txt`
//! files (checked by a unit test against the build-generated name table).

use crate::fb::Framebuffer;
use crate::glyph::{self, ICON_PUA_BASE};

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Home = 0,
    Back,
    Forward,
    Up,
    Close,
    Check,
    Menu,
    More,
    Plus,
    Minus,
    Search,
    Refresh,
    Settings,
    Trash,
    Edit,
    Save,
    Files,
    Folder,
    Document,
    Image,
    Calculator,
    Games,
    System,
    Clock,
    Play,
    Pause,
    Stop,
    Speaker,
    Mute,
    Info,
    Warning,
    Lock,
}

impl Icon {
    pub const ALL: [Icon; 32] = [
        Icon::Home, Icon::Back, Icon::Forward, Icon::Up, Icon::Close, Icon::Check,
        Icon::Menu, Icon::More, Icon::Plus, Icon::Minus, Icon::Search, Icon::Refresh,
        Icon::Settings, Icon::Trash, Icon::Edit, Icon::Save, Icon::Files, Icon::Folder,
        Icon::Document, Icon::Image, Icon::Calculator, Icon::Games, Icon::System,
        Icon::Clock, Icon::Play, Icon::Pause, Icon::Stop, Icon::Speaker, Icon::Mute,
        Icon::Info, Icon::Warning, Icon::Lock,
    ];

    /// Private Use Area codepoint for inline use in text.
    pub const fn cp(self) -> u32 {
        ICON_PUA_BASE + self as u32
    }

    /// Inline-text form of the icon (a `char` in the Private Use Area).
    pub const fn ch(self) -> char {
        // SAFETY-free: PUA codepoints U+E000.. are always valid scalar values.
        match char::from_u32(self.cp()) {
            Some(c) => c,
            None => '?',
        }
    }

    /// Human-readable label (for TTS narration, tooltips, accessibility).
    pub const fn label(self) -> &'static str {
        match self {
            Icon::Home => "Home",
            Icon::Back => "Back",
            Icon::Forward => "Forward",
            Icon::Up => "Up",
            Icon::Close => "Close",
            Icon::Check => "Check",
            Icon::Menu => "Menu",
            Icon::More => "More",
            Icon::Plus => "Plus",
            Icon::Minus => "Minus",
            Icon::Search => "Search",
            Icon::Refresh => "Refresh",
            Icon::Settings => "Settings",
            Icon::Trash => "Trash",
            Icon::Edit => "Edit",
            Icon::Save => "Save",
            Icon::Files => "Files",
            Icon::Folder => "Folder",
            Icon::Document => "Document",
            Icon::Image => "Image",
            Icon::Calculator => "Calculator",
            Icon::Games => "Games",
            Icon::System => "System",
            Icon::Clock => "Clock",
            Icon::Play => "Play",
            Icon::Pause => "Pause",
            Icon::Stop => "Stop",
            Icon::Speaker => "Speaker",
            Icon::Mute => "Mute",
            Icon::Info => "Info",
            Icon::Warning => "Warning",
            Icon::Lock => "Lock",
        }
    }
}

/// Draw `icon` into a `size x size` box at `(x, y)` using one ink `color`
/// (4 coverage steps blended over whatever is underneath).
pub fn draw_icon(fb: &mut Framebuffer, x: i32, y: i32, icon: Icon, size: i32, color: u32) {
    glyph::draw_icon_index(fb, x, y, icon as u16, size, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glyph::{ICON32_COUNT, ICON32_NAMES};

    #[test]
    fn icon_enum_matches_generated_table() {
        assert_eq!(ICON32_COUNT, Icon::ALL.len());
        for (i, icon) in Icon::ALL.iter().enumerate() {
            assert_eq!(*icon as usize, i);
            let label = icon.label();
            assert!(
                label.eq_ignore_ascii_case(ICON32_NAMES[i]),
                "Icon #{} label {} != file name {}",
                i,
                label,
                ICON32_NAMES[i]
            );
        }
    }
}
