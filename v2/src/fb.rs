//! Minimal RGBA8888 software framebuffer with sub-viewport (`stride`) support.
//!
//! Byte order in memory is R,G,B,A (what HTML Canvas ImageData expects).
//! On little-endian wasm32, a `u32` word is `0xAABBGGRR`, which [`rgba`] builds.

pub const SCREEN_W: usize = 1280;
pub const SCREEN_H: usize = 720;

#[inline]
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((g as u32) << 8) | (r as u32)
}

#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    rgba(r, g, b, 255)
}

pub struct Framebuffer<'a> {
    pub px: &'a mut [u32],
    pub w: i32,
    pub h: i32,
    pub stride: usize,
}

impl<'a> Framebuffer<'a> {
    pub fn new(px: &'a mut [u32], w: i32, h: i32) -> Self {
        Self {
            px,
            w,
            h,
            stride: w as usize,
        }
    }

    /// Carve out a full-width horizontal band starting at `y0` with height `sub_h`.
    /// Because it spans the full stride, zero coordinate translation is needed inside
    /// the child view.
    pub fn sub_rows(&mut self, y0: i32, sub_h: i32) -> Framebuffer<'_> {
        let y_clamped = y0.clamp(0, self.h) as usize;
        let h_clamped = sub_h.clamp(0, self.h - y_clamped as i32);
        let offset = y_clamped * self.stride;
        let len = (h_clamped as usize) * self.stride;
        Framebuffer {
            px: &mut self.px[offset..offset + len],
            w: self.w,
            h: h_clamped,
            stride: self.stride,
        }
    }

    pub fn clear(&mut self, color: u32) {
        self.fill(0, 0, self.w, self.h, color);
    }

    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.w);
        let y1 = (y + h).min(self.h);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for row in y0..y1 {
            let base = (row as usize) * self.stride;
            let slice = &mut self.px[base + (x0 as usize)..base + (x1 as usize)];
            slice.fill(color);
        }
    }

    pub fn border(&mut self, x: i32, y: i32, w: i32, h: i32, thickness: i32, color: u32) {
        if w <= 0 || h <= 0 || thickness <= 0 {
            return;
        }
        let t = thickness.min(w).min(h);
        self.fill(x, y, w, t, color);
        self.fill(x, y + h - t, w, t, color);
        self.fill(x, y + t, t, h - 2 * t, color);
        self.fill(x + w - t, y + t, t, h - 2 * t, color);
    }

    /// Multiply RGB channels in region by `shade / 255` (0..=255), leaving alpha=255.
    pub fn shade(&mut self, x: i32, y: i32, w: i32, h: i32, shade: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + w).min(self.w);
        let y1 = (y + h).min(self.h);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let s = shade.min(255);
        for row in y0..y1 {
            let base = (row as usize) * self.stride;
            for col in (x0 as usize)..(x1 as usize) {
                let p = self.px[base + col];
                let r = ((p & 0xff) * s) >> 8;
                let g = (((p >> 8) & 0xff) * s) >> 8;
                let b = (((p >> 16) & 0xff) * s) >> 8;
                self.px[base + col] = 0xff00_0000 | (b << 16) | (g << 8) | r;
            }
        }
    }

    /// Compute 32-bit FNV-1a checksum over the RGBA bytes of the framebuffer.
    pub fn fnv1a_32(&self) -> u32 {
        let mut h: u32 = 0x811c_9dc5;
        for row in 0..(self.h as usize) {
            let base = row * self.stride;
            for col in 0..(self.w as usize) {
                let word = self.px[base + col].to_le_bytes();
                for &b in &word {
                    h ^= b as u32;
                    h = h.wrapping_mul(0x0100_0193);
                }
            }
        }
        h
    }
}
