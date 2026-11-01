//! Native screenshot helper: renders guest frames to PPM files.
//!
//! ```sh
//! cargo run --manifest-path v2/Cargo.toml --example shot
//! ```

use std::fs::File;
use std::io::{BufWriter, Write};

fn write_ppm(path: &str, w: usize, h: usize) {
    let ptr = appliance::app_framebuffer();
    let px = unsafe { std::slice::from_raw_parts(ptr, w * h * 4) };
    let mut f = BufWriter::new(File::create(path).unwrap());
    write!(f, "P6\n{} {}\n255\n", w, h).unwrap();
    for i in 0..w * h {
        // Guest memory is R,G,B,A.
        f.write_all(&px[i * 4..i * 4 + 3]).unwrap();
    }
}

fn shot(name: &str, w: usize, h: usize) {
    write_ppm(&format!("rendered/{}", name), w, h);
    println!("HASH {} {:08x}", name, appliance::app_fb_hash());
}

fn main() {
    std::fs::create_dir_all("rendered").unwrap();
    appliance::app_set_session(1);
    appliance::app_set_date(2026, 11, 1, 6);
    appliance::app_set_time(9, 41, 0, 0);
    appliance::app_set_utc(2026, 11, 1, 14, 41, 0, -300);
    appliance::app_init(1280, 720);

    // Ctrl+1 -> System (About), 3 -> Fonts, 1/2 -> render mode.
    appliance::app_key_mods(11, 2);
    shot("v2-about.ppm", 1280, 720);
    appliance::app_key(13);
    shot("v2-fonts-smooth.ppm", 1280, 720);
    appliance::app_key(1);
    shot("v2-fonts-native.ppm", 1280, 720);
    appliance::app_key(2);

    // 5 -> Theme
    appliance::app_key(15);
    shot("v2-theme.ppm", 1280, 720);

    // 4 -> Icons
    appliance::app_key(14);
    shot("v2-icons.ppm", 1280, 720);

    // Files with a small mount catalog.
    appliance::app_mount_clear();
    let names: [&[u8]; 4] = [
        b"wallpapers/01_aurora.png",
        b"samples/app.js",
        b"samples/hello.c",
        b"github/lib.rs",
    ];
    for (i, n) in names.iter().enumerate() {
        let buf = unsafe { std::slice::from_raw_parts_mut(appliance::app_mount_name_buf(), 64) };
        buf[..n.len()].copy_from_slice(n);
        appliance::app_mount_add(n.len(), 1024 + i as u32);
    }
    // Ctrl+3 -> Files
    appliance::app_key_mods(13, 2);
    shot("v2-files.ppm", 1280, 720);

    // Calculator
    appliance::app_key_mods(12, 2);
    appliance::app_key(12);
    appliance::app_key(20);
    appliance::app_key(13);
    appliance::app_key(24);
    shot("v2-calc.ppm", 1280, 720);

    // Minesweeper
    appliance::app_key_mods(14, 2);
    shot("v2-mines.ppm", 1280, 720);

    // Small laptop and mobile Files (control-bar fit).
    appliance::app_init(800, 600);
    appliance::app_mount_clear();
    for (i, n) in names.iter().enumerate() {
        let buf = unsafe { std::slice::from_raw_parts_mut(appliance::app_mount_name_buf(), 64) };
        buf[..n.len()].copy_from_slice(n);
        appliance::app_mount_add(n.len(), 1024 + i as u32);
    }
    appliance::app_key_mods(13, 2);
    shot("v2-files-800.ppm", 800, 600);
    appliance::app_init(390, 844);
    appliance::app_mount_clear();
    for (i, n) in names.iter().enumerate() {
        let buf = unsafe { std::slice::from_raw_parts_mut(appliance::app_mount_name_buf(), 64) };
        buf[..n.len()].copy_from_slice(n);
        appliance::app_mount_add(n.len(), 1024 + i as u32);
    }
    appliance::app_key_mods(13, 2);
    shot("v2-files-mobile.ppm", 390, 844);

    // Mobile portrait Fonts
    appliance::app_init(390, 844);
    appliance::app_key_mods(11, 2);
    appliance::app_key(13);
    shot("v2-fonts-mobile.ppm", 390, 844);
    appliance::app_key_mods(14, 2);
    shot("v2-mines-mobile.ppm", 390, 844);
    // Isolated glyph sample sheet in both modes.
    appliance::app_init(1280, 240);
    let ptr = appliance::app_framebuffer() as *mut u32;
    let px = unsafe { std::slice::from_raw_parts_mut(ptr, 1280 * 240) };
    let mut fb = appliance::fb::Framebuffer::new(px, 1280, 240);
    fb.clear(0xFF10141A);
    use appliance::font::{self, RenderMode};
    font::set_render_mode(RenderMode::Native);
    font::draw_str(&mut fb, 8, 8, "Aa 0123  \u{E000} \u{2190} [|] gpqy", 2, 0xFFFFFFFF);
    font::draw_str(&mut fb, 8, 40, "\u{00A1}\u{00BF}\u{00C0}\u{00C1}\u{00C2}\u{00C3}\u{00C4}\u{00C5}\u{00C6}\u{00C7}\u{00C8}\u{00C9}\u{00CA}\u{00CB}", 2, 0xFFFFFFFF);
    font::set_render_mode(RenderMode::Smooth);
    font::draw_str(&mut fb, 8, 80, "Aa 0123  \u{E000} \u{2190} [|] gpqy", 2, 0xFFFFFFFF);
    font::draw_str(&mut fb, 8, 112, "\u{00A1}\u{00BF}\u{00C0}\u{00C1}\u{00C2}\u{00C3}\u{00C4}\u{00C5}\u{00C6}\u{00C7}\u{00C8}\u{00C9}\u{00CA}\u{00CB}", 2, 0xFFFFFFFF);
    font::draw_str_mode(&mut fb, RenderMode::Native, 8, 160, "Aa 0123 gpqy", 4, 0xFFFFFFFF);
    font::draw_str_mode(&mut fb, RenderMode::Smooth, 300, 160, "Aa 0123 gpqy", 4, 0xFFFFFFFF);
    font::draw_str_mode(&mut fb, RenderMode::Native, 640, 150, "Aa 0123", 8, 0xFFFFFFFF);
    font::draw_str_mode(&mut fb, RenderMode::Smooth, 940, 150, "Aa 0123", 8, 0xFFFFFFFF);
    write_ppm("rendered/v2-sample.ppm", 1280, 240);

    println!("wrote rendered/*.ppm");
}
