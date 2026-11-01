//! Decode-all benchmark for the folded glyph backend.
//!
//! ```sh
//! cargo run --release --manifest-path v2/Cargo.toml \
//!     --features folded-glyphs --example bench_decode
//! ```

#[cfg(feature = "folded-glyphs")]
fn main() {
    use std::time::Instant;

    let glyphs = appliance::glyph::FIVESEVEN_GLYPH_COUNT;
    // Warm up.
    appliance::folded::decode_all();

    let n = 2000u32;
    let start = Instant::now();
    for _ in 0..n {
        appliance::folded::decode_all();
    }
    let d = start.elapsed();
    println!(
        "{} decodes of {} glyphs in {:?} ({:.3} ms/decode, {:.1} ns/glyph)",
        n,
        glyphs,
        d,
        d.as_secs_f64() * 1000.0 / n as f64,
        d.as_nanos() as f64 / (n as f64 * glyphs as f64)
    );
}

#[cfg(not(feature = "folded-glyphs"))]
fn main() {
    println!("build with --features folded-glyphs");
}
