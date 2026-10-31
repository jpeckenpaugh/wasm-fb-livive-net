//! Shared geometry primitives and responsive size classes (`layout.rs`).
//!
//! Apps used to invent their own width/height breakpoints. Prefer
//! [`Metrics::new`] and one of the three classes so compact/medium/full
//! behavior stays predictable across the appliance.

/// Coarse viewport size class shared by the WM and every app.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SizeClass {
    /// Phone / small portrait: the WM uses one text scale and tight spacing.
    Compact,
    /// Tablet / VGA: medium spacing.
    Medium,
    /// Default desktop / HD.
    Full,
}

/// Viewport metrics plus the derived size class.
#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub w: i32,
    pub h: i32,
    pub class: SizeClass,
}

impl Metrics {
    pub fn new(w: i32, h: i32) -> Self {
        let class = if w < 540 || h < 450 {
            SizeClass::Compact
        } else if w < 900 || h < 600 {
            SizeClass::Medium
        } else {
            SizeClass::Full
        };
        Self { w, h, class }
    }

    pub fn is_compact(self) -> bool {
        self.class == SizeClass::Compact
    }

    pub fn is_full(self) -> bool {
        self.class == SizeClass::Full
    }

    /// Pick a value for compact / medium / full.
    pub fn pick(self, compact: i32, medium: i32, full: i32) -> i32 {
        match self.class {
            SizeClass::Compact => compact,
            SizeClass::Medium => medium,
            SizeClass::Full => full,
        }
    }

    /// Standard body text scale for UI labels.
    pub fn body_scale(self) -> i32 {
        if self.w < 480 {
            1
        } else {
            2
        }
    }

    /// Standard section-heading text scale.
    pub fn head_scale(self) -> i32 {
        if self.is_compact() {
            2
        } else {
            3
        }
    }
}

/// Integer rectangle used for layout and hit testing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn right(self) -> i32 {
        self.x + self.w
    }

    pub const fn bottom(self) -> i32 {
        self.y + self.h
    }

    pub const fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    pub const fn inset(self, n: i32) -> Self {
        Self::new(self.x + n, self.y + n, self.w - 2 * n, self.h - 2 * n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_classes_cover_session_resolutions() {
        assert_eq!(Metrics::new(390, 844).class, SizeClass::Compact);
        assert_eq!(Metrics::new(640, 480).class, SizeClass::Medium);
        assert_eq!(Metrics::new(800, 600).class, SizeClass::Medium);
        assert_eq!(Metrics::new(1280, 720).class, SizeClass::Full);
        assert_eq!(Metrics::new(1920, 1080).class, SizeClass::Full);
    }

    #[test]
    fn pick_and_scales_are_stable() {
        let compact = Metrics::new(390, 844);
        let full = Metrics::new(1280, 720);
        assert_eq!(compact.pick(1, 2, 3), 1);
        assert_eq!(full.pick(1, 2, 3), 3);
        assert_eq!(compact.head_scale(), 2);
        assert_eq!(full.head_scale(), 3);
        assert_eq!(Metrics::new(400, 800).body_scale(), 1);
        assert_eq!(full.body_scale(), 2);
    }

    #[test]
    fn rect_contains_is_half_open() {
        let r = Rect::new(10, 20, 30, 40);
        assert!(r.contains(10, 20));
        assert!(r.contains(39, 59));
        assert!(!r.contains(40, 59));
        assert!(!r.contains(39, 60));
        assert_eq!(r.right(), 40);
        assert_eq!(r.bottom(), 60);
        assert_eq!(r.inset(5), Rect::new(15, 25, 20, 30));
    }
}
