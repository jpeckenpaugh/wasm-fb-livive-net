use std::process::Command;

fn cmd_out(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/wm.rs");
    println!("cargo:rerun-if-changed=src/system.rs");
    println!("cargo:rerun-if-changed=src/calc.rs");
    println!("cargo:rerun-if-changed=src/files.rs");
    println!("cargo:rerun-if-changed=src/fb.rs");
    println!("cargo:rerun-if-changed=src/font.rs");
    println!("cargo:rerun-if-changed=src/glyph.rs");
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=src/theme.rs");
    println!("cargo:rerun-if-changed=src/layout.rs");
    println!("cargo:rerun-if-changed=src/input.rs");
    println!("cargo:rerun-if-changed=src/ui.rs");
    println!("cargo:rerun-if-changed=src/fmt.rs");
    println!("cargo:rerun-if-changed=glyphs");
    println!("cargo:rerun-if-changed=glyphs/fiveseven");
    println!("cargo:rerun-if-changed=glyphs/icons32");

    let glyph_sources = generate_glyph_tables();

    let rustc_ver = cmd_out("rustc", &["--version"]).unwrap_or_else(|| "rustc".to_string());
    let target = std::env::var("TARGET").unwrap_or_else(|_| "wasm32-unknown-unknown".to_string());
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "release".to_string());

    // Deterministic FNV-1a 64-bit digest across the v2 source tree.
    let mut combined = Vec::new();
    for path in &[
        "Cargo.toml",
        "src/lib.rs",
        "src/fb.rs",
        "src/font.rs",
        "src/wm.rs",
        "src/system.rs",
        "src/calc.rs",
        "src/files.rs",
        "src/glyph.rs",
        "src/icon.rs",
        "src/theme.rs",
        "src/layout.rs",
        "src/input.rs",
        "src/ui.rs",
        "src/fmt.rs",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            combined.extend_from_slice(bytes.as_slice());
        }
    }
    for path in &glyph_sources {
        if let Ok(bytes) = std::fs::read(path) {
            combined.extend_from_slice(bytes.as_slice());
        }
    }
    let src_hash = format!("{:016x}", fnv1a_64(&combined));

    println!("cargo:rustc-env=APPLIANCE_RUSTC={}", rustc_ver);
    println!("cargo:rustc-env=APPLIANCE_TARGET={}", target);
    println!("cargo:rustc-env=APPLIANCE_PROFILE={}", profile);
    println!("cargo:rustc-env=APPLIANCE_SRC_HASH={}", src_hash);
}

// ---------------------------------------------------------------------------
// Glyph table generation: glyphs/**/*.txt (ASCII art) -> $OUT_DIR/glyph_tables.rs
// ---------------------------------------------------------------------------

fn sorted_txt_files(dir: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("glyph dir {}: {}", dir, e))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "txt").unwrap_or(false))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

struct TextGlyph {
    cp: u32,
    rows: [u8; 7],
    dy: u8,
}

/// Parse a FiveSeven sheet: `@U+XXXX <label> [dy=N]` followed by 7 rows of 5 cells.
fn parse_fiveseven(path: &str, out: &mut Vec<TextGlyph>) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut lines = text.lines().enumerate().peekable();
    while let Some((ln, line)) = lines.next() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let hdr = line
            .strip_prefix("@U+")
            .unwrap_or_else(|| panic!("{}:{}: expected '@U+XXXX', got {:?}", path, ln + 1, line));
        let mut parts = hdr.split_whitespace();
        let cp = u32::from_str_radix(parts.next().unwrap(), 16)
            .unwrap_or_else(|_| panic!("{}:{}: bad codepoint", path, ln + 1));
        let mut dy = 0u8;
        for p in parts {
            if let Some(v) = p.strip_prefix("dy=") {
                dy = v
                    .parse()
                    .unwrap_or_else(|_| panic!("{}:{}: bad dy", path, ln + 1));
                assert!(dy <= 3, "{}:{}: dy must be 0..=3", path, ln + 1);
            }
        }
        let mut rows = [0u8; 7];
        for r in rows.iter_mut() {
            let (rln, row) = lines
                .next()
                .unwrap_or_else(|| panic!("{}: U+{:04X} truncated", path, cp));
            let row = row.trim_end();
            assert!(
                row.chars().count() == 5,
                "{}:{}: U+{:04X} row must be 5 cells, got {:?}",
                path,
                rln + 1,
                cp,
                row
            );
            for (i, c) in row.chars().enumerate() {
                match c {
                    '#' => *r |= 1 << (4 - i),
                    '.' => {}
                    _ => panic!("{}:{}: bad cell {:?}", path, rln + 1, c),
                }
            }
        }
        out.push(TextGlyph { cp, rows, dy });
    }
}

