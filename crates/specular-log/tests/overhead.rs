//! A disabled callsite costs next to nothing: with the default filter (warnings only), a
//! `debug!` with formatted arguments must not format or allocate.

use std::hint::black_box;
use std::time::Instant;

#[test]
fn disabled_callsites_are_nearly_free() {
    let buf = specular_log::capture("").expect("first logger in this process");
    let n = 5_000_000u64;
    let name = String::from("2.jpeg");
    let t = Instant::now();
    for i in 0..n {
        specular_log::debug!(target: specular_log::DECODE, img = black_box(i), "{} level {}/{} decoded", black_box(&name), i, 8);
    }
    let disabled = t.elapsed().as_secs_f64() * 1e9 / n as f64;
    let t = Instant::now();
    for i in 0..n {
        black_box(i);
    }
    let empty = t.elapsed().as_secs_f64() * 1e9 / n as f64;
    eprintln!("disabled debug!: {disabled:.2} ns/call (empty loop {empty:.2} ns)");
    assert!(buf.lock().is_empty(), "nothing logged");
    // Generous bound (shared CI machines); in practice this is about a nanosecond.
    assert!(disabled < 25.0, "{disabled:.2} ns per disabled call");
    // An enabled warning does get through.
    specular_log::warn!(target: specular_log::DECODE, "w");
    assert!(String::from_utf8_lossy(&buf.lock()).contains("WARN w"));
}
