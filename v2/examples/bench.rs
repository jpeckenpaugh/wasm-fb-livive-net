//! Microbenchmark for glyph lookup (decode + metric derivation).
//!
//! ```sh
//! cargo run --release --manifest-path v2/Cargo.toml --example bench
//! ```

use std::hint::black_box;
use std::time::Instant;

fn main() {
    // Warm the page in / branch predictors.
    for cp in 0x20..=0x26FF {
        black_box(appliance::glyph::lookup_fiveseven(cp));
    }

    let n = 2_000_000u32;
    let start = Instant::now();
    let mut acc = 0u64;
    for i in 0..n {
        let cp = 0x20 + (i % 0x0800);
        if let Some(g) = appliance::glyph::lookup_fiveseven(cp) {
            acc += (g.lsb as u64) + (g.ink_w as u64);
        }
    }
    let d = start.elapsed();
    println!(
        "{} lookups in {:?} ({:.1} ns/lookup) acc={}",
        n,
        d,
        d.as_nanos() as f64 / n as f64,
        acc
    );
}
