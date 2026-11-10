//! Timings. Run with:
//! `cargo test --release -p specular-metadata --test bench -- --ignored --nocapture`
use specular_metadata::*;
use std::time::Instant;

fn per_iter(n: u32, mut f: impl FnMut()) -> f64 {
    let t = Instant::now();
    for _ in 0..n {
        f();
    }
    t.elapsed().as_secs_f64() * 1e6 / n as f64
}

#[test]
#[ignore]
fn timings() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/lisbon.jpg");
    let Ok(data) = std::fs::read(path) else {
        return;
    };

    let t = Instant::now();
    let en = Localizer::new(langid!("en"));
    let first_localizer = t.elapsed();
    let t = Instant::now();
    let de = Localizer::new(langid!("de"));
    let second_localizer = t.elapsed();

    // warm_up_geocoder() returns at once and builds the index on a thread;
    // time until it is ready.
    let t = Instant::now();
    warm_up_geocoder();
    let warm_call = t.elapsed();
    while !geocoder_ready() {
        std::thread::yield_now();
    }
    let warm = t.elapsed();

    let scan_us = per_iter(2000, || {
        std::hint::black_box(jpeg::scan(&data).unwrap());
    });
    let seg = jpeg::scan(&data).unwrap();
    let input = seg.input(FileFacts::default());
    let read_en = per_iter(2000, || {
        std::hint::black_box(read(&input, &en));
    });
    let read_de = per_iter(2000, || {
        std::hint::black_box(read(&input, &de));
    });
    let pts = [
        (38.7223, -9.1393),
        (-34.6132, -58.3772),
        (40.0, -40.0),
        (35.68, 139.69),
        (51.5, -0.12),
    ];
    let mut i = 0;
    let lookup = per_iter(5000, || {
        let (a, b) = pts[i % pts.len()];
        i += 1;
        std::hint::black_box(geocode(a, b));
    });
    let mut r = read(&input, &en);
    let attach = per_iter(2000, || {
        attach_place(std::hint::black_box(&mut r), &en);
    });

    println!(
        "Localizer::new(en), first (parses catalogues):  {:>8.2} ms",
        first_localizer.as_secs_f64() * 1e3
    );
    println!(
        "Localizer::new(de):                              {:>8.2} ms",
        second_localizer.as_secs_f64() * 1e3
    );
    println!(
        "warm_up_geocoder() call (returns at once):       {:>8.3} ms",
        warm_call.as_secs_f64() * 1e3
    );
    println!(
        "geocoder index build until ready:                {:>8.1} ms",
        warm.as_secs_f64() * 1e3
    );
    println!("jpeg::scan (lisbon.jpg):                         {scan_us:>8.2} µs");
    println!("read() en (lisbon.jpg):                          {read_en:>8.2} µs");
    println!("read() de (lisbon.jpg):                          {read_de:>8.2} µs");
    println!("geocode() per lookup (warm):                     {lookup:>8.2} µs");
    println!("attach_place() (place cached; formatting only): {attach:>8.2} µs");
    assert!(read_en < 2000.0, "read() over the 2 ms budget");
}
