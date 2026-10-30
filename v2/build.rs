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
    ] {
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
