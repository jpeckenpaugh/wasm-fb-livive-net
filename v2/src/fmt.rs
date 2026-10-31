//! Shared fixed-buffer formatting helpers (`fmt.rs`).
//!
//! The guest is `no_std` and allocation-free, so every formatter writes into a
//! caller-provided byte buffer and returns a `&str` view. Keep all numeric /
//! date / size formatting here instead of growing per-app copies.

#[inline]
fn push(buf: &mut [u8], pos: &mut usize, b: u8) {
    if *pos < buf.len() {
        buf[*pos] = b;
        *pos += 1;
    }
}

fn push_bytes(buf: &mut [u8], pos: &mut usize, bytes: &[u8]) {
    for &b in bytes {
        push(buf, pos, b);
    }
}

/// Write an unsigned decimal at `pos`; returns the new position.
pub fn write_u32(buf: &mut [u8], mut pos: usize, mut val: u32) -> usize {
    if val == 0 {
        push(buf, &mut pos, b'0');
        return pos;
    }
    let mut tmp = [0u8; 10];
    let mut n = 0;
    while val > 0 && n < tmp.len() {
        tmp[n] = b'0' + (val % 10) as u8;
        val /= 10;
        n += 1;
    }
    while n > 0 {
        n -= 1;
        push(buf, &mut pos, tmp[n]);
    }
    pos
}

pub fn u32_str(buf: &mut [u8; 16], val: u32) -> &str {
    let pos = write_u32(buf, 0, val);
    core::str::from_utf8(&buf[..pos]).unwrap_or("0")
}