/// Parse one 32x32 2-bit icon file into 256 bytes (8 bytes per row, MSB-first pixels).
fn parse_icon(path: &str) -> [u8; 256] {
    let text = std::fs::read_to_string(path).unwrap();
    let rows: Vec<&str> = text
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert!(
        rows.len() == 32,
        "{}: expected 32 rows, got {}",
        path,
        rows.len()
    );
    let mut out = [0u8; 256];
    for (y, row) in rows.iter().enumerate() {
        assert!(
            row.chars().count() == 32,
            "{}: row {} must be 32 cells",
            path,
            y
        );
        for (x, c) in row.chars().enumerate() {
            let v: u8 = match c {
                '.' => 0,
                '-' => 1,
                '+' => 2,
                '#' => 3,
                _ => panic!("{}: row {}: bad cell {:?}", path, y, c),
            };
            out[y * 8 + x / 4] |= v << (6 - 2 * (x % 4));
        }
    }
    out
}

fn generate_glyph_tables() -> Vec<String> {
    use std::fmt::Write;
    let mut sources = Vec::new();

    // --- FiveSeven text sheets
    let mut glyphs = Vec::new();
    for f in sorted_txt_files("glyphs/fiveseven") {
        parse_fiveseven(&f, &mut glyphs);
        sources.push(f);
    }
    glyphs.sort_by_key(|g| g.cp);
    for w in glyphs.windows(2) {
        assert!(w[0].cp != w[1].cp, "duplicate glyph U+{:04X}", w[0].cp);
    }
    // Contiguous codepoint runs -> (first, last, base_index)
    let mut ranges: Vec<(u32, u32, usize)> = Vec::new();
    for (i, g) in glyphs.iter().enumerate() {
        match ranges.last_mut() {
            Some(r) if r.1 + 1 == g.cp => r.1 = g.cp,
            _ => ranges.push((g.cp, g.cp, i)),
        }
    }

    let mut o = String::new();
    writeln!(o, "// @generated by build.rs from glyphs/ -- do not edit.").unwrap();
    writeln!(
        o,
        "pub const FIVESEVEN_GLYPH_COUNT: usize = {};",
        glyphs.len()
    )
    .unwrap();
    writeln!(
        o,
        "pub static FIVESEVEN_RANGES: [(u32, u32, u16); {}] = [",
        ranges.len()
    )
    .unwrap();
    for r in &ranges {
        writeln!(o, "    (0x{:04X}, 0x{:04X}, {}),", r.0, r.1, r.2).unwrap();
    }
    writeln!(o, "];").unwrap();
    writeln!(
        o,
        "pub static FIVESEVEN_ROWS: [[u8; 7]; {}] = [",
        glyphs.len()
    )
    .unwrap();
    for g in &glyphs {
        let r = g.rows;
        writeln!(
            o,
            "    [0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X}], // U+{:04X}",
            r[0], r[1], r[2], r[3], r[4], r[5], r[6], g.cp
        )
        .unwrap();
    }
    writeln!(o, "];").unwrap();
    // Metrics byte: dy (bits 6-7) | left bearing (bits 3-5) | ink width (bits 0-2)
    writeln!(
        o,
        "pub static FIVESEVEN_METRICS: [u8; {}] = [",
        glyphs.len()
    )
    .unwrap();
    for g in &glyphs {
        let mask = g.rows.iter().fold(0u8, |a, &r| a | r);
        let (lsb, w) = if mask == 0 {
            (0u8, 0u8)
        } else {
            let left = (0..5).find(|&c| mask & (1 << (4 - c)) != 0).unwrap();
            let right = (0..5).rev().find(|&c| mask & (1 << (4 - c)) != 0).unwrap();
            (left as u8, (right - left + 1) as u8)
        };
        writeln!(o, "    0x{:02X},", (g.dy << 6) | (lsb << 3) | w).unwrap();
    }
    writeln!(o, "];").unwrap();

    // --- Icons32 (2 bits per pixel)
    let icon_files = sorted_txt_files("glyphs/icons32");
    writeln!(o, "pub const ICON32_COUNT: usize = {};", icon_files.len()).unwrap();
    writeln!(
        o,
        "pub static ICON32_NAMES: [&str; {}] = [",
        icon_files.len()
    )
    .unwrap();
    for f in &icon_files {
        let stem = std::path::Path::new(f)
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let name = stem
            .split_once('_')
            .map(|(_, suffix)| suffix)
            .unwrap_or(&stem)
            .to_string();
        writeln!(o, "    {:?},", name).unwrap();
    }
    writeln!(o, "];").unwrap();
    writeln!(
        o,
        "pub static ICON32_DATA: [[u8; 256]; {}] = [",
        icon_files.len()
    )
    .unwrap();
    for f in &icon_files {
        let d = parse_icon(f);
        write!(o, "    [").unwrap();
        for b in d.iter() {
            write!(o, "{},", b).unwrap();
        }
        writeln!(o, "],").unwrap();
        sources.push(f.clone());
    }
    writeln!(o, "];").unwrap();

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{}/glyph_tables.rs", out_dir), o).unwrap();
    sources
}
