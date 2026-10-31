//! Guest input contract (`input.rs`).
//!
//! Host key codes are a flat integer namespace (`web/v2/host.js` `KEY_CODES`).
//! Apps should match [`Key`] variants, never bare integers, and return
//! [`InputResult`] instead of ad-hoc `bool` / `0..3` values. The C ABI still
//! exposes the numeric code (`0 = noop, 1 = changed, 2 = snapshot, 3 = TTS`).

/// Logical key identifiers shared by the host keymap and the guest.
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Left = 1,
    Right = 2,
    Space = 3,
    Digit0 = 10,
    Digit1 = 11,
    Digit2 = 12,
    Digit3 = 13,
    Digit4 = 14,
    Digit5 = 15,
    Digit6 = 16,
    Digit7 = 17,
    Digit8 = 18,
    Digit9 = 19,
    Plus = 20,
    Minus = 21,
    Star = 22,
    Slash = 23,
    Enter = 24,
    Escape = 25,
    Backspace = 26,
    Dot = 27,
    Flag = 30,
    NewSeed = 31,
    Replay = 32,
    Tab = 33,
    PageUp = 34,
    PageDown = 35,
    ScrollLeft = 36,
    ScrollRight = 37,
    ScrollDown = 38,
    ScrollUp = 39,
    Up = 50,
    Down = 51,
}

impl Key {
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            1 => Some(Key::Left),
            2 => Some(Key::Right),
            3 => Some(Key::Space),
            10 => Some(Key::Digit0),
            11 => Some(Key::Digit1),
            12 => Some(Key::Digit2),
            13 => Some(Key::Digit3),
            14 => Some(Key::Digit4),
            15 => Some(Key::Digit5),
            16 => Some(Key::Digit6),
            17 => Some(Key::Digit7),
            18 => Some(Key::Digit8),
            19 => Some(Key::Digit9),
            20 => Some(Key::Plus),
            21 => Some(Key::Minus),
            22 => Some(Key::Star),
            23 => Some(Key::Slash),
            24 => Some(Key::Enter),
            25 => Some(Key::Escape),
            26 => Some(Key::Backspace),
            27 => Some(Key::Dot),
            30 => Some(Key::Flag),
            31 => Some(Key::NewSeed),
            32 => Some(Key::Replay),
            33 => Some(Key::Tab),
            34 => Some(Key::PageUp),
            35 => Some(Key::PageDown),
            36 => Some(Key::ScrollLeft),
            37 => Some(Key::ScrollRight),
            38 => Some(Key::ScrollDown),
            39 => Some(Key::ScrollUp),
            50 => Some(Key::Up),
            51 => Some(Key::Down),
            _ => None,
        }
    }

    /// Digit key for `0..=9`.
    pub const fn digit(d: u8) -> Option<Self> {
        match d {
            0 => Some(Key::Digit0),
            1 => Some(Key::Digit1),
            2 => Some(Key::Digit2),
            3 => Some(Key::Digit3),
            4 => Some(Key::Digit4),
            5 => Some(Key::Digit5),
            6 => Some(Key::Digit6),
            7 => Some(Key::Digit7),
            8 => Some(Key::Digit8),
            9 => Some(Key::Digit9),
            _ => None,
        }
    }
}

/// Result of an input event, mirrored to the host as an `i32`.
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputResult {
    Noop = 0,
    Changed = 1,
    Snapshot = 2,
    Tts = 3,
}

impl InputResult {
    pub const fn is_changed(self) -> bool {
        !matches!(self, InputResult::Noop)
    }

    pub const fn to_i32(self) -> i32 {
        self as i32
    }

    pub const fn from_changed(changed: bool) -> Self {
        if changed {
            InputResult::Changed
        } else {
            InputResult::Noop
        }
    }
}

impl From<bool> for InputResult {
    fn from(changed: bool) -> Self {
        Self::from_changed(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_codes_round_trip() {
        for code in 0..=64 {
            if let Some(key) = Key::from_code(code) {
                assert_eq!(key as i32, code);
            }
        }
        assert_eq!(Key::Escape as i32, 25);
        assert_eq!(Key::Tab as i32, 33);
        assert_eq!(Key::Up as i32, 50);
        assert_eq!(Key::Down as i32, 51);
        assert_eq!(Key::from_code(99), None);
    }

    #[test]
    fn result_codes_match_host_contract() {
        assert_eq!(InputResult::Noop.to_i32(), 0);
        assert_eq!(InputResult::Changed.to_i32(), 1);
        assert_eq!(InputResult::Snapshot.to_i32(), 2);
        assert_eq!(InputResult::Tts.to_i32(), 3);
        assert!(!InputResult::Noop.is_changed());
        assert!(InputResult::Tts.is_changed());
        assert_eq!(InputResult::from(true), InputResult::Changed);
        assert_eq!(InputResult::from(false), InputResult::Noop);
    }

    #[test]
    fn digit_helper_matches_variants() {
        for d in 0..=9u8 {
            assert_eq!(Key::digit(d).unwrap() as i32, 10 + d as i32);
        }
        assert_eq!(Key::digit(10), None);
    }
}