/// `0-999 B`, `1-1023 K`, `1-1023 M`, then `G`.
pub fn size_str(buf: &mut [u8; 16], bytes: u32) -> &str {
    let (value, suffix) = if bytes < 1024 {
        (bytes, b'B')
    } else if bytes < 1024 * 1024 {
        ((bytes + 512) / 1024, b'K')
    } else if bytes < 1024 * 1024 * 1024 {
        ((bytes + 512 * 1024) / (1024 * 1024), b'M')
    } else {
        ((bytes + 512 * 1024 * 1024) / (1024 * 1024 * 1024), b'G')
    };
    let mut pos = write_u32(buf, 0, value.max(if suffix == b'B' { 0 } else { 1 }));
    push(buf, &mut pos, suffix);
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

/// `N ITEMS`
pub fn items_str(buf: &mut [u8; 16], count: u32) -> &str {
    let mut pos = write_u32(buf, 0, count);
    push_bytes(buf, &mut pos, b" ITEMS");
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

/// `a/b`
pub fn ratio_str(buf: &mut [u8; 16], a: u32, b: u32) -> &str {
    let mut pos = write_u32(buf, 0, a);
    push(buf, &mut pos, b'/');
    pos = write_u32(buf, pos, b);
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

pub fn line_no(buf: &mut [u8; 8], val: u32) -> &str {
    let pos = write_u32(buf, 0, val);
    core::str::from_utf8(&buf[..pos]).unwrap_or("0")
}

/// `start..end / total`
pub fn line_range(buf: &mut [u8; 24], start: u32, end: u32, total: u32) -> &str {
    let mut pos = write_u32(buf, 0, start);
    push_bytes(buf, &mut pos, b"..");
    pos = write_u32(buf, pos, end);
    push_bytes(buf, &mut pos, b" / ");
    pos = write_u32(buf, pos, total);
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

/// `~/ (N)`
pub fn mount_count(buf: &mut [u8; 16], count: u32) -> &str {
    let mut pos = 0;
    push_bytes(buf, &mut pos, b"~/ (");
    pos = write_u32(buf, pos, count);
    push(buf, &mut pos, b')');
    core::str::from_utf8(&buf[..pos]).unwrap_or("~/")
}

/// `WxH RGBA8888 (N B)`
pub fn fb_spec(buf: &mut [u8; 48], w: u32, h: u32) -> &str {
    let mut pos = write_u32(buf, 0, w);
    push(buf, &mut pos, b'X');
    pos = write_u32(buf, pos, h);
    push_bytes(buf, &mut pos, b" RGBA8888 (");
    pos = write_u32(buf, pos, w.saturating_mul(h).saturating_mul(4));
    push_bytes(buf, &mut pos, b" B)");
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

/// `YYYY-MM-DD HH:MM:SS`
#[allow(clippy::too_many_arguments)]
pub fn datetime24(
    buf: &mut [u8; 24],
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    min: u8,
    sec: u8,
) -> &str {
    let mut pos = 0;
    push(buf, &mut pos, b'0' + ((year / 1000) % 10) as u8);
    push(buf, &mut pos, b'0' + ((year / 100) % 10) as u8);
    push(buf, &mut pos, b'0' + ((year / 10) % 10) as u8);
    push(buf, &mut pos, b'0' + (year % 10) as u8);
    push(buf, &mut pos, b'-');
    push(buf, &mut pos, b'0' + (month / 10) % 10);
    push(buf, &mut pos, b'0' + (month % 10));
    push(buf, &mut pos, b'-');
    push(buf, &mut pos, b'0' + (day / 10) % 10);
    push(buf, &mut pos, b'0' + (day % 10));
    push(buf, &mut pos, b' ');
    push(buf, &mut pos, b'0' + (hour / 10) % 10);
    push(buf, &mut pos, b'0' + (hour % 10));
    push(buf, &mut pos, b':');
    push(buf, &mut pos, b'0' + (min / 10) % 10);
    push(buf, &mut pos, b'0' + (min % 10));
    push(buf, &mut pos, b':');
    push(buf, &mut pos, b'0' + (sec / 10) % 10);
    push(buf, &mut pos, b'0' + (sec % 10));
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

/// `UTC+HH:MM`
pub fn tz_offset(buf: &mut [u8; 24], offset_min: i16) -> &str {
    let mut pos = 0;
    push_bytes(buf, &mut pos, b"UTC");
    push(buf, &mut pos, if offset_min < 0 { b'-' } else { b'+' });
    let abs_min = offset_min.unsigned_abs();
    let hh = (abs_min / 60) as u8;
    let mm = (abs_min % 60) as u8;
    push(buf, &mut pos, b'0' + (hh / 10) % 10);
    push(buf, &mut pos, b'0' + (hh % 10));
    push(buf, &mut pos, b':');
    push(buf, &mut pos, b'0' + (mm / 10) % 10);
    push(buf, &mut pos, b'0' + (mm % 10));
    core::str::from_utf8(&buf[..pos]).unwrap_or("UTC+00:00")
}

/// WM title-bar right side: `TESTER  VM-01 YYYY-MM-DD HH:MM:SS` or the compact
/// `VM-01 HH:MM:SS`.
#[allow(clippy::too_many_arguments)]
pub fn title_right(
    buf: &mut [u8; 64],
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
    push_bytes(buf, &mut pos, if compact { b"VM-" } else { b"TESTER  VM-" });
    push(buf, &mut pos, b'0' + ((session_id / 10) % 10) as u8);
    push(buf, &mut pos, b'0' + (session_id % 10) as u8);
    push(buf, &mut pos, b' ');
    if !compact {
        push(buf, &mut pos, b' ');
        push(buf, &mut pos, b'0' + ((year / 1000) % 10) as u8);
        push(buf, &mut pos, b'0' + ((year / 100) % 10) as u8);
        push(buf, &mut pos, b'0' + ((year / 10) % 10) as u8);
        push(buf, &mut pos, b'0' + (year % 10) as u8);
        push(buf, &mut pos, b'-');
        push(buf, &mut pos, b'0' + (month / 10) % 10);
        push(buf, &mut pos, b'0' + (month % 10));
        push(buf, &mut pos, b'-');
        push(buf, &mut pos, b'0' + (day / 10) % 10);
        push(buf, &mut pos, b'0' + (day % 10));
        push(buf, &mut pos, b' ');
    }
    push(buf, &mut pos, b'0' + (hour / 10) % 10);
    push(buf, &mut pos, b'0' + (hour % 10));
    push(buf, &mut pos, b':');
    push(buf, &mut pos, b'0' + (min / 10) % 10);
    push(buf, &mut pos, b'0' + (min % 10));
    push(buf, &mut pos, b':');
    push(buf, &mut pos, b'0' + (sec / 10) % 10);
    push(buf, &mut pos, b'0' + (sec % 10));
    core::str::from_utf8(&buf[..pos]).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_sizes() {
        let mut b = [0u8; 16];
        assert_eq!(u32_str(&mut b, 0), "0");
        assert_eq!(u32_str(&mut b, 42), "42");
        assert_eq!(size_str(&mut b, 999), "999B");
        assert_eq!(size_str(&mut b, 1024), "1K");
        assert_eq!(size_str(&mut b, 493_568), "482K");
        assert_eq!(ratio_str(&mut b, 12, 54), "12/54");
        assert_eq!(mount_count(&mut b, 13), "~/ (13)");
    }

    #[test]
    fn dates_and_titles() {
        let mut b = [0u8; 24];
        assert_eq!(
            datetime24(&mut b, 2026, 10, 30, 7, 5, 9),
            "2026-10-30 07:05:09"
        );
        assert_eq!(tz_offset(&mut b, -240), "UTC-04:00");
        assert_eq!(tz_offset(&mut b, 330), "UTC+05:30");

        let mut t = [0u8; 64];
        assert_eq!(
            title_right(&mut t, 1, 2026, 10, 30, 7, 5, 9, false),
            "TESTER  VM-01  2026-10-30 07:05:09"
        );
        assert_eq!(
            title_right(&mut t, 12, 2026, 10, 30, 7, 5, 9, true),
            "VM-12 07:05:09"
        );
    }

    #[test]
    fn line_ranges() {
        let mut b = [0u8; 24];
        assert_eq!(line_range(&mut b, 1, 30, 96), "1..30 / 96");
        let mut n = [0u8; 8];
        assert_eq!(line_no(&mut n, 7), "7");
    }
}
